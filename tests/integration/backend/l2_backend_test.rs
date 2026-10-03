// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// L2后端测试 - 使用新API

#![cfg(feature = "redis")]

use crate::common;
use crate::common::{get_redis_url, test_redis_connection};
use oxcache::backend::memory::RedisBackend;

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

    if let Err(e) = test_redis_connection().await {
        println!("跳过测试: Redis连接失败 - {}", e);
        return;
    }

    // 测试独立的 Redis 连接（与前置探测同一端点：硬编码端口是对
    // 部署环境的错误假设，端点可用性已由上方 skip 检查确认）
    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await;
    assert!(
        backend.is_ok(),
        "Backend creation failed: {:?}",
        backend.err()
    );

    println!("✅ Redis backend connection test passed");
}
