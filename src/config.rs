//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的配置结构和解析逻辑。

use secrecy::SecretString;
use serde::Deserialize;
use std::collections::HashMap;

pub const CONFIG_VERSION: u32 = 1;
pub const CONFIG_VERSION_FIELD: &str = "config_version";

#[derive(Debug, Deserialize, Clone, Default)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
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
#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct L2Config {
    /// Redis模式
    pub mode: RedisMode,
    /// 连接字符串
    pub connection_string: SecretString,
    /// 连接超时时间（毫秒）
    pub connection_timeout_ms: u64,
    /// 命令执行超时时间（毫秒）
    pub command_timeout_ms: u64,
    /// Redis 密码（可选，使用 SecretString 保护）
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
#[derive(Deserialize, Clone, Debug)]
pub struct SentinelConfig {
    /// 主节点名称
    pub master_name: String,
    ////// 哨兵节点列表
    pub nodes: Vec<String>,
}

/// 集群配置
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Debug)]
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
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RedisMode {
    /// 单机模式
    Standalone,
    /// 哨兵模式
    Sentinel,
    /// 集群模式
    Cluster,
}
