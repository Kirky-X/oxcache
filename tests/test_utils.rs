// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 该模块定义了测试的通用工具函数和设置。

// 包含数据库测试工具
#[path = "database_test_utils.rs"]
pub mod database_test_utils;

// 包含 Redis 测试工具
#[path = "redis_test_utils.rs"]
pub mod redis_test_utils;

use oxcache::{Cache, OxcacheConfig};
use redis_test_utils::{
    is_redis_available_default, wait_for_redis as redis_test_wait_for_redis,
    wait_for_redis_cluster as redis_test_wait_for_redis_cluster,
    wait_for_sentinel as redis_test_wait_for_sentinel,
};
use std::sync::Once;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

static INIT: Once = Once::new();

pub fn setup_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter(EnvFilter::new("debug"))
            .try_init()
            .ok();
    });
}

/// 设置缓存
///
/// 根据提供的配置创建缓存实例
///
/// # 参数
///
/// * `config` - 缓存配置
#[allow(dead_code)]
pub async fn setup_cache(config: OxcacheConfig) -> Cache<String, Vec<u8>> {
    setup_logging();

    // 根据配置创建相应类型的缓存
    if let Some(l2_config) = config.get_global_config().and_then(|g| g.l2.as_ref()) {
        let connection_string = l2_config.connection_string();
        if connection_string.contains("redis://") {
            match Cache::redis(connection_string).await {
                Ok(cache) => cache,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("Authentication required") || msg.contains("authentication failed") {
                        panic!("Redis认证失败，请检查REDIS_URL环境变量: {}", msg);
                    }
                    tracing::warn!("Redis缓存初始化失败: {}", e);
                    // 回退到内存缓存
                    Cache::new().await.unwrap()
                }
            }
        } else {
            Cache::new().await.unwrap()
        }
    } else {
        Cache::new().await.unwrap()
    }
}

/// 检查Redis是否可用 (默认URL)
///
/// 尝试连接到本地Redis实例，检查其是否可用
#[allow(dead_code)]
pub async fn is_redis_available() -> bool {
    is_redis_available_default().await
}

/// 等待Redis可用
///
/// 循环检查Redis是否可用，直到超时
#[allow(dead_code)]
pub async fn wait_for_redis(url: &str) -> bool {
    redis_test_wait_for_redis(url).await
}

/// 等待Redis可用 (别名)
///
/// 循环检查Redis是否可用，直到超时
#[allow(dead_code)]
pub async fn wait_for_redis_url(url: &str) -> bool {
    wait_for_redis(url).await
}

/// 等待Redis集群可用
///
/// 检查所有Redis节点是否可用且集群状态正常
#[allow(dead_code)]
pub async fn wait_for_redis_cluster(urls: &[&str]) -> bool {
    redis_test_wait_for_redis_cluster(urls).await
}

/// 等待Redis Sentinel可用
///
/// 检查所有Sentinel节点是否可用且master已配置
#[allow(dead_code)]
pub async fn wait_for_sentinel() -> bool {
    redis_test_wait_for_sentinel().await
}

/// 生成唯一的服务器名称
///
/// 在基础名称后附加UUID，确保测试之间的隔离
///
/// # 参数
///
/// * `base` - 基础名称
///
/// # 返回值
///
/// 返回唯一的服务器名称
#[allow(dead_code)]
pub fn generate_unique_service_name(base: &str) -> String {
    format!("{}_{}", base, uuid::Uuid::new_v4().simple())
}

/// 清理测试服务资源
///
/// 测试结束后清理WAL数据库文件和缓存数据
///
/// # 参数
///
/// * `service_name` - 服务名称
#[allow(dead_code)]
pub async fn cleanup_service(service_name: &str) {
    tokio::fs::remove_file(format!("{}_wal.db", service_name))
        .await
        .ok();
    tokio::fs::remove_file(format!("{}.db", service_name))
        .await
        .ok();
}