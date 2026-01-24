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
    /// let cache: Cache<String, User> = Cache::builder()
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
}
