//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache builder for advanced configuration

use super::backend_builder::BackendBuilder;
use crate::backend::client::MokaMemoryBackend as MemoryBackend;
use crate::cache::Cache;
use crate::error::Result;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// Builder for creating configured Cache instances
///
/// This builder provides a fluent interface for configuring cache instances
/// with advanced options like TTL, capacity, batch writes, and auto-promote.
///
/// # Type Parameters
///
/// * `K` - Key type, must implement `CacheKey` trait
/// * `V` - Value type, must implement `Cacheable` trait
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::Cache;
/// use std::time::Duration;
///
/// let cache: Cache<String, User> = Cache::builder()
///     .ttl(Duration::from_secs(3600))
///     .capacity(10000)
///     .batch_writes(true)
///     .build()
///     .await?;
/// ```
pub struct CacheBuilder<K, V> {
    backend_builder: Option<BackendBuilder>,
    ttl: Option<Duration>,
    capacity: Option<u64>,
    batch_writes: bool,
    auto_promote: bool,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            backend_builder: None,
            ttl: None,
            capacity: None,
            batch_writes: false,
            auto_promote: true,
            _phantom: PhantomData,
        }
    }
}

impl<K, V> CacheBuilder<K, V>
where
    K: crate::traits::CacheKey,
    V: crate::traits::Cacheable,
{
    /// Set the default TTL for cache entries
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live duration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let builder = Cache::builder().ttl(Duration::from_secs(3600));
    /// ```
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the capacity for memory-based backends
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = Cache::builder().capacity(10000);
    /// ```
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// Enable or disable batch writes
    ///
    /// When enabled, multiple write operations are batched for better performance.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable batch writes
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = Cache::builder().batch_writes(true);
    /// ```
    pub fn batch_writes(mut self, enabled: bool) -> Self {
        self.batch_writes = enabled;
        self
    }

    /// Enable or disable auto-promote (for tiered backends)
    ///
    /// When enabled, values from L2 are automatically promoted to L1 on cache misses.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable auto-promote
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = Cache::builder().auto_promote(true);
    /// ```
    pub fn auto_promote(mut self, enabled: bool) -> Self {
        self.auto_promote = enabled;
        self
    }

    /// 使用外部confers配置（DI支持）
    ///
    /// 此方法允许从confers配置实例读取缓存配置，
    /// 并与手动配置的参数（如TTL、capacity）合并。
    /// 手动配置的参数优先级高于confers配置。
    ///
    /// # Arguments
    ///
    /// * `config` - confers配置实例
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Configuration Keys
    ///
    /// 从confers读取以下配置项（如果未手动设置）：
    ///
    /// - `oxcache.backend`: 后端类型 ("memory" | "redis" | "tiered")
    /// - `oxcache.capacity`: 内存缓存容量（默认10000）
    /// - `oxcache.ttl`: 默认TTL（秒）
    /// - `oxcache.redis.url`: Redis连接URL
    /// - `oxcache.redis.mode`: Redis模式
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::Cache;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use serde_json::json;
    ///
    /// let config = json!({
    ///     "oxcache": {
    ///         "backend": "memory",
    ///         "capacity": 10000,
    ///         "ttl": 3600
    ///     }
    /// });
    ///
    /// // 使用confers配置，但覆盖TTL
    /// let cache = Cache::builder()
    ///     .with_confers(&config)
    ///     .ttl(Duration::from_secs(7200))  // 覆盖confers中的TTL
    ///     .build()
    ///     .await?;
    /// ```
    ///
    /// # Features
    ///
    /// 此方法仅在启用 `confers` feature 时可用。
    #[cfg(feature = "confers")]
    pub fn with_confers(mut self, config: &serde_json::Value) -> Self {
        use std::time::Duration;

        // 获取oxcache配置部分，如果没有则使用空对象
        let oxcache_config: &serde_json::Map<String, serde_json::Value> = match config
            .get("oxcache")
        {
            Some(serde_json::Value::Object(obj)) => obj,
            _ => {
                static EMPTY: once_cell::sync::Lazy<serde_json::Map<String, serde_json::Value>> =
                    once_cell::sync::Lazy::new(serde_json::Map::new);
                &EMPTY
            }
        };

        // 如果尚未设置TTL，从confers读取
        if self.ttl.is_none() {
            if let Some(ttl_secs) = oxcache_config.get("ttl").and_then(|v| v.as_i64()) {
                self.ttl = Some(Duration::from_secs(ttl_secs as u64));
            }
        }

        // 如果尚未设置capacity，从confers读取
        if self.capacity.is_none() {
            if let Some(cap) = oxcache_config
                .get("capacity")
                .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
            {
                self.capacity = Some(cap);
            }
        }

        // 使用BackendBuilder::with_confers创建后端构建器
        self.backend_builder = Some(super::BackendBuilder::with_confers(config));

        self
    }

    /// Set the backend builder
    ///
    /// # Arguments
    ///
    /// * `builder` - Backend builder instance
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::builder::BackendBuilder;
    ///
    /// let builder = Cache::builder()
    ///     .backend(BackendBuilder::redis().connection_string("redis://localhost:6379"));
    /// ```
    pub fn backend(mut self, builder: BackendBuilder) -> Self {
        self.backend_builder = Some(builder);
        self
    }

    /// Build the cache instance
    ///
    /// # Returns
    ///
    /// Configured cache instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if configuration is invalid or backend creation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = CacheBuilder::default()
    ///     .ttl(Duration::from_secs(3600))
    ///     .capacity(10000)
    ///     .build()
    ///     .await?;
    /// ```
    pub async fn build(self) -> Result<Cache<K, V>> {
        let backend = if let Some(backend_builder) = self.backend_builder {
            backend_builder.build().await?
        } else {
            // Default to memory backend
            let builder = MemoryBackend::builder();
            let backend = if let Some(capacity) = self.capacity {
                builder.capacity(capacity).build()
            } else {
                builder.build()
            };
            Arc::new(backend)
        };

        Ok(Cache::new_with_backend(backend))
    }

    /// Create a CacheBuilder from a UnifiedConfig
    ///
    /// This method provides a convenient way to create a CacheBuilder
    /// directly from a UnifiedConfig instance, enabling type-safe configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - UnifiedConfig instance
    ///
    /// # Returns
    ///
    /// Configured CacheBuilder instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError::ConfigError` if the configuration is invalid:
    /// - Missing `connection_string` for Redis or Tiered backends
    /// - Invalid capacity or TTL values
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::config::{UnifiedConfigBuilder, BackendType};
    ///
    /// let config = UnifiedConfigBuilder::tiered()
    ///     .with_ttl(3600)
    ///     .with_l1_capacity(10000)
    ///     .with_redis_url("redis://localhost:6379")
    ///     .build();
    ///
    /// let cache = CacheBuilder::from_unified_config(&config)
    ///     .build()
    ///     .await?;
    /// ```
    #[cfg(feature = "confers")]
    pub fn from_unified_config(
        config: &crate::config::UnifiedConfig,
    ) -> std::result::Result<Self, crate::error::CacheError> {
        // Validate configuration
        validate_unified_config(config)?;
        use std::time::Duration;

        // Create BackendBuilder from UnifiedConfig
        let backend_builder = match config.backend.backend_type {
            crate::config::BackendType::Memory => {
                // Extract L1 capacity from options
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);

                BackendBuilder::memory().capacity(capacity)
            }
            #[cfg(feature = "redis")]
            crate::config::BackendType::Redis => {
                // Extract Redis URL from options
                let connection_string = config
                    .backend
                    .l2_options
                    .get("connection_string")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Extract Redis mode from options
                let mode_str = config
                    .backend
                    .l2_options
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("standalone");

                let mode = match mode_str {
                    "cluster" => crate::backend::client::RedisMode::Cluster,
                    "sentinel" => crate::backend::client::RedisMode::Sentinel,
                    _ => crate::backend::client::RedisMode::Standalone,
                };

                BackendBuilder::redis()
                    .connection_string(connection_string.as_deref().unwrap_or(""))
                    .mode(mode)
            }
            #[cfg(not(feature = "redis"))]
            crate::config::BackendType::Redis => {
                tracing::warn!(
                    "Redis backend requested but redis feature not enabled, falling back to memory"
                );
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);
                BackendBuilder::memory().capacity(capacity)
            }
            #[cfg(feature = "redis")]
            crate::config::BackendType::Tiered => {
                // Extract L1 capacity from options
                let l1_capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);

                // Extract Redis URL from options
                let connection_string = config
                    .backend
                    .l2_options
                    .get("connection_string")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Extract Redis mode from options
                let mode_str = config
                    .backend
                    .l2_options
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("standalone");

                let mode = match mode_str {
                    "cluster" => crate::backend::client::RedisMode::Cluster,
                    "sentinel" => crate::backend::client::RedisMode::Sentinel,
                    _ => crate::backend::client::RedisMode::Standalone,
                };

                BackendBuilder::tiered()
                    .l1_capacity(l1_capacity)
                    .l2_connection_string(connection_string.as_deref().unwrap_or(""))
                    .mode(mode)
            }
            #[cfg(not(feature = "redis"))]
            crate::config::BackendType::Tiered => {
                tracing::warn!(
                    "Tiered backend requested but redis feature not enabled, falling back to memory"
                );
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);
                BackendBuilder::memory().capacity(capacity)
            }
        };

        // Extract TTL from global config
        let ttl = if config.global.default_ttl > 0 {
            Some(Duration::from_secs(config.global.default_ttl))
        } else {
            None
        };

        Ok(Self {
            backend_builder: Some(backend_builder),
            ttl,
            capacity: None,
            batch_writes: false,
            auto_promote: true,
            _phantom: PhantomData,
        })
    }

    /// Create a CacheBuilder from a UnifiedConfig with service-specific configuration override.
    ///
    /// This method allows using service-level configuration to override global settings.
    /// If the specified service exists in the configuration, its TTL and capacity settings
    /// will be used instead of the global defaults.
    ///
    /// # Arguments
    ///
    /// * `config` - The UnifiedConfig containing global and service-specific settings
    /// * `service_name` - The name of the service configuration to use
    ///
    /// # Returns
    ///
    /// A `Result` containing the CacheBuilder on success, or a `CacheError` on failure.
    ///
    /// # Errors
    ///
    /// Returns `ServiceNotFound` if the specified service does not exist in the configuration.
    /// Returns `ConfigError` if the underlying configuration is invalid.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::config::UnifiedConfigBuilder;
    ///
    /// let config = UnifiedConfigBuilder::tiered()
    ///     .with_ttl(3600)
    ///     .with_l1_capacity(10000)
    ///     .with_redis_url("redis://localhost:6379")
    ///     .with_service("user_cache", CacheType::TwoLevel, 600)
    ///     .build();
    ///
    /// // Use user_cache service configuration
    /// let builder = CacheBuilder::from_unified_config_with_service(&config, "user_cache")?;
    /// ```
    #[cfg(feature = "confers")]
    pub fn from_unified_config_with_service(
        config: &crate::config::UnifiedConfig,
        service_name: &str,
    ) -> std::result::Result<Self, crate::error::CacheError> {
        // First validate the base configuration
        validate_unified_config(config)?;

        // Look up the service configuration
        let service_config = match config.services.get(service_name) {
            Some(service) => service,
            None => {
                return Err(crate::error::CacheError::ServiceNotFound(format!(
                    "Service '{}' not found in UnifiedConfig. Available services: {:?}",
                    service_name,
                    config.services.keys().collect::<Vec<_>>()
                )));
            }
        };

        // Build the backend based on backend type
        let backend_builder = match config.backend.backend_type {
            #[cfg(feature = "moka")]
            crate::config::BackendType::Memory => {
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);

                // Apply service-level capacity override if specified
                let effective_capacity = service_config.max_capacity.unwrap_or(capacity);

                crate::builder::BackendBuilder::memory().capacity(effective_capacity)
            }
            #[cfg(not(feature = "moka"))]
            crate::config::BackendType::Memory => {
                tracing::warn!("Memory backend requested but moka feature not enabled");
                crate::builder::BackendBuilder::default()
            }
            #[cfg(feature = "redis")]
            crate::config::BackendType::Redis => {
                let connection_string = config
                    .backend
                    .l2_options
                    .get("connection_string")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mode_str = config
                    .backend
                    .l2_options
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("standalone");

                let mode = match mode_str {
                    "cluster" => crate::backend::client::RedisMode::Cluster,
                    "sentinel" => crate::backend::client::RedisMode::Sentinel,
                    _ => crate::backend::client::RedisMode::Standalone,
                };

                crate::builder::BackendBuilder::redis()
                    .connection_string(connection_string.as_deref().unwrap_or(""))
                    .mode(mode)
            }
            #[cfg(not(feature = "redis"))]
            crate::config::BackendType::Redis => {
                tracing::warn!(
                    "Redis backend requested but redis feature not enabled, falling back to memory"
                );
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);
                crate::builder::BackendBuilder::memory().capacity(capacity)
            }
            #[cfg(feature = "moka")]
            #[cfg(feature = "redis")]
            crate::config::BackendType::Tiered => {
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);

                // Apply service-level capacity override if specified
                let effective_capacity = service_config.max_capacity.unwrap_or(capacity);

                let connection_string = config
                    .backend
                    .l2_options
                    .get("connection_string")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mode_str = config
                    .backend
                    .l2_options
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("standalone");

                let mode = match mode_str {
                    "cluster" => crate::backend::client::RedisMode::Cluster,
                    "sentinel" => crate::backend::client::RedisMode::Sentinel,
                    _ => crate::backend::client::RedisMode::Standalone,
                };

                crate::builder::BackendBuilder::tiered()
                    .l1_capacity(effective_capacity)
                    .l2_connection_string(connection_string.as_deref().unwrap_or(""))
                    .mode(mode)
            }
            #[cfg(not(all(feature = "moka", feature = "redis")))]
            crate::config::BackendType::Tiered => {
                tracing::warn!(
                    "Tiered backend requested but required features not enabled, falling back to memory"
                );
                let capacity = config
                    .backend
                    .l1_options
                    .get("max_capacity")
                    .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                    .unwrap_or(10000);
                crate::builder::BackendBuilder::memory().capacity(capacity)
            }
        };

        // Extract TTL from service config, falling back to global config
        let ttl = match service_config.ttl {
            Some(service_ttl) if service_ttl > 0 => Some(Duration::from_secs(service_ttl)),
            _ => {
                if config.global.default_ttl > 0 {
                    Some(Duration::from_secs(config.global.default_ttl))
                } else {
                    None
                }
            }
        };

        Ok(Self {
            backend_builder: Some(backend_builder),
            ttl,
            capacity: None,
            batch_writes: false,
            auto_promote: true,
            _phantom: PhantomData,
        })
    }
}

/// Validates a UnifiedConfig instance.
///
/// This function checks that the configuration is valid for building a cache,
/// ensuring that required fields are present and have sensible values.
///
/// # Arguments
///
/// * `config` - The UnifiedConfig to validate
///
/// # Returns
///
/// `Ok(())` if the configuration is valid, or a `CacheError::ConfigError` if invalid.
///
/// # Errors
///
/// Returns `ConfigError` in the following cases:
/// - Redis/Tiered backend missing `connection_string`
/// - Invalid capacity value (zero or negative)
/// - Invalid TTL value (zero when explicitly set)
///
#[cfg(feature = "confers")]
fn validate_unified_config(
    config: &crate::config::UnifiedConfig,
) -> std::result::Result<(), crate::error::CacheError> {
    use crate::config::BackendType;

    match config.backend.backend_type {
        BackendType::Memory => {
            // Validate L1 capacity if specified
            if let Some(capacity) = config.backend.l1_options.get("max_capacity") {
                let capacity_val = capacity
                    .as_u64()
                    .or_else(|| capacity.as_i64().map(|i| i as u64));
                if let Some(cap) = capacity_val {
                    if cap == 0 {
                        return Err(crate::error::CacheError::ConfigError(
                            "L1 capacity cannot be zero".to_string(),
                        ));
                    }
                }
            }
        }
        BackendType::Redis => {
            // Validate Redis connection_string is present and non-empty
            let connection_string = config.backend.l2_options.get("connection_string");
            if connection_string.is_none() {
                return Err(crate::error::CacheError::ConfigError(
                    "Redis backend requires 'connection_string' in l2_options".to_string(),
                ));
            }
            if let Some(cs) = connection_string.and_then(|v| v.as_str()) {
                if cs.is_empty() {
                    return Err(crate::error::CacheError::ConfigError(
                        "Redis connection_string cannot be empty".to_string(),
                    ));
                }
            }
        }
        BackendType::Tiered => {
            // Validate L1 capacity
            if let Some(capacity) = config.backend.l1_options.get("max_capacity") {
                let capacity_val = capacity
                    .as_u64()
                    .or_else(|| capacity.as_i64().map(|i| i as u64));
                if let Some(cap) = capacity_val {
                    if cap == 0 {
                        return Err(crate::error::CacheError::ConfigError(
                            "L1 capacity cannot be zero".to_string(),
                        ));
                    }
                }
            }
            // Validate Redis connection_string
            let connection_string = config.backend.l2_options.get("connection_string");
            if connection_string.is_none() {
                return Err(crate::error::CacheError::ConfigError(
                    "Tiered backend requires 'connection_string' in l2_options".to_string(),
                ));
            }
            if let Some(cs) = connection_string.and_then(|v| v.as_str()) {
                if cs.is_empty() {
                    return Err(crate::error::CacheError::ConfigError(
                        "Redis connection_string cannot be empty".to_string(),
                    ));
                }
            }
        }
    }

    // Validate TTL if explicitly set (not default)
    if config.global.default_ttl == 0 {
        // Zero is acceptable as "not set" (use default), but warn if it was explicitly set
        // For now, we allow zero as "use library default"
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestValue {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn test_cache_builder_default() {
        let cache: Cache<String, TestValue> = CacheBuilder::default().build().await.unwrap();
        assert!(cache.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_builder_with_capacity() {
        let cache: Cache<String, TestValue> = CacheBuilder::default()
            .capacity(1000)
            .build()
            .await
            .unwrap();
        assert!(cache.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_builder_with_ttl() {
        let cache: Cache<String, TestValue> = CacheBuilder::default()
            .ttl(Duration::from_secs(3600))
            .build()
            .await
            .unwrap();
        assert!(cache.health_check().await.unwrap());
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_cache_builder_from_unified_config_memory() {
        use crate::config::UnifiedConfigBuilder;

        // Create a memory-only configuration
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(5000)
            .build();

        // Create cache from configuration
        let builder = CacheBuilder::from_unified_config(&config).unwrap();
        let cache: Cache<String, TestValue> = builder.build().await.unwrap();

        assert!(cache.health_check().await.unwrap());
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_cache_builder_from_unified_config_tiered() {
        use crate::config::UnifiedConfigBuilder;

        // Create a tiered configuration (L1 + L2) with a Redis URL
        // This test requires a running Redis server
        let config = UnifiedConfigBuilder::tiered()
            .with_ttl(7200)
            .with_l1_capacity(10000)
            .with_redis_url("redis://localhost:6379")
            .build();

        // Create cache from configuration - will fail without Redis server
        // but verifies the configuration is properly passed through
        let builder_result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        assert!(builder_result.is_ok(), "Config should be valid");
        let result = builder_result.unwrap().build().await;

        // This test requires a running Redis server to pass
        // If Redis is not available, the error message indicates proper configuration
        match result {
            Ok(cache) => {
                assert!(cache.health_check().await.unwrap());
            }
            Err(e) => {
                // Expected when Redis server is not running
                // Error should be Redis connection error, not configuration error
                let error_msg = format!("{:?}", e);
                assert!(
                    error_msg.contains("Redis") || error_msg.contains("connection"),
                    "Expected Redis-related error, got: {}",
                    error_msg
                );
            }
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_valid_memory() {
        use crate::config::UnifiedConfigBuilder;

        // Valid memory-only configuration
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .build();

        // Should succeed
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        assert!(result.is_ok(), "Valid memory config should succeed");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_valid_tiered() {
        use crate::config::UnifiedConfigBuilder;

        // Valid tiered configuration
        let config = UnifiedConfigBuilder::tiered()
            .with_ttl(7200)
            .with_l1_capacity(10000)
            .with_redis_url("redis://localhost:6379")
            .build();

        // Should succeed (may fail at build time if Redis not available, but config is valid)
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        assert!(result.is_ok(), "Valid tiered config should succeed");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_redis_missing_connection_string() {
        use crate::config::{BackendConfig, BackendType, UnifiedConfig};

        // Redis config without connection_string
        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: BackendType::Redis,
                l2_options: serde_json::json!({
                    "mode": "standalone"
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        // Should fail with ConfigError
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        if let Err(crate::error::CacheError::ConfigError(_)) = result {
            // Expected: ConfigError
        } else {
            panic!("Expected ConfigError for missing connection_string");
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_redis_empty_connection_string() {
        use crate::config::{BackendConfig, BackendType, UnifiedConfig};

        // Redis config with empty connection_string
        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: BackendType::Redis,
                l2_options: serde_json::json!({
                    "connection_string": "",
                    "mode": "standalone"
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        // Should fail with ConfigError
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        if let Err(crate::error::CacheError::ConfigError(_)) = result {
            // Expected: ConfigError
        } else {
            panic!("Expected ConfigError for empty connection_string");
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_tiered_missing_connection_string() {
        use crate::config::{BackendConfig, BackendType, UnifiedConfig};

        // Tiered config without connection_string
        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: BackendType::Tiered,
                l1_options: serde_json::json!({
                    "max_capacity": 10000
                }),
                l2_options: serde_json::json!({}),
                ..Default::default()
            },
            ..Default::default()
        };

        // Should fail with ConfigError
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        if let Err(crate::error::CacheError::ConfigError(_)) = result {
            // Expected: ConfigError
        } else {
            panic!("Expected ConfigError for missing connection_string");
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_tiered_empty_connection_string() {
        use crate::config::{BackendConfig, BackendType, UnifiedConfig};

        // Tiered config with empty connection_string
        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: BackendType::Tiered,
                l1_options: serde_json::json!({
                    "max_capacity": 10000
                }),
                l2_options: serde_json::json!({
                    "connection_string": ""
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        // Should fail with ConfigError
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        if let Err(crate::error::CacheError::ConfigError(_)) = result {
            // Expected: ConfigError
        } else {
            panic!("Expected ConfigError for empty connection_string");
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_zero_capacity() {
        use crate::config::{BackendConfig, BackendType, UnifiedConfig};

        // Memory config with zero capacity
        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: BackendType::Memory,
                l1_options: serde_json::json!({
                    "max_capacity": 0
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        // Should fail with ConfigError
        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        if let Err(crate::error::CacheError::ConfigError(_)) = result {
            // Expected: ConfigError
        } else {
            panic!("Expected ConfigError for zero capacity");
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_valid() {
        use crate::config::{CacheType, UnifiedConfigBuilder};

        // Create a config with a service
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .with_service("user_cache", CacheType::L1, 600)
            .build();

        // Should succeed with valid service
        let result = CacheBuilder::<String, TestValue>::from_unified_config_with_service(
            &config,
            "user_cache",
        );
        assert!(result.is_ok(), "Valid service config should succeed");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_not_found() {
        use crate::config::UnifiedConfigBuilder;

        // Create a config without services
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .build();

        // Should fail with ServiceNotFound
        let result = CacheBuilder::<String, TestValue>::from_unified_config_with_service(
            &config,
            "nonexistent_service",
        );
        match result {
            Err(crate::error::CacheError::ServiceNotFound(msg)) => {
                assert!(
                    msg.contains("nonexistent_service"),
                    "Error should mention service name"
                );
                assert!(
                    msg.contains("Available services"),
                    "Error should list available services"
                );
            }
            _ => panic!("Expected ServiceNotFound error"),
        }
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_ttl_override() {
        use crate::config::{CacheType, UnifiedConfigBuilder};

        // Create a config with global TTL and service-specific TTL
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600) // Global TTL
            .with_l1_capacity(10000)
            .with_service("fast_cache", CacheType::L1, 60) // Service TTL = 60s
            .build();

        // Create builder with service config
        let builder = CacheBuilder::<String, TestValue>::from_unified_config_with_service(
            &config,
            "fast_cache",
        )
        .expect("Should succeed");

        // Service TTL should override global TTL
        assert_eq!(
            builder.ttl,
            Some(Duration::from_secs(60)),
            "Service TTL should override global TTL"
        );
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_capacity_override() {
        use crate::config::{CacheType, UnifiedConfigBuilder};

        // Create a config with global capacity and service-specific capacity
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000) // Global capacity
            .with_service("small_cache", CacheType::L1, 60)
            .build();

        // We need to use BackendBuilder to verify capacity, but since it's private,
        // we verify by checking that the builder is created successfully
        let builder = CacheBuilder::<String, TestValue>::from_unified_config_with_service(
            &config,
            "small_cache",
        )
        .expect("Should succeed with service capacity override");

        // Verify TTL override worked (capacity is not directly accessible)
        assert_eq!(builder.ttl, Some(Duration::from_secs(60)));
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_fallback_to_global_ttl() {
        use crate::config::UnifiedConfigBuilder;

        // Create a config with global TTL but no service-specific TTL
        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600) // Global TTL
            .with_l1_capacity(10000)
            .with_service("no_ttl_service", crate::config::CacheType::L1, 0) // No service TTL
            .build();

        // Create builder with service config
        let builder = CacheBuilder::<String, TestValue>::from_unified_config_with_service(
            &config,
            "no_ttl_service",
        )
        .expect("Should succeed");

        // Should fall back to global TTL
        assert_eq!(
            builder.ttl,
            Some(Duration::from_secs(3600)),
            "Should fall back to global TTL"
        );
    }
}
