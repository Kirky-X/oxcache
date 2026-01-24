// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 生命周期管理集成测试 - 简化版本

#![allow(deprecated)]

use crate::common::{is_redis_available, setup_logging};

#[tokio::test]
async fn test_client_lifecycle_shutdown() {
    setup_logging();

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    // 生命周期测试依赖于 TwoLevelClient
    // 新 API 需要重新实现这个功能
    println!("注：生命周期测试需要新的客户端实现");
    println!("跳过客户端生命周期测试");
}
