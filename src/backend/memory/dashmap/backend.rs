//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! DashMap backend implementation for high-performance concurrent in-memory caching

use crate::backend::interface::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::backend::score::{BackendScore, Scores};
use crate::error::Result;
use crate::impl_backend_builder;
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Entry with metadata for TTL tracking
#[derive(Clone, Debug)]
pub(crate) struct CacheEntry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

/// DashMap cache backend
///
/// This backend uses DashMap for high-performance concurrent in-memory caching.
/// Unlike Moka, DashMap provides lock-free concurrent access but requires
/// manual TTL management.
///
/// # Features
///
/// - **High Concurrency**: Lock-free design for minimal contention
/// - **No Eviction**: Unlike Moka, DashMap doesn't auto-evict entries
/// - **Manual TTL**: TTL must be checked on access
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::memory::dashmap::DashMapMemoryBackend;
/// use std::time::Duration;
///
/// // Create with default settings
/// let backend = DashMapMemoryBackend::new();
///
/// // Create with custom capacity and TTL
/// let backend = DashMapMemoryBackend::builder()
///     .capacity(10000)
///     .default_ttl(Duration::from_secs(3600))
///     .build();
/// ```
#[derive(Clone)]
pub struct DashMapMemoryBackend {
    /// The main cache storage
    cache: Arc<DashMap<String, CacheEntry>>,
    /// TTL tracking for expiration
    ttl_map: Arc<DashMap<String, Instant>>,
    /// Statistics counters
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
    /// Maximum capacity
    capacity: usize,
    /// Default TTL for new entries
    default_ttl: Option<Duration>,
}

impl_backend_builder!(DashMapMemoryBackend, DashMapBackendBuilder);

impl DashMapMemoryBackend {
    /// Remove oldest entries when at capacity
    fn evict_if_full(&self) {
        // Find the entry with the oldest (soonest) expiration time
        if let Some(key) = self.ttl_map.iter().min_by_key(|r| *r.value()).map(|r| r.key().clone()) {
            self.cache.remove(&key);
            self.ttl_map.remove(&key);
        }
    }

    /// Get the current capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the current entry count
    pub fn entry_count(&self) -> usize {
        self.cache.len()
    }

    /// Get the hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Get the underlying cache
    pub(crate) fn cache(&self) -> &DashMap<String, CacheEntry> {
        &self.cache
    }
}

impl Default for DashMapMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DashMapMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashMapMemoryBackend")
            .field("capacity", &self.capacity)
            .field("entry_count", &self.cache.len())
            .field("hit_rate", &self.hit_rate())
            .finish()
    }
}

#[async_trait]
impl CacheReader for DashMapMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let now = Instant::now();

        // Use atomic operations to reduce race conditions
        let result = self.cache.get(key).map(|entry_ref| {
            let entry = entry_ref.value();

            // Check expiration atomically
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    // Atomically remove expired entry
                    drop(entry_ref);
                    self.cache.remove(key);
                    self.ttl_map.remove(key);
                    self.misses.fetch_add(1, Ordering::SeqCst);
                    return None;
                }
            }

            self.hits.fetch_add(1, Ordering::SeqCst);
            Some(entry.value.clone())
        });

        if result.is_none() {
            self.misses.fetch_add(1, Ordering::SeqCst);
        }

        Ok(result.flatten())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    // Entry expired, remove it
                    drop(entry_ref);
                    self.cache.remove(key);
                    self.ttl_map.remove(key);
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at {
                if expires_at > now {
                    return Ok(Some(expires_at.duration_since(now)));
                } else {
                    // Entry expired
                    drop(entry_ref);
                    self.cache.remove(key);
                    self.ttl_map.remove(key);
                    return Ok(None);
                }
            }
            Ok(None) // No expiration set
        } else {
            Ok(None) // Key doesn't exist
        }
    }

    async fn len(&self) -> Result<u64> {
        Ok(self.cache.len() as u64)
    }

    async fn is_empty(&self) -> Result<bool> {
        Ok(self.cache.is_empty())
    }

    async fn capacity(&self) -> Result<u64> {
        Ok(self.capacity as u64)
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "dashmap".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert("entry_count".to_string(), self.cache.len().to_string());
        stats.insert("hits".to_string(), self.hits.load(Ordering::Relaxed).to_string());
        stats.insert("misses".to_string(), self.misses.load(Ordering::Relaxed).to_string());
        stats.insert("hit_rate".to_string(), format!("{:.4}", self.hit_rate()));
        Ok(stats)
    }
}

#[async_trait]
impl CacheWriter for DashMapMemoryBackend {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let now = Instant::now();
        let expires_at = ttl.or(self.default_ttl).map(|duration| now + duration);

        let entry = CacheEntry { value, expires_at };

        // Insert the entry
        self.cache.insert(key.to_string(), entry.clone());

        // Track TTL if applicable
        if let Some(expiration) = entry.expires_at {
            self.ttl_map.insert(key.to_string(), expiration);
        }

        // Evict if at capacity
        if self.cache.len() > self.capacity {
            self.evict_if_full();
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.cache.remove(key);
        self.ttl_map.remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.cache.clear();
        self.ttl_map.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let now = Instant::now();
        let new_expires_at = now + ttl;

        if let Some(mut entry_ref) = self.cache.get_mut(key) {
            entry_ref.expires_at = Some(new_expires_at);
            self.ttl_map.insert(key.to_string(), new_expires_at);
            Ok(true)
        } else {
            Ok(false) // Key doesn't exist
        }
    }
}

#[async_trait]
impl CacheConnector for DashMapMemoryBackend {
    async fn health_check(&self) -> Result<()> {
        // DashMap is always healthy as in-memory
        Ok(())
    }

    async fn shutdown(&self) {
        self.cache.clear();
        self.ttl_map.clear();
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::DashMap
    }
}

// CacheBackend is automatically implemented via blanket implementation

impl BackendScore for DashMapMemoryBackend {
    fn score(&self) -> u8 {
        Scores::DASHMAP
    }

    fn is_persistent(&self) -> bool {
        false
    }

    fn backend_name(&self) -> &'static str {
        "dashmap"
    }
}

/// Builder for DashMapMemoryBackend
#[derive(Debug, Clone, Default)]
pub struct DashMapBackendBuilder {
    capacity: usize,
    default_ttl: Option<Duration>,
}

impl DashMapBackendBuilder {
    /// Set the maximum number of entries
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the default TTL for new entries
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Build the DashMap backend
    pub fn build(self) -> DashMapMemoryBackend {
        // Use a reasonable default capacity if not set
        let capacity = if self.capacity > 0 {
            self.capacity
        } else {
            10_000 // Default capacity of 10,000 entries
        };

        DashMapMemoryBackend {
            cache: Arc::new(DashMap::new()),
            ttl_map: Arc::new(DashMap::new()),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
            capacity,
            default_ttl: self.default_ttl,
        }
    }
}

/// Convenience function to create a DashMap memory backend
pub fn dashmap_memory() -> DashMapMemoryBackend {
    DashMapMemoryBackend::new()
}

/// Convenience function to create a DashMap memory backend with capacity
pub fn dashmap_memory_with_capacity(capacity: usize) -> DashMapMemoryBackend {
    DashMapMemoryBackend::builder().capacity(capacity).build()
}

/// Convenience function to create a DashMap memory backend with capacity and TTL
pub fn dashmap_memory_with_capacity_and_ttl(capacity: usize, ttl: Duration) -> DashMapMemoryBackend {
    DashMapMemoryBackend::builder()
        .capacity(capacity)
        .default_ttl(ttl)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashmap_backend_builder() {
        let backend = DashMapMemoryBackend::builder()
            .capacity(1000)
            .default_ttl(Duration::from_secs(3600))
            .build();

        assert_eq!(backend.capacity(), 1000);
    }

    #[test]
    fn test_dashmap_backend_default() {
        let backend = DashMapMemoryBackend::default();
        // Default capacity should be reasonable
        assert!(backend.capacity() > 0);
    }

    #[tokio::test]
    async fn test_dashmap_basic_operations() {
        let backend = DashMapMemoryBackend::new();

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

        // Test health check
        backend.health_check().await.unwrap();

        // Test stats
        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"dashmap".to_string()));
        assert_eq!(stats.get("capacity"), Some(&backend.capacity().to_string()));
    }

    #[tokio::test]
    async fn test_dashmap_ttl() {
        let backend = DashMapMemoryBackend::new();

        // Set with TTL
        backend
            .set("key1", b"value1".to_vec(), Some(Duration::from_millis(100)))
            .await
            .unwrap();

        // Should exist immediately
        assert!(backend.exists("key1").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[test]
    fn test_convenience_functions() {
        let backend1 = dashmap_memory();
        let backend2 = dashmap_memory_with_capacity(1000);
        let backend3 = dashmap_memory_with_capacity_and_ttl(1000, Duration::from_secs(3600));

        assert!(backend1.capacity() > 0);
        assert_eq!(backend2.capacity(), 1000);
        assert_eq!(backend3.capacity(), 1000);
    }
}
