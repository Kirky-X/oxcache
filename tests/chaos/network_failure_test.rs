// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 网络故障混沌测试

#[path = "../common/mod.rs"]
mod common;

use common::test_containers::container_or_skip;
use common::{
    docker_wait_for_redis, free_local_port, get_redis_url, is_redis_available,
    setup_redis_container_on_port,
};
use oxcache::Cache;
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[cfg(feature = "memory")]
use oxcache::backend::MokaMemoryBackend;
#[cfg(feature = "memory")]
use oxcache::cache::{ChainCache, ChainLink};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: u64,
    value: String,
}

/// 真实断连注入：Redis 容器停机 → 操作显性报错；重启 → 同一后端自动恢复。
///
/// 使用 testcontainers 托管的独立 Redis（`stop_with_timeout` 注入断连，
/// 不由共享栈承担故障注入），结束后容器随句柄 drop 自清。
#[tokio::test]
async fn test_connection_recovery_after_failure() {
    println!("=== Connection failure recovery test (real container-stop injection) ===");

    let port = free_local_port();
    let Some((container, redis_url)) = container_or_skip(
        "chaos(connection-recovery)",
        setup_redis_container_on_port(port),
    )
    .await
    else {
        return;
    };

    // 关闭外层重试（内层 ConnectionManager 重连仍保留）：断连错误在
    // 数秒内显性返回，避免 3 级重试叠加把单次失败断言拖到 ~30s
    let backend = RedisBackend::builder()
        .connection_string(&redis_url)
        .retry_count(0)
        .build()
        .await
        .expect("connect redis container");

    // 正常操作
    backend
        .set(
            Arc::from("recovery_key"),
            Arc::new(b"initial_value".to_vec()),
            None,
        )
        .await
        .unwrap();
    let value = backend.get("recovery_key").await.unwrap();
    assert_eq!(value, Some(b"initial_value".to_vec()));

    // 故障注入：停止 Redis 容器（真实断连，非"再建一个连接"模拟）
    container
        .stop_with_timeout(Some(0))
        .await
        .expect("stop redis container");

    // 断连期间操作必须显性失败（错误传播，不得静默冒充成功/miss）
    let get_result = backend.get("recovery_key").await;
    assert!(
        get_result.is_err(),
        "get should return Err during the disconnect, got {get_result:?}"
    );
    let set_result = backend
        .set(Arc::from("after_stop"), Arc::new(b"x".to_vec()), None)
        .await;
    assert!(
        set_result.is_err(),
        "set should return Err during the disconnect, got {set_result:?}"
    );

    // 恢复：重启容器，同一后端连接池应自动恢复读写
    container.start().await.expect("restart redis container");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match backend.ping().await {
            Ok(_) => break,
            Err(e) => {
                assert!(
                    Instant::now() < deadline,
                    "the same backend failed to reconnect within 30s after restart: {e}"
                );
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    backend
        .set(
            Arc::from("recovery_key_after_restart"),
            Arc::new(b"v2".to_vec()),
            None,
        )
        .await
        .expect("write after recovery");
    assert_eq!(
        backend.get("recovery_key_after_restart").await.unwrap(),
        Some(b"v2".to_vec())
    );

    println!("✓ Connection failure recovery test passed (real stop + restart recovery)");
}

/// L1/L2 断连降级语义：L2（Redis）停机时 L1 命中不回落、L1 miss 降级为
/// 部分 miss（Ok(None)）、写入部分成功（Ok），重启后 L2 写穿恢复。
///
/// 真实故障路径（testcontainers `stop_with_timeout` + `start`），非 mock。
#[cfg(feature = "memory")]
#[tokio::test]
async fn test_two_level_degradation_and_recovery_on_disconnect() {
    println!(
        "=== Two-tier cache disconnect degradation and recovery test (real container-stop injection) ==="
    );

    let port = free_local_port();
    let Some((container, redis_url)) = container_or_skip(
        "chaos(l1-l2-degradation)",
        setup_redis_container_on_port(port),
    )
    .await
    else {
        return;
    };

    let build_chain = || async {
        ChainCache::builder()
            .link(ChainLink::from_backend(
                MokaMemoryBackend::builder().capacity(128).build(),
            ))
            .link(ChainLink::from_backend(
                RedisBackend::builder()
                    .connection_string(&redis_url)
                    .retry_count(0)
                    .build()
                    .await
                    .expect("connect redis L2"),
            ))
            .build()
    };
    let chain = build_chain().await;

    chain
        .set("degrade:key", b"v1".to_vec(), None)
        .await
        .expect("bootstrap set");

    // 故障注入：停止 L2
    container
        .stop_with_timeout(Some(0))
        .await
        .expect("stop redis container");

    // L1 命中不回落：L2 断连不影响 L1 读取
    assert_eq!(
        chain.get("degrade:key").await.unwrap(),
        Some(b"v1".to_vec()),
        "an L1-hitting read must not fail because L2 is disconnected"
    );

    // L1 miss + L2 失败 → 部分明确 miss，整体降级为 Ok(None)（不整体失败）
    let ghost = chain
        .get("degrade:ghost")
        .await
        .expect("get should not fail wholesale when some backends fail");
    assert_eq!(
        ghost, None,
        "an L1 miss plus an L2 disconnect should degrade to a miss"
    );

    // 写：L1 成功 + L2 失败 → 部分成功语义（Ok）
    chain
        .set("degrade:write", b"v2".to_vec(), None)
        .await
        .expect("set should keep success semantics when some backends fail");
    assert_eq!(
        chain.get("degrade:write").await.unwrap(),
        Some(b"v2".to_vec()),
        "writes during the disconnect should remain in L1"
    );

    // 恢复：重启容器并就绪；恢复后写入必须落 L2
    container.start().await.expect("restart redis container");
    assert!(
        docker_wait_for_redis(&redis_url, 30).await,
        "Redis was not ready within 30s after the container restart"
    );
    chain
        .set("degrade:recovered", b"v3".to_vec(), None)
        .await
        .expect("write after recovery");

    // 新实例（空 L1、独立连接）必须经 L2 读穿 → 证明恢复后的写入已落 L2
    let chain2 = build_chain().await;
    assert_eq!(
        chain2.get("degrade:recovered").await.unwrap(),
        Some(b"v3".to_vec()),
        "after recovery, writes should write through L2 and be readable by new instances"
    );
    // 断连期间只落 L1 的写入不应出现在 L2（部分成功语义的边界）
    assert_eq!(
        chain2.get("degrade:write").await.unwrap(),
        None,
        "writes during the disconnect must not appear in L2"
    );

    println!("✓ Two-tier cache disconnect degradation and recovery test passed");
}

#[tokio::test]
async fn test_timeout_handling() {
    println!("=== Timeout handling test ===");

    if !is_redis_available().await {
        println!("Skipping test: Redis unavailable");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 设置大量数据，测试超时处理
    let large_data = vec![0u8; 10 * 1024 * 1024]; // 10MB

    let result = tokio::time::timeout(
        Duration::from_secs(30),
        backend.set(Arc::from("timeout_key"), Arc::new(large_data.clone()), None),
    )
    .await;

    // 验证操作完成（无论成功还是超时）
    match result {
        Ok(Ok(())) => println!("large value write succeeded"),
        Ok(Err(e)) => println!("write error: {}", e),
        Err(_) => println!("operation timed out"),
    }

    backend.delete("timeout_key").await.ok();

    println!("✓ Timeout handling test passed");
}

#[tokio::test]
async fn test_retry_logic() {
    println!("=== Retry logic test ===");

    if !is_redis_available().await {
        println!("Skipping test: Redis unavailable");
        return;
    }

    let redis_url = get_redis_url();

    // 模拟重试场景
    let mut attempts = 0;
    let max_attempts = 3;
    let backend = loop {
        attempts += 1;
        match RedisBackend::new(&redis_url).await {
            Ok(b) => break b,
            Err(e) if attempts < max_attempts => {
                println!("connection attempt {} failed: {}, retrying...", attempts, e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => panic!("reached the maximum retry count: {}", e),
        }
    };

    // 验证连接成功
    backend.ping().await.unwrap();

    println!("✓ Retry logic test passed ({} attempts)", attempts);
}

#[tokio::test]
async fn test_graceful_degradation() {
    println!("=== Graceful degradation test ===");

    let cache: Cache<String, TestData> = Cache::memory().await.unwrap();

    // 模拟 Redis 不可用时的降级策略
    let fallback_data = TestData {
        id: 999,
        value: "fallback_value".to_string(),
    };

    // 使用 get_or 提供降级数据
    let result: TestData = cache
        .get_or(&"degraded_key".to_string(), || async {
            Ok(fallback_data.clone())
        })
        .await
        .unwrap();

    assert_eq!(result.id, 999);
    assert_eq!(result.value, "fallback_value");

    println!("✓ Graceful degradation test passed");
}

#[tokio::test]
async fn test_partial_failure_handling() {
    println!("=== Partial failure handling test ===");

    if !is_redis_available().await {
        println!("Skipping test: Redis unavailable");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 批量操作，部分可能失败
    let mut success_count = 0;
    let mut failure_count = 0;

    for i in 0..100 {
        let key = format!("partial_key_{}", i);
        let value = format!("value_{}", i);

        match backend
            .set(
                Arc::from(key.as_str()),
                Arc::new(value.as_bytes().to_vec()),
                None,
            )
            .await
        {
            Ok(_) => success_count += 1,
            Err(_) => failure_count += 1,
        }
    }

    println!("success: {}, failed: {}", success_count, failure_count);

    // 清理
    for i in 0..100 {
        let key = format!("partial_key_{}", i);
        backend.delete(&key).await.ok();
    }

    println!("✓ Partial failure handling test passed");
}

#[tokio::test]
async fn test_connection_pool_exhaustion() {
    println!("=== Connection pool exhaustion test ===");

    if !is_redis_available().await {
        println!("Skipping test: Redis unavailable");
        return;
    }

    let redis_url = get_redis_url();

    // 创建多个连接
    let mut backends = Vec::new();
    for i in 0..10 {
        match RedisBackend::new(&redis_url).await {
            Ok(backend) => backends.push(backend),
            Err(e) => {
                println!("connection {} creation failed: {}", i, e);
                break;
            }
        }
    }

    println!("created {} connections", backends.len());

    // 所有连接执行操作
    for (i, backend) in backends.iter().enumerate() {
        let key = format!("pool_key_{}", i);
        backend
            .set(Arc::from(key.as_str()), Arc::new(b"test".to_vec()), None)
            .await
            .ok();
    }

    // 清理
    for (i, backend) in backends.iter().enumerate() {
        let key = format!("pool_key_{}", i);
        backend.delete(&key).await.ok();
    }

    println!("✓ Connection pool exhaustion test passed");
}

#[tokio::test]
async fn test_network_latency_simulation() {
    println!("=== Network latency simulation test ===");

    if !is_redis_available().await {
        println!("Skipping test: Redis unavailable");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 测量操作延迟
    let start = std::time::Instant::now();

    for _ in 0..100 {
        backend
            .set(
                Arc::from("latency_key"),
                Arc::new(b"latency_value".to_vec()),
                None,
            )
            .await
            .unwrap();
        backend.get("latency_key").await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_millis() / 200;

    println!(
        "total time: {:?}, average latency: {}ms",
        elapsed, avg_latency
    );

    backend.delete("latency_key").await.unwrap();

    println!("✓ Network latency simulation test passed");
}

#[tokio::test]
async fn test_memory_cache_under_stress() {
    println!("=== In-memory cache stress test ===");

    let cache: Cache<String, TestData> = Cache::memory().await.unwrap();
    let cache = Arc::new(Mutex::new(cache));

    // 高并发压力测试
    let mut handles = Vec::new();

    for thread_id in 0..20 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = format!("stress_{}_{}", thread_id, i);
                let data = TestData {
                    id: thread_id * 1000 + i,
                    value: format!("stress_value_{}_{}", thread_id, i),
                };

                cache.lock().await.set(&key, &data).await.unwrap();
                let _: Option<TestData> = cache.lock().await.get(&key).await.unwrap();
                cache.lock().await.delete(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!("✓ In-memory cache stress test passed");
}
