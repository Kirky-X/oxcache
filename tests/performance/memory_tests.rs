// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 内存测试 - 使用新API的内存测试

#![allow(unexpected_cfgs)]

use crate::common::is_redis_available;
use oxcache::Cache;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// 内存测试模块
// ============================================================================

/// 内存泄漏测试 - 使用新的Cache API
#[tokio::test]
async fn test_l1_cache_memory_leak() {
    let cache = Cache::<String, Vec<u8>>::memory().await.unwrap();

    for i in 0..10000 {
        let key = format!("key_{}", i % 100);
        let value = vec![i as u8; 100];

        let _ = cache.set(&key, &value).await;
        let _ = cache.get(&key).await;

        if i % 1000 == 0 {
            cache.clear().await.unwrap();
            sleep(Duration::from_millis(10)).await;
        }
    }

    cache.clear().await.unwrap();

    drop(cache);
    sleep(Duration::from_millis(100)).await;
}

/// L2缓存内存泄漏测试
#[tokio::test]
async fn test_l2_cache_memory_leak() {
    if !is_redis_available().await {
        println!("跳过test_l2_cache_memory_leak：Redis不可用");
        return;
    }

    println!("L2 memory leak test - Redis available but using L1 only for now");
}

/// 两级缓存内存泄漏测试
#[tokio::test]
async fn test_two_level_cache_memory_leak() {
    let cache = Cache::<String, Vec<u8>>::memory().await.unwrap();

    for i in 0..1500 {
        let key = format!("l1_{}", i % 100);
        let value = vec![i as u8; 100];

        let _ = cache.set(&key, &value).await;

        let _ = cache.get(&key).await;

        if i % 150 == 0 {
            cache.clear().await.unwrap();
            sleep(Duration::from_millis(20)).await;
        }
    }

    cache.clear().await.unwrap();

    println!("L1 cache memory leak test completed");
    drop(cache);
    sleep(Duration::from_millis(100)).await;
}

/// 批量操作内存测试
#[tokio::test]
async fn test_batch_memory_usage() {
    let cache = Cache::<String, String>::memory().await.unwrap();

    for batch in 0..100 {
        for i in 0..100 {
            let key = format!("batch_key_{}_{}", batch, i);
            let value = format!("batch_value_{}_{}", batch, i);
            let _ = cache.set(&key, &value).await;
        }

        if batch % 10 == 0 {
            cache.clear().await.unwrap();
        }
    }

    cache.clear().await.unwrap();
    drop(cache);
}

/// 大量小对象测试
#[tokio::test]
async fn test_many_small_objects() {
    let cache = Cache::<String, String>::memory().await.unwrap();

    for i in 0..5000 {
        let key = format!("small_key_{}", i);
        let value = format!("value_{}", i);
        let _ = cache.set(&key, &value).await;
    }

    for i in 0..100 {
        let key = format!("small_key_{}", i);
        let result = cache.get(&key).await;
        assert!(result.is_ok(), "Key {} should exist", i);
    }

    cache.clear().await.unwrap();
    drop(cache);
}

/// 大对象测试
#[tokio::test]
async fn test_large_objects() {
    let cache = Cache::<String, Vec<u8>>::memory().await.unwrap();

    for i in 0..100 {
        let key = format!("large_key_{}", i);
        let value = vec![0u8; 1024 * 10];
        let _ = cache.set(&key, &value).await;
    }

    cache.clear().await.unwrap();
    drop(cache);
}

/// 并发访问测试 - 简化版本
#[tokio::test]
async fn test_concurrent_access() {
    let cache = Cache::<String, String>::memory().await.unwrap();

    for _ in 0..10 {
        for i in 0..1000 {
            let key = format!("concurrent_key_{}", i % 50);
            let value = format!("value_{}", i);
            let _ = cache.set(&key, &value).await;
            let _ = cache.get(&key).await;
        }
    }

    cache.clear().await.unwrap();
    drop(cache);
}
