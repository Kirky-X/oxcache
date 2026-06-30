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
        if let Some(key) = self
            .cache
            .iter()
            .filter_map(|r| {
                let entry = r.value();
                entry.expires_at.map(|exp| (r.key().clone(), exp))
            })
            .min_by_key(|(_, exp)| *exp)
            .map(|(key, _)| key)
        {
            self.cache.remove(&key);
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
                    // Entry expired — cannot remove while holding Ref, just return None
                    // The expired entry will be cleaned up on next access or eviction
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
                    return Ok(None);
                }
            }
            Ok(None)
        } else {
            Ok(None)
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
        self.cache.insert(key.to_string(), entry);

        // Evict if at capacity
        if self.cache.len() > self.capacity {
            self.evict_if_full();
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.cache.remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.cache.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let now = Instant::now();
        let new_expires_at = now + ttl;

        if let Some(mut entry_ref) = self.cache.get_mut(key) {
            entry_ref.expires_at = Some(new_expires_at);
            Ok(true)
        } else {
            Ok(false)
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
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::DashMap
    }
}

// ============================================================================
// Synchronous trait implementations (任务组 7)
// ============================================================================
//
// DashMap 本身是同步的，sync impl 直接复用 async 方法逻辑（去掉 async/.await），
// 无需像 moka 那样通过 `block_on` 桥接。实现使用全限定路径
// (`impl crate::backend::interface::SyncCacheReader for DashMapMemoryBackend`)，
// 避免将 sync trait 名导入本模块作用域后，经 `mod tests` 的 `use super::*`
// 与同名 async trait 方法（如 `get`）产生歧义。

impl crate::backend::interface::SyncCacheReader for DashMapMemoryBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let now = Instant::now();

        // Use atomic operations to reduce race conditions
        let result = self.cache.get(key).map(|entry_ref| {
            let entry = entry_ref.value();

            // Check expiration atomically
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    // Entry expired — cannot remove while holding Ref, just return None
                    // The expired entry will be cleaned up on next access or eviction
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

    fn exists(&self, key: &str) -> Result<bool> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at {
                if expires_at > now {
                    return Ok(Some(expires_at.duration_since(now)));
                } else {
                    return Ok(None);
                }
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    fn len(&self) -> Result<u64> {
        Ok(self.cache.len() as u64)
    }

    fn capacity(&self) -> Result<u64> {
        Ok(self.capacity as u64)
    }

    fn stats(&self) -> Result<HashMap<String, String>> {
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

impl crate::backend::interface::SyncCacheWriter for DashMapMemoryBackend {
    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let now = Instant::now();
        let expires_at = ttl.or(self.default_ttl).map(|duration| now + duration);

        let entry = CacheEntry { value, expires_at };

        // Insert the entry
        self.cache.insert(key.to_string(), entry);

        // Evict if at capacity
        if self.cache.len() > self.capacity {
            self.evict_if_full();
        }

        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.cache.remove(key);
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        self.cache.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let now = Instant::now();
        let new_expires_at = now + ttl;

        if let Some(mut entry_ref) = self.cache.get_mut(key) {
            entry_ref.expires_at = Some(new_expires_at);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl crate::backend::interface::SyncCacheConnector for DashMapMemoryBackend {
    fn health_check(&self) -> Result<()> {
        // DashMap is always healthy as in-memory
        Ok(())
    }

    fn shutdown(&self) {
        self.cache.clear();
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

    // ========================================================================
    // Synchronous trait hierarchy tests (任务组 7)
    //
    // 隔离在嵌套 `mod sync_tests` 内：sync trait 的 import 仅在此模块可见，
    // 避免与父模块 `mod tests` 中 async `CacheReader::get` 等同名方法产生
    // 歧义。方法调用通过 trait object (`&dyn SyncCacheReader` 等) 消歧。
    // ========================================================================
    mod sync_tests {
        use super::DashMapMemoryBackend;
        use crate::backend::interface::{BackendKind, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
        use std::time::Duration;

        #[test]
        fn test_dashmap_sync_get_set_basic() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("key1", b"value1".to_vec(), None).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert_eq!(reader.get("key1").unwrap(), Some(b"value1".to_vec()));
            assert!(reader.exists("key1").unwrap());
            assert!(!reader.exists("key2").unwrap());
            assert!(reader.capacity().unwrap() > 0);
            assert_eq!(reader.len().unwrap(), 1);
            assert!(!reader.is_empty().unwrap());

            let stats = reader.stats().unwrap();
            assert_eq!(stats.get("type"), Some(&"dashmap".to_string()));
        }

        #[test]
        fn test_dashmap_sync_set_with_ttl() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k", b"v".to_vec(), Some(Duration::from_millis(50))).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            // 立即可读
            assert_eq!(reader.get("k").unwrap(), Some(b"v".to_vec()));

            // DashMap 在访问时按 expires_at 校验，无后台驱逐；等待过期后读应返回 None
            std::thread::sleep(Duration::from_millis(120));
            assert_eq!(reader.get("k").unwrap(), None);
            assert!(!reader.exists("k").unwrap());
        }

        #[test]
        fn test_dashmap_sync_expire() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k", b"v".to_vec(), Some(Duration::from_secs(60))).unwrap();

            // expire 已存在 key → true，TTL 延长至 120s
            let ok = writer.expire("k", Duration::from_secs(120)).unwrap();
            assert!(ok, "expire on existing key should return true");

            let reader: &dyn SyncCacheReader = &backend;
            let new_ttl = reader.ttl("k").unwrap().expect("ttl should be Some after expire");
            assert!(
                new_ttl > Duration::from_secs(118),
                "new_ttl={} should be > 118s",
                new_ttl.as_secs_f64()
            );

            // expire 不存在 key → false
            let ok = writer.expire("missing", Duration::from_secs(10)).unwrap();
            assert!(!ok, "expire on missing key should return false");

            // shrink TTL 后应过期
            let ok = writer.expire("k", Duration::from_millis(50)).unwrap();
            assert!(ok, "expire shrink on existing key should return true");
            std::thread::sleep(Duration::from_millis(120));
            assert_eq!(reader.get("k").unwrap(), None);
        }

        #[test]
        fn test_dashmap_sync_ttl_query() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k", b"v".to_vec(), Some(Duration::from_secs(60))).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            let ttl = reader.ttl("k").unwrap().expect("ttl should be Some for TTL'd key");
            assert!(
                ttl > Duration::from_secs(58),
                "ttl={} should be > 58s",
                ttl.as_secs_f64()
            );
            assert!(
                ttl <= Duration::from_secs(60),
                "ttl={} should be <= 60s",
                ttl.as_secs_f64()
            );

            // 无 TTL 的 key 返回 None
            writer.set("no_ttl", b"v".to_vec(), None).unwrap();
            assert_eq!(reader.ttl("no_ttl").unwrap(), None);
            // 不存在的 key 返回 None
            assert_eq!(reader.ttl("missing").unwrap(), None);

            // connector: health_check / shutdown / backend_kind
            let connector: &dyn SyncCacheConnector = &backend;
            connector.health_check().unwrap();
            assert_eq!(connector.backend_kind(), BackendKind::DashMap);
            connector.shutdown();
        }
    }
}
