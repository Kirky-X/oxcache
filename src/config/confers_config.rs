// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration structures for the cache library.
//
// Note: The confers library has known issues with its Config derive macro
// that prevent proper usage. This module provides compatible structures
// using standard serde derive for now.

use serde::{Deserialize, Serialize};

/// Backend type enumeration for cache backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// Memory-only backend (L1)
    Memory,
    /// Redis-only backend (L2)
    Redis,
    /// Tiered backend (L1 + L2)
    Tiered,
}

impl Default for BackendType {
    #[inline]
    fn default() -> Self {
        BackendType::Memory
    }
}

/// Cache type enumeration for service configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheType {
    /// L1 (memory) cache only
    L1,
    /// L2 (Redis) cache only
    L2,
    /// Two-level cache (L1 + L2)
    TwoLevel,
}

impl Default for CacheType {
    #[inline]
    fn default() -> Self {
        CacheType::L1
    }
}

/// Global configuration settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub default_ttl: u64,
    pub default_tti: u64,
    pub health_check_interval: u32,
}

/// Backend configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Backend type for the cache
    #[serde(default)]
    pub backend_type: BackendType,
    /// L1 cache type
    #[serde(default)]
    pub l1_type: String,
    /// L1 cache options
    #[serde(default)]
    pub l1_options: serde_json::Value,
    /// L2 cache type
    #[serde(default)]
    pub l2_type: String,
    /// L2 cache options
    #[serde(default)]
    pub l2_options: serde_json::Value,
    /// Whether L1 is enabled
    #[serde(default)]
    pub l1_enabled: bool,
    /// Whether L2 is enabled
    #[serde(default)]
    pub l2_enabled: bool,
}

/// Service-specific configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub cache_type: CacheType,
    pub ttl: Option<u64>,
    pub max_capacity: Option<u64>,
    pub enable_metrics: bool,
}

impl ServiceConfig {
    /// Create an L1-only service configuration
    #[inline]
    pub fn l1_only() -> Self {
        Self {
            cache_type: CacheType::L1,
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }

    /// Create an L2-only service configuration
    #[inline]
    pub fn l2_only() -> Self {
        Self {
            cache_type: CacheType::L2,
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }

    /// Create a two-level service configuration
    #[inline]
    pub fn two_level() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }

    /// Set the TTL for this service configuration
    #[inline]
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// Performance settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_operations: usize,
    pub command_timeout: u64,
    pub enable_prefetching: bool,
}

/// Security settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub connection_string_redaction: bool,
    pub enable_rate_limiting: u64,
    pub rate_limit_max_requests: u64,
    pub rate_limit_window_size: u64,
}

/// Metrics settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub detailed: bool,
    pub export_format: String,
    pub export_endpoint: Option<String>,
}

/// Recovery settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub enable_wal: bool,
    pub wal_directory: String,
    pub enable_auto_recovery: bool,
}

/// Unified configuration combining all sections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnifiedConfig {
    pub global: GlobalConfig,
    pub backend: BackendConfig,
    pub services: std::collections::HashMap<String, ServiceConfig>,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub recovery: RecoveryConfig,
}

/// Builder for creating UnifiedConfig instances with a fluent API
///
/// This builder provides type-safe configuration for oxcache services.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::config::UnifiedConfigBuilder;
///
/// let config = UnifiedConfigBuilder::memory_only()
///     .with_ttl(3600)
///     .with_capacity(10000)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct UnifiedConfigBuilder(UnifiedConfig);

impl UnifiedConfigBuilder {
    /// Create a new empty builder
    #[inline]
    pub fn new() -> Self {
        Self(UnifiedConfig::default())
    }

    /// Create a memory-only (L1) cache configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = UnifiedConfigBuilder::memory_only()
    ///     .with_capacity(10000)
    ///     .build();
    /// ```
    #[inline]
    pub fn memory_only() -> Self {
        let mut config = UnifiedConfig::default();
        config.backend.backend_type = BackendType::Memory;
        config.backend.l1_enabled = true;
        config.backend.l2_enabled = false;
        config.backend.l1_type = "moka".to_string();
        Self(config)
    }

    /// Create a Redis-only (L2) cache configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = UnifiedConfigBuilder::redis_only()
    ///     .with_redis_url("redis://localhost:6379")
    ///     .build();
    /// ```
    #[inline]
    pub fn redis_only() -> Self {
        let mut config = UnifiedConfig::default();
        config.backend.backend_type = BackendType::Redis;
        config.backend.l1_enabled = false;
        config.backend.l2_enabled = true;
        config.backend.l2_type = "redis".to_string();
        Self(config)
    }

    /// Create a tiered (L1 + L2) cache configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = UnifiedConfigBuilder::tiered()
    ///     .with_l1_capacity(10000)
    ///     .with_redis_url("redis://localhost:6379")
    ///     .build();
    /// ```
    #[inline]
    pub fn tiered() -> Self {
        let mut config = UnifiedConfig::default();
        config.backend.backend_type = BackendType::Tiered;
        config.backend.l1_enabled = true;
        config.backend.l2_enabled = true;
        config.backend.l1_type = "moka".to_string();
        config.backend.l2_type = "redis".to_string();
        Self(config)
    }

    /// Set the default TTL for cache entries
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.0.global.default_ttl = ttl;
        self
    }

    /// Set the default TTI (Time-to-Inactive) for cache entries
    ///
    /// # Arguments
    ///
    /// * `tti` - Time-to-inactive in seconds
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_tti(mut self, tti: u64) -> Self {
        self.0.global.default_tti = tti;
        self
    }

    /// Set the health check interval
    ///
    /// # Arguments
    ///
    /// * `interval` - Health check interval in seconds
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_health_check_interval(mut self, interval: u32) -> Self {
        self.0.global.health_check_interval = interval;
        self
    }

    /// Set the L1 (memory) cache capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries in L1 cache
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_l1_capacity(mut self, capacity: u64) -> Self {
        self.0.backend.l1_options["max_capacity"] = serde_json::json!(capacity);
        self
    }

    /// Set the Redis connection URL
    ///
    /// # Arguments
    ///
    /// * `url` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_redis_url(mut self, url: &str) -> Self {
        self.0.backend.l2_options["connection_string"] = serde_json::json!(url);
        self
    }

    /// Set the Redis mode
    ///
    /// # Arguments
    ///
    /// * `mode` - Redis mode ("standalone", "sentinel", or "cluster")
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_redis_mode(mut self, mode: &str) -> Self {
        self.0.backend.l2_options["mode"] = serde_json::json!(mode);
        self
    }

    /// Set the maximum number of concurrent operations
    ///
    /// # Arguments
    ///
    /// * `max_ops` - Maximum concurrent operations
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_max_concurrent_operations(mut self, max_ops: usize) -> Self {
        self.0.performance.max_concurrent_operations = max_ops;
        self
    }

    /// Set the command timeout
    ///
    /// # Arguments
    ///
    /// * `timeout` - Command timeout in milliseconds
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_command_timeout(mut self, timeout: u64) -> Self {
        self.0.performance.command_timeout = timeout;
        self
    }

    /// Enable or disable metrics
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether metrics are enabled
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.0.metrics.enabled = enabled;
        self
    }

    /// Enable or disable WAL (Write-Ahead Log)
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether WAL is enabled
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_wal(mut self, enabled: bool) -> Self {
        self.0.recovery.enable_wal = enabled;
        self
    }

    /// Set the WAL directory
    ///
    /// # Arguments
    ///
    /// * `directory` - WAL directory path
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_wal_directory(mut self, directory: &str) -> Self {
        self.0.recovery.wal_directory = directory.to_string();
        self
    }

    /// Enable or disable auto-recovery
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether auto-recovery is enabled
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_auto_recovery(mut self, enabled: bool) -> Self {
        self.0.recovery.enable_auto_recovery = enabled;
        self
    }

    /// Add a service configuration
    ///
    /// # Arguments
    ///
    /// * `name` - Service name
    /// * `cache_type` - Cache type (CacheType::L1, CacheType::L2, or CacheType::TwoLevel)
    /// * `ttl` - Cache TTL in seconds
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[inline]
    pub fn with_service(mut self, name: &str, cache_type: CacheType, ttl: u64) -> Self {
        let service = ServiceConfig {
            cache_type,
            ttl: Some(ttl),
            max_capacity: None,
            enable_metrics: true,
        };
        self.0.services.insert(name.to_string(), service);
        self
    }

    /// Build the UnifiedConfig
    ///
    /// # Returns
    ///
    /// The configured UnifiedConfig instance
    #[inline]
    pub fn build(self) -> UnifiedConfig {
        self.0
    }

    /// Build the UnifiedConfig as a JSON Value
    ///
    /// # Returns
    ///
    /// The configured UnifiedConfig as a serde_json::Value
    #[inline]
    pub fn build_json(self) -> serde_json::Value {
        serde_json::to_value(self.0).expect("UnifiedConfig should be serializable")
    }
}

impl Default for UnifiedConfigBuilder {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
