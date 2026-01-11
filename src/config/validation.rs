//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 配置验证模块

use secrecy::ExposeSecret;
use std::collections::HashMap;

use crate::config::{CacheType, GlobalConfig, L1Config, L2Config, OxcacheConfig, ServiceConfig};

/// 配置验证trait
pub trait ConfigValidation {
    /// 验证配置
    fn validate(&self) -> Result<(), String>;
}

/// 验证逻辑实现
impl ConfigValidation for OxcacheConfig {
    fn validate(&self) -> Result<(), String> {
        // 验证全局配置
        self.validate_global()?;

        // 验证服务配置
        for (name, service) in &self.services {
            self.validate_service(name, service)?;
        }

        Ok(())
    }
}

impl OxcacheConfig {
    /// 验证全局配置
    pub fn validate_global(&self) -> Result<(), String> {
        let global = &self.global;

        if global.default_ttl == 0 {
            return Err("Global default_ttl cannot be zero".to_string());
        }

        if global.default_ttl > 86400 * 30 {
            return Err("Global default_ttl cannot exceed 30 days".to_string());
        }

        Ok(())
    }

    /// 验证单个服务配置
    pub fn validate_service(&self, name: &str, service: &ServiceConfig) -> Result<(), String> {
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

        let global = &self.global;
        let service_ttl = service.ttl.unwrap_or(global.default_ttl);

        // 验证 TTL
        if service_ttl == 0 {
            return Err(format!("Service '{}' TTL cannot be zero", name));
        }

        if service_ttl > 86400 * 30 {
            return Err(format!("Service '{}' TTL cannot exceed 30 days", name));
        }

        // 验证 L1 配置
        if let Some(l1_config) = &service.l1 {
            Self::validate_l1_config(name, l1_config, service_ttl)?;
        }

        // 验证 L2 配置
        if let Some(l2_config) = &service.l2 {
            Self::validate_l2_config(name, l2_config, service_ttl)?;
        }

        // 验证双层缓存配置
        if let Some(two_level_config) = &service.two_level {
            Self::validate_two_level_config(name, two_level_config)?;
        }

        Ok(())
    }

    /// 验证 L1 配置
    fn validate_l1_config(
        name: &str,
        l1_config: &L1Config,
        service_ttl: u64,
    ) -> Result<(), String> {
        if l1_config.max_capacity == 0 {
            return Err(format!("Service '{}' L1 max_capacity cannot be zero", name));
        }

        if l1_config.max_capacity > 10_000_000 {
            return Err(format!(
                "Service '{}' L1 max_capacity cannot exceed 10,000,000",
                name
            ));
        }

        if l1_config.cleanup_interval_secs > 0 && l1_config.cleanup_interval_secs > service_ttl {
            return Err(format!(
                "Service '{}' L1 cleanup_interval_secs ({}) must be <= service TTL ({})",
                name, l1_config.cleanup_interval_secs, service_ttl
            ));
        }

        Ok(())
    }

    /// 验证 L2 配置
    fn validate_l2_config(
        name: &str,
        l2_config: &L2Config,
        service_ttl: u64,
    ) -> Result<(), String> {
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

        // 生产环境安全检查
        let conn_str = l2_config.connection_string.expose_secret();
        let is_production = conn_str.contains("production")
            || conn_str.contains("prod")
            || (!conn_str.contains("localhost")
                && !conn_str.contains("127.0.0.1")
                && !conn_str.contains("192.168.")
                && !conn_str.contains("10."));

        if is_production {
            // 检查密码
            if l2_config.password.is_none() {
                return Err(format!(
                    "Service '{}' is in production but Redis password is not configured",
                    name
                ));
            }

            // 检查 TLS
            if !l2_config.enable_tls {
                return Err(format!(
                    "Service '{}' is in production but TLS is not enabled",
                    name
                ));
            }
        }

        Ok(())
    }

    /// 验证双层缓存配置
    fn validate_two_level_config(
        name: &str,
        two_level_config: &crate::config::TwoLevelConfig,
    ) -> Result<(), String> {
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

        // 验证键大小限制
        if let Some(max_key_length) = two_level_config.max_key_length {
            if max_key_length == 0 || max_key_length > 1024 {
                return Err(format!(
                    "Service '{}' max_key_length must be between 1 and 1024",
                    name
                ));
            }
        }

        // 验证值大小限制
        if let Some(max_value_size) = two_level_config.max_value_size {
            if max_value_size == 0 || max_value_size > 10 * 1024 * 1024 {
                return Err(format!(
                    "Service '{}' max_value_size must be between 1 and 10MB",
                    name
                ));
            }
        }

        Ok(())
    }
}

/// 从旧配置迁移验证逻辑
pub fn validate_service(
    name: &str,
    service: &ServiceConfig,
    global: &GlobalConfig,
) -> Result<(), String> {
    let config = OxcacheConfig {
        config_version: None,
        global: global.clone(),
        services: HashMap::new(),
        layer: None,
        #[cfg(feature = "confers")]
        extensions: HashMap::new(),
        #[cfg(feature = "confers")]
        source: None,
    };

    config.validate_service(name, service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{oxcache_config, GlobalConfig, L1Config, L2Config, ServiceConfig};

    #[test]
    fn test_validate_empty_service_name() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        let service = ServiceConfig::l1_only();
        assert!(config.validate_service("", &service).is_err());
    }

    #[test]
    fn test_validate_valid_service() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        let service = ServiceConfig::l1_only();
        assert!(config.validate_service("valid_service", &service).is_ok());
    }

    #[test]
    fn test_validate_zero_ttl() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        let service = ServiceConfig::l1_only().with_ttl(0);
        assert!(config.validate_service("test", &service).is_err());
    }

    #[test]
    fn test_validate_l1_capacity() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        let mut l1 = L1Config::default();
        l1.max_capacity = 0;
        let service = ServiceConfig::l1_only().with_l1(l1);

        assert!(config.validate_service("test", &service).is_err());
    }
}
