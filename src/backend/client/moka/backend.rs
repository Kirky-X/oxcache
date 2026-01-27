//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Moka-based memory backend implementation

use crate::backend::interface::CacheBackend;
use crate::error::Result;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Moka-based memory backend
///
/// This backend uses Moka's high-performance in-memory cache with
/// LRU/TinyLFU eviction policies and built-in TTL support.
#[derive(Clone)]
pub struct MokaMemoryBackend {
    cache: Arc<moka::future::Cache<String, Vec<u8>>>,
    capacity: u64,
}

impl MokaMemoryBackend {
    /// Create a new Moka memory backend
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a builder for Moka backend
    pub fn builder() -> MokaMemoryBackendBuilder {
        MokaMemoryBackendBuilder::default()
    }

    /// Get the capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get the entry count
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    /// Get the underlying cache
    pub fn cache(&self) -> &moka::future::Cache<String, Vec<u8>> {
        &self.cache
    }
}

impl Default for MokaMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MokaMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaMemoryBackend")
            .field("capacity", &self.capacity)
            .field("entry_count", &self.cache.entry_count())
            .finish()
    }
}

#[async_trait]
impl CacheBackend for MokaMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.cache.get(key).await)
    }

    async fn set(&self, key: &str, value: Vec<u8>, _ttl: Option<Duration>) -> Result<()> {
        // Moka doesn't support per-entry TTL insertion
        // TTL is set at cache creation time
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

    async fn ttl(&self, _key: &str) -> Result<Option<Duration>> {
        // Moka doesn't expose per-entry TTL information
        Ok(None)
    }

    async fn expire(&self, _key: &str, _ttl: Duration) -> Result<bool> {
        // Moka doesn't support per-entry TTL updates after insertion
        Ok(false)
    }

    async fn health_check(&self) -> Result<bool> {
        // Moka is always healthy as it's in-memory
        Ok(true)
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "moka".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert(
            "entry_count".to_string(),
            self.cache.entry_count().to_string(),
        );
        Ok(stats)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Builder for MokaMemoryBackend
#[derive(Default)]
pub struct MokaMemoryBackendBuilder {
    capacity: u64,
    ttl: Option<Duration>,
    time_to_idle: Option<Duration>,
}

impl MokaMemoryBackendBuilder {
    /// Set the maximum number of entries
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the time-to-live for entries
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the time-to-idle for entries
    pub fn time_to_idle(mut self, ttl: Duration) -> Self {
        self.time_to_idle = Some(ttl);
        self
    }

    /// Build the Moka backend
    pub fn build(self) -> MokaMemoryBackend {
        // Use a reasonable default capacity if not set
        let capacity = if self.capacity > 0 {
            self.capacity
        } else {
            10_000 // Default capacity of 10,000 entries
        };

        let mut builder = moka::future::Cache::builder().max_capacity(capacity);

        if let Some(ttl) = self.ttl {
            builder = builder.time_to_live(ttl);
        }

        if let Some(tti) = self.time_to_idle {
            builder = builder.time_to_idle(tti);
        }

        let cache = Arc::new(builder.build());

        MokaMemoryBackend { cache, capacity }
    }
}

/// Convenience function to create a Moka memory backend
pub fn moka_memory() -> MokaMemoryBackend {
    MokaMemoryBackend::new()
}

/// Convenience function to create a Moka memory backend with capacity
pub fn moka_memory_with_capacity(capacity: u64) -> MokaMemoryBackend {
    MokaMemoryBackend::builder().capacity(capacity).build()
}

/// Convenience function to create a Moka memory backend with capacity and TTL
pub fn moka_memory_with_capacity_and_ttl(capacity: u64, ttl: Duration) -> MokaMemoryBackend {
    MokaMemoryBackend::builder()
        .capacity(capacity)
        .ttl(ttl)
        .build()
}

/// Default memory backend (Moka-based)
pub fn default_memory_backend() -> MokaMemoryBackend {
    moka_memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moka_backend_builder() {
        let backend = MokaMemoryBackend::builder()
            .capacity(1000)
            .ttl(Duration::from_secs(3600))
            .time_to_idle(Duration::from_secs(1800))
            .build();

        assert_eq!(backend.capacity(), 1000);
    }

    #[test]
    fn test_moka_backend_default() {
        let backend = MokaMemoryBackend::default();
        // Default capacity should be reasonable
        assert!(backend.capacity() > 0);
    }

    #[tokio::test]
    async fn test_moka_basic_operations() {
        let backend = MokaMemoryBackend::new();

        // Set a value
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();

        // Use tokio::time::sleep to ensure async operations complete
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Get the value
        let result = backend.get("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Check exists
        let exists = backend.exists("key1").await.unwrap();
        assert!(exists);

        // Delete
        backend.delete("key1").await.unwrap();

        // Verify deletion
        let exists_after = backend.exists("key1").await.unwrap();
        assert!(!exists_after);
    }

    #[test]
    fn test_convenience_functions() {
        let backend1 = moka_memory();
        let backend2 = moka_memory_with_capacity(1000);
        let backend3 = moka_memory_with_capacity_and_ttl(1000, Duration::from_secs(3600));

        assert!(backend1.capacity() > 0);
        assert_eq!(backend2.capacity(), 1000);
        assert_eq!(backend3.capacity(), 1000);
    }
}
