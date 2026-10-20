// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 综合集成测试 - 简化版本

use crate::common::setup_logging;
use oxcache::Cache;

/// 测试新Cache API的综合功能
#[tokio::test]
async fn test_comprehensive_cache_api() {
    setup_logging();

    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    // 测试基本操作
    println!("1. Testing basic SET/GET operations");
    for i in 0..10 {
        let key = format!("comprehensive_key_{}", i);
        let value = format!("comprehensive_value_{}", i);
        let result = cache.set(&key, &value).await;
        assert!(result.is_ok(), "SET should succeed for key {}", i);
    }

    println!("2. Verifying all data");
    for i in 0..10 {
        let key = format!("comprehensive_key_{}", i);
        let result = cache.get(&key).await;
        assert!(result.is_ok(), "GET should succeed for key {}", i);
        assert!(
            result.unwrap().is_some(),
            "Value should exist for key {}",
            i
        );
    }

    println!("3. Testing batch delete");
    for i in 0..5 {
        let key = format!("comprehensive_key_{}", i);
        let result = cache.delete(&key).await;
        assert!(result.is_ok(), "DELETE should succeed for key {}", i);
    }

    println!("4. Verifying the deletion result");
    for i in 0..5 {
        let key = format!("comprehensive_key_{}", i);
        let result = cache.get(&key).await;
        assert!(
            result.unwrap().is_none(),
            "Value should be None for deleted key {}",
            i
        );
    }

    println!("5. Cleaning up all data");
    cache.clear().await.unwrap();

    println!("Comprehensive test complete");
}

/// 测试不同类型的Cache
#[tokio::test]
async fn test_different_cache_types() {
    let string_cache: Cache<String, String> = Cache::builder().build().await.unwrap();
    let bytes_cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let i32_cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
    let i64_cache: Cache<String, i64> = Cache::builder().build().await.unwrap();
    let bool_cache: Cache<String, bool> = Cache::builder().build().await.unwrap();

    assert!(
        string_cache
            .set(&"k1".to_string(), &"v1".to_string())
            .await
            .is_ok()
    );
    assert!(
        bytes_cache
            .set(&"k2".to_string(), &vec![1u8, 2u8])
            .await
            .is_ok()
    );
    assert!(i32_cache.set(&"k3".to_string(), &42).await.is_ok());
    assert!(
        i64_cache
            .set(&"k4".to_string(), &123456789i64)
            .await
            .is_ok()
    );
    assert!(bool_cache.set(&"k5".to_string(), &true).await.is_ok());

    assert_eq!(
        string_cache.get(&"k1".to_string()).await.unwrap().unwrap(),
        "v1"
    );
    assert_eq!(
        bytes_cache.get(&"k2".to_string()).await.unwrap().unwrap(),
        vec![1u8, 2u8]
    );
    assert_eq!(i32_cache.get(&"k3".to_string()).await.unwrap().unwrap(), 42);
    assert_eq!(
        i64_cache.get(&"k4".to_string()).await.unwrap().unwrap(),
        123456789i64
    );
    assert!(bool_cache.get(&"k5".to_string()).await.unwrap().unwrap());
}

/// 测试边界情况
#[tokio::test]
async fn test_edge_cases() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    // 空键测试
    assert!(
        cache
            .set(&"".to_string(), &"empty_key".to_string())
            .await
            .is_ok()
    );
    assert!(cache.get(&"".to_string()).await.unwrap().is_some());

    // 空值测试
    assert!(
        cache
            .set(&"empty_value".to_string(), &"".to_string())
            .await
            .is_ok()
    );
    assert!(
        cache
            .get(&"empty_value".to_string())
            .await
            .unwrap()
            .is_some()
    );

    // 长键测试
    let long_key = "k".repeat(256);
    assert!(cache.set(&long_key, &"long_key".to_string()).await.is_ok());

    cache.clear().await.unwrap();
}
