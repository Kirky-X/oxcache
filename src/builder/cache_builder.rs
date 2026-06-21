//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache builder for advanced configuration

use crate::backend::interface::CacheBackend;
use crate::backend::MokaMemoryBackend as MemoryBackend;
use crate::cache::Cache;
use crate::core::types::RedisModeType;
use crate::error::Result;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// 解析 Redis 模式字符串，失败时返回默认值
fn parse_redis_mode(mode_str: &str) -> RedisModeType {
    mode_str.parse().unwrap_or_default()
}

/// 内部后端配置枚举
///
/// 用于存储后端配置，在 build() 时异步创建后端实例。
#[derive(Clone)]
enum InternalBackendConfig {
    /// 内存后端配置
    Memory { capacity: u64 },
    /// Redis 后端配置
    #[cfg(feature = "redis")]
    Redis {
        connection_string: String,
        mode: crate::backend::client::RedisMode,
    },
    /// 分层后端配置
    #[cfg(feature = "redis")]
    Tiered {
        l1_capacity: u64,
        l2_connection_string: String,
        l2_mode: crate::backend::client::RedisMode,
    },
    /// 预构建的后端实例
    Prebuilt(Arc<dyn CacheBackend>),
}

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
    backend_config: Option<InternalBackendConfig>,
    ttl: Option<Duration>,
    capacity: Option<u64>,
    batch_writes: bool,
    auto_promote: bool,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            backend_config: None,
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
    K: crate::core::traits::CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
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

    /// Set a custom backend
    ///
    /// # Arguments
    ///
    /// * `backend` - Custom backend implementing CacheBackend trait
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::backend::MokaMemoryBackend;
    ///
    /// let backend = MokaMemoryBackend::builder().capacity(10000).build();
    /// let builder = Cache::builder().with_backend(backend);
    /// ```
    pub fn with_backend<B>(mut self, backend: B) -> Self
    where
        B: CacheBackend + 'static,
    {
        self.backend_config = Some(InternalBackendConfig::Prebuilt(Arc::new(backend)));
        self
    }

    /// Set a pre-built backend (Arc wrapped)
    ///
    /// # Arguments
    ///
    /// * `backend` - Arc-wrapped backend
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn backend(mut self, backend: Arc<dyn CacheBackend>) -> Self {
        self.backend_config = Some(InternalBackendConfig::Prebuilt(backend));
        self
    }

    /// Configure Redis backend
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn redis(mut self, connection_string: impl Into<String>) -> Self {
        self.backend_config = Some(InternalBackendConfig::Redis {
            connection_string: connection_string.into(),
            mode: crate::backend::client::RedisMode::Standalone,
        });
        self
    }

    /// Configure Redis backend with mode
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    /// * `mode` - Redis mode (Standalone, Sentinel, Cluster)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn redis_with_mode(
        mut self,
        connection_string: impl Into<String>,
        mode: crate::backend::client::RedisMode,
    ) -> Self {
        self.backend_config = Some(InternalBackendConfig::Redis {
            connection_string: connection_string.into(),
            mode,
        });
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
        let backend = match self.backend_config {
            Some(InternalBackendConfig::Prebuilt(backend)) => backend,
            Some(InternalBackendConfig::Memory { capacity }) => {
                let mut builder = MemoryBackend::builder().capacity(capacity);
                if let Some(ttl) = self.ttl {
                    builder = builder.ttl(ttl);
                }
                Arc::new(builder.build())
            }
            #[cfg(feature = "redis")]
            Some(InternalBackendConfig::Redis {
                connection_string,
                mode,
            }) => {
                let backend = crate::backend::client::RedisBackend::builder()
                    .connection_string(&connection_string)
                    .mode(mode)
                    .build()
                    .await?;
                Arc::new(backend)
            }
            #[cfg(feature = "redis")]
            Some(InternalBackendConfig::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode,
            }) => {
                let l1 = MemoryBackend::builder().capacity(l1_capacity).build();
                let l2 = crate::backend::client::RedisBackend::builder()
                    .connection_string(&l2_connection_string)
                    .mode(l2_mode)
                    .build()
                    .await?;

                Arc::new(
                    crate::cache::chain::ChainCache::builder()
                        .link(crate::cache::chain::ChainLink::from_backend(l1))
                        .link(crate::cache::chain::ChainLink::from_backend(l2))
                        .build(),
                )
            }
            None => {
                let capacity = self.capacity.unwrap_or(10000);
                let mut builder = MemoryBackend::builder().capacity(capacity);
                if let Some(ttl) = self.ttl {
                    builder = builder.ttl(ttl);
                }
                Arc::new(builder.build())
            }
        };

        Ok(Cache::new_with_backend(backend))
    }

    /// Configure from confers JSON configuration
    ///
    /// This method allows using external confers configuration to create a cache builder.
    /// The configuration is parsed and applied to the builder.
    ///
    /// # Arguments
    ///
    /// * `config` - JSON configuration value
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Configuration Keys
    ///
    /// Reads the following from `oxcache` section:
    /// - `backend`: Backend type ("memory" | "redis" | "tiered"), default "memory"
    /// - `capacity`: Memory cache capacity, default 10000
    /// - `ttl`: Default TTL in seconds
    /// - `redis.url`: Redis connection URL
    /// - `tiered.l1_capacity`: L1 cache capacity for tiered backend
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    ///
    /// let config = json!({
    ///     "oxcache": {
    ///         "backend": "memory",
    ///         "capacity": 5000
    ///     }
    /// });
    ///
    /// let cache: Cache<String, String> = Cache::builder()
    ///     .with_confers(&config)
    ///     .build()
    ///     .await?;
    /// ```
    #[cfg(feature = "confers")]
    pub fn with_confers(mut self, config: &serde_json::Value) -> Self {
        let oxcache_config: &serde_json::Map<String, serde_json::Value> = match config.get("oxcache") {
            Some(serde_json::Value::Object(obj)) => obj,
            _ => return self,
        };

        let backend_type = oxcache_config
            .get("backend")
            .and_then(|v| v.as_str())
            .unwrap_or("memory");

        let capacity = oxcache_config.get("capacity").and_then(|v| v.as_u64()).unwrap_or(10000);

        if let Some(ttl_secs) = oxcache_config.get("ttl").and_then(|v| v.as_u64()) {
            self.ttl = Some(Duration::from_secs(ttl_secs));
        }

        self.capacity = Some(capacity);

        match backend_type {
            "memory" => {
                self.backend_config = Some(InternalBackendConfig::Memory { capacity });
            }
            #[cfg(feature = "redis")]
            "redis" => {
                let redis_config = oxcache_config
                    .get("redis")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                let connection_string = redis_config
                    .get("url")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "redis://localhost:6379".to_string());
                self.backend_config = Some(InternalBackendConfig::Redis {
                    connection_string,
                    mode: crate::backend::client::RedisMode::Standalone,
                });
            }
            #[cfg(feature = "redis")]
            "tiered" => {
                let tiered_config = oxcache_config
                    .get("tiered")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                let l1_capacity = tiered_config
                    .get("l1_capacity")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(capacity);
                let redis_config = oxcache_config
                    .get("redis")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                let l2_connection_string = redis_config
                    .get("url")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "redis://localhost:6379".to_string());
                self.backend_config = Some(InternalBackendConfig::Tiered {
                    l1_capacity,
                    l2_connection_string,
                    l2_mode: crate::backend::client::RedisMode::Standalone,
                });
            }
            _ => {
                self.backend_config = Some(InternalBackendConfig::Memory { capacity });
            }
        }

        self
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
    /// Returns `CacheError::InvalidInput` if the configuration is invalid:
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
        // 配置已由 garde 在加载时验证，无需重复验证
        let backend_config = backend_config_from_unified_config(config)?;

        let ttl = if config.global.default_ttl > 0 {
            Some(Duration::from_secs(config.global.default_ttl))
        } else {
            None
        };

        Ok(Self {
            backend_config: Some(backend_config),
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
        // 配置已由 garde 在加载时验证，无需重复验证
        let services = config.services();
        let service_config = match services.get(service_name) {
            Some(service) => service,
            None => {
                return Err(crate::error::CacheError::ServiceNotFound(format!(
                    "Service '{}' not found in UnifiedConfig. Available services: {:?}",
                    service_name,
                    services.keys().collect::<Vec<_>>()
                )));
            }
        };

        let backend_config = backend_config_from_unified_config_with_service(config, service_config)?;

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
            backend_config: Some(backend_config),
            ttl,
            capacity: None,
            batch_writes: false,
            auto_promote: true,
            _phantom: PhantomData,
        })
    }
}

#[cfg(feature = "confers")]
fn backend_config_from_unified_config(
    config: &crate::config::UnifiedConfig,
) -> std::result::Result<InternalBackendConfig, crate::error::CacheError> {
    match config.backend.backend_type_enum() {
        crate::config::BackendType::Memory => {
            let capacity = config
                .backend
                .l1_options()
                .get("max_capacity")
                .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                .unwrap_or(10000);

            Ok(InternalBackendConfig::Memory { capacity })
        }
        #[cfg(feature = "redis")]
        crate::config::BackendType::Redis => {
            let connection_string = config
                .backend
                .l2_options()
                .get("connection_string")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let l2_opts = config.backend.l2_options();
            let mode_str = l2_opts.get("mode").and_then(|v| v.as_str()).unwrap_or("standalone");
            let mode = parse_redis_mode(mode_str);

            Ok(InternalBackendConfig::Redis {
                connection_string,
                mode,
            })
        }
        #[cfg(not(feature = "redis"))]
        crate::config::BackendType::Redis => {
            // Redis feature not enabled, falling back to memory backend
            let capacity = config
                .backend
                .l1_options()
                .get("max_capacity")
                .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                .unwrap_or(10000);
            Ok(InternalBackendConfig::Memory { capacity })
        }
        #[cfg(feature = "redis")]
        crate::config::BackendType::Tiered => {
            let l1_capacity = config
                .backend
                .l1_options()
                .get("max_capacity")
                .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                .unwrap_or(10000);

            let connection_string = config
                .backend
                .l2_options()
                .get("connection_string")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let l2_opts = config.backend.l2_options();
            let mode_str = l2_opts.get("mode").and_then(|v| v.as_str()).unwrap_or("standalone");
            let mode = parse_redis_mode(mode_str);

            Ok(InternalBackendConfig::Tiered {
                l1_capacity,
                l2_connection_string: connection_string,
                l2_mode: mode,
            })
        }
        #[cfg(not(feature = "redis"))]
        crate::config::BackendType::Tiered => {
            // Redis feature not enabled, falling back to memory backend
            let capacity = config
                .backend
                .l1_options()
                .get("max_capacity")
                .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
                .unwrap_or(10000);
            Ok(InternalBackendConfig::Memory { capacity })
        }
    }
}

#[cfg(feature = "confers")]
fn backend_config_from_unified_config_with_service(
    config: &crate::config::UnifiedConfig,
    service_config: &crate::config::ServiceConfig,
) -> std::result::Result<InternalBackendConfig, crate::error::CacheError> {
    let capacity = config
        .backend
        .l1_options()
        .get("max_capacity")
        .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
        .unwrap_or(10000);

    let effective_capacity = service_config.max_capacity.unwrap_or(capacity);

    match config.backend.backend_type_enum() {
        #[cfg(feature = "memory")]
        crate::config::BackendType::Memory => Ok(InternalBackendConfig::Memory {
            capacity: effective_capacity,
        }),
        #[cfg(not(feature = "memory"))]
        crate::config::BackendType::Memory => {
            // Moka feature not enabled, using fallback memory backend
            Ok(InternalBackendConfig::Memory {
                capacity: effective_capacity,
            })
        }
        #[cfg(feature = "redis")]
        crate::config::BackendType::Redis => {
            let connection_string = config
                .backend
                .l2_options()
                .get("connection_string")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let l2_opts = config.backend.l2_options();
            let mode_str = l2_opts.get("mode").and_then(|v| v.as_str()).unwrap_or("standalone");
            let mode = parse_redis_mode(mode_str);

            Ok(InternalBackendConfig::Redis {
                connection_string: connection_string.unwrap_or_default(),
                mode,
            })
        }
        #[cfg(not(feature = "redis"))]
        crate::config::BackendType::Redis => {
            // Redis feature not enabled, falling back to memory backend
            Ok(InternalBackendConfig::Memory {
                capacity: effective_capacity,
            })
        }
        #[cfg(all(feature = "memory", feature = "redis"))]
        crate::config::BackendType::Tiered => {
            let connection_string = config
                .backend
                .l2_options()
                .get("connection_string")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let l2_opts = config.backend.l2_options();
            let mode_str = l2_opts.get("mode").and_then(|v| v.as_str()).unwrap_or("standalone");
            let mode = parse_redis_mode(mode_str);

            Ok(InternalBackendConfig::Tiered {
                l1_capacity: effective_capacity,
                l2_connection_string: connection_string.unwrap_or_default(),
                l2_mode: mode,
            })
        }
        #[cfg(not(all(feature = "memory", feature = "redis")))]
        crate::config::BackendType::Tiered => {
            // Required features not enabled, falling back to memory backend
            Ok(InternalBackendConfig::Memory {
                capacity: effective_capacity,
            })
        }
    }
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
        cache.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_builder_with_capacity() {
        let cache: Cache<String, TestValue> = CacheBuilder::default().capacity(1000).build().await.unwrap();
        cache.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_builder_with_ttl() {
        let cache: Cache<String, TestValue> = CacheBuilder::default()
            .ttl(Duration::from_secs(3600))
            .build()
            .await
            .unwrap();
        cache.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_builder_with_backend() {
        let backend = MemoryBackend::builder().capacity(5000).build();
        let cache: Cache<String, TestValue> = CacheBuilder::default().with_backend(backend).build().await.unwrap();
        cache.health_check().await.unwrap();
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_cache_builder_from_unified_config_memory() {
        use crate::config::UnifiedConfigBuilder;

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(5000)
            .build()
            .unwrap();

        let builder = CacheBuilder::from_unified_config(&config).unwrap();
        let cache: Cache<String, TestValue> = builder.build().await.unwrap();

        cache.health_check().await.unwrap();
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_cache_builder_from_unified_config_tiered() {
        use crate::config::UnifiedConfigBuilder;

        let config = UnifiedConfigBuilder::tiered()
            .with_ttl(7200)
            .with_l1_capacity(10000)
            .with_redis_url("redis://localhost:6379")
            .build()
            .unwrap();

        let builder_result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        assert!(builder_result.is_ok(), "Config should be valid");
        let result = builder_result.unwrap().build().await;

        match result {
            Ok(cache) => {
                cache.health_check().await.unwrap();
            }
            Err(e) => {
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

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .build()
            .unwrap();

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        assert!(result.is_ok(), "Valid memory config should succeed");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_valid_tiered() {
        use crate::config::UnifiedConfigBuilder;

        let config = UnifiedConfigBuilder::tiered()
            .with_ttl(7200)
            .with_l1_capacity(10000)
            .with_redis_url("redis://localhost:6379")
            .build()
            .unwrap();

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        assert!(result.is_ok(), "Valid tiered config should succeed");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_redis_missing_connection_string() {
        use crate::config::{BackendConfig, UnifiedConfig};

        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: "Redis".to_string(),
                l2_options_json: serde_json::json!({
                    "mode": "standalone"
                })
                .to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        // 缺少连接字符串现在允许通过，Redis 连接会在实际使用时失败
        assert!(
            result.is_ok(),
            "Missing connection string should be allowed at config time"
        );
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_redis_empty_connection_string() {
        use crate::config::{BackendConfig, UnifiedConfig};

        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: "Redis".to_string(),
                l2_options_json: serde_json::json!({
                    "connection_string": "",
                    "mode": "standalone"
                })
                .to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        // 空连接字符串现在允许通过，Redis 连接会在实际使用时失败
        assert!(
            result.is_ok(),
            "Empty connection string should be allowed at config time"
        );
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_tiered_missing_connection_string() {
        use crate::config::{BackendConfig, UnifiedConfig};

        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: "Tiered".to_string(),
                l1_options_json: serde_json::json!({
                    "max_capacity": 10000
                })
                .to_string(),
                l2_options_json: serde_json::json!({}).to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        // 缺少连接字符串现在允许通过，Redis 连接会在实际使用时失败
        assert!(
            result.is_ok(),
            "Missing connection string should be allowed at config time"
        );
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_tiered_empty_connection_string() {
        use crate::config::{BackendConfig, UnifiedConfig};

        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: "Tiered".to_string(),
                l1_options_json: serde_json::json!({
                    "max_capacity": 10000
                })
                .to_string(),
                l2_options_json: serde_json::json!({
                    "connection_string": ""
                })
                .to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        // 空连接字符串现在允许通过，Redis 连接会在实际使用时失败
        assert!(
            result.is_ok(),
            "Empty connection string should be allowed at config time"
        );
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_zero_capacity() {
        use crate::config::{BackendConfig, UnifiedConfig};

        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: "Memory".to_string(),
                l1_options_json: serde_json::json!({
                    "max_capacity": 0
                })
                .to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
        // 零容量现在使用默认值
        assert!(result.is_ok(), "Zero capacity should use default");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_valid() {
        use crate::config::{CacheType, UnifiedConfigBuilder};

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .with_service("user_cache", CacheType::L1, 600)
            .build()
            .unwrap();

        let result = CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "user_cache");
        assert!(result.is_ok(), "Valid service config should succeed");
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_not_found() {
        use crate::config::UnifiedConfigBuilder;

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .build()
            .unwrap();

        let result =
            CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "nonexistent_service");
        match result {
            Err(crate::error::CacheError::ServiceNotFound(msg)) => {
                assert!(msg.contains("nonexistent_service"), "Error should mention service name");
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

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .with_service("fast_cache", CacheType::L1, 60)
            .build()
            .unwrap();

        let builder = CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "fast_cache")
            .expect("Should succeed");

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

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .with_service("small_cache", CacheType::L1, 60)
            .build()
            .unwrap();

        let builder = CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "small_cache")
            .expect("Should succeed with service capacity override");

        assert_eq!(builder.ttl, Some(Duration::from_secs(60)));
    }

    #[tokio::test]
    #[cfg(feature = "confers")]
    async fn test_from_unified_config_with_service_fallback_to_global_ttl() {
        use crate::config::UnifiedConfigBuilder;

        let config = UnifiedConfigBuilder::memory_only()
            .with_ttl(3600)
            .with_l1_capacity(10000)
            .with_service("no_ttl_service", crate::config::CacheType::L1, 0)
            .build()
            .unwrap();

        let builder = CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "no_ttl_service")
            .expect("Should succeed");

        assert_eq!(
            builder.ttl,
            Some(Duration::from_secs(3600)),
            "Should fall back to global TTL"
        );
    }

    #[test]
    #[cfg(feature = "confers")]
    fn test_config_format_from_path() {
        use crate::config::ConfigFormat;

        assert_eq!(ConfigFormat::from_path("config.toml"), Some(ConfigFormat::Toml));
        assert_eq!(ConfigFormat::from_path("config.json"), Some(ConfigFormat::Json));
        assert_eq!(ConfigFormat::from_path("config.yaml"), None);
        assert_eq!(ConfigFormat::from_path("config.xml"), None);
        assert_eq!(ConfigFormat::from_path("config"), None);
    }

    #[test]
    #[cfg(feature = "confers")]
    fn test_config_format_extension() {
        use crate::config::ConfigFormat;

        assert_eq!(ConfigFormat::Toml.extension(), "toml");
        assert_eq!(ConfigFormat::Json.extension(), "json");
    }

    #[test]
    #[cfg(feature = "confers")]
    fn test_config_format_mime_type() {
        use crate::config::ConfigFormat;

        assert_eq!(ConfigFormat::Toml.mime_type(), "application/toml");
        assert_eq!(ConfigFormat::Json.mime_type(), "application/json");
    }
}
