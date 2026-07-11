// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// L2后端测试 - 使用新API

#![cfg(feature = "redis")]

use crate::common;
use crate::common::redis_test_utils::test_redis_connection;
use oxcache::backend::memory::redis::RedisBackend;

/// 测试 Redis Standalone/Cluster 连接模式
///
/// 验证 RedisBackend 可以成功创建并连接到 Redis 服务器
#[tokio::test]
async fn test_redis_backend_connection_modes() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    };
    if let Err(e) = test_redis_connection().await {
        println!("跳过测试: Redis连接失败 - {}", e);
        return;
    }

    // 测试独立的 Redis 连接
    let redis_url = "redis://127.0.0.1:6379";
    let backend = RedisBackend::new(redis_url).await;
    assert!(backend.is_ok(), "Backend creation failed: {:?}", backend.err());

    println!("✅ Redis backend connection test passed");
}
