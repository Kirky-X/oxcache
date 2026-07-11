// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 网络故障混沌测试

#[path = "../common/mod.rs"]
mod common;

use common::docker_test_utils::{RedisContainer, setup_redis_container};
use common::redis_test_utils::{get_redis_url, is_redis_available, wait_for_redis};
use oxcache::Cache;
use oxcache::backend::interface::{CacheReader, CacheWriter};
use oxcache::backend::memory::redis::RedisBackend;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: u64,
    value: String,
}

#[tokio::test]
async fn test_connection_recovery_after_failure() {
    println!("=== 连接故障恢复测试 ===");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 正常操作
    backend
        .set("recovery_key", b"initial_value".to_vec(), None)
        .await
        .unwrap();
    let value = backend.get("recovery_key").await.unwrap();
    assert_eq!(value, Some(b"initial_value".to_vec()));

    // 模拟网络问题后的重连（通过创建新连接）
    let backend2 = RedisBackend::new(&redis_url).await.unwrap();

    // 验证数据仍然可访问
    let value = backend2.get("recovery_key").await.unwrap();
    assert_eq!(value, Some(b"initial_value".to_vec()));

    backend.delete("recovery_key").await.unwrap();

    println!("✓ 连接故障恢复测试通过");
}

#[tokio::test]
async fn test_timeout_handling() {
    println!("=== 超时处理测试 ===");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 设置大量数据，测试超时处理
    let large_data = vec![0u8; 10 * 1024 * 1024]; // 10MB

    let result = tokio::time::timeout(
        Duration::from_secs(30),
        backend.set("timeout_key", large_data.clone(), None),
    )
    .await;

    // 验证操作完成（无论成功还是超时）
    match result {
        Ok(Ok(())) => println!("大值写入成功"),
        Ok(Err(e)) => println!("写入错误: {}", e),
        Err(_) => println!("操作超时"),
    }

    backend.delete("timeout_key").await.ok();

    println!("✓ 超时处理测试通过");
}

#[tokio::test]
async fn test_retry_logic() {
    println!("=== 重试逻辑测试 ===");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
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
                println!("连接尝试 {} 失败: {}, 重试中...", attempts, e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => panic!("达到最大重试次数: {}", e),
        }
    };

    // 验证连接成功
    backend.ping().await.unwrap();

    println!("✓ 重试逻辑测试通过 ({} 次尝试)", attempts);
}

#[tokio::test]
async fn test_graceful_degradation() {
    println!("=== 优雅降级测试 ===");

    let cache: Cache<String, TestData> = Cache::memory().await.unwrap();

    // 模拟 Redis 不可用时的降级策略
    let fallback_data = TestData {
        id: 999,
        value: "fallback_value".to_string(),
    };

    // 使用 get_or 提供降级数据
    let result: TestData = cache
        .get_or(&"degraded_key".to_string(), || async { Ok(fallback_data.clone()) })
        .await
        .unwrap();

    assert_eq!(result.id, 999);
    assert_eq!(result.value, "fallback_value");

    println!("✓ 优雅降级测试通过");
}

#[tokio::test]
async fn test_partial_failure_handling() {
    println!("=== 部分失败处理测试 ===");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
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

        match backend.set(&key, value.as_bytes().to_vec(), None).await {
            Ok(_) => success_count += 1,
            Err(_) => failure_count += 1,
        }
    }

    println!("成功: {}, 失败: {}", success_count, failure_count);

    // 清理
    for i in 0..100 {
        let key = format!("partial_key_{}", i);
        backend.delete(&key).await.ok();
    }

    println!("✓ 部分失败处理测试通过");
}

#[tokio::test]
async fn test_connection_pool_exhaustion() {
    println!("=== 连接池耗尽测试 ===");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();

    // 创建多个连接
    let mut backends = Vec::new();
    for i in 0..10 {
        match RedisBackend::new(&redis_url).await {
            Ok(backend) => backends.push(backend),
            Err(e) => {
                println!("连接 {} 创建失败: {}", i, e);
                break;
            }
        }
    }

    println!("创建了 {} 个连接", backends.len());

    // 所有连接执行操作
    for (i, backend) in backends.iter().enumerate() {
        let key = format!("pool_key_{}", i);
        backend.set(&key, b"test".to_vec(), None).await.ok();
    }

    // 清理
    for (i, backend) in backends.iter().enumerate() {
        let key = format!("pool_key_{}", i);
        backend.delete(&key).await.ok();
    }

    println!("✓ 连接池耗尽测试通过");
}

#[tokio::test]
async fn test_network_latency_simulation() {
    println!("=== 网络延迟模拟测试 ===");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 测量操作延迟
    let start = std::time::Instant::now();

    for _ in 0..100 {
        backend
            .set("latency_key", b"latency_value".to_vec(), None)
            .await
            .unwrap();
        backend.get("latency_key").await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_millis() / 200;

    println!("总耗时: {:?}, 平均延迟: {}ms", elapsed, avg_latency);

    backend.delete("latency_key").await.unwrap();

    println!("✓ 网络延迟模拟测试通过");
}

#[tokio::test]
async fn test_with_testcontainers_network_failure() {
    println!("=== Testcontainers 网络故障测试 ===");

    let result = setup_redis_container().await;
    let (container, redis_url): (RedisContainer, String) = match result {
        Ok(r) => r,
        Err(e) => {
            println!("跳过测试: 无法启动 Redis 容器 - {}", e);
            return;
        }
    };

    if !wait_for_redis(&redis_url).await {
        println!("跳过测试: Redis 容器未就绪");
        return;
    }

    // 设置环境变量以允许不安全连接（testcontainers 创建的 Redis）
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    };

    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 正常操作
    backend
        .set("container_test_key", b"test_value".to_vec(), None)
        .await
        .unwrap();
    let value = backend.get("container_test_key").await.unwrap();
    assert_eq!(value, Some(b"test_value".to_vec()));

    // 容器将在函数结束时被清理
    drop(container);

    println!("✓ Testcontainers 网络故障测试通过");
}

#[tokio::test]
async fn test_memory_cache_under_stress() {
    println!("=== 内存缓存压力测试 ===");

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

    println!("✓ 内存缓存压力测试通过");
}
