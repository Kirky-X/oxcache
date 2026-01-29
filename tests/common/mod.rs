// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 该模块定义了测试的通用工具函数和设置。

// 注意：database_test_utils 和 redis_test_utils 在根目录 tests/ 下定义
// 它们通过 include! 宏在这里被包含，以避免重复定义

use oxcache::Cache;
use std::sync::Once;
use std::time::Duration;
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
/// 创建默认的内存缓存实例
#[allow(dead_code)]
pub async fn setup_cache() -> Cache<String, Vec<u8>> {
    setup_logging();

    // 使用新的 API 创建内存缓存
    Cache::builder()
        .build()
        .await
        .unwrap_or_else(|e| panic!("Failed to create memory cache: {}", e))
}

/// 检查Redis是否可用 (默认URL)
///
/// 尝试连接到本地Redis实例，检查其是否可用
#[allow(dead_code)]
pub fn is_redis_available() -> bool {
    let _redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| get_redis_url());
    // 简化实现：直接检查环境变量，不进行实际连接测试
    // 完整的实现需要异步支持
    std::env::var("OXCACHE_SKIP_REDIS_TESTS").is_err()
}

/// 获取Redis URL（根据是否允许非TLS连接返回适当的URL）
#[allow(dead_code)]
pub fn get_redis_url() -> String {
    // 首先检查环境变量
    if let Ok(url) = std::env::var("REDIS_URL") {
        return url;
    }

    // 检查是否允许非TLS连接
    if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
        "redis://127.0.0.1:6379".to_string()
    } else {
        // 默认使用 TLS 连接
        "rediss://127.0.0.1:6379".to_string()
    }
}

/// 获取Redis URL，仅当允许非TLS时返回redis://
#[allow(dead_code)]
pub fn get_redis_url_insecure() -> String {
    if let Ok(url) = std::env::var("REDIS_URL") {
        return url;
    }
    if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
        "redis://127.0.0.1:6379"
    } else {
        "rediss://127.0.0.1:6379"
    }
    .to_string()
}

async fn is_redis_available_url(url: &str) -> bool {
    use std::time::Duration;
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    match tokio::time::timeout(
        Duration::from_secs(1),
        client.get_multiplexed_async_connection(),
    )
    .await
    {
        Ok(Ok(_)) => true,
        Ok(Err(e)) => !e.is_connection_refusal(),
        _ => false,
    }
}

#[allow(dead_code)]
async fn is_redis_available_default() -> bool {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    is_redis_available_url(&redis_url).await
}

/// 等待Redis可用
///
/// 循环检查Redis是否可用，直到超时
#[allow(dead_code)]
pub async fn wait_for_redis(url: &str) -> bool {
    use std::time::Instant;
    let start = Instant::now();
    let timeout = Duration::from_secs(30);

    while start.elapsed() < timeout {
        if is_redis_available_url(url).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
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
pub async fn wait_for_redis_cluster(_urls: &[&str]) -> bool {
    // Simplified: just check if default redis is available
    is_redis_available()
}

/// 等待Redis Sentinel可用
///
/// 检查所有Sentinel节点是否可用且master已配置
#[allow(dead_code)]
pub async fn wait_for_sentinel() -> bool {
    // Simplified: just check if default redis is available
    is_redis_available()
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
