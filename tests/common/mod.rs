// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 测试公共模块 - 统一导出所有测试工具

// 禁用 clippy 警告 - Rust 测试框架正常行为，多个测试入口文件会重复加载此模块
#![allow(clippy::duplicate_mod)]

// 子模块
pub mod docker_test_utils;
pub mod mock_backend;
pub mod redis_test_utils;
pub mod storage_test_utils;
pub mod test_containers;

// ============================================================================
// 重新导出常用函数
// ============================================================================

// Redis 测试工具
#[allow(unused_imports)]
pub use redis_test_utils::{
    create_cluster_redis_urls, create_standalone_redis_url, get_redis_url, get_redis_url_insecure, is_redis_available,
    is_redis_available_url, wait_for_redis, wait_for_redis_cluster, wait_for_sentinel,
};

// 数据库测试工具
#[allow(unused_imports)]
pub use storage_test_utils::{
    create_partition_config, create_temp_sqlite_db, test_concurrent_partition_operations, verify_partition_cleanup,
    verify_partition_creation, TestConfig,
};

// Docker 测试工具
#[allow(unused_imports)]
pub use docker_test_utils::{
    is_redis_available as docker_is_redis_available, setup_redis_cluster_nodes, setup_redis_container,
    wait_for_redis as docker_wait_for_redis, RedisContainer,
};

// Testcontainers 工具
#[allow(unused_imports)]
pub use test_containers::{
    is_redis_available as tc_is_redis_available, start_redis_container, RedisClusterManager,
    RedisContainer as AsyncRedisContainer, TestEnvironment,
};

// Mock 后端
#[allow(unused_imports)]
pub use mock_backend::MockBackend;

// ============================================================================
// 日志设置
// ============================================================================

use std::sync::Once;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

#[allow(dead_code)]
static INIT: Once = Once::new();

/// 初始化日志系统
///
/// 在测试开始时调用，确保日志只初始化一次。
#[allow(dead_code)]
pub fn setup_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter(EnvFilter::new("debug"))
            .try_init()
            .ok();
    });
}

// ============================================================================
// 缓存设置工具
// ============================================================================

use oxcache::Cache;

/// 设置缓存 - 用于测试
///
/// 创建默认的内存缓存实例，简化测试设置。
#[allow(dead_code)]
pub async fn setup_cache() -> Cache<String, Vec<u8>> {
    setup_logging();

    Cache::builder()
        .build()
        .await
        .unwrap_or_else(|e| panic!("Failed to create memory cache: {}", e))
}

/// 生成唯一的服务器名称
///
/// 在基础名称后附加 UUID，确保测试之间的隔离。
#[allow(dead_code)]
pub fn generate_unique_service_name(base: &str) -> String {
    format!("{}_{}", base, uuid::Uuid::new_v4().simple())
}

/// 清理测试服务资源
///
/// 测试结束后清理 WAL 数据库文件和缓存数据。
#[allow(dead_code)]
pub async fn cleanup_service(service_name: &str) {
    tokio::fs::remove_file(format!("{}_wal.db", service_name)).await.ok();
    tokio::fs::remove_file(format!("{}.db", service_name)).await.ok();
}
