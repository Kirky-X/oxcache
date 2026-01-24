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
pub mod layer;
pub mod legacy_config;
pub mod unified;
pub mod validation;

#[cfg(feature = "confers")]
pub mod confers_macro;

#[cfg(feature = "config-dynamic")]
pub mod dynamic;

#[cfg(feature = "confers")]
pub use confers_macro::confers_load as load_from_file;

#[cfg(feature = "l2-redis")]
use crate::config::legacy_config::ServiceConfig;
#[cfg(feature = "l2-redis")]
pub use crate::config::legacy_config::{ClusterConfig, SentinelConfig};
pub use builder::OxcacheConfigBuilder;
#[cfg(feature = "l1-moka")]
pub use layer::{EvictionPolicy, L1LayerConfig, L2LayerConfig, LayerConfig, TwoLevelLayerConfig};
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
///
/// 提供构建器模式来创建不可变配置,
/// 确保配置对象的线程安全和共享安全。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    default_ttl: u64,
    health_check_interval: u64,
    serialization: SerializationType,
    enable_metrics: bool,
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
    /// 创建新的全局配置（使用默认值）
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建 builder 模式配置
    pub fn builder() -> GlobalConfigBuilder {
        GlobalConfigBuilder::new()
    }

    /// 获取默认 TTL
    pub fn default_ttl(&self) -> u64 {
        self.default_ttl
    }

    /// 获取健康检查间隔
    pub fn health_check_interval(&self) -> u64 {
        self.health_check_interval
    }

    /// 获取序列化类型
    pub fn serialization(&self) -> &SerializationType {
        &self.serialization
    }

    /// 是否启用指标收集
    pub fn is_metrics_enabled(&self) -> bool {
        self.enable_metrics
    }

    /// 设置默认 TTL（返回新的配置实例）
    #[must_use]
    pub fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// 设置健康检查间隔（返回新的配置实例）
    #[must_use]
    pub fn with_health_check_interval(mut self, interval: u64) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// 设置序列化类型（返回新的配置实例）
    #[must_use]
    pub fn with_serialization(mut self, serialization: SerializationType) -> Self {
        self.serialization = serialization;
        self
    }

    /// 是否启用指标收集（返回新的配置实例）
    #[must_use]
    pub fn with_enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }
}

/// 全局配置构建器
#[derive(Debug, Clone, Default)]
pub struct GlobalConfigBuilder {
    default_ttl: u64,
    health_check_interval: u64,
    serialization: SerializationType,
    enable_metrics: bool,
}

impl GlobalConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置默认 TTL（秒）
    pub fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// 设置健康检查间隔（秒）
    pub fn with_health_check_interval(mut self, interval: u64) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// 设置序列化类型
    pub fn with_serialization(mut self, serialization: SerializationType) -> Self {
        self.serialization = serialization;
        self
    }

    /// 是否启用指标收集
    pub fn with_enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }

    /// 构建全局配置
    pub fn build(self) -> GlobalConfig {
        GlobalConfig {
            default_ttl: self.default_ttl,
            health_check_interval: self.health_check_interval,
            serialization: self.serialization,
            enable_metrics: self.enable_metrics,
        }
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

        add_feature_if_enabled!(features, "l1-moka");
        add_feature_if_enabled!(features, "l2-redis");
        add_feature_if_enabled!(features, "bloom-filter");
        add_feature_if_enabled!(features, "rate-limiting");
        add_feature_if_enabled!(features, "batch-write");
        add_feature_if_enabled!(features, "wal-recovery");
        add_feature_if_enabled!(features, "serialization");
        add_feature_if_enabled!(features, "compression");
        add_feature_if_enabled!(features, "database");
        add_feature_if_enabled!(features, "cli");
        add_feature_if_enabled!(features, "opentelemetry");
        add_feature_if_enabled!(features, "metrics");
        add_feature_if_enabled!(features, "confers");

        features
    }
}

/// 配置入口函数
pub fn oxcache_config() -> OxcacheConfigBuilder {
    OxcacheConfigBuilder::new()
}

/// 从文件加载配置（统一走 confers 路径）
///
/// 此函数在 confers feature 启用时可用
#[cfg(feature = "confers")]
pub fn load_config_from_file(path: &str) -> Result<OxcacheConfig, String> {
    confers_macro::confers_load(path)
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

// ============================================================================
// Unified Configuration Exports
// ============================================================================

// Re-export unified configuration
pub use unified::{
    UnifiedConfig, UnifiedConfigBuilder, BackendConfig, BackendType,
    MemoryBackendConfig, RedisBackendConfig, TieredBackendConfig,
    PerformanceConfig, SerializationConfig, BatchConfig, ConcurrencyConfig,
    FeatureConfig, MonitoringConfig, MetricsConfig, HealthCheckConfig,
    LoggingConfig, SecurityConfig, convenience as config_convenience
};
