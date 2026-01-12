//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 统一配置模块入口
//!
//! Feature-gated 配置系统：
//! - L1 配置需要 l1-moka feature  
//! - L2 配置需要 l2-redis feature
//! - confers 配置需要 confers feature

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod builder;
#[cfg(feature = "l1-moka")]
pub mod layer;
pub mod legacy_config;
pub mod service;
pub mod validation;

#[cfg(all(feature = "confers", feature = "config-toml"))]
pub mod confers_macro;

#[cfg(feature = "l2-redis")]
pub use crate::config::legacy_config::{ClusterConfig, SentinelConfig};
pub use builder::OxcacheConfigBuilder;
#[cfg(feature = "l1-moka")]
pub use layer::{EvictionPolicy, L1LayerConfig, L2LayerConfig, LayerConfig, TwoLevelLayerConfig};
#[cfg(feature = "l1-moka")]
pub use service::L1Config;
pub use service::{CacheType, RedisMode, ServiceConfig};
#[cfg(feature = "l2-redis")]
pub use service::{L2Config, TwoLevelConfig};
pub use validation::ConfigValidation;

pub use self::legacy_config::{
    CacheStrategy, CacheWarmupConfig, Config as LegacyConfig, DynamicConfig,
    EvictionPolicy as LegacyEvictionPolicy, GlobalConfig as LegacyGlobalConfig,
    InvalidationChannelConfig, RedisMode as LegacyRedisMode, SerializationType, WarmupDataSource,
};

/// 配置来源枚举
#[cfg(feature = "confers")]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum ConfigSource {
    Code,
    Macro(String),
    File(String),
}

/// 配置版本
pub const CONFIG_VERSION: u32 = 2;
pub const CONFIG_VERSION_FIELD: &str = "config_version";

/// 全局配置（始终可用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub default_ttl: u64,
    pub health_check_interval: u64,
    pub serialization: SerializationType,
    pub enable_metrics: bool,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_ttl: 300,
            health_check_interval: 60,
            serialization: SerializationType::default(),
            enable_metrics: false,
        }
    }
}

impl GlobalConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = ttl;
        self
    }

    pub fn with_health_check_interval(mut self, interval: u64) -> Self {
        self.health_check_interval = interval;
        self
    }

    pub fn with_serialization(mut self, serialization: SerializationType) -> Self {
        self.serialization = serialization;
        self
    }

    pub fn with_enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }
}

/// 统一的配置入口结构体
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OxcacheConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    pub global: GlobalConfig,
    pub services: HashMap<String, ServiceConfig>,
    #[cfg(feature = "l1-moka")]
    pub layer: Option<LayerConfig>,
    #[cfg(feature = "confers")]
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extensions: HashMap<String, serde_json::Value>,
    #[cfg(feature = "confers")]
    pub source: Option<ConfigSource>,
}

impl OxcacheConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> OxcacheConfigBuilder {
        OxcacheConfigBuilder::new()
    }

    pub fn validate(&self) -> Result<(), String> {
        ConfigValidation::validate(self)
    }

    #[cfg(feature = "confers")]
    pub fn source(&self) -> &Option<ConfigSource> {
        &self.source
    }

    #[cfg(feature = "confers")]
    pub fn set_source(&mut self, source: ConfigSource) {
        self.source = Some(source);
    }

    pub fn is_l1_enabled(&self) -> bool {
        cfg!(feature = "l1-moka")
    }

    pub fn is_l2_enabled(&self) -> bool {
        cfg!(feature = "l2-redis")
    }

    pub fn available_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();

        if cfg!(feature = "l1-moka") {
            features.push("l1-moka");
        }
        if cfg!(feature = "l2-redis") {
            features.push("l2-redis");
        }
        if cfg!(feature = "bloom-filter") {
            features.push("bloom-filter");
        }
        if cfg!(feature = "rate-limiting") {
            features.push("rate-limiting");
        }
        if cfg!(feature = "batch-write") {
            features.push("batch-write");
        }
        if cfg!(feature = "wal-recovery") {
            features.push("wal-recovery");
        }
        if cfg!(feature = "serialization") {
            features.push("serialization");
        }
        if cfg!(feature = "compression") {
            features.push("compression");
        }
        if cfg!(feature = "database") {
            features.push("database");
        }
        if cfg!(feature = "cli") {
            features.push("cli");
        }
        if cfg!(feature = "opentelemetry") {
            features.push("opentelemetry");
        }
        if cfg!(feature = "metrics") {
            features.push("metrics");
        }
        if cfg!(feature = "confers") {
            features.push("confers");
        }

        features
    }
}

/// 配置入口函数
pub fn oxcache_config() -> OxcacheConfigBuilder {
    OxcacheConfigBuilder::new()
}

#[deprecated(since = "0.2.0", note = "请使用 `OxcacheConfig` 替代 `Config`")]
pub type Config = OxcacheConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oxcache_config_default() {
        let config = OxcacheConfig::default();
        assert!(config.services.is_empty());
        #[cfg(feature = "l1-moka")]
        assert!(config.layer.is_none());
    }

    #[test]
    fn test_oxcache_config_builder() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();
        assert!(!config.services.is_empty() || config.global.default_ttl == 300);
    }

    #[test]
    fn test_global_config_builder() {
        let global = GlobalConfig::new()
            .with_default_ttl(600)
            .with_health_check_interval(30)
            .with_serialization(SerializationType::Json)
            .with_enable_metrics(true);
        assert_eq!(global.default_ttl, 600);
    }

    #[test]
    fn test_feature_flags() {
        let config = OxcacheConfig::new();
        #[cfg(feature = "l1-moka")]
        assert!(config.is_l1_enabled());
        #[cfg(feature = "l2-redis")]
        assert!(config.is_l2_enabled());
    }
}
