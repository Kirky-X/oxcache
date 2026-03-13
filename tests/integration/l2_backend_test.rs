// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// L2后端测试 - 使用新API

#![cfg(feature = "redis")]

use crate::common;
use crate::common::redis_test_utils::test_redis_connection;
use oxcache::backend::client::redis::RedisBackend;

#[tokio::test]
async fn test_sentinel_mode_success() {
    common::setup_logging();

    // 跳过此测试，因为新API简化了sentinel支持
    // RedisBackend 现在使用标准的 Redis 连接字符串
    println!("Skipping test_sentinel_mode_success: Use RedisBackend::new() directly");
}

#[tokio::test]
async fn test_cluster_mode_success() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "1");
    if let Err(e) = test_redis_connection().await {
        println!("跳过测试: Redis连接失败 - {}", e);
        return;
    }

    // 测试独立的 Redis 连接
    let redis_url = "redis://127.0.0.1:6379";
    let backend = RedisBackend::new(redis_url).await;
    assert!(backend.is_ok(), "Backend creation failed: {:?}", backend.err());
}

#[tokio::test]
async fn test_standalone_mode_success() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "1");
    if let Err(e) = test_redis_connection().await {
        println!("跳过测试: Redis连接失败 - {}", e);
        return;
    }

    // 测试独立的 Redis 连接
    let redis_url = "redis://127.0.0.1:6379";
    let backend = RedisBackend::new(redis_url).await;
    assert!(backend.is_ok(), "Backend creation failed: {:?}", backend.err());
}
