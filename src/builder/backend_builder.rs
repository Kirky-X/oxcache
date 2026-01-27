//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Backend builder for creating cache backends

use crate::backend::client::MokaMemoryBackend as MemoryBackend;
#[cfg(feature = "redis")]
use crate::backend::client::{RedisBackend, RedisMode};
use crate::backend::CacheBackend;
use crate::error::Result;
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

/// Simple tiered backend combining L1 (memory) and L2 (Redis) caches
///
/// This implements a write-through cache strategy:
/// - On get: check L1 first, then L2
/// - On set: write to both L1 and L2
#[derive(Clone)]
pub struct TieredBackend {
    /// L1 cache - local in-memory cache
    l1: Arc<MemoryBackend>,
    /// L2 cache - distributed Redis cache
    l2: Arc<RedisBackend>,
}

impl TieredBackend {
    /// Create a new tiered backend with specified dependencies
    ///
    /// This is the dependency injection constructor for TieredBackend.
    /// Use this when you want to inject pre-configured L1 and L2 backends.
    ///
    /// # Arguments
    ///
    /// * `l1` - The L1 (memory) cache backend
    /// * `l2` - The L2 (Redis) cache backend
    ///
    /// # Returns
    ///
    /// A new TieredBackend instance
    pub fn with_dependencies(l1: Arc<MemoryBackend>, l2: Arc<RedisBackend>) -> Self {
        Self { l1, l2 }
    }

    /// Create a new tiered backend (alias for with_dependencies)
    pub fn new(l1: Arc<MemoryBackend>, l2: Arc<RedisBackend>) -> Self {
        Self { l1, l2 }
    }
}

#[async_trait]
impl CacheBackend for TieredBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // Try L1 first
        let l1_result = self.l1.get(key).await?;
        if l1_result.is_some() {
            return Ok(l1_result);
        }

        // Fall back to L2
        let l2_result = self.l2.get(key).await?;
        if let Some(ref value) = l2_result {
            // Promote to L1
            let _ = self.l1.set(key, value.clone(), None).await;
        }
        Ok(l2_result)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        // Write to L1
        self.l1.set(key, value.clone(), ttl).await?;

        // Write to L2
        self.l2.set(key, value, ttl).await?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Delete from both
        self.l1.delete(key).await?;
        self.l2.delete(key).await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        // Check L1 first
        if self.l1.exists(key).await? {
            return Ok(true);
        }
        // Check L2
        self.l2.exists(key).await
    }

    async fn clear(&self) -> Result<()> {
        self.l1.clear().await?;
        self.l2.clear().await?;
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.l2.close().await?;
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        // Check L1 first
        let l1_ttl = self.l1.ttl(key).await?;
        if l1_ttl.is_some() {
            return Ok(l1_ttl);
        }
        self.l2.ttl(key).await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        self.l1.expire(key, ttl).await?;
        self.l2.expire(key, ttl).await
    }

    async fn health_check(&self) -> Result<bool> {
        // Check L2 health (L1 is always healthy)
        self.l2.health_check().await
    }

    async fn stats(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stats = self.l1.stats().await?;
        let l2_stats = self.l2.stats().await?;
        stats.extend(l2_stats);
        Ok(stats)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Backend builder enum for creating different backend types
///
/// This builder provides a fluent interface for creating cache backends.
/// Use the factory methods to specify the backend type and configuration.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::builder::BackendBuilder;
///
/// // Create memory backend
/// let backend = BackendBuilder::memory().build().await?;
///
/// // Create Redis backend
/// let backend = BackendBuilder::redis()
///     .connection_string("redis://localhost:6379")
///     .build()
///     .await?;
///
/// // Create tiered backend (L1 + L2)
/// let backend = BackendBuilder::tiered()
///     .l1_capacity(10000)
///     .l2_connection_string("redis://localhost:6379")
///     .build()
///     .await?;
/// ```
pub enum BackendBuilder {
    /// Memory backend configuration
    Memory {
        capacity: u64,
        ttl: Option<std::time::Duration>,
    },
    /// Redis backend configuration
    #[cfg(feature = "redis")]
    Redis {
        connection_string: Option<String>,
        mode: RedisMode,
    },
    /// Tiered backend (L1 + L2) configuration
    #[cfg(feature = "redis")]
    Tiered {
        l1_capacity: u64,
        l2_connection_string: Option<String>,
        l2_mode: RedisMode,
        write_through: bool,
        promote_on_hit: bool,
    },
}

impl BackendBuilder {
    /// Create a memory backend builder
    ///
    /// # Returns
    ///
    /// Memory backend builder
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = BackendBuilder::memory()
    ///     .capacity(10000)
    ///     .ttl(std::time::Duration::from_secs(3600))
    ///     .build()
    ///     .await?;
    /// ```
    pub fn memory() -> Self {
        BackendBuilder::Memory {
            capacity: 10000,
            ttl: None,
        }
    }

    /// Create a Redis backend builder
    ///
    /// # Returns
    ///
    /// Redis backend builder
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = BackendBuilder::redis()
    ///     .connection_string("redis://localhost:6379")
    ///     .mode(RedisMode::Standalone)
    ///     .build()
    ///     .await?;
    /// ```
    #[cfg(feature = "redis")]
    pub fn redis() -> Self {
        BackendBuilder::Redis {
            connection_string: None,
            mode: RedisMode::Standalone,
        }
    }

    /// Create a tiered backend builder (L1 + L2)
    ///
    /// # Returns
    ///
    /// Tiered backend builder
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = BackendBuilder::tiered()
    ///     .l1_capacity(10000)
    ///     .l2_connection_string("redis://localhost:6379")
    ///     .build()
    ///     .await?;
    /// ```
    #[cfg(feature = "redis")]
    pub fn tiered() -> Self {
        BackendBuilder::Tiered {
            l1_capacity: 10000,
            l2_connection_string: None,
            l2_mode: RedisMode::Standalone,
            write_through: true,
            promote_on_hit: true,
        }
    }

    // Memory backend configuration methods

    /// Set the capacity for memory backend
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn capacity(mut self, capacity: u64) -> Self {
        if let BackendBuilder::Memory { capacity: _c, ttl } = self {
            self = BackendBuilder::Memory { capacity, ttl };
        }
        self
    }

    /// Set the TTL for memory backend
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live duration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn ttl(mut self, ttl: std::time::Duration) -> Self {
        if let BackendBuilder::Memory { capacity, ttl: _t } = self {
            self = BackendBuilder::Memory {
                capacity,
                ttl: Some(ttl),
            };
        }
        self
    }

    // Redis backend configuration methods

    /// Set the connection string for Redis backend
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn connection_string(mut self, connection_string: &str) -> Self {
        match self {
            BackendBuilder::Redis { mode, .. } => {
                self = BackendBuilder::Redis {
                    connection_string: Some(connection_string.to_string()),
                    mode,
                };
            }
            BackendBuilder::Tiered {
                l1_capacity,
                l2_mode,
                write_through,
                promote_on_hit,
                ..
            } => {
                self = BackendBuilder::Tiered {
                    l1_capacity,
                    l2_connection_string: Some(connection_string.to_string()),
                    l2_mode,
                    write_through,
                    promote_on_hit,
                };
            }
            _ => {}
        }
        self
    }

    /// Set the Redis mode
    ///
    /// # Arguments
    ///
    /// * `mode` - Redis mode (Standalone, Sentinel, Cluster)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn mode(mut self, mode: RedisMode) -> Self {
        match self {
            BackendBuilder::Redis {
                connection_string, ..
            } => {
                self = BackendBuilder::Redis {
                    connection_string,
                    mode,
                };
            }
            BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                write_through,
                promote_on_hit,
                ..
            } => {
                self = BackendBuilder::Tiered {
                    l1_capacity,
                    l2_connection_string,
                    l2_mode: mode,
                    write_through,
                    promote_on_hit,
                };
            }
            _ => {}
        }
        self
    }

    // Tiered backend configuration methods

    /// Set the L1 capacity for tiered backend
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries in L1 cache
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn l1_capacity(mut self, capacity: u64) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity: _,
            l2_connection_string,
            l2_mode,
            write_through,
            promote_on_hit,
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity: capacity,
                l2_connection_string,
                l2_mode,
                write_through,
                promote_on_hit,
            };
        }
        self
    }

    /// Set the L2 connection string for tiered backend
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn l2_connection_string(mut self, connection_string: &str) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity,
            l2_connection_string: _,
            l2_mode,
            write_through,
            promote_on_hit,
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string: Some(connection_string.to_string()),
                l2_mode,
                write_through,
                promote_on_hit,
            };
        }
        self
    }

    /// Enable or disable write-through for tiered backend
    ///
    /// When enabled, writes go to both L1 and L2 simultaneously.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable write-through
    ///
    /// # Returns
    ///
    /// Self for method chaining
    #[cfg(feature = "redis")]
    pub fn write_through(mut self, enabled: bool) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity,
            l2_connection_string,
            l2_mode,
            write_through: _,
            promote_on_hit,
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode,
                write_through: enabled,
                promote_on_hit,
            };
        }
        self
    }

    /// Enable or disable auto-promote for tiered backend
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
    #[cfg(feature = "redis")]
    pub fn promote_on_hit(mut self, enabled: bool) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity,
            l2_connection_string,
            l2_mode,
            write_through,
            promote_on_hit: _,
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode,
                write_through,
                promote_on_hit: enabled,
            };
        }
        self
    }

    /// Build backend
    ///
    /// # Returns
    ///
    /// Configured backend instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if configuration is invalid or connection fails
    pub async fn build(self) -> Result<Arc<dyn CacheBackend>> {
        match self {
            BackendBuilder::Memory { capacity, ttl } => {
                let builder = MemoryBackend::builder().capacity(capacity);
                let backend = if let Some(ttl) = ttl {
                    builder.ttl(ttl).build()
                } else {
                    builder.build()
                };
                Ok(Arc::new(backend))
            }
            #[cfg(feature = "redis")]
            BackendBuilder::Redis {
                connection_string,
                mode,
            } => {
                let connection_string = connection_string.ok_or_else(|| {
                    crate::error::CacheError::ConfigError(
                        "Redis connection string is required".to_string(),
                    )
                })?;

                let builder = RedisBackend::builder()
                    .connection_string(&connection_string)
                    .mode(mode);
                let backend = builder.build().await?;
                Ok(Arc::new(backend))
            }
            #[cfg(feature = "redis")]
            BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode,
                write_through: _,
                promote_on_hit: _,
            } => {
                let l2_connection_string = l2_connection_string.ok_or_else(|| {
                    crate::error::CacheError::ConfigError(
                        "L2 connection string is required for tiered backend".to_string(),
                    )
                })?;

                // Create L1 (Memory) backend
                let l1_backend = MemoryBackend::builder().capacity(l1_capacity).build();

                // Create L2 (Redis) backend
                let l2_builder = RedisBackend::builder()
                    .connection_string(&l2_connection_string)
                    .mode(l2_mode);
                let l2_backend = l2_builder.build().await?;

                // Create tiered backend
                let tiered_backend = TieredBackend::new(Arc::new(l1_backend), Arc::new(l2_backend));
                Ok(Arc::new(tiered_backend))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_builder_memory() {
        let backend = BackendBuilder::memory()
            .capacity(1000)
            .build()
            .await
            .unwrap();
        assert!(backend.health_check().await.unwrap());
    }

    #[tokio::test]
    #[ignore] // Requires running Redis server
    #[cfg(feature = "redis")]
    async fn test_backend_builder_redis() {
        let backend = BackendBuilder::redis()
            .connection_string("redis://localhost:6379")
            .mode(RedisMode::Standalone)
            .build()
            .await
            .unwrap();
        assert!(backend.health_check().await.unwrap());
    }

    #[tokio::test]
    #[ignore] // Requires running Redis server
    #[cfg(feature = "redis")]
    async fn test_backend_builder_tiered() {
        let backend = BackendBuilder::tiered()
            .l1_capacity(1000)
            .l2_connection_string("redis://localhost:6379")
            .build()
            .await
            .unwrap();
        assert!(backend.health_check().await.unwrap());
    }
}
