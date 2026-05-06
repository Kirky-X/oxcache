// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration builder

use std::collections::HashMap;

use crate::core::confers_config::config::{ServiceConfig, UnifiedConfig};
use crate::core::confers_config::types::CacheType;

/// 统一配置构建器
pub struct UnifiedConfigBuilder {
    builder: confers::ConfigBuilder<UnifiedConfig>,
    services: HashMap<String, ServiceConfig>,
}

impl Default for UnifiedConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedConfigBuilder {
    pub fn new() -> Self {
        let builder = confers::ConfigBuilder::new()
            .default("global.default_ttl".to_string(), confers::ConfigValue::uint(0))
            .default("global.default_tti".to_string(), confers::ConfigValue::uint(0))
            .default(
                "global.health_check_interval".to_string(),
                confers::ConfigValue::uint(30),
            )
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Memory"),
            )
            .default("backend.l1_type".to_string(), confers::ConfigValue::string("moka"))
            .default("backend.l1_options_json".to_string(), confers::ConfigValue::string(""))
            .default("backend.l2_type".to_string(), confers::ConfigValue::string("redis"))
            .default("backend.l2_options_json".to_string(), confers::ConfigValue::string(""))
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(false))
            .default("services_json".to_string(), confers::ConfigValue::string(""))
            .default(
                "performance.max_concurrent_operations".to_string(),
                confers::ConfigValue::uint(1000),
            )
            .default(
                "performance.command_timeout".to_string(),
                confers::ConfigValue::uint(5000),
            )
            .default(
                "performance.enable_prefetching".to_string(),
                confers::ConfigValue::bool(false),
            )
            .default("metrics.enabled".to_string(), confers::ConfigValue::bool(false))
            .default("metrics.detailed".to_string(), confers::ConfigValue::bool(false))
            .default(
                "metrics.export_format".to_string(),
                confers::ConfigValue::string("prometheus"),
            )
            .default("recovery.enable_wal".to_string(), confers::ConfigValue::bool(false))
            .default(
                "recovery.wal_directory".to_string(),
                confers::ConfigValue::string("./wal"),
            )
            .default(
                "recovery.enable_auto_recovery".to_string(),
                confers::ConfigValue::bool(true),
            );
        Self {
            builder,
            services: HashMap::new(),
        }
    }

    pub fn memory_only() -> Self {
        let mut builder = Self::new();
        builder.builder = builder
            .builder
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Memory"),
            )
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(false))
            .default("backend.l1_type".to_string(), confers::ConfigValue::string("moka"));
        builder
    }

    pub fn redis_only() -> Self {
        let mut builder = Self::new();
        builder.builder = builder
            .builder
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Redis"),
            )
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(false))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_type".to_string(), confers::ConfigValue::string("redis"));
        builder
    }

    pub fn tiered() -> Self {
        let mut builder = Self::new();
        builder.builder = builder
            .builder
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Tiered"),
            )
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(true));
        builder
    }

    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.builder = self
            .builder
            .default("global.default_ttl".to_string(), confers::ConfigValue::uint(ttl));
        self
    }

    pub fn with_tti(mut self, tti: u64) -> Self {
        self.builder = self
            .builder
            .default("global.default_tti".to_string(), confers::ConfigValue::uint(tti));
        self
    }

    pub fn with_health_check_interval(mut self, interval: u32) -> Self {
        self.builder = self.builder.default(
            "global.health_check_interval".to_string(),
            confers::ConfigValue::uint(interval as u64),
        );
        self
    }

    pub fn with_l1_capacity(mut self, capacity: u64) -> Self {
        let options = serde_json::json!({"max_capacity": capacity}).to_string();
        self.builder = self.builder.default(
            "backend.l1_options_json".to_string(),
            confers::ConfigValue::string(&options),
        );
        self
    }

    pub fn with_redis_url(mut self, url: &str) -> Self {
        let options = serde_json::json!({"connection_string": url}).to_string();
        self.builder = self.builder.default(
            "backend.l2_options_json".to_string(),
            confers::ConfigValue::string(&options),
        );
        self
    }

    pub fn with_redis_mode(mut self, mode: &str) -> Self {
        let options = serde_json::json!({"mode": mode}).to_string();
        self.builder = self.builder.default(
            "backend.l2_options_json".to_string(),
            confers::ConfigValue::string(&options),
        );
        self
    }

    pub fn with_max_concurrent_operations(mut self, max_ops: usize) -> Self {
        self.builder = self.builder.default(
            "performance.max_concurrent_operations".to_string(),
            confers::ConfigValue::uint(max_ops as u64),
        );
        self
    }

    pub fn with_command_timeout(mut self, timeout: u32) -> Self {
        self.builder = self.builder.default(
            "performance.command_timeout".to_string(),
            confers::ConfigValue::uint(timeout as u64),
        );
        self
    }

    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.builder = self
            .builder
            .default("metrics.enabled".to_string(), confers::ConfigValue::bool(enabled));
        self
    }

    pub fn with_wal(mut self, enabled: bool) -> Self {
        self.builder = self
            .builder
            .default("recovery.enable_wal".to_string(), confers::ConfigValue::bool(enabled));
        self
    }

    pub fn with_wal_directory(mut self, dir: &str) -> Self {
        self.builder = self
            .builder
            .default("recovery.wal_directory".to_string(), confers::ConfigValue::string(dir));
        self
    }

    pub fn with_auto_recovery(mut self, enabled: bool) -> Self {
        self.builder = self.builder.default(
            "recovery.enable_auto_recovery".to_string(),
            confers::ConfigValue::bool(enabled),
        );
        self
    }

    pub fn with_service(mut self, name: &str, cache_type: CacheType, ttl: u64) -> Self {
        let service = ServiceConfig {
            cache_type: cache_type.to_string(),
            ttl: if ttl == 0 { None } else { Some(ttl) },
            max_capacity: None,
            enable_metrics: true,
        };
        self.services.insert(name.to_string(), service);
        self
    }

    pub fn with_dependencies(config: UnifiedConfig) -> Self {
        let services_json = if config.services_json.is_empty() {
            String::new()
        } else {
            config.services_json.clone()
        };
        let builder = confers::ConfigBuilder::new()
            .default(
                "global.default_ttl".to_string(),
                confers::ConfigValue::uint(config.global.default_ttl),
            )
            .default(
                "global.default_tti".to_string(),
                confers::ConfigValue::uint(config.global.default_tti),
            )
            .default(
                "global.health_check_interval".to_string(),
                confers::ConfigValue::uint(config.global.health_check_interval as u64),
            )
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string(&config.backend.backend_type),
            )
            .default(
                "backend.l1_type".to_string(),
                confers::ConfigValue::string(&config.backend.l1_type),
            )
            .default(
                "backend.l1_options_json".to_string(),
                confers::ConfigValue::string(&config.backend.l1_options_json),
            )
            .default(
                "backend.l2_type".to_string(),
                confers::ConfigValue::string(&config.backend.l2_type),
            )
            .default(
                "backend.l2_options_json".to_string(),
                confers::ConfigValue::string(&config.backend.l2_options_json),
            )
            .default(
                "backend.l1_enabled".to_string(),
                confers::ConfigValue::bool(config.backend.l1_enabled),
            )
            .default(
                "backend.l2_enabled".to_string(),
                confers::ConfigValue::bool(config.backend.l2_enabled),
            )
            .default(
                "services_json".to_string(),
                confers::ConfigValue::string(&services_json),
            )
            .default(
                "performance.max_concurrent_operations".to_string(),
                confers::ConfigValue::uint(config.performance.max_concurrent_operations as u64),
            )
            .default(
                "performance.command_timeout".to_string(),
                confers::ConfigValue::uint(config.performance.command_timeout as u64),
            )
            .default(
                "performance.enable_prefetching".to_string(),
                confers::ConfigValue::bool(config.performance.enable_prefetching),
            )
            .default(
                "metrics.enabled".to_string(),
                confers::ConfigValue::bool(config.metrics.enabled),
            )
            .default(
                "metrics.detailed".to_string(),
                confers::ConfigValue::bool(config.metrics.detailed),
            )
            .default(
                "metrics.export_format".to_string(),
                confers::ConfigValue::string(&config.metrics.export_format),
            )
            .default(
                "metrics.export_endpoint".to_string(),
                confers::ConfigValue::string(config.metrics.export_endpoint.as_deref().unwrap_or("")),
            )
            .default(
                "recovery.enable_wal".to_string(),
                confers::ConfigValue::bool(config.recovery.enable_wal),
            )
            .default(
                "recovery.wal_directory".to_string(),
                confers::ConfigValue::string(&config.recovery.wal_directory),
            )
            .default(
                "recovery.enable_auto_recovery".to_string(),
                confers::ConfigValue::bool(config.recovery.enable_auto_recovery),
            );
        Self {
            builder,
            services: config.services(),
        }
    }

    pub fn build(self) -> confers::ConfigResult<UnifiedConfig> {
        let mut config = self.builder.build()?;
        if !self.services.is_empty() {
            config.services_json = serde_json::to_string(&self.services).unwrap_or_default();
        }
        Ok(config)
    }

    pub fn build_json(self) -> serde_json::Value {
        self.build()
            .map(|c| serde_json::to_value(c).unwrap_or_default())
            .unwrap_or_default()
    }
}
