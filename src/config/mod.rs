//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 统一配置模块入口
//!
//! 提供 `OxcacheConfig` 作为配置统一入口，支持：
//! - 嵌入式/集成模式：使用 Builder 模式构建配置
//! - confers 宏模式：使用声明式宏定义配置（需要 confers 特性）

use std::collections::HashMap;

pub mod builder;
pub mod layer;
pub mod legacy_config;
pub mod service;
pub mod validation; // 旧配置保持兼容

#[cfg(feature = "confers")]
pub mod confers_macro;

pub use builder::OxcacheConfigBuilder;
pub use layer::{EvictionPolicy, L1LayerConfig, L2LayerConfig, LayerConfig, TwoLevelLayerConfig};
pub use service::{CacheType, L1Config, L2Config, RedisMode, ServiceConfig, TwoLevelConfig};
pub use validation::ConfigValidation;

pub use crate::config::legacy_config::{
    CacheStrategy, CacheWarmupConfig, ClusterConfig, Config as LegacyConfig, DynamicConfig,
    EvictionPolicy as LegacyEvictionPolicy, GlobalConfig, InvalidationChannelConfig,
    RedisMode as LegacyRedisMode, SentinelConfig, SerializationType, WarmupDataSource,
};

/// 配置来源枚举
#[cfg(feature = "confers")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    /// 代码构建
    Code,
    /// confers 宏
    Macro(String),
    /// 文件加载
    File(String),
}

/// 配置版本
pub const CONFIG_VERSION: u32 = 2;
pub const CONFIG_VERSION_FIELD: &str = "config_version";

/// 统一的配置入口结构体
///
/// 这是配置系统的统一入口点，支持：
/// - Builder 模式构建（嵌入式/集成场景）
/// - confers 宏配置（声明式配置场景）
///
/// # 示例
///
/// ## Builder 模式
///
/// ```rust
/// use oxcache::{oxcache_config, CacheType, ServiceConfig};
///
/// let config = oxcache_config()
///     .with_global(GlobalConfig::default())
///     .with_service("default", ServiceConfig::two_level())
///     .build();
/// ```
///
/// ## 服务配置示例
///
/// ```rust
/// use oxcache::{ServiceConfig, L2Config, RedisMode, CacheType};
/// use secrecy::SecretString;
///
/// // 创建双层缓存服务配置
/// let service = ServiceConfig::two_level()
///     .with_ttl(3600)
///     .with_l2_config(
///         L2Config::new()
///             .with_mode(RedisMode::Standalone)
///             .with_connection_string("redis://localhost:6379")
///             .with_password("secret_password")
///     );
/// ```
///
/// ## 从 TOML 文件加载（需要 confers 特性）
///
/// ```rust,ignore
/// #[cfg(feature = "confers")]
/// async fn load_config() -> anyhow::Result<()> {
///     let config = confers_load("oxcache.toml")?;
///     oxcache::init(config).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct OxcacheConfig {
    /// 配置版本（向后兼容）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    /// 全局配置
    pub global: GlobalConfig,
    /// 服务配置字典
    pub services: HashMap<String, ServiceConfig>,
    /// 层级化配置
    pub layer: Option<LayerConfig>,
    /// 扩展配置
    #[cfg(feature = "confers")]
    pub extensions: HashMap<String, toml::Value>,
    /// 配置来源（用于诊断）
    #[cfg(feature = "confers")]
    pub source: Option<ConfigSource>,
}

impl OxcacheConfig {
    /// 创建新的空配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用 Builder 模式创建配置
    pub fn builder() -> OxcacheConfigBuilder {
        OxcacheConfigBuilder::new()
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), String> {
        ConfigValidation::validate(self)
    }

    /// 获取配置来源描述
    #[cfg(feature = "confers")]
    pub fn source(&self) -> &Option<ConfigSource> {
        &self.source
    }

    #[cfg(feature = "confers")]
    pub fn set_source(&mut self, source: ConfigSource) {
        self.source = Some(source);
    }
}

/// 配置入口函数
///
/// 提供便捷的配置构建入口。
///
/// # 示例
///
/// ```rust
/// use oxcache::{oxcache_config, ServiceConfig};
///
/// let config = oxcache_config()
///     .with_service("default", ServiceConfig::l1_only())
///     .build();
/// ```
pub fn oxcache_config() -> OxcacheConfigBuilder {
    OxcacheConfigBuilder::new()
}

// 向后兼容：旧 Config 类型别名
#[deprecated(
    since = "0.2.0",
    note = "请使用 `OxcacheConfig` 替代 `Config`。迁移方式：\n\
            1. 使用 `oxcache_config()` Builder 创建配置\n\
            2. 或使用 `type Config = OxcacheConfig` 临时兼容"
)]
pub type Config = OxcacheConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oxcache_config_default() {
        let config = OxcacheConfig::default();
        assert!(config.services.is_empty());
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
    fn test_oxcache_config_validate() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        assert!(config.validate().is_ok());
    }
}
