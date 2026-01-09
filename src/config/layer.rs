//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 层级配置模块

use serde::Deserialize;
use std::fmt;

/// 层级化配置
///
/// 提供 L1、L2 和双层缓存的默认配置层级。
/// 服务配置可以继承层级默认配置，减少重复。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LayerConfig {
    /// L1 默认配置
    pub l1: L1LayerConfig,
    /// L2 默认配置
    pub l2: L2LayerConfig,
    /// 双层缓存默认配置
    pub two_level: TwoLevelLayerConfig,
}

impl LayerConfig {
    /// 创建新的层级配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 L1 默认配置
    pub fn with_l1(mut self, l1: L1LayerConfig) -> Self {
        self.l1 = l1;
        self
    }

    /// 设置 L2 默认配置
    pub fn with_l2(mut self, l2: L2LayerConfig) -> Self {
        self.l2 = l2;
        self
    }

    /// 设置双层缓存默认配置
    pub fn with_two_level(mut self, two_level: TwoLevelLayerConfig) -> Self {
        self.two_level = two_level;
        self
    }
}

/// L1 层级配置
///
/// 定义 L1 内存缓存的默认配置。
#[derive(Debug, Clone, Deserialize)]
pub struct L1LayerConfig {
    /// 最大缓存容量
    pub max_capacity: u64,
    /// 键的最大长度
    pub max_key_length: usize,
    /// 值的最大大小（字节）
    pub max_value_size: usize,
    /// 过期清理间隔（秒）
    pub cleanup_interval_secs: u64,
    /// 淘汰策略
    pub eviction_policy: EvictionPolicy,
}

impl L1LayerConfig {
    /// 创建新的 L1 层级配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置最大容量
    pub fn with_max_capacity(mut self, capacity: u64) -> Self {
        self.max_capacity = capacity;
        self
    }

    /// 设置淘汰策略
    pub fn with_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }
}

impl Default for L1LayerConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            max_key_length: 512,
            max_value_size: 1024 * 1024, // 1MB
            cleanup_interval_secs: 300,
            eviction_policy: EvictionPolicy::default(),
        }
    }
}

/// L2 层级配置
///
/// 定义 L2 分布式缓存的默认配置。
#[derive(Debug, Clone, Deserialize)]
pub struct L2LayerConfig {
    /// Redis 模式
    pub mode: RedisMode,
    /// 连接字符串
    pub connection_string: String,
    /// 连接超时（毫秒）
    pub connection_timeout_ms: u64,
    /// 命令超时（毫秒）
    pub command_timeout_ms: u64,
    /// 默认 TTL（秒）
    pub default_ttl: u64,
    /// 键的最大长度
    pub max_key_length: usize,
    /// 值的最大大小（字节）
    pub max_value_size: usize,
}

impl L2LayerConfig {
    /// 创建新的 L2 层级配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置连接字符串
    pub fn with_connection_string(mut self, connection_string: String) -> Self {
        self.connection_string = connection_string;
        self
    }

    /// 设置 Redis 模式
    pub fn with_mode(mut self, mode: RedisMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置默认 TTL
    pub fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = ttl;
        self
    }
}

impl Default for L2LayerConfig {
    fn default() -> Self {
        Self {
            mode: RedisMode::Standalone,
            connection_string: String::new(),
            connection_timeout_ms: 5000,
            command_timeout_ms: 30000,
            default_ttl: 3600,
            max_key_length: 512,
            max_value_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// 双层缓存层级配置
///
/// 定义双层缓存的默认行为配置。
#[derive(Debug, Clone, Deserialize)]
pub struct TwoLevelLayerConfig {
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
}

impl TwoLevelLayerConfig {
    /// 创建新的双层缓存层级配置
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

impl Default for TwoLevelLayerConfig {
    fn default() -> Self {
        Self {
            promote_on_hit: true,
            enable_batch_write: false,
            batch_size: 100,
            batch_interval_ms: 10,
            max_key_length: Some(512),
            max_value_size: Some(10 * 1024 * 1024), // 10MB
        }
    }
}

/// 淘汰策略
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EvictionPolicy {
    /// 最近最少使用
    #[default]
    Lru,
    /// 最不经常使用
    Lfu,
    /// TinyLFU
    TinyLfu,
    /// 随机淘汰
    Random,
}

impl fmt::Display for EvictionPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvictionPolicy::Lru => write!(f, "lru"),
            EvictionPolicy::Lfu => write!(f, "lfu"),
            EvictionPolicy::TinyLfu => write!(f, "tiny_lfu"),
            EvictionPolicy::Random => write!(f, "random"),
        }
    }
}

/// Redis 模式
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_config_default() {
        let layer = LayerConfig::default();
        assert_eq!(layer.l1.max_capacity, 10000);
        assert_eq!(layer.l2.default_ttl, 3600);
    }

    #[test]
    fn test_l1_layer_config() {
        let l1 = L1LayerConfig::new()
            .with_max_capacity(50000)
            .with_eviction_policy(EvictionPolicy::Lru);

        assert_eq!(l1.max_capacity, 50000);
        assert_eq!(l1.eviction_policy, EvictionPolicy::Lru);
    }

    #[test]
    fn test_l2_layer_config() {
        let l2 = L2LayerConfig::new()
            .with_connection_string("redis://localhost:6379".to_string())
            .with_default_ttl(7200);

        assert_eq!(l2.connection_string, "redis://localhost:6379");
        assert_eq!(l2.default_ttl, 7200);
    }

    #[test]
    fn test_two_level_layer_config() {
        let two_level = TwoLevelLayerConfig::new()
            .with_enable_batch_write(true)
            .with_batch_size(200);

        assert!(two_level.enable_batch_write);
        assert_eq!(two_level.batch_size, 200);
    }
}
