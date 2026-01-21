//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Memory backend implementation based on Moka cache

use super::new_backend::CacheBackend;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use moka::future::Cache;
use std::collections::HashMap;
use std::time::Duration;

/// Memory cache backend using Moka
///
/// This backend uses Moka's high-performance in-memory cache with
/// LRU/TinyLFU eviction policies.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::MemoryBackend;
/// use std::time::Duration;
///
/// // Create with default settings
/// let backend = MemoryBackend::new();
///
/// // Create with custom capacity and TTL
/// let backend = MemoryBackend::builder()
///     .capacity(10000)
///     .ttl(Duration::from_secs(3600))
///     .build();
/// ```
#[derive(Clone)]
pub struct MemoryBackend {
    cache: Cache<String, Vec<u8>>,
}

impl MemoryBackend {
    /// Create a new memory backend with default settings
    ///
    /// # Default Settings
    ///
    /// - Capacity: 10,000 entries
    /// - TTL: None (no expiration)
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a new builder for configuring the memory backend
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::backend::MemoryBackend;
    /// use std::time::Duration;
    ///
    /// let backend = MemoryBackend::builder()
    ///     .capacity(10000)
    ///     .ttl(Duration::from_secs(3600))
    ///     .build();
    /// ```
    pub fn builder() -> MemoryBackendBuilder {
        MemoryBackendBuilder::default()
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for MemoryBackend
pub struct MemoryBackendBuilder {
    capacity: u64,
    ttl: Option<Duration>,
    time_to_idle: Option<Duration>,
}

impl Default for MemoryBackendBuilder {
    fn default() -> Self {
        Self {
            capacity: 10_000, // Default capacity
            ttl: None,
            time_to_idle: None,
        }
    }
}

impl MemoryBackendBuilder {
    /// Set the maximum number of entries
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries (default: 10,000)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the time-to-live for entries
    ///
    /// Entries will expire after this duration from creation.
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live duration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the time-to-idle for entries
    ///
    /// Entries will expire after this duration from last access.
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-idle duration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn time_to_idle(mut self, ttl: Duration) -> Self {
        self.time_to_idle = Some(ttl);
        self
    }

    /// Build the memory backend
    ///
    /// # Returns
    ///
    /// Configured MemoryBackend instance
    pub fn build(self) -> MemoryBackend {
        let mut builder = Cache::builder().max_capacity(self.capacity);

        if let Some(ttl) = self.ttl {
            builder = builder.time_to_live(ttl);
        }

        if let Some(tti) = self.time_to_idle {
            builder = builder.time_to_idle(tti);
        }

        MemoryBackend {
            cache: builder.build(),
        }
    }
}

#[async_trait]
impl CacheBackend for MemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.cache.get(key).await)
    }

    async fn set(&self, key: &str, value: Vec<u8>, _ttl: Option<Duration>) -> Result<()> {
        // Moka doesn't support per-entry TTL insertion
        // We just insert the value; TTL is set at cache creation time
        self.cache.insert(key.to_string(), value).await;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.cache.invalidate(key).await;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.cache.contains_key(key))
    }

    async fn clear(&self) -> Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        // Moka doesn't expose TTL directly, so we return None
        // This is a limitation of the Moka API
        if self.cache.contains_key(key) {
            Ok(None)
        } else {
            Err(CacheError::NotFound(key.to_string()))
        }
    }

    async fn expire(&self, _key: &str, _ttl: Duration) -> Result<bool> {
        // Moka doesn't support updating TTL on existing entries
        // Return false to indicate not supported
        Ok(false)
    }

    async fn health_check(&self) -> Result<bool> {
        // Memory backend is always healthy
        Ok(true)
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "memory".to_string());
        stats.insert(
            "entry_count".to_string(),
            self.cache.entry_count().to_string(),
        );
        stats.insert(
            "weighted_size".to_string(),
            self.cache.weighted_size().to_string(),
        );
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_memory_backend_basic() {
        let backend = MemoryBackend::new();

        // Test set and get
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_ttl() {
        let backend = MemoryBackend::builder()
            .ttl(Duration::from_millis(100))
            .build();

        backend.set("key1", b"value1".to_vec(), None).await.unwrap();

        // Value should exist immediately
        assert!(backend.exists("key1").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Value should be expired
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_capacity() {
        let backend = MemoryBackend::builder().capacity(2).build();

        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        backend.set("key2", b"value2".to_vec(), None).await.unwrap();
        backend.set("key3", b"value3".to_vec(), None).await.unwrap();

        // With capacity of 2, one key should be evicted
        let count = backend.stats().await.unwrap();
        // The entry count should be at most 2
        let entry_count: u64 = count.get("entry_count").unwrap().parse().unwrap_or(0);
        assert!(entry_count <= 2);
    }

    #[tokio::test]
    async fn test_memory_backend_clear() {
        let backend = MemoryBackend::new();

        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        backend.set("key2", b"value2".to_vec(), None).await.unwrap();

        backend.clear().await.unwrap();

        assert!(!backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_stats() {
        let backend = MemoryBackend::new();

        backend.set("key1", b"value1".to_vec(), None).await.unwrap();

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"memory".to_string()));
        assert!(stats.contains_key("entry_count"));
    }
}
