// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 综合集成测试 - 简化版本

use oxcache::Cache;

#[path = "../common/mod.rs"]
mod common;

/// 测试新Cache API的综合功能
#[tokio::test]
async fn test_comprehensive_cache_api() {
    common::setup_logging();

    let cache: Cache<String, String> = Cache::new().await.unwrap();

    // 测试基本操作
    println!("1. 测试基本SET/GET操作");
    for i in 0..10 {
        let key = format!("comprehensive_key_{}", i);
        let value = format!("comprehensive_value_{}", i);
        let result = cache.set(&key, &value).await;
        assert!(result.is_ok(), "SET should succeed for key {}", i);
    }

    println!("2. 验证所有数据");
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

    println!("3. 测试批量删除");
    for i in 0..5 {
        let key = format!("comprehensive_key_{}", i);
        let result = cache.delete(&key).await;
        assert!(result.is_ok(), "DELETE should succeed for key {}", i);
    }

    println!("4. 验证删除结果");
    for i in 0..5 {
        let key = format!("comprehensive_key_{}", i);
        let result = cache.get(&key).await;
        assert!(
            result.unwrap().is_none(),
            "Value should be None for deleted key {}",
            i
        );
    }

    println!("5. 清理所有数据");
    cache.clear().await.unwrap();

    println!("综合测试完成");
}

/// 测试不同类型的Cache
#[tokio::test]
async fn test_different_cache_types() {
    let string_cache: Cache<String, String> = Cache::new().await.unwrap();
    let bytes_cache: Cache<String, Vec<u8>> = Cache::new().await.unwrap();
    let i32_cache: Cache<String, i32> = Cache::new().await.unwrap();
    let i64_cache: Cache<String, i64> = Cache::new().await.unwrap();
    let bool_cache: Cache<String, bool> = Cache::new().await.unwrap();

    // 验证每种类型都可以正常工作
    assert!(string_cache
        .set(&"k1".to_string(), &"v1".to_string())
        .await
        .is_ok());
    assert!(bytes_cache
        .set(&"k2".to_string(), &vec![1u8, 2u8])
        .await
        .is_ok());
    assert!(i32_cache.set(&"k3".to_string(), &42).await.is_ok());
    assert!(i64_cache
        .set(&"k4".to_string(), &123456789i64)
        .await
        .is_ok());
    assert!(bool_cache.set(&"k5".to_string(), &true).await.is_ok());

    // 验证读取
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
    assert_eq!(
        bool_cache.get(&"k5".to_string()).await.unwrap().unwrap(),
        true
    );
}

/// 测试边界情况
#[tokio::test]
async fn test_edge_cases() {
    let cache: Cache<String, String> = Cache::new().await.unwrap();

    // 空键测试
    assert!(cache
        .set(&"".to_string(), &"empty_key".to_string())
        .await
        .is_ok());
    assert!(cache.get(&"".to_string()).await.unwrap().is_some());

    // 空值测试
    assert!(cache
        .set(&"empty_value".to_string(), &"".to_string())
        .await
        .is_ok());
    assert!(cache
        .get(&"empty_value".to_string())
        .await
        .unwrap()
        .is_some());

    // 长键测试
    let long_key = "k".repeat(256);
    assert!(cache.set(&long_key, &"long_key".to_string()).await.is_ok());

    // 清理
    cache.clear().await.unwrap();
}
