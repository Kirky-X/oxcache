//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了测试的通用工具函数和设置。

pub mod database_test_utils;
pub mod redis_test_utils;

use oxcache::{CacheManager, Config};
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

/// 设置缓存管理器
///
/// 根据提供的配置初始化缓存管理器
///
/// # 参数
///
/// * `config` - 缓存配置
#[allow(dead_code)]
pub async fn setup_cache(config: Config) {
    setup_logging();

    if let Err(e) = CacheManager::init(config).await {
        let msg = e.to_string();
        if msg.contains("Authentication required") || msg.contains("authentication failed") {
            panic!("Redis认证失败，请检查REDIS_URL环境变量: {}", msg);
        }
        tracing::warn!("CacheManager初始化失败 (可能已初始化): {}", e);
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
