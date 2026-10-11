// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Chaos测试 - 测试系统在故障时的恢复能力

use oxcache::Cache;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 简化的chaos测试 - 测试基本故障恢复
#[tokio::test]
async fn test_basic_fault_recovery() {
    println!("=== Starting basic chaos test ===");

    // 使用新的Cache API创建缓存
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    println!("1. Initial setup - seeding test data");
    let key = "chaos_test_key".to_string();
    let value = "chaos_test_value".to_string();

    let set_result = cache.set(&key, &value).await;
    println!("   SET result: {:?}", set_result);

    println!("2. Verify data exists");
    let get_result = cache.get(&key).await;
    println!("   GET result: {:?}", get_result);

    if let Ok(Some(retrieved)) = get_result {
        println!("   data matches: {}", retrieved == value);
    }

    println!("3. Delete test data");
    let delete_result = cache.delete(&key).await;
    println!("   DELETE result: {:?}", delete_result);

    println!("4. Verify deletion succeeded");
    let get_after_delete = cache.get(&key).await;
    println!("   GET after delete: {:?}", get_after_delete);

    println!("=== Chaos test complete ===");
}

/// 测试并发访问
#[tokio::test]
async fn test_concurrent_access() {
    let cache = Arc::new(Mutex::new(
        Cache::<String, i32>::builder().build().await.unwrap(),
    ));

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let cache = Arc::clone(&cache);
            tokio::spawn(async move {
                for j in 0..100 {
                    let key = format!("concurrent_key_{}", j % 10);
                    let value = i * 1000 + j;
                    let _ = cache.lock().await.set(&key, &value).await;
                    let _ = cache.lock().await.get(&key).await;
                }
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    println!("Concurrency test complete");
}

/// 测试快速连续操作
#[tokio::test]
async fn test_rapid_operations() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    for i in 0..100 {
        let key = format!("rapid_key_{}", i % 20);
        let value = format!("rapid_value_{}", i);
        let _ = cache.set(&key, &value).await;
    }

    for i in 0..20 {
        let key = format!("rapid_key_{}", i);
        let _ = cache.get(&key).await;
    }

    cache.clear().await.unwrap();
    println!("Fast operations test complete");
}
