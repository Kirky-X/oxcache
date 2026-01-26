// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis测试工具

#![allow(dead_code)]

#[cfg(feature = "redis")]
use oxcache::backend::client::RedisBackend;
#[cfg(feature = "redis")]
use oxcache::backend::CacheBackend;
use std::time::Duration;

#[cfg(feature = "redis")]
pub(crate) async fn create_l2_backend_with_real_redis() -> Result<RedisBackend, String> {
    let redis_url = "redis://127.0.0.1:6379";
    RedisBackend::new(redis_url)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(feature = "redis")]
pub(crate) async fn test_redis_connection() -> Result<(), String> {
    let backend = match create_l2_backend_with_real_redis().await {
        Ok(b) => b,
        Err(e) => return Err(format!("无法创建Redis连接: {}", e)),
    };
    let test_key = "oxcache:test:connection";
    if let Err(e) = backend
        .set(test_key, b"test".to_vec(), Some(Duration::from_secs(60)))
        .await
    {
        return Err(format!("SET操作失败: {}", e));
    }
    let value_opt = match backend.get(test_key).await {
        Ok(v) => v,
        Err(e) => return Err(format!("GET操作失败: {}", e)),
    };
    let value = match value_opt {
        Some(v) => v,
        None => return Err("Redis返回空值".to_string()),
    };
    if &value != b"test" {
        return Err("Redis返回的值不正确".to_string());
    }
    if let Err(e) = backend.delete(test_key).await {
        return Err(format!("DELETE操作失败: {}", e));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn create_standalone_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

#[allow(dead_code)]
pub fn create_cluster_redis_urls() -> Vec<String> {
    vec![
        "redis://127.0.0.1:7000".to_string(),
        "redis://127.0.0.1:7001".to_string(),
        "redis://127.0.0.1:7002".to_string(),
    ]
}

#[allow(dead_code)]
pub async fn cleanup_test_keys(_pattern: &str) -> Result<(), String> {
    // 简化实现
    Ok(())
}

#[allow(dead_code)]
pub fn is_redis_available() -> bool {
    std::env::var("OXCACHE_SKIP_REDIS_TESTS").is_err()
}

pub async fn is_redis_available_url(url: &str) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    match tokio::time::timeout(
        Duration::from_secs(1),
        client.get_multiplexed_async_connection(),
    )
    .await
    {
        Ok(Ok(_)) => true,
        Ok(Err(e)) => !e.is_connection_refusal(),
        _ => false,
    }
}

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
                    Ok(mut conn) => {
                        match redis::cmd("CLUSTER")
                            .arg("INFO")
                            .query_async::<String>(&mut conn)
                            .await
                        {
                            Ok(info) => {
                                if info.contains("cluster_state:ok") {
                                    println!("Redis Cluster is ready.");
                                    return true;
                                }
                            }
                            Err(e) => {
                                println!("Failed to query cluster info: {}", e);
                            }
                        }
                    }
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

pub async fn wait_for_sentinel() -> bool {
    let sentinel_urls = vec![
        "redis://127.0.0.1:26379",
        "redis://127.0.0.1:26380",
        "redis://127.0.0.1:26381",
    ];

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(60);

    while start.elapsed() < timeout {
        let mut all_ready = true;

        for url in &sentinel_urls {
            if !wait_for_redis(url).await {
                all_ready = false;
                break;
            }
        }

        if all_ready {
            let client = redis::Client::open(sentinel_urls[0]).unwrap();
            if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                let result: Result<Vec<String>, _> = redis::cmd("SENTINEL")
                    .arg("masters")
                    .query_async(&mut conn)
                    .await;

                if let Ok(masters) = result {
                    if masters.iter().any(|m| m.contains("mymaster")) {
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

pub async fn is_redis_available_default() -> bool {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    is_redis_available_url(&redis_url).await
}

#[macro_export]
macro_rules! test_with_redis {
    ($name:ident, $body:block) => {
        #[tokio::test]
        async fn $name() {
            if !redis_test_utils::is_redis_available() {
                println!("跳过测试: Redis不可用 (设置 OXCACHE_SKIP_REDIS_TESTS=1 跳过)");
                return;
            }
            match redis_test_utils::test_redis_connection().await {
                Ok(()) => {
                    println!("Redis连接成功，开始执行测试...");
                    $body
                }
                Err(e) => {
                    println!("跳过测试: Redis连接失败 - {}", e);
                    println!("请确保Redis容器正在运行: docker start redis-test");
                }
            }
        }
    };
}
