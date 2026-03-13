// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 测试真实RedisBackend与Redis的集成

#![cfg(feature = "redis")]

use crate::common::redis_test_utils::{is_redis_available, test_redis_connection};
use oxcache::backend::client::redis::RedisBackend;
use oxcache::backend::CacheBackend;

/// 测试真实RedisBackend创建
#[tokio::test]
async fn test_real_redis_backend_creation() {
    println!("测试真实RedisBackend创建...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let redis_url = "redis://127.0.0.1:6379";

    match RedisBackend::new(redis_url).await {
        Ok(backend) => {
            println!("成功创建真实RedisBackend");
            // 验证ping操作
            let ping_result = backend.ping().await;
            assert!(ping_result.is_ok(), "Ping should succeed");
        }
        Err(e) => {
            println!("创建真实后端失败: {:?}", e);
            panic!("应该能成功创建真实后端: {}", e);
        }
    }
}

/// 测试Redis连接和基本操作
#[tokio::test]
async fn test_redis_basic_operations() {
    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = "redis://127.0.0.1:6379";
    let backend = RedisBackend::new(redis_url).await.unwrap();

    // 测试SET/GET
    let test_key = "test:basic:key";
    let test_value = b"test_value";

    let set_result = backend.set(test_key, test_value.to_vec(), None).await;
    assert!(set_result.is_ok(), "SET should succeed");

    let get_result = backend.get(test_key).await;
    assert!(get_result.is_ok(), "GET should succeed");

    if let Ok(Some(retrieved)) = get_result {
        assert_eq!(retrieved, test_value, "Retrieved value should match");
    }

    // 测试DELETE
    let delete_result = backend.delete(test_key).await;
    assert!(delete_result.is_ok(), "DELETE should succeed");

    // 验证删除
    let get_after_delete = backend.get(test_key).await;
    assert!(get_after_delete.is_ok(), "GET after delete should succeed");
    assert!(
        get_after_delete.unwrap().is_none(),
        "Value should be None after delete"
    );
}

/// 测试Redis ping操作
#[tokio::test]
async fn test_redis_ping() {
    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = "redis://127.0.0.1:6379";
    let backend = RedisBackend::new(redis_url).await.unwrap();

    let ping_result = backend.ping().await;
    assert!(ping_result.is_ok(), "Ping should succeed");

    let ping_value = ping_result.unwrap();
    assert_eq!(ping_value, "PONG", "Ping should return PONG");
}
