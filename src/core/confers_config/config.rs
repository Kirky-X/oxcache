// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration structures for the cache library.
//
// This module uses confers derive macros for zero-boilerplate configuration
// management with built-in validation using garde.

use garde::Validate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::confers_config::types::{CacheType, ConfigBackendType};

/// 配置提供者 Trait（用于依赖注入）
pub trait ConfigProvider: Send + Sync {
    fn get_string(&self, key: &str) -> Option<String>;
    fn get_int(&self, key: &str) -> Option<i64>;
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn get_json(&self, key: &str) -> Option<serde_json::Value>;
}

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GlobalConfig {
    #[garde(range(max = 31_536_000))]
    pub default_ttl: u64,
    #[garde(range(max = 31_536_000))]
    pub default_tti: u64,
    #[garde(range(min = 1, max = 3600))]
    pub health_check_interval: u32,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_ttl: 0,
            default_tti: 0,
            health_check_interval: 30,
        }
    }
}

/// 后端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub backend_type: String,
    pub l1_type: String,
    pub l1_options_json: String,
    pub l2_type: String,
    pub l2_options_json: String,
    pub l1_enabled: bool,
    pub l2_enabled: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: "Memory".to_string(),
            l1_type: "moka".to_string(),
            l1_options_json: String::new(),
            l2_type: "redis".to_string(),
            l2_options_json: String::new(),
            l1_enabled: true,
            l2_enabled: false,
        }
    }
}

impl BackendConfig {
    pub fn backend_type_enum(&self) -> ConfigBackendType {
        self.backend_type.parse().unwrap_or(ConfigBackendType::Memory)
    }
    pub fn l1_options(&self) -> serde_json::Value {
        if self.l1_options_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&self.l1_options_json).unwrap_or(serde_json::Value::Null)
        }
    }
    pub fn l2_options(&self) -> serde_json::Value {
        if self.l2_options_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&self.l2_options_json).unwrap_or(serde_json::Value::Null)
        }
    }
}

/// 验证容量值
fn validate_capacity_opt(value: &Option<u64>, _ctx: &()) -> garde::Result {
    if let Some(cap) = value {
        if *cap == 0 {
            return Err(garde::Error::new("容量不能为零"));
        }
        if *cap > 100_000_000 {
            return Err(garde::Error::new("容量超过最大值 100,000,000"));
        }
    }
    Ok(())
}

/// 服务特定配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ServiceConfig {
    #[garde(skip)]
    pub cache_type: String,
    #[garde(range(max = 31_536_000))]
    pub ttl: Option<u64>,
    #[garde(custom(validate_capacity_opt))]
    pub max_capacity: Option<u64>,
    #[garde(skip)]
    pub enable_metrics: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            cache_type: "L1".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }
}

impl ServiceConfig {
    pub fn cache_type_enum(&self) -> CacheType {
        self.cache_type.parse().unwrap_or(CacheType::L1)
    }
    pub fn l1_only() -> Self {
        Self {
            cache_type: "L1".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }
    pub fn l2_only() -> Self {
        Self {
            cache_type: "L2".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }
    pub fn two_level() -> Self {
        Self {
            cache_type: "TwoLevel".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// 性能配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PerformanceConfig {
    #[garde(range(min = 1, max = 100_000))]
    pub max_concurrent_operations: u32,
    #[garde(range(min = 1, max = 300_000))]
    pub command_timeout: u32,
    #[garde(skip)]
    pub enable_prefetching: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_operations: 1000,
            command_timeout: 5000,
            enable_prefetching: false,
        }
    }
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SecurityConfig {
    #[garde(skip)]
    pub connection_string_redaction: bool,
    #[garde(range(max = 1_000_000))]
    pub enable_rate_limiting: u32,
    #[garde(range(min = 1, max = 1_000_000))]
    pub rate_limit_max_requests: u32,
    #[garde(range(min = 1, max = 3600))]
    pub rate_limit_window_size: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            connection_string_redaction: true,
            enable_rate_limiting: 0,
            rate_limit_max_requests: 1000,
            rate_limit_window_size: 60,
        }
    }
}

/// 指标配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MetricsConfig {
    #[garde(skip)]
    pub enabled: bool,
    #[garde(skip)]
    pub detailed: bool,
    #[garde(skip)]
    pub export_format: String,
    #[garde(skip)]
    pub export_endpoint: Option<String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            detailed: false,
            export_format: "prometheus".to_string(),
            export_endpoint: None,
        }
    }
}

/// 恢复配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub enable_wal: bool,
    pub wal_directory: String,
    pub enable_auto_recovery: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            enable_wal: false,
            wal_directory: "./wal".to_string(),
            enable_auto_recovery: true,
        }
    }
}

/// 统一配置
#[derive(Debug, Clone, Default, Serialize, Deserialize, Validate)]
pub struct UnifiedConfig {
    #[garde(skip)]
    pub global: GlobalConfig,
    #[garde(skip)]
    pub backend: BackendConfig,
    #[garde(skip)]
    pub services_json: String,
    #[garde(skip)]
    pub performance: PerformanceConfig,
    #[garde(skip)]
    pub metrics: MetricsConfig,
    #[garde(skip)]
    pub recovery: RecoveryConfig,
}

impl UnifiedConfig {
    pub fn services(&self) -> HashMap<String, ServiceConfig> {
        if self.services_json.is_empty() {
            return HashMap::new();
        }
        serde_json::from_str(&self.services_json).unwrap_or_default()
    }

    pub fn validate_config(&self) -> crate::error::Result<()> {
        self.global
            .validate()
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("全局配置验证失败: {}", e)))?;
        self.performance
            .validate()
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("性能配置验证失败: {}", e)))?;
        for (name, service) in self.services() {
            service
                .validate()
                .map_err(|e| crate::error::CacheError::InvalidInput(format!("服务 '{}' 配置验证失败: {}", name, e)))?;
        }
        Ok(())
    }

    pub fn from_toml_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        toml::from_str(&content).map_err(|e| anyhow::anyhow!("TOML 解析失败: {}", e))
    }

    pub fn from_json_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        serde_json::from_str(&content).map_err(|e| anyhow::anyhow!("JSON 解析失败: {}", e))
    }

    pub fn from_file_auto(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        use crate::core::confers_config::types::ConfigFormat;
        let path_str = path.as_ref().to_string_lossy();
        match ConfigFormat::from_path(&path_str) {
            Some(ConfigFormat::Toml) => Self::from_toml_file(path),
            Some(ConfigFormat::Json) => Self::from_json_file(path),
            None => Err(anyhow::anyhow!("不支持的配置文件格式: {}", path_str)),
        }
    }
}

impl ConfigProvider for UnifiedConfig {
    fn get_string(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();
        let section = parts.first()?;
        let field = parts.get(1)?;

        match *section {
            "global" => match *field {
                "default_ttl" => Some(self.global.default_ttl.to_string()),
                "default_tti" => Some(self.global.default_tti.to_string()),
                "health_check_interval" => Some(self.global.health_check_interval.to_string()),
                _ => None,
            },
            "backend" => match *field {
                "backend_type" => Some(self.backend.backend_type.clone()),
                "l1_type" => Some(self.backend.l1_type.clone()),
                "l1_options_json" => Some(self.backend.l1_options_json.clone()),
                "l2_type" => Some(self.backend.l2_type.clone()),
                "l2_options_json" => Some(self.backend.l2_options_json.clone()),
                "l1_enabled" => Some(self.backend.l1_enabled.to_string()),
                "l2_enabled" => Some(self.backend.l2_enabled.to_string()),
                "l1_options" => Some(self.backend.l1_options().to_string()),
                "l2_options" => Some(self.backend.l2_options().to_string()),
                _ => None,
            },
            "performance" => match *field {
                "max_concurrent_operations" => Some(self.performance.max_concurrent_operations.to_string()),
                "command_timeout" => Some(self.performance.command_timeout.to_string()),
                "enable_prefetching" => Some(self.performance.enable_prefetching.to_string()),
                _ => None,
            },
            "metrics" => match *field {
                "enabled" => Some(self.metrics.enabled.to_string()),
                "detailed" => Some(self.metrics.detailed.to_string()),
                "export_format" => Some(self.metrics.export_format.clone()),
                "export_endpoint" => self.metrics.export_endpoint.clone(),
                _ => None,
            },
            "recovery" => match *field {
                "enable_wal" => Some(self.recovery.enable_wal.to_string()),
                "wal_directory" => Some(self.recovery.wal_directory.clone()),
                "enable_auto_recovery" => Some(self.recovery.enable_auto_recovery.to_string()),
                _ => None,
            },
            "services" => {
                // Support 2-part key: services.json or services.{service_name}
                if parts.len() == 2 {
                    match *field {
                        "json" => Some(self.services_json.clone()),
                        name => {
                            let services = self.services();
                            services.get(name).map(|s| s.cache_type.clone())
                        }
                    }
                } else if parts.len() == 3 {
                    // Support 3-part key: services.{service_name}.{field}
                    let service_name = *field;
                    let sub_field = parts.get(2)?;
                    let services = self.services();
                    let service = services.get(service_name)?;
                    match *sub_field {
                        "cache_type" => Some(service.cache_type.clone()),
                        "ttl" => service.ttl.map(|v| v.to_string()),
                        "max_capacity" => service.max_capacity.map(|v| v.to_string()),
                        "enable_metrics" => Some(service.enable_metrics.to_string()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.get_string(key).and_then(|s| s.parse().ok())
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_string(key).and_then(|s| s.parse().ok())
    }

    fn get_json(&self, key: &str) -> Option<serde_json::Value> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() < 2 {
            return None;
        }
        let section = parts[0];
        let field = parts[1];
        if section == "backend" && field == "l1_options" {
            return Some(self.backend.l1_options());
        }
        if section == "backend" && field == "l2_options" {
            return Some(self.backend.l2_options());
        }
        if section == "services" && field == "all" {
            return Some(serde_json::from_str(&self.services_json).unwrap_or(serde_json::Value::Null));
        }
        if section == "services" && parts.len() == 3 {
            let service_name = field;
            let sub_field = parts[2];
            let services = self.services();
            if let Some(service) = services.get(service_name) {
                match sub_field {
                    "cache_type" => return Some(serde_json::Value::String(service.cache_type.clone())),
                    "ttl" => return service.ttl.map(|v| serde_json::Value::Number(v.into())),
                    "max_capacity" => return service.max_capacity.map(|v| serde_json::Value::Number(v.into())),
                    "enable_metrics" => return Some(serde_json::Value::Bool(service.enable_metrics)),
                    _ => {}
                }
            }
        }
        self.get_string(key).map(serde_json::Value::String)
    }
}
