//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! OxcacheConfig Builder 实现

use std::collections::HashMap;

use crate::config::{GlobalConfig, LayerConfig, OxcacheConfig, ServiceConfig};

/// OxcacheConfig 构建器
///
/// 使用 Builder 模式提供链式 API 构建配置。
///
/// # 示例
///
/// ```rust
/// use oxcache::{OxcacheConfigBuilder, GlobalConfig, ServiceConfig, CacheType};
///
/// let config = OxcacheConfigBuilder::new()
///     .with_global(GlobalConfig::default())
///     .with_service("api", ServiceConfig::two_level())
///     .with_layer(LayerConfig::default())
///     .build();
/// ```
#[derive(Debug, Default, Clone)]
pub struct OxcacheConfigBuilder {
    global: Option<GlobalConfig>,
    services: HashMap<String, ServiceConfig>,
    layer: Option<LayerConfig>,
}

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

    /// 设置层级配置
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

        OxcacheConfig {
            global,
            services,
            layer: self.layer,
            #[cfg(feature = "confers")]
            extensions: HashMap::new(),
            #[cfg(feature = "confers")]
            source: Some(crate::config::ConfigSource::Code),
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
            crate::config::validation::validate_service(name, service, &global)?;
        }

        Ok(())
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
}
