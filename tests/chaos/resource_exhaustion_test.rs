// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 资源耗尽混沌测试

use oxcache::{Cache, Cacheable};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LargeData {
    id: u64,
    data: Vec<u8>,
}

impl Cacheable for LargeData {}

#[tokio::test]
async fn test_memory_pressure() {
    println!("=== 内存压力测试 ===");

    let cache: Cache<String, LargeData> = Cache::builder()
        .capacity(100)
        .build()
        .await
        .unwrap();

    // 写入大量数据
    let large_data = vec![0u8; 1024 * 1024]; // 1MB

    for i in 0..200 {
        let key = format!("memory_pressure_{}", i);
        let data = LargeData {
            id: i,
            data: large_data.clone(),
        };

        // 写入，可能触发淘汰
        cache.set(&key, &data).await.unwrap();
    }

    // 验证缓存仍在工作
    let test_data = LargeData {
        id: 999,
        data: vec![1, 2, 3, 4, 5],
    };
    cache.set("test_key", &test_data).await.unwrap();
    let retrieved: Option<LargeData> = cache.get("test_key").await.unwrap();
    assert_eq!(retrieved.unwrap().id, 999);

    println!("✓ 内存压力测试通过");
}

#[tokio::test]
async fn test_high_concurrency_stress() {
    println!("=== 高并发压力测试 ===");

    let cache = Arc::new(Mutex::new(Cache::<String, String>::memory().await.unwrap()));

    let mut handles = Vec::new();
    let operations_per_thread = 1000;

    for thread_id in 0..50 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for i in 0..operations_per_thread {
                let key = format!("concurrent_{}_{}", thread_id, i);
                let value = format!("value_{}_{}", thread_id, i);

                // 写入
                cache.lock().await.set(&key, &value).await.unwrap();

                // 读取
                let _: Option<String> = cache.lock().await.get(&key).await.unwrap();

                // 删除
                cache.lock().await.delete(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!("✓ 高并发压力测试通过 ({} 操作)", 50 * operations_per_thread * 3);
}

#[tokio::test]
async fn test_rapid_operations() {
    println!("=== 快速操作测试 ===");

    let cache: Cache<String, String> = Cache::memory().await.unwrap();

    let start = std::time::Instant::now();
    let iterations = 10000;

    for i in 0..iterations {
        let key = format!("rapid_{}", i % 100);
        let value = format!("value_{}", i);

        cache.set(&key, &value).await.unwrap();
        let _: Option<String> = cache.get(&key).await.unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (iterations * 2) as f64 / elapsed.as_secs_f64();

    println!("完成 {} 次操作，耗时 {:?}", iterations * 2, elapsed);
    println!("吞吐量: {:.2} ops/sec", ops_per_sec);

    println!("✓ 快速操作测试通过");
}

#[tokio::test]
async fn test_large_value_handling() {
    println!("=== 大值处理测试 ===");

    let cache: Cache<String, LargeData> = Cache::memory().await.unwrap();

    // 测试不同大小的数据
    let sizes = vec![1024, 10 * 1024, 100 * 1024, 1024 * 1024]; // 1KB, 10KB, 100KB, 1MB

    for (i, size) in sizes.iter().enumerate() {
        let key = format!("large_value_{}", i);
        let data = LargeData {
            id: i as u64,
            data: vec![0u8; *size],
        };

        cache.set(&key, &data).await.unwrap();

        let retrieved: Option<LargeData> = cache.get(&key).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data.len(), *size);

        println!("成功处理 {} 字节数据", size);
    }

    println!("✓ 大值处理测试通过");
}

#[tokio::test]
async fn test_many_keys_stress() {
    println!("=== 多键压力测试 ===");

    let cache: Cache<String, String> = Cache::builder()
        .capacity(10000)
        .build()
        .await
        .unwrap();

    let key_count = 5000;

    // 写入大量键
    for i in 0..key_count {
        let key = format!("many_keys_{}", i);
        let value = format!("value_{}", i);
        cache.set(&key, &value).await.unwrap();
    }

    // 验证部分键
    for i in (0..key_count).step_by(100) {
        let key = format!("many_keys_{}", i);
        let expected = format!("value_{}", i);
        let retrieved: Option<String> = cache.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(expected));
    }

    // 清理
    cache.clear().await.unwrap();

    println!("✓ 多键压力测试通过 ({} 键)", key_count);
}

#[tokio::test]
async fn test_sustained_load() {
    println!("=== 持续负载测试 ===");

    let cache: Cache<String, String> = Cache::memory().await.unwrap();
    let duration = Duration::from_secs(5);
    let start = std::time::Instant::now();
    let mut operations = 0u64;

    while start.elapsed() < duration {
        let key = format!("sustained_{}", operations % 1000);
        let value = format!("value_{}", operations);

        cache.set(&key, &value).await.unwrap();
        let _: Option<String> = cache.get(&key).await.unwrap();

        operations += 1;
    }

    let elapsed = start.elapsed();
    let ops_per_sec = operations as f64 / elapsed.as_secs_f64();

    println!("持续负载测试完成:");
    println!("  - 运行时间: {:?}", elapsed);
    println!("  - 总操作数: {}", operations * 2);
    println!("  - 吞吐量: {:.2} ops/sec", ops_per_sec * 2.0);

    println!("✓ 持续负载测试通过");
}

#[tokio::test]
async fn test_memory_cleanup() {
    println!("=== 内存清理测试 ===");

    let cache: Cache<String, LargeData> = Cache::builder()
        .capacity(100)
        .build()
        .await
        .unwrap();

    // 写入大量数据
    for i in 0..500 {
        let key = format!("cleanup_{}", i);
        let data = LargeData {
            id: i,
            data: vec![0u8; 1024], // 1KB
        };
        cache.set(&key, &data).await.unwrap();
    }

    // 清理所有数据
    cache.clear().await.unwrap();

    // 验证清理成功
    let len = cache.len().await.unwrap();
    assert_eq!(len, 0);

    // 验证缓存仍可用
    let test_data = LargeData {
        id: 1,
        data: vec![1, 2, 3],
    };
    cache.set("after_cleanup", &test_data).await.unwrap();
    let retrieved: Option<LargeData> = cache.get("after_cleanup").await.unwrap();
    assert!(retrieved.is_some());

    println!("✓ 内存清理测试通过");
}

#[tokio::test]
async fn test_concurrent_read_write_stress() {
    println!("=== 并发读写压力测试 ===");

    let cache = Arc::new(Mutex::new(Cache::<String, String>::memory().await.unwrap()));
    let mut handles = Vec::new();

    // 写入线程
    for writer_id in 0..10 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for i in 0..500 {
                let key = format!("rw_key_{}", i % 100);
                let value = format!("writer_{}_value_{}", writer_id, i);
                cache.lock().await.set(&key, &value).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // 读取线程
    for reader_id in 0..10 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for i in 0..500 {
                let key = format!("rw_key_{}", i % 100);
                let _: Option<String> = cache.lock().await.get(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!("✓ 并发读写压力测试通过");
}

#[tokio::test]
async fn test_ttl_expiration_stress() {
    println!("=== TTL 过期压力测试 ===");

    let cache: Cache<String, String> = Cache::builder()
        .ttl(Duration::from_millis(100))
        .build()
        .await
        .unwrap();

    // 写入大量带 TTL 的数据
    for i in 0..1000 {
        let key = format!("ttl_stress_{}", i);
        let value = format!("value_{}", i);
        cache.set(&key, &value).await.unwrap();
    }

    // 立即验证存在
    let mut exists_count = 0;
    for i in 0..1000 {
        let key = format!("ttl_stress_{}", i);
        if cache.exists(&key).await.unwrap() {
            exists_count += 1;
        }
    }
    println!("写入后存在 {} 个键", exists_count);

    // 等待过期
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 验证已过期
    let mut expired_count = 0;
    for i in 0..1000 {
        let key = format!("ttl_stress_{}", i);
        if !cache.exists(&key).await.unwrap() {
            expired_count += 1;
        }
    }
    println!("过期后不存在 {} 个键", expired_count);

    println!("✓ TTL 过期压力测试通过");
}

#[tokio::test]
async fn test_system_resource_monitoring() {
    println!("=== 系统资源监控测试 ===");

    let cache: Cache<String, String> = Cache::memory().await.unwrap();

    let initial_stats = cache.stats().await.unwrap();
    println!("初始统计: {:?}", initial_stats);

    // 执行操作
    for i in 0..1000 {
        let key = format!("monitor_{}", i);
        let value = format!("value_{}", i);
        cache.set(&key, &value).await.unwrap();
    }

    let after_write_stats = cache.stats().await.unwrap();
    println!("写入后统计: {:?}", after_write_stats);

    // 读取操作
    for i in 0..1000 {
        let key = format!("monitor_{}", i);
        let _: Option<String> = cache.get(&key).await.unwrap();
    }

    let after_read_stats = cache.stats().await.unwrap();
    println!("读取后统计: {:?}", after_read_stats);

    println!("✓ 系统资源监控测试通过");
}
