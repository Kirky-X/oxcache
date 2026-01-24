//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Backend builder for creating cache backends

use crate::backend::{CacheBackend, TieredBackend};
use crate::backend::client::{RedisBackend, RedisMode, MokaMemoryBackend as MemoryBackend};
use crate::error::Result;
use std::sync::Arc;

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
/// // Create tiered backend
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
    Redis {
        connection_string: Option<String>,
        mode: RedisMode,
    },
    /// Tiered backend configuration
    Tiered {
        l1_capacity: u64,
        l2_connection_string: Option<String>,
        l2_mode: RedisMode,
        auto_promote: bool,
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
    pub fn redis() -> Self {
        BackendBuilder::Redis {
            connection_string: None,
            mode: RedisMode::Standalone,
        }
    }

    /// Create a tiered backend builder
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
    ///     .auto_promote(true)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn tiered() -> Self {
        BackendBuilder::Tiered {
            l1_capacity: 10000,
            l2_connection_string: None,
            l2_mode: RedisMode::Standalone,
            auto_promote: true,
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
                auto_promote,
                ..
            } => {
                self = BackendBuilder::Tiered {
                    l1_capacity,
                    l2_connection_string: Some(connection_string.to_string()),
                    l2_mode,
                    auto_promote,
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
                auto_promote,
                ..
            } => {
                self = BackendBuilder::Tiered {
                    l1_capacity,
                    l2_connection_string,
                    l2_mode: mode,
                    auto_promote,
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
    /// * `capacity` - Maximum number of entries in L1
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn l1_capacity(mut self, capacity: u64) -> Self {
        if let BackendBuilder::Tiered {
            l2_connection_string,
            l2_mode,
            auto_promote,
            ..
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity: capacity,
                l2_connection_string,
                l2_mode,
                auto_promote,
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
    pub fn l2_connection_string(mut self, connection_string: &str) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity,
            l2_mode,
            auto_promote,
            ..
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string: Some(connection_string.to_string()),
                l2_mode,
                auto_promote,
            };
        }
        self
    }

    /// Set the L2 Redis mode for tiered backend
    ///
    /// # Arguments
    ///
    /// * `mode` - Redis mode (Standalone, Sentinel, Cluster)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn l2_mode(mut self, mode: RedisMode) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity,
            l2_connection_string,
            auto_promote,
            ..
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode: mode,
                auto_promote,
            };
        }
        self
    }

    /// Enable or disable auto-promote for tiered backend
    ///
    /// # Arguments
    ///
    /// * `auto_promote` - Whether to enable auto-promote
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn auto_promote(mut self, auto_promote: bool) -> Self {
        if let BackendBuilder::Tiered {
            l1_capacity,
            l2_connection_string,
            l2_mode,
            ..
        } = self
        {
            self = BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode,
                auto_promote,
            };
        }
        self
    }

    /// Build the backend
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
            BackendBuilder::Tiered {
                l1_capacity,
                l2_connection_string,
                l2_mode,
                auto_promote,
            } => {
                let l2_connection_string = l2_connection_string.ok_or_else(|| {
                    crate::error::CacheError::ConfigError(
                        "L2 connection string is required".to_string(),
                    )
                })?;

                let l1 = MemoryBackend::builder().capacity(l1_capacity).build();
                let l2 = RedisBackend::builder()
                    .connection_string(&l2_connection_string)
                    .mode(l2_mode)
                    .build()
                    .await?;

                let backend = TieredBackend::builder()
                    .l1(l1)
                    .l2(l2)
                    .auto_promote(auto_promote)
                    .build()?;
                Ok(Arc::new(backend))
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
    async fn test_backend_builder_tiered() {
        // Use memory backend as mock L2 for testing
        let l1 = MemoryBackend::new();
        let l2 = MemoryBackend::new();
        let backend = TieredBackend::builder()
            .l1(l1)
            .l2(l2)
            .auto_promote(true)
            .build()
            .unwrap();
        assert!(backend.health_check().await.unwrap());
    }
}
