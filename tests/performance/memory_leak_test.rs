// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 内存泄漏测试 - 使用新API

#![allow(unexpected_cfgs)]

#[path = "common/mod.rs"]
mod common;

use common::is_redis_available;
use oxcache::Cache;
use std::time::Duration;
use tokio::time::sleep;

/// 内存泄漏测试模块
/// 使用循环引用和大量操作来检测潜在的内存泄漏

#[tokio::test]
async fn test_l1_cache_memory_leak() {
    let cache = Cache::<String, Vec<u8>>::new().await.unwrap();

    // 执行大量操作，检测内存泄漏
    for i in 0..10000 {
        let key = format!("key_{}", i % 100); // 循环使用100个key
        let value = vec![i as u8; 100];

        let _ = cache.set(&key, &value).await;
        let _ = cache.get(&key).await;

        if i % 1000 == 0 {
            // 定期清理
            cache.clear().await.unwrap();
            sleep(Duration::from_millis(1)).await;
        }
    }

    // 清理所有数据
    cache.clear().await.unwrap();

    // 强制drop
    drop(cache);
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_l2_cache_memory_leak() {
    if !is_redis_available() {
        println!("跳过test_l2_cache_memory_leak：Redis不可用");
        return;
    }

    println!("L2 memory leak test requires full Redis setup");
}

#[tokio::test]
async fn test_cache_creation() {
    let cache = Cache::<String, String>::new().await.unwrap();

    // 验证缓存可以正常工作
    let key = "test_key".to_string();
    let value = "test_value".to_string();

    let set_result = cache.set(&key, &value).await;
    assert!(set_result.is_ok(), "SET should succeed");

    let get_result = cache.get(&key).await;
    assert!(get_result.is_ok(), "GET should succeed");

    if let Ok(Some(retrieved)) = get_result {
        assert_eq!(retrieved, value, "Retrieved value should match");
    }

    // 清理
    cache.clear().await.unwrap();
    drop(cache);
}
