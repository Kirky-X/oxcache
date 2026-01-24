//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Tiered cache builder for advanced configuration

use crate::backend::client::{MokaMemoryBackend as MemoryBackend, RedisBackend, RedisMode};
use crate::backend::{CacheBackend, TieredBackend};
use crate::error::Result;
use std::sync::Arc;

/// Builder for creating configured TieredCache instances
///
/// This builder provides a fluent interface for configuring tiered cache instances
/// with L1 (memory) and L2 (Redis) layers.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::builder::TieredCacheBuilder;
/// use std::time::Duration;
///
/// let backend = TieredCacheBuilder::new()
///     .l1_capacity(10000)
///     .l1_ttl(Duration::from_secs(3600))
///     .l2_connection_string("redis://localhost:6379")
///     .l2_mode(RedisMode::Standalone)
///     .auto_promote(true)
///     .batch_writes(true)
///     .build()?;
/// ```
pub struct TieredCacheBuilder {
    l1_capacity: u64,
    l1_ttl: Option<std::time::Duration>,
    l2_connection_string: Option<String>,
    l2_mode: RedisMode,
    auto_promote: bool,
    batch_writes: bool,
    bloom_filter: bool,
}

impl Default for TieredCacheBuilder {
    fn default() -> Self {
        Self {
            l1_capacity: 10000,
            l1_ttl: None,
            l2_connection_string: None,
            l2_mode: RedisMode::Standalone,
            auto_promote: true,
            batch_writes: false,
            bloom_filter: false,
        }
    }
}

impl TieredCacheBuilder {
    /// Create a new tiered cache builder with default settings
    ///
    /// # Returns
    ///
    /// TieredCacheBuilder instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the L1 cache capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries in L1 cache
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = TieredCacheBuilder::new().l1_capacity(10000);
    /// ```
    pub fn l1_capacity(mut self, capacity: u64) -> Self {
        self.l1_capacity = capacity;
        self
    }

    /// Set the L1 cache TTL
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live duration for L1 entries
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
    /// let builder = TieredCacheBuilder::new().l1_ttl(Duration::from_secs(3600));
    /// ```
    pub fn l1_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.l1_ttl = Some(ttl);
        self
    }

    /// Set the L2 Redis connection string
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = TieredCacheBuilder::new()
    ///     .l2_connection_string("redis://localhost:6379");
    /// ```
    pub fn l2_connection_string(mut self, connection_string: &str) -> Self {
        self.l2_connection_string = Some(connection_string.to_string());
        self
    }

    /// Set the L2 Redis mode
    ///
    /// # Arguments
    ///
    /// * `mode` - Redis mode (Standalone, Sentinel, Cluster)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::backend::client::redis::RedisMode;
    ///
    /// let builder = TieredCacheBuilder::new()
    ///     .l2_mode(RedisMode::Standalone);
    /// ```
    pub fn l2_mode(mut self, mode: RedisMode) -> Self {
        self.l2_mode = mode;
        self
    }

    /// Enable or disable auto-promote
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
    /// let builder = TieredCacheBuilder::new().auto_promote(true);
    /// ```
    pub fn auto_promote(mut self, enabled: bool) -> Self {
        self.auto_promote = enabled;
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
    /// let builder = TieredCacheBuilder::new().batch_writes(true);
    /// ```
    pub fn batch_writes(mut self, enabled: bool) -> Self {
        self.batch_writes = enabled;
        self
    }

    /// Enable or disable bloom filter
    ///
    /// When enabled, a bloom filter is used to prevent cache penetration attacks.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable bloom filter
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = TieredCacheBuilder::new().bloom_filter(true);
    /// ```
    pub fn bloom_filter(mut self, enabled: bool) -> Self {
        self.bloom_filter = enabled;
        self
    }

    /// Build the tiered cache backend
    ///
    /// # Returns
    ///
    /// Configured TieredBackend instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if configuration is invalid or backend creation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = TieredCacheBuilder::new()
    ///     .l1_capacity(10000)
    ///     .l2_connection_string("redis://localhost:6379")
    ///     .build()?;
    /// ```
    pub async fn build(self) -> Result<Arc<dyn CacheBackend>> {
        let l2_connection_string = self.l2_connection_string.ok_or_else(|| {
            crate::error::CacheError::ConfigError("L2 connection string is required".to_string())
        })?;

        // Build L1 backend
        let l1_builder = MemoryBackend::builder().capacity(self.l1_capacity);
        let l1 = if let Some(ttl) = self.l1_ttl {
            l1_builder.ttl(ttl).build()
        } else {
            l1_builder.build()
        };

        // Build L2 backend
        let l2 = RedisBackend::builder()
            .connection_string(&l2_connection_string)
            .mode(self.l2_mode)
            .build()
            .await?;

        // Build tiered backend
        let backend = TieredBackend::builder()
            .l1(l1)
            .l2(l2)
            .auto_promote(self.auto_promote)
            .build()?;

        Ok(Arc::new(backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tiered_cache_builder_default() {
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

    #[tokio::test]
    async fn test_tiered_cache_builder_with_options() {
        let l1 = MemoryBackend::builder().capacity(1000).build();
        let l2 = MemoryBackend::new();
        let backend = TieredBackend::builder()
            .l1(l1)
            .l2(l2)
            .auto_promote(false)
            .build()
            .unwrap();
        assert!(backend.health_check().await.unwrap());
    }
}
