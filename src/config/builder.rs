//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! OxcacheConfig Builder 实现
//!
//! 提供 feature-gated 的配置构建器。

use std::collections::HashMap;

use crate::config::{GlobalConfig, OxcacheConfig, ServiceConfig};

/// OxcacheConfig 构建器
///
/// 使用 Builder 模式提供链式 API 构建配置。
/// 支持 feature-gated 配置：
/// - LayerConfig: 需要 l1-moka feature
/// - confers 扩展: 需要 confers feature
///
/// # 示例
///
/// ```rust
/// use oxcache::{OxcacheConfigBuilder, GlobalConfig, ServiceConfig, CacheType};
///
/// let config = OxcacheConfigBuilder::new()
///     .with_global(GlobalConfig::default())
///     .with_service("api", ServiceConfig::two_level())
///     .build();
/// ```
#[derive(Debug, Default, Clone)]
pub struct OxcacheConfigBuilder {
    global: Option<GlobalConfig>,
    services: HashMap<String, ServiceConfig>,
    #[cfg(feature = "l1-moka")]
    layer: Option<LayerConfig>,
}

#[cfg(feature = "l1-moka")]
type LayerConfig = crate::config::layer::LayerConfig;

impl OxcacheConfigBuilder {
    /// 创建一个新的构建器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置全局配置
    pub fn with_global(mut self, global: GlobalConfig) -> Self {
        self.global = Some(global);
        self
    }

    /// 添加或更新服务配置
    pub fn with_service(mut self, name: &str, service: ServiceConfig) -> Self {
        self.services.insert(name.to_string(), service);
        self
    }

    /// 添加多个服务配置
    pub fn with_services(mut self, services: HashMap<String, ServiceConfig>) -> Self {
        self.services = services;
        self
    }

    /// 设置层级配置（需要 l1-moka feature）
    #[cfg(feature = "l1-moka")]
    pub fn with_layer(mut self, layer: LayerConfig) -> Self {
        self.layer = Some(layer);
        self
    }

    /// 验证配置
    ///
    /// 在构建前进行验证，如果验证失败返回错误。
    pub fn validate(self) -> Result<Self, String> {
        let config = self.clone();
        config.validate_inner()?;
        Ok(self)
    }

    /// 构建 OxcacheConfig
    ///
    /// # Panics
    ///
    /// 如果验证失败，会 panic。建议先调用 `validate()` 方法。
    pub fn build(self) -> OxcacheConfig {
        let global = self.global.unwrap_or_default();

        // 确保至少有一个默认服务
        let mut services = self.services;
        if services.is_empty() {
            services.insert("default".to_string(), ServiceConfig::default());
        }

        // 构建配置
        #[cfg(all(feature = "l1-moka", feature = "confers"))]
        {
            OxcacheConfig {
                config_version: Some(crate::config::CONFIG_VERSION),
                global,
                services,
                layer: self.layer,
                extensions: HashMap::new(),
                source: Some(crate::config::ConfigSource::Code),
            }
        }

        // 构建配置（l1-moka + 无 confers）
        #[cfg(all(feature = "l1-moka", not(feature = "confers")))]
        {
            OxcacheConfig {
                config_version: Some(crate::config::CONFIG_VERSION),
                global,
                services,
                layer: self.layer,
            }
        }

        // 构建配置（无 l1-moka + confers）
        #[cfg(all(not(feature = "l1-moka"), feature = "confers"))]
        {
            OxcacheConfig {
                config_version: Some(crate::config::CONFIG_VERSION),
                global,
                services,
                extensions: HashMap::new(),
                source: Some(crate::config::ConfigSource::Code),
            }
        }

        // 构建配置（无 l1-moka + 无 confers）
        #[cfg(all(not(feature = "l1-moka"), not(feature = "confers")))]
        {
            OxcacheConfig {
                config_version: Some(crate::config::CONFIG_VERSION),
                global,
                services,
            }
        }
    }

    /// 内部验证逻辑
    fn validate_inner(&self) -> Result<(), String> {
        let global = self.global.clone().unwrap_or_default();

        // 验证全局配置
        if global.default_ttl == 0 {
            return Err("Global default_ttl cannot be zero".to_string());
        }

        if global.default_ttl > 86400 * 30 {
            return Err("Global default_ttl cannot exceed 30 days".to_string());
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

            // 验证 TTL
            let service_ttl = service.ttl.unwrap_or(global.default_ttl);
            if service_ttl == 0 {
                return Err(format!("Service '{}' TTL cannot be zero", name));
            }

            if service_ttl > 86400 * 30 {
                return Err(format!("Service '{}' TTL cannot exceed 30 days", name));
            }

            // 验证 L1 配置（需要 l1-moka feature）
            #[cfg(feature = "l1-moka")]
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

                if l1_config.cleanup_interval_secs > 0
                    && l1_config.cleanup_interval_secs > service_ttl
                {
                    return Err(format!(
                        "Service '{}' L1 cleanup_interval_secs ({}) must be <= service TTL ({})",
                        name, l1_config.cleanup_interval_secs, service_ttl
                    ));
                }
            }

            // 验证 L2 配置（需要 l2-redis feature）
            #[cfg(feature = "l2-redis")]
            if let Some(l2_config) = &service.l2 {
                // 验证 L1 TTL <= L2 TTL
                if let Some(l2_specific_ttl) = l2_config.default_ttl {
                    if l2_specific_ttl == 0 {
                        return Err(format!("Service '{}' L2 TTL cannot be zero", name));
                    }

                    if service_ttl > l2_specific_ttl {
                        return Err(format!(
                            "Service '{}' L1 TTL ({}) must be <= L2 TTL ({})",
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
        }

        Ok(())
    }

    /// 获取当前配置的特征信息
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GlobalConfig, ServiceConfig};

    #[test]
    fn test_builder_default() {
        let builder = OxcacheConfigBuilder::new();
        assert!(builder.global.is_none());
        assert!(builder.services.is_empty());

        #[cfg(feature = "l1-moka")]
        assert!(builder.layer.is_none());
    }

    #[test]
    fn test_builder_with_global() {
        let builder = OxcacheConfigBuilder::new()
            .with_global(GlobalConfig::default())
            .build();

        assert_eq!(builder.global.default_ttl, 300);
    }

    #[test]
    fn test_builder_with_service() {
        let builder = OxcacheConfigBuilder::new()
            .with_service("api", ServiceConfig::l1_only())
            .build();

        assert!(builder.services.contains_key("api"));
    }

    #[test]
    fn test_builder_validate() {
        let builder = OxcacheConfigBuilder::new()
            .with_global(GlobalConfig::default())
            .validate()
            .unwrap();

        assert!(builder.build().validate().is_ok());
    }

    #[test]
    fn test_builder_default_service() {
        let config = OxcacheConfigBuilder::new()
            .with_global(GlobalConfig::default())
            .build();

        // 应该自动添加默认服务
        assert!(config.services.contains_key("default"));
    }

    #[test]
    #[cfg(feature = "l1-moka")]
    fn test_builder_with_layer() {
        let layer = crate::config::layer::LayerConfig::default();
        let builder = OxcacheConfigBuilder::new().with_layer(layer).build();

        assert!(builder.layer.is_some());
    }

    #[test]
    fn test_builder_validate_service_name() {
        let builder = OxcacheConfigBuilder::new()
            .with_global(GlobalConfig::default())
            .with_service("", ServiceConfig::l1_only());

        assert!(builder.validate().is_err());
    }

    #[test]
    fn test_builder_validate_ttl() {
        let global = GlobalConfig {
            default_ttl: 0,
            ..Default::default()
        };

        let builder = OxcacheConfigBuilder::new().with_global(global);

        assert!(builder.validate().is_err());
    }
}
