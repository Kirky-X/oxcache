//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 服务配置模块

use crate::config::legacy_config::{
    CacheWarmupConfig, ClusterConfig, InvalidationChannelConfig, SentinelConfig, SerializationType,
    WarmupDataSource,
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 缓存类型枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    /// 仅 L1 缓存
    L1,
    /// 仅 L2 缓存
    L2,
    /// 双层缓存（L1 + L2）
    #[default]
    TwoLevel,
}

impl fmt::Display for CacheType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheType::L1 => write!(f, "l1"),
            CacheType::L2 => write!(f, "l2"),
            CacheType::TwoLevel => write!(f, "two-level"),
        }
    }
}

/// 服务配置
///
/// 定义单个服务的缓存配置。
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// 缓存类型
    pub cache_type: CacheType,
    /// 缓存过期时间（秒）
    pub ttl: Option<u64>,
    /// 序列化类型
    pub serialization: Option<SerializationType>,
    /// L1 缓存配置
    pub l1: Option<L1Config>,
    /// L2 缓存配置
    pub l2: Option<L2Config>,
    /// 双层缓存配置
    pub two_level: Option<TwoLevelConfig>,
}

impl fmt::Debug for ServiceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceConfig")
            .field("cache_type", &self.cache_type)
            .field("ttl", &self.ttl)
            .field("serialization", &self.serialization)
            .field("l1", &self.l1)
            .field("l2", &self.l2)
            .field("two_level", &self.two_level)
            .finish()
    }
}

impl ServiceConfig {
    /// 创建 L1 仅缓存配置
    pub fn l1_only() -> Self {
        Self {
            cache_type: CacheType::L1,
            ttl: None,
            serialization: None,
            l1: Some(L1Config::default()),
            l2: None,
            two_level: None,
        }
    }

    /// 创建 L2 仅缓存配置
    pub fn l2_only() -> Self {
        Self {
            cache_type: CacheType::L2,
            ttl: None,
            serialization: None,
            l1: None,
            l2: Some(L2Config::default()),
            two_level: None,
        }
    }

    /// 创建双层缓存配置
    pub fn two_level() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: None,
            serialization: None,
            l1: Some(L1Config::default()),
            l2: Some(L2Config::default()),
            two_level: Some(TwoLevelConfig::default()),
        }
    }

    /// 创建自定义缓存配置
    pub fn with_cache_type(cache_type: CacheType) -> Self {
        match cache_type {
            CacheType::L1 => Self::l1_only(),
            CacheType::L2 => Self::l2_only(),
            CacheType::TwoLevel => Self::two_level(),
        }
    }

    /// 设置 TTL
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// 设置 L1 配置
    pub fn with_l1(mut self, l1: L1Config) -> Self {
        self.l1 = Some(l1);
        self
    }

    /// 设置 L2 配置
    pub fn with_l2(mut self, l2: L2Config) -> Self {
        self.l2 = Some(l2);
        self
    }

    /// 设置双层缓存配置
    pub fn with_two_level(mut self, two_level: TwoLevelConfig) -> Self {
        self.two_level = Some(two_level);
        self
    }
}

/// L1 缓存配置
///
/// 定义内存缓存的相关配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct L1Config {
    /// 最大缓存容量
    pub max_capacity: u64,
    /// 键的最大长度
    pub max_key_length: usize,
    /// 值的最大大小（字节）
    pub max_value_size: usize,
    /// 过期清理间隔（秒）
    pub cleanup_interval_secs: u64,
}

impl Default for L1Config {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            max_key_length: 512,
            max_value_size: 1024 * 1024, // 1MB
            cleanup_interval_secs: 300,
        }
    }
}

impl L1Config {
    /// 创建新的 L1 配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置最大容量
    pub fn with_max_capacity(mut self, capacity: u64) -> Self {
        self.max_capacity = capacity;
        self
    }
}

/// L2 缓存配置
///
/// 定义分布式缓存的相关配置。
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct L2Config {
    /// Redis 模式
    pub mode: RedisMode,
    /// 连接字符串
    #[serde(skip)]
    pub connection_string: SecretString,
    /// 连接超时（毫秒）
    pub connection_timeout_ms: u64,
    /// 命令超时（毫秒）
    pub command_timeout_ms: u64,
    /// 密码
    #[serde(skip)]
    pub password: Option<SecretString>,
    /// 是否启用 TLS
    pub enable_tls: bool,
    /// 哨兵配置
    pub sentinel: Option<SentinelConfig>,
    /// 集群配置
    pub cluster: Option<ClusterConfig>,
    /// 默认 TTL（秒）
    pub default_ttl: Option<u64>,
    /// 键的最大长度
    pub max_key_length: usize,
    /// 值的最大大小（字节）
    pub max_value_size: usize,
}

impl fmt::Debug for L2Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("L2Config")
            .field("mode", &self.mode)
            .field("connection_string", &"[REDACTED]")
            .field("connection_timeout_ms", &self.connection_timeout_ms)
            .field("command_timeout_ms", &self.command_timeout_ms)
            .field("password", &"[REDACTED]")
            .field("enable_tls", &self.enable_tls)
            .field("sentinel", &self.sentinel)
            .field("cluster", &self.cluster)
            .field("default_ttl", &self.default_ttl)
            .field("max_key_length", &self.max_key_length)
            .field("max_value_size", &self.max_value_size)
            .finish()
    }
}

impl L2Config {
    /// 创建新的 L2 配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置连接字符串
    pub fn with_connection_string(mut self, connection_string: &str) -> Self {
        self.connection_string = SecretString::new(connection_string.to_string().into());
        self
    }

    /// 设置 Redis 模式
    pub fn with_mode(mut self, mode: RedisMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置默认 TTL
    pub fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 设置密码
    pub fn with_password(mut self, password: &str) -> Self {
        self.password = Some(SecretString::new(password.to_string().into()));
        self
    }
}

/// Redis 模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RedisMode {
    /// 单机模式
    #[default]
    Standalone,
    /// 哨兵模式
    Sentinel,
    /// 集群模式
    Cluster,
}

impl fmt::Display for RedisMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedisMode::Standalone => write!(f, "standalone"),
            RedisMode::Sentinel => write!(f, "sentinel"),
            RedisMode::Cluster => write!(f, "cluster"),
        }
    }
}

/// 双层缓存配置
///
/// 定义双层缓存特有的行为配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TwoLevelConfig {
    /// 是否在命中时提升到 L1
    pub promote_on_hit: bool,
    /// 是否启用批量写入
    pub enable_batch_write: bool,
    /// 批量写入大小
    pub batch_size: usize,
    /// 批量写入间隔（毫秒）
    pub batch_interval_ms: u64,
    /// 键的最大长度
    pub max_key_length: Option<usize>,
    /// 值的最大大小（字节）
    pub max_value_size: Option<usize>,
    /// 布隆过滤器配置
    pub bloom_filter: Option<BloomFilterConfig>,
    /// 缓存失效频道配置
    pub invalidation_channel: Option<InvalidationChannelConfig>,
    /// 缓存预热配置
    pub warmup: Option<CacheWarmupConfig>,
}

impl TwoLevelConfig {
    /// 创建新的双层缓存配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置批量写入
    pub fn with_enable_batch_write(mut self, enable: bool) -> Self {
        self.enable_batch_write = enable;
        self
    }

    /// 设置批量写入大小
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }
}

/// 布隆过滤器配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BloomFilterConfig {
    /// 预期元素数量
    pub expected_elements: usize,
    /// 误判率（0.0-1.0）
    pub false_positive_rate: f64,
    /// 是否自动将查询过的键添加到布隆过滤器
    pub auto_add_keys: bool,
    /// 布隆过滤器名称
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_l1_only() {
        let config = ServiceConfig::l1_only();
        assert_eq!(config.cache_type, CacheType::L1);
        assert!(config.l1.is_some());
        assert!(config.l2.is_none());
    }

    #[test]
    fn test_service_config_l2_only() {
        let config = ServiceConfig::l2_only();
        assert_eq!(config.cache_type, CacheType::L2);
        assert!(config.l1.is_none());
        assert!(config.l2.is_some());
    }

    #[test]
    fn test_service_config_two_level() {
        let config = ServiceConfig::two_level();
        assert_eq!(config.cache_type, CacheType::TwoLevel);
        assert!(config.l1.is_some());
        assert!(config.l2.is_some());
    }

    #[test]
    fn test_service_config_with_ttl() {
        let config = ServiceConfig::two_level().with_ttl(600);
        assert_eq!(config.ttl, Some(600));
    }

    #[test]
    fn test_l2_config_with_connection() {
        let config = L2Config::new()
            .with_connection_string("redis://localhost:6379")
            .with_default_ttl(7200);

        assert_eq!(config.default_ttl, Some(7200));
    }

    #[test]
    fn test_two_level_config_batch() {
        let config = TwoLevelConfig::new()
            .with_enable_batch_write(true)
            .with_batch_size(500);

        assert!(config.enable_batch_write);
        assert_eq!(config.batch_size, 500);
    }
}
