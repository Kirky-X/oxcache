// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// E2E Tests - Simplified version

extern crate oxcache;

use crate::common;
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct User {
    id: u64,
    name: String,
}

/// 端到端测试 - 基本Cache功能
#[tokio::test]
async fn test_e2e_basic_cache() {
    let cache: Cache<String, String> = Cache::new().await.unwrap();

    // 测试基本SET/GET/DELETE
    let key = "e2e_test_key".to_string();
    let value = "e2e_test_value".to_string();

    // SET
    let set_result = cache.set(&key, &value).await;
    assert!(set_result.is_ok(), "SET should succeed");

    // GET
    let get_result = cache.get(&key).await;
    assert!(get_result.is_ok(), "GET should succeed");
    assert!(get_result.unwrap().is_some(), "Value should exist");

    // DELETE
    let delete_result = cache.delete(&key).await;
    assert!(delete_result.is_ok(), "DELETE should succeed");

    // Verify deletion
    let get_after_delete = cache.get(&key).await;
    assert!(
        get_after_delete.unwrap().is_none(),
        "Value should be None after delete"
    );
}

/// 端到端测试 - User结构体缓存
#[tokio::test]
async fn test_e2e_user_cache() {
    let cache: Cache<String, User> = Cache::new().await.unwrap();

    let user = User {
        id: 1,
        name: "Test User".to_string(),
    };

    // SET
    let key = "user:1".to_string();
    let set_result = cache.set(&key, &user).await;
    assert!(set_result.is_ok(), "SET User should succeed");

    // GET
    let get_result = cache.get(&key).await;
    assert!(get_result.is_ok(), "GET User should succeed");

    if let Ok(Some(retrieved)) = get_result {
        assert_eq!(retrieved.id, 1);
        assert_eq!(retrieved.name, "Test User");
    }

    // DELETE
    let _ = cache.delete(&key).await;
}

/// 端到端测试 - 批量操作
#[tokio::test]
async fn test_e2e_batch_operations() {
    let cache: Cache<String, i32> = Cache::new().await.unwrap();

    // 批量写入
    for i in 0..10 {
        let key = format!("batch_key_{}", i);
        let value = i * 10;
        let result = cache.set(&key, &value).await;
        assert!(result.is_ok(), "Batch SET {} should succeed", i);
    }

    // 批量读取
    for i in 0..10 {
        let key = format!("batch_key_{}", i);
        let result = cache.get(&key).await;
        assert!(result.is_ok(), "Batch GET {} should succeed", i);
        if let Ok(Some(value)) = result {
            assert_eq!(value, i * 10);
        }
    }

    // 清理
    cache.clear().await.unwrap();
}
