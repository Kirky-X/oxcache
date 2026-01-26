// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 配置单元测试 - 使用新API

use oxcache::Cache;

mod common;

/// 测试新Cache API的基本用法
#[tokio::test]
async fn test_new_cache_api() {
    common::setup_logging();

    // 使用新的Cache API
    let cache: Cache<String, String> = Cache::new().await.unwrap();

    // 测试基本SET/GET
    let key = "config_test_key".to_string();
    let value = "config_test_value".to_string();

    let set_result = cache.set(&key, &value).await;
    assert!(set_result.is_ok(), "SET should succeed");

    let get_result = cache.get(&key).await;
    assert!(get_result.is_ok(), "GET should succeed");

    if let Ok(Some(retrieved)) = get_result {
        assert_eq!(retrieved, value, "Retrieved value should match");
    }

    // 清理
    cache.clear().await.unwrap();
}

/// 测试Cache类型参数
#[tokio::test]
async fn test_cache_types() {
    // 测试不同类型的Cache
    let string_cache: Cache<String, String> = Cache::new().await.unwrap();
    let bytes_cache: Cache<String, Vec<u8>> = Cache::new().await.unwrap();
    let i32_cache: Cache<String, i32> = Cache::new().await.unwrap();

    // 验证每种类型都可以正常工作
    assert!(string_cache
        .set(&"key1".to_string(), &"value".to_string())
        .await
        .is_ok());
    assert!(bytes_cache
        .set(&"key2".to_string(), &vec![1u8, 2u8, 3u8])
        .await
        .is_ok());
    assert!(i32_cache.set(&"key3".to_string(), &42).await.is_ok());
}

/// 测试Cache选项
#[tokio::test]
async fn test_cache_options() {
    let cache: Cache<String, String> = Cache::new().await.unwrap();

    // 测试SET/GET/DELETE/CLEAR
    for i in 0..5 {
        let key = format!("option_test_{}", i);
        let value = format!("value_{}", i);
        assert!(cache.set(&key, &value).await.is_ok());
    }

    // 验证数据存在
    for i in 0..5 {
        let key = format!("option_test_{}", i);
        let result = cache.get(&key).await;
        assert!(result.is_ok(), "GET should succeed for key {}", i);
        assert!(
            result.unwrap().is_some(),
            "Value should exist for key {}",
            i
        );
    }

    // 测试DELETE
    assert!(cache.delete(&"option_test_0".to_string()).await.is_ok());
    let result = cache.get(&"option_test_0".to_string()).await;
    assert!(result.is_ok(), "GET should succeed");
    assert!(
        result.unwrap().is_none(),
        "Value should be None after delete"
    );

    // 测试CLEAR
    cache.clear().await.unwrap();
    let result = cache.get(&"option_test_1".to_string()).await;
    assert!(result.is_ok(), "GET should succeed after clear");
    assert!(
        result.unwrap().is_none(),
        "Value should be None after clear"
    );
}
