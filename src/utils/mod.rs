//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 统一工具模块
//!
//! 提供测试和示例共用的工具函数，包括：
//! - 配置创建工具
//! - Redis连接检查工具
//! - 日志设置工具
//! - 服务名称生成工具
//! - 输入验证工具

use crate::config::{
    CacheType, ClusterConfig, Config, L1Config, L2Config, RedisMode, SentinelConfig, ServiceConfig,
    TwoLevelConfig,
};
use crate::error::CacheError;
use secrecy::SecretString;
use std::collections::HashMap;
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

/// 创建独立的Redis配置
pub fn create_standalone_config() -> L2Config {
    L2Config {
        mode: RedisMode::Standalone,
        connection_string: SecretString::new("redis://127.0.0.1:6379".into()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 5000,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: Some(3600),
        max_key_length: 256,
        max_value_size: 1024 * 1024 * 10, // 10MB
    }
}

/// 创建Redis集群配置
pub fn create_cluster_config() -> L2Config {
    L2Config {
        mode: RedisMode::Cluster,
        connection_string: SecretString::new("redis://127.0.0.1:7000".into()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 5000,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: Some(ClusterConfig {
            nodes: vec![
                "127.0.0.1:7000".to_string(),
                "127.0.0.1:7001".to_string(),
                "127.0.0.1:7002".to_string(),
                "127.0.0.1:7003".to_string(),
                "127.0.0.1:7004".to_string(),
                "127.0.0.1:7005".to_string(),
            ],
        }),
        default_ttl: Some(3600),
        max_key_length: 256,
        max_value_size: 1024 * 1024 * 10, // 10MB
    }
}

/// 创建Redis Sentinel配置
pub fn create_sentinel_config() -> L2Config {
    L2Config {
        mode: RedisMode::Sentinel,
        connection_string: SecretString::new("redis://127.0.0.1:26379".into()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 5000,
        password: None,
        enable_tls: false,
        sentinel: Some(SentinelConfig {
            master_name: "mymaster".to_string(),
            nodes: vec![
                "127.0.0.1:26379".to_string(),
                "127.0.0.1:26380".to_string(),
                "127.0.0.1:26381".to_string(),
            ],
        }),
        cluster: None,
        default_ttl: Some(3600),
        max_key_length: 256,
        max_value_size: 1024 * 1024 * 10, // 10MB
    }
}

/// 创建默认的两级缓存配置
pub fn create_default_config(service_name: &str, max_capacity: usize) -> Config {
    let mut services = HashMap::new();
    services.insert(
        service_name.to_string(),
        ServiceConfig {
            cache_type: CacheType::TwoLevel,
            ttl: Some(300),
            serialization: None,
            l1: Some(L1Config {
                max_capacity: max_capacity as u64,
                cleanup_interval_secs: 60,
                max_key_length: 256,
                max_value_size: 1024 * 1024 * 10,
            }),
            l2: Some(L2Config {
                mode: RedisMode::Standalone,
                connection_string: "redis://127.0.0.1:6379".to_string().into(),
                ..Default::default()
            }),
            two_level: Some(TwoLevelConfig {
                promote_on_hit: true,
                enable_batch_write: true,
                batch_size: 10,
                batch_interval_ms: 100,
                invalidation_channel: None,
                bloom_filter: None,
                warmup: None,
                max_key_length: Some(256),
                max_value_size: Some(1024 * 1024 * 10),
            }),
        },
    );

    Config {
        services,
        ..Default::default()
    }
}

/// 检查Redis是否可用
pub fn is_redis_available() -> bool {
    std::env::var("OXCACHE_SKIP_REDIS_TESTS").is_err()
}

/// 检查指定URL的Redis是否可用
pub async fn is_redis_available_url(url: &str) -> bool {
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

/// 等待Redis可用
pub async fn wait_for_redis(url: &str) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(30);

    while start.elapsed() < timeout {
        if is_redis_available_url(url).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// 生成唯一的服务名称
pub fn generate_unique_service_name(base: &str) -> String {
    format!("{}_{}", base, uuid::Uuid::new_v4().simple())
}

const MAX_CACHE_KEY_LENGTH: usize = 1024;
const VALID_KEY_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '_', '.', ':', '/', '@',
];

pub fn validate_cache_key(key: &str) -> Result<(), CacheError> {
    if key.is_empty() {
        return Err(CacheError::InvalidInput(
            "Cache key cannot be empty".to_string(),
        ));
    }

    if key.len() > MAX_CACHE_KEY_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Cache key exceeds maximum length of {} bytes (got {} bytes)",
            MAX_CACHE_KEY_LENGTH,
            key.len()
        )));
    }

    for c in key.chars() {
        if !VALID_KEY_CHARS.contains(&c) {
            return Err(CacheError::InvalidInput(format!(
                "Cache key contains invalid character '{}'. Valid characters are: alphanumeric and -_.:/@",
                c
            )));
        }
    }

    Ok(())
}

pub fn validate_key_length(key: &str, max_length: usize) -> Result<(), CacheError> {
    if key.is_empty() {
        return Err(CacheError::InvalidInput(
            "Cache key cannot be empty".to_string(),
        ));
    }
    if key.len() > max_length {
        return Err(CacheError::InvalidInput(format!(
            "Cache key exceeds maximum length of {} bytes (got {} bytes)",
            max_length,
            key.len()
        )));
    }
    Ok(())
}

pub fn validate_value_size(value: &[u8], max_size: usize) -> Result<(), CacheError> {
    if value.len() > max_size {
        return Err(CacheError::InvalidInput(format!(
            "Cache value exceeds maximum size of {} bytes (got {} bytes)",
            max_size,
            value.len()
        )));
    }
    Ok(())
}
