//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的配置结构和解析逻辑。
//!
//! # 新旧 API
//!
//! 推荐使用新的统一配置 API（OxcacheConfig），旧的 Config 类型已废弃。

use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

pub const CONFIG_VERSION: u32 = 2;
pub const CONFIG_VERSION_FIELD: &str = "config_version";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    #[serde(default)]
    pub global: GlobalConfig,
    pub services: HashMap<String, ServiceConfig>,
}

/// 全局配置
///
/// 定义适用于所有服务的默认配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GlobalConfig {
    /// 默认的缓存过期时间（秒）
    pub default_ttl: u64,
    /// 健康检查间隔（秒）
    pub health_check_interval: u64,
    /// 序列化类型
    pub serialization: SerializationType,
    /// 是否启用指标收集
    pub enable_metrics: bool,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_ttl: 300,
            health_check_interval: 60,
            serialization: SerializationType::Json,
            enable_metrics: true,
        }
    }
}

/// 服务配置
///
/// 定义单个服务的缓存配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServiceConfig {
    /// 缓存类型
    pub cache_type: CacheType,
    /// 缓存过期时间（秒），可覆盖全局配置
    pub ttl: Option<u64>,
    /// 序列化类型，可覆盖全局配置
    pub serialization: Option<SerializationType>,
    /// L1缓存配置
    pub l1: Option<L1Config>,
    /// L2缓存配置
    pub l2: Option<L2Config>,
    /// 双层缓存配置
    pub two_level: Option<TwoLevelConfig>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: None,
            serialization: None,
            l1: Some(L1Config::default()),
            l2: Some(L2Config::default()),
            two_level: Some(TwoLevelConfig::default()),
        }
    }
}

/// 序列化类型枚举
///
/// 支持JSON和Bincode两种序列化方式
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SerializationType {
    /// JSON序列化
    #[default]
    Json,
    /// Bincode序列化
    Bincode,
}

/// 缓存类型枚举
///
/// 定义支持的缓存架构类型
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    /// 仅L1缓存
    L1,
    /// 仅L2缓存
    L2,
    /// 双层缓存（L1+L2）
    #[default]
    TwoLevel,
}

/// L1缓存配置
///
/// 定义内存缓存的相关配置
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct L1Config {
    /// 最大缓存容量（字节）
    pub max_capacity: u64,
    /// 键的最大长度
    pub max_key_length: usize,
    /// 值的最大大小（字节）
    pub max_value_size: usize,
    /// 过期清理间隔（秒），0表示禁用自动清理
    pub cleanup_interval_secs: u64,
}

impl Default for L1Config {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            max_key_length: 256,
            max_value_size: 1024 * 1024, // 1MB
            cleanup_interval_secs: 300,  // 5 minutes
        }
    }
}

/// L2缓存配置
///
/// 定义分布式缓存（如Redis）的相关配置
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct L2Config {
    /// Redis模式
    pub mode: RedisMode,
    /// 连接字符串
    #[serde(skip)]
    pub connection_string: SecretString,
    /// 连接超时时间（毫秒）
    pub connection_timeout_ms: u64,
    /// 命令执行超时时间（毫秒）
    pub command_timeout_ms: u64,
    /// Redis 密码（可选，使用 SecretString 保护）
    #[serde(skip)]
    pub password: Option<SecretString>,
    /// 是否启用 TLS
    pub enable_tls: bool,
    /// 哨兵配置
    pub sentinel: Option<SentinelConfig>,
    /// 集群配置
    pub cluster: Option<ClusterConfig>,
    /// L2缓存默认TTL（可选）
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

impl Default for L2Config {
    fn default() -> Self {
        Self {
            mode: RedisMode::Standalone,
            connection_string: SecretString::new("redis://localhost:6379".to_string().into()),
            connection_timeout_ms: 5000,
            command_timeout_ms: 3000,
            password: None,
            enable_tls: false,
            sentinel: None,
            cluster: None,
            default_ttl: Some(3600),
            max_key_length: 256,
            max_value_size: 1024 * 1024 * 10, // 10MB
        }
    }
}

/// 哨兵配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SentinelConfig {
    /// 主节点名称
    pub master_name: String,
    ////// 哨兵节点列表
    pub nodes: Vec<String>,
}

/// 集群配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClusterConfig {
    /// 初始节点列表
    pub nodes: Vec<String>,
}

impl Config {
    /// 验证配置
    ///
    /// 检查配置的有效性，确保所有必需的字段都已设置，并且值在合理范围内
    pub fn validate(&self) -> Result<(), String> {
        // 验证配置版本
        if let Some(version) = &self.config_version {
            if *version > CONFIG_VERSION {
                return Err(format!(
                    "Configuration version {} is not supported. Current version is {}.",
                    version, CONFIG_VERSION
                ));
            }
        }

        // 验证全局配置
        if self.global.default_ttl == 0 {
            return Err("Global default_ttl cannot be zero".to_string());
        }

        if self.global.default_ttl > 86400 * 30 {
            return Err("Global default_ttl cannot exceed 30 days (2592000 seconds)".to_string());
        }

        if self.global.health_check_interval == 0 {
            return Err("Global health_check_interval cannot be zero".to_string());
        }

        if self.global.health_check_interval < 1 || self.global.health_check_interval > 3600 {
            return Err(
                "Global health_check_interval must be between 1 and 3600 seconds".to_string(),
            );
        }

        // 验证服务配置
        for (name, service) in &self.services {
            // 验证服务名称
            if name.is_empty() {
                return Err("Service name cannot be empty".to_string());
            }

            if name.len() > 64 {
                return Err(format!(
                    "Service name '{}' exceeds maximum length of 64 characters",
                    name
                ));
            }

            // 验证 TTL 配置
            let service_ttl = service.ttl.unwrap_or(self.global.default_ttl);
            if service_ttl == 0 {
                return Err(format!("Service '{}' TTL cannot be zero", name));
            }

            if service_ttl > 86400 * 30 {
                return Err(format!("Service '{}' TTL cannot exceed 30 days", name));
            }

            // 验证 L1 TTL <= L2 TTL
            if let Some(l2_config) = &service.l2 {
                if let Some(l2_specific_ttl) = l2_config.default_ttl {
                    if l2_specific_ttl == 0 {
                        return Err(format!("Service '{}' L2 TTL cannot be zero", name));
                    }

                    if service_ttl > l2_specific_ttl {
                        return Err(format!(
                            "Service '{}' configuration error: L1 TTL ({}) must be <= L2 TTL ({})",
                            name, service_ttl, l2_specific_ttl
                        ));
                    }
                }

                // 验证连接超时
                let timeout = l2_config.connection_timeout_ms;
                if !(100..=30000).contains(&timeout) {
                    return Err(format!(
                        "Service '{}' connection_timeout_ms must be between 100 and 30000 ms",
                        name
                    ));
                }

                // 验证命令超时
                let timeout = l2_config.command_timeout_ms;
                if !(100..=60000).contains(&timeout) {
                    return Err(format!(
                        "Service '{}' command_timeout_ms must be between 100 and 60000 ms",
                        name
                    ));
                }

                // 生产环境安全检查：强制使用认证
                if l2_config.password.is_none() {
                    // 检查是否是生产环境（通过连接字符串判断）
                    let conn_str = l2_config.connection_string.expose_secret();
                    let is_production = conn_str.contains("production")
                        || conn_str.contains("prod")
                        || (!conn_str.contains("localhost")
                            && !conn_str.contains("127.0.0.1")
                            && !conn_str.contains("192.168.")
                            && !conn_str.contains("10."));

                    if is_production {
                        return Err(format!(
                            "Service '{}' is in production environment but Redis password is not configured. \
                            For security reasons, production Redis connections must use authentication. \
                            Please set 'password' in L2Config.",
                            name
                        ));
                    }
                }

                // 生产环境安全检查：强制使用TLS
                if !l2_config.enable_tls {
                    // 检查是否是生产环境
                    let conn_str = l2_config.connection_string.expose_secret();
                    let is_production = conn_str.contains("production")
                        || conn_str.contains("prod")
                        || (!conn_str.contains("localhost")
                            && !conn_str.contains("127.0.0.1")
                            && !conn_str.contains("192.168.")
                            && !conn_str.contains("10."));

                    if is_production {
                        return Err(format!(
                            "Service '{}' is in production environment but TLS is not enabled. \
                            For security reasons, production Redis connections must use TLS encryption. \
                            Please set 'enable_tls = true' in L2Config.",
                            name
                        ));
                    }
                }
            }

            // 验证 L1 配置
            if let Some(l1_config) = &service.l1 {
                if l1_config.max_capacity == 0 {
                    return Err(format!("Service '{}' L1 max_capacity cannot be zero", name));
                }

                if l1_config.max_capacity > 10_000_000 {
                    return Err(format!(
                        "Service '{}' L1 max_capacity cannot exceed 10,000,000",
                        name
                    ));
                }

                // L1 清理间隔必须小于等于服务 TTL
                if l1_config.cleanup_interval_secs > 0
                    && l1_config.cleanup_interval_secs > service_ttl
                {
                    return Err(format!(
                        "Service '{}' L1 cleanup_interval_secs ({}) must be <= service TTL ({})",
                        name, l1_config.cleanup_interval_secs, service_ttl
                    ));
                }
            }

            // 验证双层缓存配置
            if let Some(two_level_config) = &service.two_level {
                // 验证批量写入配置
                if two_level_config.enable_batch_write {
                    if two_level_config.batch_size == 0 {
                        return Err(format!(
                            "Service '{}' batch_size cannot be zero when batch_write is enabled",
                            name
                        ));
                    }

                    if two_level_config.batch_size > 10000 {
                        return Err(format!("Service '{}' batch_size cannot exceed 10000", name));
                    }

                    if two_level_config.batch_interval_ms == 0 {
                        return Err(format!(
                            "Service '{}' batch_interval_ms cannot be zero when batch_write is enabled",
                            name
                        ));
                    }

                    if two_level_config.batch_interval_ms > 60000 {
                        return Err(format!(
                            "Service '{}' batch_interval_ms cannot exceed 60000 ms",
                            name
                        ));
                    }
                }

                // 验证键和值的大小限制
                if let Some(max_key_length) = two_level_config.max_key_length {
                    if max_key_length == 0 || max_key_length > 1024 {
                        return Err(format!(
                            "Service '{}' max_key_length must be between 1 and 1024",
                            name
                        ));
                    }
                }

                if let Some(max_value_size) = two_level_config.max_value_size {
                    if max_value_size == 0 || max_value_size > 10 * 1024 * 1024 {
                        return Err(format!(
                            "Service '{}' max_value_size must be between 1 and 10MB",
                            name
                        ));
                    }
                }

                // 验证布隆过滤器配置
                if let Some(bloom_config) = &two_level_config.bloom_filter {
                    if bloom_config.expected_elements == 0 {
                        return Err(format!(
                            "Service '{}' bloom_filter expected_elements cannot be zero",
                            name
                        ));
                    }

                    if bloom_config.false_positive_rate <= 0.0
                        || bloom_config.false_positive_rate >= 1.0
                    {
                        return Err(format!(
                            "Service '{}' bloom_filter false_positive_rate must be between 0 and 1",
                            name
                        ));
                    }
                }
            }

            // 验证预热配置
            if let Some(warmup_config) = &service.two_level.as_ref().and_then(|c| c.warmup.as_ref())
            {
                if warmup_config.enabled {
                    if warmup_config.timeout_seconds == 0 {
                        return Err(format!(
                            "Service '{}' warmup timeout_seconds cannot be zero",
                            name
                        ));
                    }

                    if warmup_config.timeout_seconds > 3600 {
                        return Err(format!(
                            "Service '{}' warmup timeout_seconds cannot exceed 3600 seconds",
                            name
                        ));
                    }

                    if warmup_config.batch_size == 0 {
                        return Err(format!(
                            "Service '{}' warmup batch_size cannot be zero",
                            name
                        ));
                    }

                    if warmup_config.batch_size > 10000 {
                        return Err(format!(
                            "Service '{}' warmup batch_size cannot exceed 10000",
                            name
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

/// 双层缓存配置
///
/// 定义双层缓存特有的行为配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TwoLevelConfig {
    /// 是否在命中时提升到L1
    pub promote_on_hit: bool,
    /// 是否启用批量写入
    pub enable_batch_write: bool,
    /// 批量写入大小
    pub batch_size: usize,
    /// 批量写入间隔（毫秒）
    pub batch_interval_ms: u64,
    /// 缓存失效频道配置
    pub invalidation_channel: Option<InvalidationChannelConfig>,
    /// 布隆过滤器配置
    pub bloom_filter: Option<BloomFilterConfig>,
    /// 缓存预热配置
    pub warmup: Option<CacheWarmupConfig>,
    /// 键的最大长度
    pub max_key_length: Option<usize>,
    /// 值的最大大小（字节）
    pub max_value_size: Option<usize>,
}

/// 缓存预热配置
///
/// 定义缓存预热的行为配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheWarmupConfig {
    /// 是否启用自动预热
    pub enabled: bool,
    /// 预热超时时间（秒）
    pub timeout_seconds: u64,
    /// 预热批次大小
    pub batch_size: usize,
    /// 预热批次间隔（毫秒）
    pub batch_interval_ms: u64,
    /// 预热数据源配置
    pub data_sources: Vec<WarmupDataSource>,
}

/// 预热数据源配置
///
/// 定义预热数据的来源
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum WarmupDataSource {
    /// 从配置文件加载预热键
    Static {
        /// 预热键列表
        keys: Vec<String>,
    },
    /// 从Redis列表加载预热键
    RedisList {
        /// Redis键名
        key: String,
        /// 最大加载数量
        max_count: usize,
    },
    /// 从数据库加载预热键
    Database {
        /// SQL查询语句
        query: String,
        /// 键字段名
        key_field: String,
        /// 值字段名
        value_field: String,
    },
    /// 从API加载预热键
    Api {
        /// API端点URL
        url: String,
        /// 请求超时（秒）
        timeout_seconds: u64,
    },
}

impl Default for CacheWarmupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_seconds: 300,
            batch_size: 100,
            batch_interval_ms: 50,
            data_sources: Vec::new(),
        }
    }
}

/// 布隆过滤器配置
///
/// 用于防止缓存穿透攻击
#[derive(Serialize, Deserialize, Clone, Debug)]
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

impl Default for BloomFilterConfig {
    fn default() -> Self {
        Self {
            expected_elements: 100000,
            false_positive_rate: 0.01,
            auto_add_keys: true,
            name: "default_bloom_filter".to_string(),
        }
    }
}

/// 缓存失效频道配置
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum InvalidationChannelConfig {
    /// 完整自定义频道名称
    Custom(String),
    /// 结构化配置
    Structured {
        /// 频道名称前缀
        prefix: Option<String>,
        /// 是否使用服务名称作为后缀
        use_service_name: bool,
    },
}

impl Default for TwoLevelConfig {
    fn default() -> Self {
        Self {
            promote_on_hit: true,
            enable_batch_write: false,
            batch_size: 100,
            batch_interval_ms: 1000,
            invalidation_channel: None,
            bloom_filter: None,
            warmup: None,
            max_key_length: Some(256),
            max_value_size: Some(1024 * 1024 * 10),
        }
    }
}

/// Redis模式枚举
///
/// 定义支持的Redis部署模式
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RedisMode {
    /// 单机模式
    Standalone,
    /// 哨兵模式
    Sentinel,
    /// 集群模式
    Cluster,
}

/// L1 缓存淘汰策略枚举
///
/// 定义 L1 内存缓存使用的淘汰策略
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EvictionPolicy {
    /// 最近最少使用
    Lru,
    /// 最不经常使用
    Lfu,
    /// TinyLFU (Sampled LFU)
    TinyLfu,
    /// 随机淘汰
    Random,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self::TinyLfu
    }
}

/// 运行时缓存策略配置
///
/// 用于动态调整缓存策略，支持运行时更新
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct CacheStrategy {
    /// 服务名称
    pub service_name: String,
    /// 缓存过期时间（秒）
    pub ttl: u64,
    /// L1 最大容量
    pub l1_max_capacity: u64,
    /// L1 淘汰策略
    pub l1_eviction_policy: EvictionPolicy,
    /// L2 默认 TTL（秒）
    pub l2_default_ttl: u64,
    /// 是否启用批量写入
    pub enable_batch_write: bool,
    /// 批量写入大小
    pub batch_size: usize,
    /// 更新时间戳
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl CacheStrategy {
    /// 创建新的缓存策略配置
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            ttl: 300,
            l1_max_capacity: 10000,
            l1_eviction_policy: EvictionPolicy::default(),
            l2_default_ttl: 3600,
            enable_batch_write: true,
            batch_size: 100,
            updated_at: chrono::Utc::now(),
        }
    }

    /// 设置 TTL
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = ttl;
        self.updated_at = chrono::Utc::now();
        self
    }

    /// 设置 L1 最大容量
    pub fn with_l1_max_capacity(mut self, capacity: u64) -> Self {
        self.l1_max_capacity = capacity;
        self.updated_at = chrono::Utc::now();
        self
    }

    /// 设置 L1 淘汰策略
    pub fn with_l1_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.l1_eviction_policy = policy;
        self.updated_at = chrono::Utc::now();
        self
    }

    /// 设置 L2 默认 TTL
    pub fn with_l2_default_ttl(mut self, ttl: u64) -> Self {
        self.l2_default_ttl = ttl;
        self.updated_at = chrono::Utc::now();
        self
    }

    /// 设置批量写入
    pub fn with_enable_batch_write(mut self, enable: bool) -> Self {
        self.enable_batch_write = enable;
        self.updated_at = chrono::Utc::now();
        self
    }

    /// 设置批量写入大小
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self.updated_at = chrono::Utc::now();
        self
    }
}

/// 动态配置管理
///
/// 用于运行时配置更新和热重载
#[derive(Serialize, Debug, Clone, Default)]
pub struct DynamicConfig {
    /// 存储各服务的运行时策略配置
    #[serde(skip)]
    strategies: dashmap::DashMap<String, CacheStrategy>,
}

impl DynamicConfig {
    /// 创建新的动态配置管理器
    pub fn new() -> Self {
        Self {
            strategies: dashmap::DashMap::new(),
        }
    }

    /// 获取服务的运行时策略配置
    ///
    /// 如果服务不存在，返回 None
    pub fn get_strategy(&self, service_name: &str) -> Option<CacheStrategy> {
        self.strategies.get(service_name).map(|s| s.clone())
    }

    /// 更新服务的运行时策略配置
    ///
    /// 如果服务不存在，会创建一个新的策略配置
    pub fn update_strategy(&self, strategy: CacheStrategy) {
        self.strategies
            .insert(strategy.service_name.clone(), strategy);
    }

    /// 删除服务的运行时策略配置
    ///
    /// 删除后，服务将使用静态配置
    pub fn remove_strategy(&self, service_name: &str) {
        self.strategies.remove(service_name);
    }

    /// 检查服务是否有运行时策略配置
    pub fn has_strategy(&self, service_name: &str) -> bool {
        self.strategies.contains_key(service_name)
    }

    /// 获取所有已配置的服务名称
    pub fn service_names(&self) -> Vec<String> {
        self.strategies.iter().map(|s| s.key().clone()).collect()
    }

    /// 清空所有运行时策略配置
    pub fn clear(&self) {
        self.strategies.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eviction_policy_default() {
        assert_eq!(EvictionPolicy::default(), EvictionPolicy::TinyLfu);
    }

    #[test]
    fn test_cache_strategy_builder() {
        let strategy = CacheStrategy::new("test_service")
            .with_ttl(600)
            .with_l1_max_capacity(20000)
            .with_l1_eviction_policy(EvictionPolicy::Lru);

        assert_eq!(strategy.service_name, "test_service");
        assert_eq!(strategy.ttl, 600);
        assert_eq!(strategy.l1_max_capacity, 20000);
        assert_eq!(strategy.l1_eviction_policy, EvictionPolicy::Lru);
    }

    #[test]
    fn test_dynamic_config() {
        let config = DynamicConfig::new();

        // 初始状态
        assert!(!config.has_strategy("test"));

        // 添加策略
        let strategy = CacheStrategy::new("test").with_ttl(500);
        config.update_strategy(strategy.clone());

        assert!(config.has_strategy("test"));
        assert_eq!(config.get_strategy("test"), Some(strategy));

        // 删除策略
        config.remove_strategy("test");
        assert!(!config.has_strategy("test"));
    }

    #[test]
    fn test_dynamic_config_service_names() {
        let config = DynamicConfig::new();

        config.update_strategy(CacheStrategy::new("service1"));
        config.update_strategy(CacheStrategy::new("service2"));

        let names = config.service_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"service1".to_string()));
        assert!(names.contains(&"service2".to_string()));
    }
}
