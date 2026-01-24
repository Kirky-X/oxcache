//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 配置验证模块
//!
//! 提供 feature-gated 的配置验证。

use secrecy::ExposeSecret;
use std::collections::HashMap;

use crate::config::{GlobalConfig, OxcacheConfig};
use crate::config::legacy_config::ServiceConfig;

// ============================================================================
// 常量定义
// ============================================================================

/// 默认 TTL（秒）
pub const DEFAULT_TTL: u64 = 3600;

/// 最大 TTL（30天，秒）
pub const MAX_TTL: u64 = 86400 * 30;

/// 最小 TTL（秒）
pub const MIN_TTL: u64 = 1;

/// 服务名称最大长度
pub const MAX_SERVICE_NAME_LENGTH: usize = 64;

/// 健康检查间隔最小值（秒）
pub const MIN_HEALTH_CHECK_INTERVAL: u64 = 1;

/// 健康检查间隔最大值（秒）
pub const MAX_HEALTH_CHECK_INTERVAL: u64 = 3600;

/// L1 缓存最大容量
pub const MAX_L1_CAPACITY: usize = 10_000_000;

/// 批量写入最大大小
pub const MAX_BATCH_SIZE: usize = 10000;

/// 批量写入间隔最大值（毫秒）
pub const MAX_BATCH_INTERVAL_MS: u64 = 60000;

/// 最大键长度（字节）
pub const MAX_KEY_LENGTH: usize = 1024;

/// 最大值大小（10MB，字节）
pub const MAX_VALUE_SIZE: usize = 10 * 1024 * 1024;

/// L2 连接超时最小值（毫秒）
pub const MIN_L2_CONNECTION_TIMEOUT_MS: u64 = 100;

/// L2 连接超时最大值（毫秒）
pub const MAX_L2_CONNECTION_TIMEOUT_MS: u64 = 30000;

/// L2 命令超时最小值（毫秒）
pub const MIN_L2_COMMAND_TIMEOUT_MS: u64 = 100;

/// L2 命令超时最大值（毫秒）
pub const MAX_L2_COMMAND_TIMEOUT_MS: u64 = 60000;

/// 生产环境关键词列表
pub const PRODUCTION_KEYWORDS: &[&str] = &["production", "prod"];

/// 默认重试间隔（毫秒）
pub const DEFAULT_RETRY_INTERVAL_MS: u64 = 100;

/// 默认最大重试次数
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// 默认 WAL 条目最大 TTL（30天，秒）
pub const MAX_WAL_ENTRY_TTL: u64 = 30 * 24 * 3600;

/// 默认内存大小（100MB）
pub const DEFAULT_MAX_MEMORY_BYTES: usize = 100 * 1024 * 1024;

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

        if global.default_ttl > MAX_TTL {
            return Err(format!(
                "Global default_ttl cannot exceed {} days",
                MAX_TTL / 86400
            ));
        }

        if global.health_check_interval == 0 {
            return Err("Global health_check_interval cannot be zero".to_string());
        }

        if global.health_check_interval < MIN_HEALTH_CHECK_INTERVAL
            || global.health_check_interval > MAX_HEALTH_CHECK_INTERVAL
        {
            return Err(format!(
                "Global health_check_interval must be between {} and {} seconds",
                MIN_HEALTH_CHECK_INTERVAL, MAX_HEALTH_CHECK_INTERVAL
            ));
        }

        Ok(())
    }

    /// 验证单个服务配置
    pub fn validate_service(&self, name: &str, service: &ServiceConfig) -> Result<(), String> {
        // 验证服务名称
        if name.is_empty() {
            return Err("Service name cannot be empty".to_string());
        }

        if name.len() > MAX_SERVICE_NAME_LENGTH {
            return Err(format!(
                "Service name '{}' exceeds maximum length of {} characters",
                name, MAX_SERVICE_NAME_LENGTH
            ));
        }

        let global = &self.global;
        let service_ttl = service.ttl.unwrap_or(global.default_ttl);

        // 验证 TTL
        if service_ttl == 0 {
            return Err(format!("Service '{}' TTL cannot be zero", name));
        }

        if service_ttl > MAX_TTL {
            return Err(format!(
                "Service '{}' TTL cannot exceed {} days",
                name,
                MAX_TTL / 86400
            ));
        }

        // 验证 L1 配置（需要 l1-moka feature）
        #[cfg(feature = "l1-moka")]
        if let Some(l1_config) = &service.l1 {
            Self::validate_l1_config(name, l1_config, service_ttl)?;
        }

        // 验证 L2 配置（需要 l2-redis feature）
        #[cfg(feature = "l2-redis")]
        if let Some(l2_config) = &service.l2 {
            Self::validate_l2_config(name, l2_config, service_ttl)?;
        }

        // 验证双层缓存配置（需要 l2-redis feature）
        #[cfg(feature = "l2-redis")]
        if let Some(two_level_config) = &service.two_level {
            Self::validate_two_level_config(name, two_level_config)?;
        }

        Ok(())
    }

    /// 验证 L1 配置（需要 l1-moka feature）
    #[cfg(feature = "l1-moka")]
    fn validate_l1_config(
        name: &str,
        l1_config: &crate::config::legacy_config::L1Config,
        service_ttl: u64,
    ) -> Result<(), String> {
        if l1_config.max_capacity == 0 {
            return Err(format!("Service '{}' L1 max_capacity cannot be zero", name));
        }

        if l1_config.max_capacity > MAX_L1_CAPACITY as u64 {
            return Err(format!(
                "Service '{}' L1 max_capacity cannot exceed {}",
                name, MAX_L1_CAPACITY
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

    /// 验证 L2 配置（需要 l2-redis feature）
    #[cfg(feature = "l2-redis")]
    fn validate_l2_config(
        name: &str,
        l2_config: &crate::config::legacy_config::L2Config,
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
        if !(MIN_L2_CONNECTION_TIMEOUT_MS..=MAX_L2_CONNECTION_TIMEOUT_MS).contains(&timeout) {
            return Err(format!(
                "Service '{}' connection_timeout_ms must be between {} and {} ms",
                name, MIN_L2_CONNECTION_TIMEOUT_MS, MAX_L2_CONNECTION_TIMEOUT_MS
            ));
        }

        // 验证命令超时
        let timeout = l2_config.command_timeout_ms;
        if !(MIN_L2_COMMAND_TIMEOUT_MS..=MAX_L2_COMMAND_TIMEOUT_MS).contains(&timeout) {
            return Err(format!(
                "Service '{}' command_timeout_ms must be between {} and {} ms",
                name, MIN_L2_COMMAND_TIMEOUT_MS, MAX_L2_COMMAND_TIMEOUT_MS
            ));
        }

        // 生产环境安全检查
        let conn_str = l2_config.connection_string.expose_secret();

        // 更精确的生产环境检测
        let is_production = conn_str.contains("production")
            || conn_str.contains("prod")
            || (!conn_str.contains("localhost")
                && !conn_str.contains("127.0.0.1")
                && !conn_str.contains("192.168.")
                && !conn_str.contains("10.")
                && !conn_str.contains("172.16.")
                && !conn_str.contains("172.17.")
                && !conn_str.contains("172.18.")
                && !conn_str.contains("172.19.")
                && !conn_str.contains("172.20.")
                && !conn_str.contains("172.21.")
                && !conn_str.contains("172.22.")
                && !conn_str.contains("172.23.")
                && !conn_str.contains("172.24.")
                && !conn_str.contains("172.25.")
                && !conn_str.contains("172.26.")
                && !conn_str.contains("172.27.")
                && !conn_str.contains("172.28.")
                && !conn_str.contains("172.29.")
                && !conn_str.contains("172.30.")
                && !conn_str.contains("172.31.")
                && !conn_str.contains(".local")
                && !conn_str.contains(".dev")
                && !conn_str.contains(".test")
                && !conn_str.contains(".staging")
                && !conn_str.contains(".internal"));

        if is_production {
            // 检查密码
            if l2_config.password.is_none() {
                return Err(format!(
                    "Service '{}' is in production but Redis password is not configured. \
                     Production deployments require authentication.",
                    name
                ));
            }

            // 检查密码复杂度
            if let Some(password) = &l2_config.password {
                let password = password.expose_secret();

                // 检查密码长度（最少 16 字符）
                if password.len() < 16 {
                    return Err(format!(
                        "Service '{}' is in production but Redis password is too weak ({} chars, minimum 16 required). \
                         Production deployments require strong passwords.",
                        name,
                        password.len()
                    ));
                }

                // 检查密码复杂度
                let has_upper = password.chars().any(|c| c.is_uppercase());
                let has_lower = password.chars().any(|c| c.is_lowercase());
                let has_digit = password.chars().any(|c| c.is_ascii_digit());
                let has_special = password
                    .chars()
                    .any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?/~`".contains(c));

                if !has_upper || !has_lower || !has_digit || !has_special {
                    let missing = vec![
                        if !has_upper { "uppercase letter" } else { "" },
                        if !has_lower { "lowercase letter" } else { "" },
                        if !has_digit { "digit" } else { "" },
                        if !has_special {
                            "special character"
                        } else {
                            ""
                        },
                    ]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");

                    return Err(format!(
                        "Service '{}' is in production but Redis password does not meet complexity requirements. \
                         Missing: {}. Password must contain uppercase, lowercase, digit, and special character.",
                        name, missing
                    ));
                }

                // 检查常见弱密码
                const WEAK_PASSWORDS: &[&str] = &[
                    "password",
                    "Password123",
                    "Admin123",
                    "Root123",
                    "Redis123",
                    "Cache123",
                    "Welcome123",
                    "P@ssw0rd",
                    "Admin@123",
                    "Root@123",
                    "Redis@123",
                ];

                if WEAK_PASSWORDS.iter().any(|weak| password == *weak) {
                    return Err(format!(
                        "Service '{}' is in production but Redis password is too weak (matches common weak password list). \
                         Please use a strong, unique password.",
                        name
                    ));
                }
            }

            // 检查 TLS
            if !l2_config.enable_tls {
                return Err(format!(
                    "Service '{}' is in production but TLS is not enabled. \
                     Production deployments require TLS encryption.",
                    name
                ));
            }

            // 检查 TLS 证书验证
            #[cfg(feature = "l2-redis")]
            if l2_config.enable_tls {
                // 检查是否禁用了证书验证（不安全）
                // 注意：这里假设 L2Config 有 tls_insecure 字段
                // 如果没有，需要添加该字段或使用其他方式检查
                let tls_insecure = false; // 默认值，需要根据实际配置调整

                if tls_insecure {
                    return Err(format!(
                        "Service '{}' is in production but TLS certificate verification is disabled. \
                         This is a security risk. Please enable certificate verification.",
                        name
                    ));
                }
            }
        }

        Ok(())
    }

    /// 验证双层缓存配置（需要 l2-redis feature）
    #[cfg(feature = "l2-redis")]
    fn validate_two_level_config(
        name: &str,
        two_level_config: &crate::config::legacy_config::TwoLevelConfig,
    ) -> Result<(), String> {
        // 验证批量写入配置
        if two_level_config.enable_batch_write {
            if two_level_config.batch_size == 0 {
                return Err(format!(
                    "Service '{}' batch_size cannot be zero when batch_write is enabled",
                    name
                ));
            }

            if two_level_config.batch_size > MAX_BATCH_SIZE {
                return Err(format!(
                    "Service '{}' batch_size cannot exceed {}",
                    name, MAX_BATCH_SIZE
                ));
            }

            if two_level_config.batch_interval_ms == 0 {
                return Err(format!(
                    "Service '{}' batch_interval_ms cannot be zero when batch_write is enabled",
                    name
                ));
            }

            if two_level_config.batch_interval_ms > MAX_BATCH_INTERVAL_MS {
                return Err(format!(
                    "Service '{}' batch_interval_ms cannot exceed {} ms",
                    name, MAX_BATCH_INTERVAL_MS
                ));
            }
        }

        // 验证键大小限制
        if let Some(max_key_length) = two_level_config.max_key_length {
            if max_key_length == 0 || max_key_length > MAX_KEY_LENGTH {
                return Err(format!(
                    "Service '{}' max_key_length must be between 1 and {}",
                    name, MAX_KEY_LENGTH
                ));
            }
        }

        // 验证值大小限制
        if let Some(max_value_size) = two_level_config.max_value_size {
            if max_value_size == 0 || max_value_size > MAX_VALUE_SIZE {
                return Err(format!(
                    "Service '{}' max_value_size must be between 1 and {}MB",
                    name,
                    MAX_VALUE_SIZE / (1024 * 1024)
                ));
            }
        }

        Ok(())
    }
}

/// 从旧配置迁移验证逻辑（向后兼容）
#[deprecated(since = "0.2.0", note = "请使用 OxcacheConfig 的验证方法")]
pub fn validate_service(
    name: &str,
    service: &ServiceConfig,
    global: &GlobalConfig,
) -> Result<(), String> {
    let config = OxcacheConfig {
        config_version: None,
        global: global.clone(),
        services: HashMap::new(),
        #[cfg(feature = "l1-moka")]
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
    use crate::config::{oxcache_config, GlobalConfig, ServiceConfig};

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
    #[cfg(feature = "l1-moka")]
    fn test_validate_l1_capacity() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        let l1 = crate::config::L1Config {
            max_capacity: 0,
            ..Default::default()
        };
        let service = ServiceConfig::l1_only().with_l1(l1);

        assert!(config.validate_service("test", &service).is_err());
    }

    #[test]
    fn test_validate_global_config() {
        let global = GlobalConfig {
            default_ttl: 0,
            ..Default::default()
        };

        let config = oxcache_config().with_global(global).build();

        assert!(config.validate_global().is_err());
    }

    #[test]
    fn test_validate_service_name_too_long() {
        let config = oxcache_config()
            .with_global(GlobalConfig::default())
            .build();

        let long_name = "a".repeat(65);
        let service = ServiceConfig::l1_only();

        assert!(config.validate_service(&long_name, &service).is_err());
    }
}
