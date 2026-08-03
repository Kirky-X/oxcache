// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Redis 测试工具 - 支持环境变量控制测试跳过

#[cfg(feature = "redis")]
use oxcache::backend::memory::RedisBackend;

use std::sync::Arc;
use std::time::Duration;

/// 获取 Redis URL（根据是否允许非 TLS 连接返回适当的 URL）
///
/// 优先级：
/// 1. 环境变量 `REDIS_URL`
/// 2. 如果设置了 `OXCACHE_ALLOW_INSECURE_REDIS`，使用 `redis://`
/// 3. 默认使用 TLS 连接 `rediss://`
#[allow(dead_code)]
pub fn get_redis_url() -> String {
    if let Ok(url) = std::env::var("REDIS_URL") {
        return url;
    }

    if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
        "redis://localhost:6380".to_string()
    } else {
        "rediss://127.0.0.1:6379".to_string()
    }
}

/// 获取 Redis URL（仅当允许非 TLS 时返回 redis://）
///
/// 与 `get_redis_url` 相同，但明确表示可能返回非安全连接。
#[allow(dead_code)]
pub fn get_redis_url_insecure() -> String {
    get_redis_url()
}

/// 检查 Redis 是否可用（异步版本）
///
/// 检查环境变量并尝试实际连接 Redis。
/// 如果设置了 `OXCACHE_SKIP_REDIS_TESTS`，直接返回 false。
#[allow(dead_code)]
pub async fn is_redis_available() -> bool {
    if std::env::var("OXCACHE_SKIP_REDIS_TESTS").is_ok() {
        println!("[TEST-SKIP] Redis tests skipped via OXCACHE_SKIP_REDIS_TESTS");
        return false;
    }

    let redis_url = get_redis_url();

    if !is_redis_available_url(&redis_url).await {
        println!(
            "[TEST-SKIP] Redis not available at {} (set OXCACHE_SKIP_REDIS_TESTS=1 to skip)",
            redis_url
        );
        return false;
    }

    true
}

/// 创建 Redis 后端用于测试
#[cfg(feature = "redis")]
#[allow(dead_code)]
pub async fn create_l2_backend_with_real_redis() -> Result<Arc<dyn oxcache::backend::CacheBackend>, String> {
    let redis_url = get_redis_url();
    match RedisBackend::new(&redis_url).await {
        Ok(backend) => Ok(Arc::new(backend)),
        Err(e) => Err(format!("无法创建Redis连接: {}", e)),
    }
}

/// 测试 Redis 连接
#[cfg(feature = "redis")]
#[allow(dead_code)]
pub async fn test_redis_connection() -> Result<(), String> {
    let redis_url = get_redis_url();
    println!("[TEST-REDIS] Testing connection to: {}", redis_url);

    let backend = match create_l2_backend_with_real_redis().await {
        Ok(b) => b,
        Err(e) => {
            println!("[TEST-SKIP] Cannot connect to Redis at {}: {}", redis_url, e);
            return Err(format!("Failed to create Redis connection: {}", e));
        }
    };

    let test_key = "oxcache:test:connection";
    if let Err(e) = backend
        .set(
            std::sync::Arc::from(test_key),
            std::sync::Arc::new(b"test".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
    {
        return Err(format!("SET operation failed: {}", e));
    }
    let value_opt = match backend.get(test_key).await {
        Ok(v) => v,
        Err(e) => return Err(format!("GET operation failed: {}", e)),
    };
    let value = match value_opt {
        Some(v) => v,
        None => return Err("Redis returned empty value".to_string()),
    };
    if &value != b"test" {
        return Err("Redis returned incorrect value".to_string());
    }
    if let Err(e) = backend.delete(test_key).await {
        return Err(format!("DELETE operation failed: {}", e));
    }
    Ok(())
}

/// 创建独立 Redis URL
#[allow(dead_code)]
pub fn create_standalone_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// 创建集群 Redis URLs
#[allow(dead_code)]
pub fn create_cluster_redis_urls() -> Vec<String> {
    vec![
        "redis://127.0.0.1:7000".to_string(),
        "redis://127.0.0.1:7001".to_string(),
        "redis://127.0.0.1:7002".to_string(),
    ]
}

/// 清理测试键（简化版）
#[allow(dead_code)]
pub async fn cleanup_test_keys(_pattern: &str) -> Result<(), String> {
    Ok(())
}

/// 检查 Redis 服务是否可达（实际网络检查）
///
/// 尝试连接到指定 URL，超时时间为 2 秒。
#[allow(dead_code)]
pub async fn is_redis_available_url(url: &str) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    match tokio::time::timeout(Duration::from_secs(2), client.get_multiplexed_async_connection()).await {
        Ok(Ok(_)) => true,
        Ok(Err(e)) => !e.is_connection_refusal(),
        _ => false,
    }
}

/// 等待 Redis 可用
///
/// 循环检查 Redis 是否可用，直到超时（30 秒）。
#[allow(dead_code)]
pub async fn wait_for_redis(url: &str) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(30);

    while start.elapsed() < timeout {
        if is_redis_available_url(url).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// 等待 Redis 集群可用
///
/// 检查所有节点是否可用且集群状态正常。
#[allow(dead_code)]
pub async fn wait_for_redis_cluster(urls: &[&str]) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(60);

    while start.elapsed() < timeout {
        let mut all_ready = true;
        for url in urls {
            if !is_redis_available_url(url).await {
                all_ready = false;
                break;
            }
        }

        if all_ready {
            let nodes: Vec<String> = urls.iter().map(|s| s.to_string()).collect();
            match redis::cluster::ClusterClient::new(nodes) {
                Ok(client) => match client.get_async_connection().await {
                    Ok(mut conn) => match redis::cmd("CLUSTER").arg("INFO").query_async::<String>(&mut conn).await {
                        Ok(info) => {
                            if info.contains("cluster_state:ok") {
                                println!("Redis Cluster is ready.");
                                return true;
                            }
                        }
                        Err(e) => {
                            println!("Failed to query cluster info: {}", e);
                        }
                    },
                    Err(e) => {
                        println!("Failed to get cluster connection: {}", e);
                    }
                },
                Err(e) => {
                    println!("Failed to create cluster client: {}", e);
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    println!("Timeout waiting for Redis Cluster to be ready.");
    false
}

/// 等待 Redis Sentinel 可用
///
/// 检查所有 Sentinel 节点是否可用且 master 已配置。
#[allow(dead_code)]
pub async fn wait_for_sentinel() -> bool {
    let sentinel_urls = vec![
        "redis://127.0.0.1:26382",
        "redis://127.0.0.1:26383",
        "redis://127.0.0.1:26384",
    ];

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(60);

    while start.elapsed() < timeout {
        let mut all_ready = true;

        for url in &sentinel_urls {
            if !is_redis_available_url(url).await {
                all_ready = false;
                break;
            }
        }

        if all_ready {
            let client = match redis::Client::open(sentinel_urls[0]) {
                Ok(c) => c,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
            if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                // SENTINEL masters returns Vec<Vec<String>> (array of flat key-value arrays)
                let result: Result<Vec<Vec<String>>, _> =
                    redis::cmd("SENTINEL").arg("masters").query_async(&mut conn).await;

                if let Ok(masters) = result {
                    // Each master is a flat [key, value, key, value, ...] list
                    if masters.iter().any(|m| {
                        m.windows(2).any(|w| w[0] == "name" && w[1] == "mymaster")
                    }) {
                        println!("Redis Sentinel is ready.");
                        return true;
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    println!("Timeout waiting for Redis Sentinel to be ready.");
    false
}

/// 检查默认 Redis 是否可用
#[allow(dead_code)]
pub async fn is_redis_available_default() -> bool {
    let redis_url = get_redis_url();
    is_redis_available_url(&redis_url).await
}
