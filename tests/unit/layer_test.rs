// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 分层缓存测试 - 使用新API

use oxcache::Cache;

#[path = "../common/mod.rs"]
mod common;

/// 测试L1-only缓存模式
#[tokio::test]
async fn test_l1_only_mode() {
    common::setup_logging();

    // 使用新的Cache API创建L1缓存
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    // 验证缓存可以正常工作
    let test_key = "l1_test_key".to_string();
    let test_value = "l1_test_value".to_string();

    let set_result = cache.set(&test_key, &test_value).await;
    assert!(set_result.is_ok(), "L1 SET should succeed");

    let get_result = cache.get(&test_key).await;
    assert!(get_result.is_ok(), "L1 GET should succeed");

    if let Ok(Some(retrieved)) = get_result {
        assert_eq!(retrieved, test_value, "Retrieved value should match");
    }

    // 清理
    let _ = cache.delete(&test_key).await;
}

/// 测试缓存基本操作
#[tokio::test]
async fn test_cache_basic_operations() {
    common::setup_logging();

    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();

    // 测试SET/GET
    let key = "basic_test".to_string();
    let value = vec![1u8, 2u8, 3u8];

    let set_result = cache.set(&key, &value).await;
    assert!(set_result.is_ok(), "SET should succeed");

    let get_result = cache.get(&key).await;
    assert!(get_result.is_ok(), "GET should succeed");

    if let Ok(Some(retrieved)) = get_result {
        assert_eq!(retrieved, value, "Retrieved value should match");
    }

    // 测试DELETE
    let delete_result = cache.delete(&key).await;
    assert!(delete_result.is_ok(), "DELETE should succeed");

    // 验证删除
    let get_after_delete = cache.get(&key).await;
    assert!(get_after_delete.is_ok(), "GET after delete should succeed");
    assert!(
        get_after_delete.unwrap().is_none(),
        "Value should be None after delete"
    );
}

/// 测试clear操作
#[tokio::test]
async fn test_cache_clear() {
    common::setup_logging();

    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    // 填充数据
    for i in 0..10 {
        let key = format!("clear_test_{}", i);
        let value = format!("value_{}", i);
        let _ = cache.set(&key, &value).await;
    }

    // 验证数据存在
    let get_result = cache.get(&"clear_test_5".to_string()).await;
    assert!(get_result.is_ok(), "GET should succeed");
    assert!(
        get_result.unwrap().is_some(),
        "Key should exist before clear"
    );

    // 清空缓存
    let clear_result = cache.clear().await;
    assert!(clear_result.is_ok(), "CLEAR should succeed");

    // 验证数据已被清空
    let get_after_clear = cache.get(&"clear_test_5".to_string()).await;
    assert!(get_after_clear.is_ok(), "GET after clear should succeed");
    assert!(
        get_after_clear.unwrap().is_none(),
        "Value should be None after clear"
    );
}
