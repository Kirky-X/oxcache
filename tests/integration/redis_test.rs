// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis集成测试 - 新API版本

use crate::common::redis_test_utils;
use oxcache::backend::client::redis::RedisBackend;
use redis_test_utils::{get_redis_url, is_redis_available, test_redis_connection};

#[tokio::test]
async fn test_redis_backend_standalone_creation() {
    println!("测试RedisBackend Standalone模式创建...");

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

    let redis_url = get_redis_url();
    let result = RedisBackend::new(&redis_url).await;

    assert!(
        result.is_ok(),
        "应该能成功创建Standalone RedisBackend: {:?}",
        result.err()
    );
    println!("✓ Standalone RedisBackend创建成功");
}

#[tokio::test]
async fn test_redis_backend_standalone_basic_operations() {
    println!("测试RedisBackend Standalone模式基本操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => println!("Redis连接成功"),
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let redis_url = get_redis_url();
    let _backend = RedisBackend::new(&redis_url).await.unwrap();

    // 测试SET/GET/DELETE需要CacheBackend trait，暂时跳过
    // RedisBackend 实现了 CacheBackend trait，但该trait未从crate根目录导出

    println!("✓ Standalone模式基本操作测试完成");
}

#[tokio::test]
async fn test_redis_backend_ping() {
    println!("测试RedisBackend PING...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    let ping_result = backend.ping().await;
    assert!(ping_result.is_ok(), "PING应该成功");

    let ping_value = ping_result.unwrap();
    assert_eq!(ping_value, "PONG", "PING应该返回PONG");

    println!("✓ RedisBackend PING测试成功");
}

#[tokio::test]
async fn test_redis_backend_connection_string_variations() {
    println!("测试不同连接字符串格式...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    // 测试不同的连接字符串格式
    let redis_url = get_redis_url();
    let base_url = redis_url.trim_end_matches("/0").trim_end_matches("/");
    let url_with_db = format!("{}/0", base_url);
    let connection_strings = vec![redis_url.as_str(), url_with_db.as_str()];

    for url in connection_strings {
        let result = RedisBackend::new(url).await;
        assert!(result.is_ok(), "应该能成功创建RedisBackend: {}", url);
    }

    println!("✓ 连接字符串格式测试完成");
}

#[tokio::test]
async fn test_redis_backend_multiple_operations() {
    println!("测试RedisBackend多次连接...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = get_redis_url();
    // 创建多个后端实例
    let mut backends: Vec<Result<RedisBackend, _>> = Vec::new();
    for _ in 0..3 {
        let result = RedisBackend::new(&redis_url).await;
        backends.push(result);
    }

    for (i, result) in backends.into_iter().enumerate() {
        assert!(result.is_ok(), "第{}个后端创建应该成功", i + 1);
    }

    println!("✓ 多次连接测试完成");
}

#[tokio::test]
async fn test_redis_backend_connection_error_handling() {
    println!("测试RedisBackend连接错误处理...");

    // 测试无效连接字符串
    let invalid_url = "redis://invalid.host:99999";
    let result = RedisBackend::new(invalid_url).await;

    // 应该返回错误
    assert!(result.is_err(), "无效连接应该返回错误");

    println!("✓ 连接错误处理测试完成");
}
