// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 测试公共模块 - 统一导出所有测试工具

// 禁用 clippy 警告 - Rust 测试框架正常行为，多个测试入口文件会重复加载此模块
#![allow(clippy::duplicate_mod)]

// 子模块
#[cfg(feature = "redis")]
pub mod docker_test_utils;
pub mod mock_backend;
#[cfg(feature = "redis")]
pub mod redis_test_utils;
#[cfg(feature = "redis")]
pub mod test_containers;

// ============================================================================
// 重新导出常用函数
// ============================================================================

// Redis 测试工具
#[cfg(feature = "redis")]
#[allow(unused_imports)]
pub use redis_test_utils::{
    create_cluster_redis_urls, create_standalone_redis_url, get_redis_url, get_redis_url_insecure,
    get_sentinel_addr_map, get_sentinel_urls, is_redis_available, is_redis_available_url,
    test_redis_connection, wait_for_redis, wait_for_redis_cluster, wait_for_sentinel,
};

// Docker 测试工具
#[cfg(feature = "redis")]
#[allow(unused_imports)]
pub use docker_test_utils::{
    RedisContainer, is_redis_available as docker_is_redis_available, setup_redis_cluster_nodes,
    setup_redis_container, wait_for_redis as docker_wait_for_redis,
};

// Testcontainers 工具
#[cfg(feature = "redis")]
#[allow(unused_imports)]
pub use test_containers::{
    RedisClusterManager, RedisContainer as AsyncRedisContainer, TestEnvironment,
    is_redis_available as tc_is_redis_available, start_redis_container,
};

// Mock 后端（unit 专用测试替身，见 mock_backend.rs 头注释；
// 仅 tests/unit/ 引用，e2e/集成/chaos 不得使用）
#[allow(unused_imports)]
pub use mock_backend::MockBackend;

// ============================================================================
// 环境变量一次性初始化
// ============================================================================

// libtest 默认多线程并发运行测试，POSIX `environ` 非线程安全（std 将
// `set_var` 标记 `unsafe` 即因此）；测试进程对该变量的取值恒为同一常量，
// 故在进程加载期（main 前、单线程期）由 ctor 一次性写入，测试体内不再
// 出现任何并发写环境变量的位点。取值必须与
// `src/backend/memory/redis/builder.rs` 的白名单字面量一致。
#[cfg(feature = "redis")]
#[ctor::ctor(unsafe)]
fn init_allow_insecure_redis_env() {
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    }
}

// ============================================================================
// 日志设置
// ============================================================================

use std::sync::Once;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

#[allow(dead_code)] // tests/common 为多个测试二进制共享，仅部分二进制引用此助手
static INIT: Once = Once::new();

/// 初始化日志系统
///
/// 在测试开始时调用，确保日志只初始化一次。
#[allow(dead_code)] // tests/common 为多个测试二进制共享，仅部分二进制引用此助手
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

// Cache 仅在 backend 基线（memory/redis/disk 任一）下导出，而
// setup_cache 构造的是内存后端实例；共享模块会参与所有组合的测试
// 编译，故此助手随 memory 门控，非 memory 组合不再编译失败。
#[cfg(feature = "memory")]
use oxcache::Cache;

/// 设置缓存 - 用于测试
///
/// 创建默认的内存缓存实例，简化测试设置。
#[cfg(feature = "memory")]
#[allow(dead_code)] // tests/common 为多个测试二进制共享，仅部分二进制引用此助手
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
#[allow(dead_code)] // tests/common 为多个测试二进制共享，仅部分二进制引用此助手
pub fn generate_unique_service_name(base: &str) -> String {
    format!("{}_{}", base, uuid::Uuid::new_v4().simple())
}

/// 清理测试服务资源
///
/// 测试结束后清理 WAL 数据库文件和缓存数据。
#[allow(dead_code)] // tests/common 为多个测试二进制共享，仅部分二进制引用此助手
pub async fn cleanup_service(service_name: &str) {
    tokio::fs::remove_file(format!("{}_wal.db", service_name))
        .await
        .ok();
    tokio::fs::remove_file(format!("{}.db", service_name))
        .await
        .ok();
}
