// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! [`BloomFilterBackend`] — a `CacheBackend` decorator that wraps an inner
//! backend with a [`BloomFilter`] for negative query filtering.
//!
//! On `get`, the Bloom filter is consulted first: if it says the key is
//! absent, `Ok(None)` is returned without touching the inner backend. If the
//! filter says the key may be present, the inner backend's `get` is called.
//! When the inner backend returns `None` (e.g. TTL expiry) the Bloom filter is
//! left untouched — Bloom filters do not support deletion.
//!
//! `set` inserts the key into the Bloom filter before delegating to the inner
//! backend. `delete` only delegates to the inner backend (the Bloom filter is
//! not modified). `clear` clears both. All TTL operations (`set` ttl, `ttl`,
//! `expire`) pass through to the inner backend unchanged.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;

use crate::backend::{
    BackendKind, BackendScore, CacheBackend, CacheConnector, CacheReader, CacheWriter, SyncCacheBackend,
    SyncCacheConnector, SyncCacheReader, SyncCacheWriter,
};
use crate::error::{OxCacheError, OxCacheResult};

use super::BloomFilter;

/// `CacheBackend` decorator wrapping an inner backend `B` with a Bloom filter
/// for negative query filtering.
///
/// The Bloom filter is shared via `Arc<RwLock<>>` inside [`BloomFilter`], so
/// cloning the backend (or the filter) shares state.
pub struct BloomFilterBackend<B: CacheBackend> {
    inner: B,
    bloom: BloomFilter,
}

impl<B: CacheBackend> BloomFilterBackend<B> {
    /// Create a decorator over `inner` with the default Bloom filter
    /// configuration (capacity `100_000`, false positive rate `0.01`).
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            bloom: BloomFilter::new(100_000, 0.01),
        }
    }

    /// Create a decorator over `inner` with an explicit Bloom filter
    /// configuration.
    pub fn with_capacity_and_rate(inner: B, capacity: usize, false_positive_rate: f64) -> Self {
        Self {
            inner,
            bloom: BloomFilter::new(capacity, false_positive_rate),
        }
    }

    /// Start a builder for configurable construction.
    pub fn builder() -> BloomFilterBackendBuilder<B> {
        BloomFilterBackendBuilder {
            capacity: 100_000,
            false_positive_rate: 0.01,
            inner: None,
        }
    }

    /// Borrow the inner backend.
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// Borrow the Bloom filter.
    pub fn bloom(&self) -> &BloomFilter {
        &self.bloom
    }
}

/// Builder for [`BloomFilterBackend`].
pub struct BloomFilterBackendBuilder<B: CacheBackend> {
    capacity: usize,
    false_positive_rate: f64,
    inner: Option<B>,
}

impl<B: CacheBackend> BloomFilterBackendBuilder<B> {
    /// Set the Bloom filter capacity (estimated max items).
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the target false positive rate (must be in `(0.0, 1.0)`).
    pub fn false_positive_rate(mut self, rate: f64) -> Self {
        self.false_positive_rate = rate;
        self
    }

    /// Set the inner backend to wrap.
    pub fn inner(mut self, inner: B) -> Self {
        self.inner = Some(inner);
        self
    }

    /// Build the decorator. Returns `Err` if no inner backend was set.
    pub fn build(self) -> OxCacheResult<BloomFilterBackend<B>> {
        let inner = self.inner.ok_or_else(|| {
            OxCacheError::InvalidInput("inner backend is required for BloomFilterBackend".to_string())
        })?;
        Ok(BloomFilterBackend {
            inner,
            bloom: BloomFilter::new(self.capacity, self.false_positive_rate),
        })
    }
}

#[async_trait]
impl<B: CacheBackend> CacheReader for BloomFilterBackend<B> {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        // BF first: if the filter says the key is absent, skip the inner
        // backend entirely (no false negatives).
        if !self.bloom.contains(key) {
            return Ok(None);
        }
        // BF says maybe present — delegate to inner. If inner returns None
        // (e.g. TTL expiry) the BF is left untouched: Bloom filters do not
        // support deletion, and the spec forbids mutating BF on a miss.
        self.inner.get(key).await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key).await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = self.inner.stats().await?;
        stats.insert("bloom_capacity".to_string(), self.bloom.capacity().to_string());
        stats.insert("bloom_load_factor".to_string(), self.bloom.load_factor().to_string());
        stats.insert(
            "bloom_false_positive_rate".to_string(),
            self.bloom.false_positive_rate().to_string(),
        );
        stats.insert("bloom_estimated_count".to_string(), self.bloom.len().to_string());
        Ok(stats)
    }
}

#[async_trait]
impl<B: CacheBackend> CacheWriter for BloomFilterBackend<B> {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
        // Record the key in the BF first, then delegate (with TTL) to inner.
        self.bloom.insert(key);
        self.inner.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        // Only delegate to inner; BF does not support removal.
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await?;
        self.bloom.clear();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }
}

#[async_trait]
impl<B: CacheBackend> CacheConnector for BloomFilterBackend<B> {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

impl<B: CacheBackend + BackendScore> BackendScore for BloomFilterBackend<B> {
    fn score(&self) -> u8 {
        self.inner.score()
    }

    fn is_persistent(&self) -> bool {
        self.inner.is_persistent()
    }

    fn backend_name(&self) -> &'static str {
        self.inner.backend_name()
    }
}

// ============================================================================
// Synchronous trait hierarchy (任务组 14)
// ============================================================================
//
// Mirror of the async `CacheBackend` impl. Only available when the inner
// backend `B` also supports sync access (`B: SyncCacheBackend`). UFCS is used
// throughout the bodies to disambiguate from the async trait methods that `B`
// also implements (both hierarchies define `get`/`set`/etc.).

impl<B: CacheBackend + SyncCacheBackend> SyncCacheReader for BloomFilterBackend<B> {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        // BF first: if the filter says the key is absent, skip the inner
        // backend entirely (no false negatives). Mirrors the async impl.
        if !self.bloom.contains(key) {
            return Ok(None);
        }
        // BF says maybe present — delegate to inner's sync get. If inner
        // returns None (e.g. TTL expiry) the BF is left untouched.
        SyncCacheReader::get(&self.inner, key)
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        // BF cannot confirm existence (only filter), so always delegate.
        SyncCacheReader::exists(&self.inner, key)
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        SyncCacheReader::ttl(&self.inner, key)
    }

    fn len(&self) -> OxCacheResult<u64> {
        SyncCacheReader::len(&self.inner)
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        SyncCacheReader::capacity(&self.inner)
    }

    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = SyncCacheReader::stats(&self.inner)?;
        stats.insert("bloom_capacity".to_string(), self.bloom.capacity().to_string());
        stats.insert("bloom_load_factor".to_string(), self.bloom.load_factor().to_string());
        stats.insert(
            "bloom_false_positive_rate".to_string(),
            self.bloom.false_positive_rate().to_string(),
        );
        stats.insert("bloom_estimated_count".to_string(), self.bloom.len().to_string());
        Ok(stats)
    }
}

impl<B: CacheBackend + SyncCacheBackend> SyncCacheWriter for BloomFilterBackend<B> {
    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
        // Record the key in the BF first, then delegate (with TTL) to inner.
        self.bloom.insert(key);
        SyncCacheWriter::set(&self.inner, key, value, ttl)
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        // Only delegate to inner; BF does not support removal.
        SyncCacheWriter::delete(&self.inner, key)
    }

    fn clear(&self) -> OxCacheResult<()> {
        SyncCacheWriter::clear(&self.inner)?;
        self.bloom.clear();
        Ok(())
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        SyncCacheWriter::expire(&self.inner, key, ttl)
    }
}

impl<B: CacheBackend + SyncCacheBackend> SyncCacheConnector for BloomFilterBackend<B> {
    fn health_check(&self) -> OxCacheResult<()> {
        SyncCacheConnector::health_check(&self.inner)
    }

    fn shutdown(&self) {
        SyncCacheConnector::shutdown(&self.inner)
    }

    fn backend_kind(&self) -> BackendKind {
        SyncCacheConnector::backend_kind(&self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Shared mock data store: key → (value, optional TTL), guarded by a Mutex.
    type MockDataStore = Arc<Mutex<HashMap<String, (Vec<u8>, Option<Duration>)>>>;

    // ========================================================================
    // SpyMock — test infrastructure recording all method calls.
    // Stores (value, ttl) pairs so TTL passthrough can be verified.
    // ========================================================================

    #[derive(Default, Debug)]
    struct CallLog {
        get_calls: Vec<String>,
        set_calls: Vec<(String, Vec<u8>, Option<Duration>)>,
        delete_calls: Vec<String>,
        clear_calls: u64,
        expire_calls: Vec<(String, Duration)>,
        ttl_calls: Vec<String>,
    }

    struct SpyMock {
        log: Arc<Mutex<CallLog>>,
        data: MockDataStore,
    }

    impl SpyMock {
        fn new() -> Self {
            Self {
                log: Arc::new(Mutex::new(CallLog::default())),
                data: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        /// Clone the call-log handle before the mock is moved into a backend.
        fn log_handle(&self) -> Arc<Mutex<CallLog>> {
            Arc::clone(&self.log)
        }
    }

    impl BackendScore for SpyMock {
        fn score(&self) -> u8 {
            42
        }
        fn is_persistent(&self) -> bool {
            false
        }
        fn backend_name(&self) -> &'static str {
            "spy"
        }
    }

    #[async_trait]
    impl CacheReader for SpyMock {
        async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            self.log.lock().unwrap().get_calls.push(key.to_string());
            let data = self.data.lock().unwrap();
            Ok(data.get(key).map(|(v, _)| v.clone()))
        }

        async fn exists(&self, key: &str) -> OxCacheResult<bool> {
            let data = self.data.lock().unwrap();
            Ok(data.contains_key(key))
        }

        async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            self.log.lock().unwrap().ttl_calls.push(key.to_string());
            let data = self.data.lock().unwrap();
            Ok(data.get(key).and_then(|(_, ttl)| *ttl))
        }

        async fn len(&self) -> OxCacheResult<u64> {
            let data = self.data.lock().unwrap();
            Ok(data.len() as u64)
        }

        async fn capacity(&self) -> OxCacheResult<u64> {
            Ok(1000)
        }

        async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
            let mut stats = HashMap::new();
            stats.insert("type".to_string(), "spy".to_string());
            Ok(stats)
        }
    }

    #[async_trait]
    impl CacheWriter for SpyMock {
        async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
            self.log
                .lock()
                .unwrap()
                .set_calls
                .push((key.to_string(), value.clone(), ttl));
            self.data.lock().unwrap().insert(key.to_string(), (value, ttl));
            Ok(())
        }

        async fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.log.lock().unwrap().delete_calls.push(key.to_string());
            self.data.lock().unwrap().remove(key);
            Ok(())
        }

        async fn clear(&self) -> OxCacheResult<()> {
            self.log.lock().unwrap().clear_calls += 1;
            self.data.lock().unwrap().clear();
            Ok(())
        }

        async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            self.log.lock().unwrap().expire_calls.push((key.to_string(), ttl));
            let mut data = self.data.lock().unwrap();
            if let Some(entry) = data.get_mut(key) {
                entry.1 = Some(ttl);
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    #[async_trait]
    impl CacheConnector for SpyMock {
        async fn health_check(&self) -> OxCacheResult<()> {
            Ok(())
        }

        async fn shutdown(&self) {}

        fn backend_kind(&self) -> BackendKind {
            BackendKind::Mock
        }
    }

    // CacheBackend via blanket impl.

    // ========================================================================
    // Tests
    // ========================================================================

    #[tokio::test]
    async fn test_bf_backend_get_miss_skips_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        // Key never inserted → BF miss → inner not called.
        let result = backend.get("never_inserted").await.unwrap();
        assert!(result.is_none());
        let log = log.lock().unwrap();
        assert!(log.get_calls.is_empty(), "inner.get should not be called on BF miss");
    }

    #[tokio::test]
    async fn test_bf_backend_get_hit_calls_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        let result = backend.get("k").await.unwrap();
        assert_eq!(result, Some(b"v".to_vec()));
        let log = log.lock().unwrap();
        assert_eq!(log.get_calls.len(), 1);
        assert_eq!(log.get_calls[0], "k");
    }

    #[tokio::test]
    async fn test_bf_backend_set_updates_bloom_and_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        // BF should contain the key.
        assert!(backend.bloom().contains("k"));
        // Inner should have received the set.
        let log = log.lock().unwrap();
        assert_eq!(log.set_calls.len(), 1);
        assert_eq!(log.set_calls[0].0, "k");
        assert_eq!(log.set_calls[0].1, b"v".to_vec());
        assert_eq!(log.set_calls[0].2, None);
    }

    #[tokio::test]
    async fn test_bf_backend_delete_does_not_modify_bloom() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        assert!(backend.bloom().contains("k"));
        backend.delete("k").await.unwrap();
        // BF still contains the key (BF does not support deletion).
        assert!(backend.bloom().contains("k"));
        // Inner received the delete.
        let log = log.lock().unwrap();
        assert_eq!(log.delete_calls.len(), 1);
        assert_eq!(log.delete_calls[0], "k");
    }

    #[tokio::test]
    async fn test_bf_backend_clear_clears_both() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        backend.set("k1", b"v1".to_vec(), None).await.unwrap();
        backend.set("k2", b"v2".to_vec(), None).await.unwrap();
        backend.clear().await.unwrap();
        // BF cleared.
        assert!(!backend.bloom().contains("k1"));
        assert!(!backend.bloom().contains("k2"));
        assert_eq!(backend.bloom().len(), 0);
        // Inner cleared.
        let log = log.lock().unwrap();
        assert_eq!(log.clear_calls, 1);
    }

    #[tokio::test]
    async fn test_bf_backend_set_with_ttl_passes_through_to_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        let ttl = Duration::from_secs(60);
        backend.set("k", b"v".to_vec(), Some(ttl)).await.unwrap();
        let log = log.lock().unwrap();
        assert_eq!(log.set_calls.len(), 1);
        assert_eq!(log.set_calls[0].2, Some(ttl));
    }

    #[tokio::test]
    async fn test_bf_backend_ttl_passes_through_to_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        let ttl = Duration::from_secs(60);
        backend.set("k", b"v".to_vec(), Some(ttl)).await.unwrap();
        let result = backend.ttl("k").await.unwrap();
        assert_eq!(result, Some(ttl));
        let log = log.lock().unwrap();
        assert_eq!(log.ttl_calls.len(), 1);
        assert_eq!(log.ttl_calls[0], "k");
    }

    #[tokio::test]
    async fn test_bf_backend_expire_passes_through_to_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        let new_ttl = Duration::from_secs(120);
        let result = backend.expire("k", new_ttl).await.unwrap();
        assert!(result);
        let log = log.lock().unwrap();
        assert_eq!(log.expire_calls.len(), 1);
        assert_eq!(log.expire_calls[0].0, "k");
        assert_eq!(log.expire_calls[0].1, new_ttl);
    }

    #[tokio::test]
    async fn test_bf_backend_stats_contains_bloom_fields() {
        let spy = SpyMock::new();
        let backend = BloomFilterBackend::new(spy);
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        let stats = backend.stats().await.unwrap();
        assert!(stats.contains_key("bloom_capacity"));
        assert!(stats.contains_key("bloom_load_factor"));
        assert!(stats.contains_key("bloom_false_positive_rate"));
        assert!(stats.contains_key("bloom_estimated_count"));
    }

    // ========================================================================
    // Synchronous trait hierarchy tests (任务组 14)
    // ========================================================================
    //
    // Isolated in a nested module so the sync trait methods (imported below)
    // don't conflict with the async trait methods in the outer `tests` module.
    // All sync calls use UFCS to disambiguate from async methods on the same
    // `MockSyncInner` type (which implements both hierarchies).
    mod sync_tests {
        use super::*;
        // Bring the sync traits into scope explicitly (the outer `use super::*`
        // only pulls in the async traits from the backend.rs root imports).
        use crate::backend::{SyncCacheConnector, SyncCacheReader, SyncCacheWriter};

        #[derive(Default, Debug)]
        struct SyncCallLog {
            get_calls: Vec<String>,
            set_calls: Vec<(String, Vec<u8>, Option<Duration>)>,
        }

        /// Inner mock implementing both `CacheBackend` (async) and
        /// `SyncCacheBackend` (sync). Async methods delegate to the sync ones so
        /// both hierarchies share one state. Sync `get`/`set` record calls so
        /// tests can assert whether the Bloom filter short-circuited.
        struct MockSyncInner {
            log: Arc<Mutex<SyncCallLog>>,
            data: MockDataStore,
        }

        impl MockSyncInner {
            fn new() -> Self {
                Self {
                    log: Arc::new(Mutex::new(SyncCallLog::default())),
                    data: Arc::new(Mutex::new(HashMap::new())),
                }
            }

            /// Clone the call-log handle before the mock is moved into a backend.
            fn log_handle(&self) -> Arc<Mutex<SyncCallLog>> {
                Arc::clone(&self.log)
            }
        }

        // --- Async trait impls (required because `B: CacheBackend`).
        // Delegate to the sync methods; no `.await` is needed because the sync
        // impls are trivial and non-blocking.

        #[async_trait]
        impl CacheReader for MockSyncInner {
            async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
                SyncCacheReader::get(self, key)
            }
            async fn exists(&self, key: &str) -> OxCacheResult<bool> {
                SyncCacheReader::exists(self, key)
            }
            async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
                SyncCacheReader::ttl(self, key)
            }
            async fn len(&self) -> OxCacheResult<u64> {
                SyncCacheReader::len(self)
            }
            async fn capacity(&self) -> OxCacheResult<u64> {
                SyncCacheReader::capacity(self)
            }
            async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
                SyncCacheReader::stats(self)
            }
        }

        #[async_trait]
        impl CacheWriter for MockSyncInner {
            async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
                SyncCacheWriter::set(self, key, value, ttl)
            }
            async fn delete(&self, key: &str) -> OxCacheResult<()> {
                SyncCacheWriter::delete(self, key)
            }
            async fn clear(&self) -> OxCacheResult<()> {
                SyncCacheWriter::clear(self)
            }
            async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
                SyncCacheWriter::expire(self, key, ttl)
            }
        }

        #[async_trait]
        impl CacheConnector for MockSyncInner {
            async fn health_check(&self) -> OxCacheResult<()> {
                SyncCacheConnector::health_check(self)
            }
            async fn shutdown(&self) {
                SyncCacheConnector::shutdown(self)
            }
            fn backend_kind(&self) -> BackendKind {
                SyncCacheConnector::backend_kind(self)
            }
        }

        // CacheBackend via blanket impl.

        // --- Sync trait impls ---

        impl SyncCacheReader for MockSyncInner {
            fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
                self.log.lock().unwrap().get_calls.push(key.to_string());
                let data = self.data.lock().unwrap();
                Ok(data.get(key).map(|(v, _)| v.clone()))
            }
            fn exists(&self, key: &str) -> OxCacheResult<bool> {
                let data = self.data.lock().unwrap();
                Ok(data.contains_key(key))
            }
            fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
                let data = self.data.lock().unwrap();
                Ok(data.get(key).and_then(|(_, ttl)| *ttl))
            }
            fn len(&self) -> OxCacheResult<u64> {
                Ok(self.data.lock().unwrap().len() as u64)
            }
            fn capacity(&self) -> OxCacheResult<u64> {
                Ok(1000)
            }
            fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
                let mut stats = HashMap::new();
                stats.insert("type".to_string(), "mock_sync_inner".to_string());
                Ok(stats)
            }
        }

        impl SyncCacheWriter for MockSyncInner {
            fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
                self.log
                    .lock()
                    .unwrap()
                    .set_calls
                    .push((key.to_string(), value.clone(), ttl));
                self.data.lock().unwrap().insert(key.to_string(), (value, ttl));
                Ok(())
            }
            fn delete(&self, key: &str) -> OxCacheResult<()> {
                self.data.lock().unwrap().remove(key);
                Ok(())
            }
            fn clear(&self) -> OxCacheResult<()> {
                self.data.lock().unwrap().clear();
                Ok(())
            }
            fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
                let mut data = self.data.lock().unwrap();
                if let Some(entry) = data.get_mut(key) {
                    entry.1 = Some(ttl);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }

        impl SyncCacheConnector for MockSyncInner {
            fn health_check(&self) -> OxCacheResult<()> {
                Ok(())
            }
            fn shutdown(&self) {
                let _ = SyncCacheWriter::clear(self);
            }
            fn backend_kind(&self) -> BackendKind {
                BackendKind::Mock
            }
        }

        // SyncCacheBackend via blanket impl.

        // --- Tests for BloomFilterBackend sync cache behavior ---

        #[test]
        fn test_bf_backend_sync_get_miss_skips_inner() {
            let inner = MockSyncInner::new();
            let log = inner.log_handle();
            let backend = BloomFilterBackend::new(inner);
            // Key never inserted → BF miss → inner.get not called.
            let result = SyncCacheReader::get(&backend, "never_inserted").unwrap();
            assert!(result.is_none());
            let log = log.lock().unwrap();
            assert!(log.get_calls.is_empty(), "inner.get should not be called on BF miss");
        }

        #[test]
        fn test_bf_backend_sync_get_hit_calls_inner() {
            let inner = MockSyncInner::new();
            let log = inner.log_handle();
            let backend = BloomFilterBackend::new(inner);
            // set via sync writer to populate both BF and inner.
            SyncCacheWriter::set(&backend, "k", b"v".to_vec(), None).unwrap();
            let result = SyncCacheReader::get(&backend, "k").unwrap();
            assert_eq!(result, Some(b"v".to_vec()));
            let log = log.lock().unwrap();
            assert_eq!(log.get_calls.len(), 1);
            assert_eq!(log.get_calls[0], "k");
        }

        #[test]
        fn test_bf_backend_sync_set_with_ttl_passes_through() {
            let inner = MockSyncInner::new();
            let log = inner.log_handle();
            let backend = BloomFilterBackend::new(inner);
            let ttl = Duration::from_secs(60);
            SyncCacheWriter::set(&backend, "k", b"v".to_vec(), Some(ttl)).unwrap();
            let log = log.lock().unwrap();
            assert_eq!(log.set_calls.len(), 1);
            assert_eq!(log.set_calls[0].2, Some(ttl));
        }
    }
}
