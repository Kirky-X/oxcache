//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 服务配置模块
//!
//! 提供 feature-gated 的服务配置：
//! - L1Config: 需要 moka feature
//! - L2Config: 需要 redis feature
//! - ServiceConfig: 始终可用，但内部配置字段是 feature-gated

/// 序列化类型枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SerializationType {
    /// JSON 序列化（默认）
    #[default]
    Json,
    /// Bincode 序列化
    Bincode,
    /// MessagePack 序列化
    MessagePack,
    /// CBOR 序列化
    Cbor,
}

impl fmt::Display for SerializationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerializationType::Json => write!(f, "json"),
            SerializationType::Bincode => write!(f, "bincode"),
            SerializationType::MessagePack => write!(f, "messagepack"),
            SerializationType::Cbor => write!(f, "cbor"),
        }
    }
}
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 哨兵配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SentinelConfig {
    /// 主节点名称
    pub master_name: String,
    /// 哨兵节点列表
    pub nodes: Vec<String>,
}

/// 集群配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClusterConfig {
    /// 集群节点列表
    pub nodes: Vec<String>,
}

/// 缓存类型枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    /// 仅 L1 缓存（需要 moka feature）
    L1,
    /// 仅 L2 缓存（需要 redis feature）
    L2,
    /// 双层缓存（L1 + L2，需要 moka 和 redis features）
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

/// Redis 模式（始终可用，用于序列化）
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

/// 服务配置（始终可用）
///
/// 定义单个服务的缓存配置。内部字段根据 feature 进行条件编译：
/// - l1: Option<L1Config> - 需要 moka feature
/// - l2: Option<L2Config> - 需要 redis feature
/// - two_level: Option<TwoLevelConfig> - 需要 redis feature
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// 缓存类型
    pub cache_type: CacheType,
    /// 缓存过期时间（秒）
    pub ttl: Option<u64>,
    /// 序列化类型
    pub serialization: Option<SerializationType>,
    /// L1 缓存配置（需要 moka feature）
    #[cfg(feature = "moka")]
    pub l1: Option<L1Config>,
    /// L2 缓存配置（需要 redis feature）
    #[cfg(feature = "redis")]
    pub l2: Option<L2Config>,
    /// 双层缓存配置（需要 redis feature）
    #[cfg(feature = "redis")]
    pub two_level: Option<TwoLevelConfig>,
}

impl fmt::Debug for ServiceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceConfig")
            .field("cache_type", &self.cache_type)
            .field("ttl", &self.ttl)
            .field("serialization", &self.serialization)
            .finish()
    }
}

impl ServiceConfig {
    /// 创建 L1 仅缓存配置（需要 moka feature）
    #[cfg(feature = "moka")]
    pub fn l1_only() -> Self {
        Self {
            cache_type: CacheType::L1,
            ttl: None,
            serialization: None,
            l1: Some(L1Config::default()),
            #[cfg(feature = "redis")]
            l2: None,
            #[cfg(feature = "redis")]
            two_level: None,
        }
    }

    /// 创建 L1 仅缓存配置（无 feature 时的降级版本）
    #[cfg(not(feature = "moka"))]
    pub fn l1_only() -> Self {
        Self {
            cache_type: CacheType::L1,
            ttl: None,
            serialization: None,
        }
    }

    /// 创建 L2 仅缓存配置（需要 redis feature）
    #[cfg(feature = "redis")]
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

    /// 创建 L2 仅缓存配置（无 redis feature 时的降级版本）
    #[cfg(all(not(feature = "redis"), feature = "moka"))]
    pub fn l2_only() -> Self {
        Self {
            cache_type: CacheType::L2,
            ttl: None,
            serialization: None,
            l1: None,
        }
    }

    /// 创建 L2 仅缓存配置（无 redis feature 且无 moka 时的降级版本）
    #[cfg(all(not(feature = "redis"), not(feature = "moka")))]
    pub fn l2_only() -> Self {
        Self {
            cache_type: CacheType::L2,
            ttl: None,
            serialization: None,
        }
    }

    /// 创建双层缓存配置（需要 moka 和 redis features）
    #[cfg(all(feature = "moka", feature = "redis"))]
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

    /// 创建双层缓存配置（仅 redis，无 moka）
    #[cfg(all(feature = "redis", not(feature = "moka")))]
    pub fn two_level() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: None,
            serialization: None,
            l1: None,
            l2: Some(L2Config::default()),
            two_level: Some(TwoLevelConfig::default()),
        }
    }

    /// 创建双层缓存配置（仅 moka，无 redis）
    #[cfg(all(feature = "moka", not(feature = "redis")))]
    pub fn two_level() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: None,
            serialization: None,
            l1: Some(L1Config::default()),
        }
    }

    /// 创建双层缓存配置（无 features 时的降级版本）
    #[cfg(not(any(feature = "moka", feature = "redis")))]
    pub fn two_level() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: None,
            serialization: None,
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

    /// 设置 L1 配置（需要 moka feature）
    #[cfg(feature = "moka")]
    pub fn with_l1(mut self, l1: L1Config) -> Self {
        self.l1 = Some(l1);
        self
    }

    /// 设置 L2 配置（需要 redis feature）
    #[cfg(feature = "redis")]
    pub fn with_l2(mut self, l2: L2Config) -> Self {
        self.l2 = Some(l2);
        self
    }

    /// 设置双层缓存配置（需要 redis feature）
    #[cfg(feature = "redis")]
    pub fn with_two_level(mut self, two_level: TwoLevelConfig) -> Self {
        self.two_level = Some(two_level);
        self
    }

    /// 检查是否可以创建 L1 配置
    pub fn can_use_l1(&self) -> bool {
        cfg!(feature = "moka")
    }

    /// 检查是否可以创建 L2 配置
    pub fn can_use_l2(&self) -> bool {
        cfg!(feature = "redis")
    }

    /// 检查是否可以创建双层缓存配置
    pub fn can_use_two_level(&self) -> bool {
        cfg!(feature = "moka") && cfg!(feature = "redis")
    }
}

/// L1 缓存配置（需要 moka feature）
///
/// 定义内存缓存的相关配置。
#[cfg(feature = "moka")]
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

#[cfg(feature = "moka")]
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

#[cfg(feature = "moka")]
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

    /// 设置键的最大长度
    pub fn with_max_key_length(mut self, length: usize) -> Self {
        self.max_key_length = length;
        self
    }

    /// 设置值的最大大小
    pub fn with_max_value_size(mut self, size: usize) -> Self {
        self.max_value_size = size;
        self
    }

    /// 设置清理间隔
    pub fn with_cleanup_interval_secs(mut self, secs: u64) -> Self {
        self.cleanup_interval_secs = secs;
        self
    }
}

/// L2 缓存配置（需要 redis feature）
///
/// 定义分布式缓存的相关配置。
#[cfg(feature = "redis")]
#[derive(Clone, Serialize, Deserialize)]
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

#[cfg(feature = "redis")]
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

#[cfg(feature = "redis")]
impl L2Config {
    /// 创建新的 L2 配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置连接字符串
    pub fn with_connection_string(mut self, connection_string: &str) -> Self {
        self.connection_string = SecretString::new(connection_string.to_string());
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
        self.password = Some(SecretString::new(password.to_string()));
        self
    }

    /// 设置连接超时
    pub fn with_connection_timeout_ms(mut self, timeout: u64) -> Self {
        self.connection_timeout_ms = timeout;
        self
    }

    /// 设置命令超时
    pub fn with_command_timeout_ms(mut self, timeout: u64) -> Self {
        self.command_timeout_ms = timeout;
        self
    }

    /// 设置是否启用 TLS
    pub fn with_enable_tls(mut self, enable: bool) -> Self {
        self.enable_tls = enable;
        self
    }

    /// 设置哨兵配置
    pub fn with_sentinel(mut self, sentinel: SentinelConfig) -> Self {
        self.sentinel = Some(sentinel);
        self
    }

    /// 设置集群配置
    pub fn with_cluster(mut self, cluster: ClusterConfig) -> Self {
        self.cluster = Some(cluster);
        self
    }
}

#[cfg(feature = "redis")]
impl Default for L2Config {
    fn default() -> Self {
        Self {
            mode: RedisMode::default(),
            connection_string: SecretString::new("".to_string()),
            connection_timeout_ms: 5000,
            command_timeout_ms: 30000,
            password: None,
            enable_tls: false,
            sentinel: None,
            cluster: None,
            default_ttl: None,
            max_key_length: 512,
            max_value_size: 10 * 1024 * 1024,
        }
    }
}

/// 双层缓存配置
#[cfg(feature = "redis")]
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

#[cfg(feature = "redis")]
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

    /// 设置批量写入间隔
    pub fn with_batch_interval_ms(mut self, ms: u64) -> Self {
        self.batch_interval_ms = ms;
        self
    }
}

/// 缓存预热配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheWarmupConfig {
    /// 是否启用预热
    pub enabled: bool,
    /// 预热超时时间（秒）
    pub timeout_secs: u64,
    /// 并发预热的最大数量
    pub max_concurrent: usize,
}

/// 缓存失效频道配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InvalidationChannelConfig {
    /// 频道名称
    pub channel_name: String,
    /// 是否启用
    pub enabled: bool,
}

/// 布隆过滤器配置（需要 bloom-filter feature）
#[cfg(all(feature = "redis", feature = "bloom-filter"))]
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

#[cfg(all(feature = "redis", feature = "bloom-filter"))]
impl BloomFilterConfig {
    /// 创建新的布隆过滤器配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置预期元素数量
    pub fn with_expected_elements(mut self, elements: usize) -> Self {
        self.expected_elements = elements;
        self
    }

    /// 设置误判率
    pub fn with_false_positive_rate(mut self, rate: f64) -> Self {
        self.false_positive_rate = rate;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_l1_only() {
        #[cfg(feature = "moka")]
        {
            let config = ServiceConfig::l1_only();
            assert_eq!(config.cache_type, CacheType::L1);
            assert!(config.l1.is_some());
            #[cfg(feature = "redis")]
            assert!(config.l2.is_none());
        }
    }

    #[test]
    fn test_service_config_l2_only() {
        #[cfg(feature = "redis")]
        {
            let config = ServiceConfig::l2_only();
            assert_eq!(config.cache_type, CacheType::L2);
            assert!(config.l2.is_some());
            #[cfg(feature = "moka")]
            assert!(config.l1.is_none());
        }
    }

    #[test]
    fn test_service_config_two_level() {
        let config = ServiceConfig::two_level();
        assert_eq!(config.cache_type, CacheType::TwoLevel);

        #[cfg(feature = "moka")]
        assert!(config.l1.is_some());

        #[cfg(feature = "redis")]
        assert!(config.l2.is_some());
    }

    #[test]
    fn test_service_config_with_ttl() {
        let config = ServiceConfig::two_level().with_ttl(600);
        assert_eq!(config.ttl, Some(600));
    }

    #[test]
    fn test_l2_config_with_connection() {
        #[cfg(feature = "redis")]
        {
            let config = L2Config::new()
                .with_connection_string("redis://localhost:6379")
                .with_default_ttl(7200);

            assert_eq!(config.default_ttl, Some(7200));
        }
    }

    #[test]
    fn test_two_level_config_batch() {
        #[cfg(feature = "redis")]
        {
            let config = TwoLevelConfig::new()
                .with_enable_batch_write(true)
                .with_batch_size(500);

            assert!(config.enable_batch_write);
            assert_eq!(config.batch_size, 500);
        }
    }

    #[test]
    fn test_service_config_feature_flags() {
        let config = ServiceConfig::default();

        #[cfg(feature = "moka")]
        assert!(config.can_use_l1());

        #[cfg(feature = "redis")]
        assert!(config.can_use_l2());
    }
}
