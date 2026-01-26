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
    #[cfg(feature = "redis")]
    Redis {
        connection_string: Option<String>,
        mode: RedisMode,
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
        if let BackendBuilder::Redis { mode, .. } = self {
            self = BackendBuilder::Redis {
                connection_string: Some(connection_string.to_string()),
                mode,
            };
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
        if let BackendBuilder::Redis {
            connection_string, ..
        } = self
        {
            self = BackendBuilder::Redis {
                connection_string,
                mode,
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
}
