#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

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
        async fn set(
            &self,
            key: Arc<str>,
            value: Arc<Vec<u8>>,
            ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            self.log
                .lock()
                .unwrap()
                .set_calls
                .push((key.to_string(), (*value).clone(), ttl));
            self.data
                .lock()
                .unwrap()
                .insert(key.to_string(), ((*value).clone(), ttl));
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
            self.log
                .lock()
                .unwrap()
                .expire_calls
                .push((key.to_string(), ttl));
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
        assert!(
            log.get_calls.is_empty(),
            "inner.get should not be called on BF miss"
        );
    }

    #[tokio::test]
    async fn test_bf_backend_get_hit_calls_inner() {
        let spy = SpyMock::new();
        let log = spy.log_handle();
        let backend = BloomFilterBackend::new(spy);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("k2"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(ttl))
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(ttl))
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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
            async fn set(
                &self,
                key: Arc<str>,
                value: Arc<Vec<u8>>,
                ttl: Option<Duration>,
            ) -> OxCacheResult<()> {
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
            fn set(
                &self,
                key: Arc<str>,
                value: Arc<Vec<u8>>,
                ttl: Option<Duration>,
            ) -> OxCacheResult<()> {
                self.log
                    .lock()
                    .unwrap()
                    .set_calls
                    .push((key.to_string(), (*value).clone(), ttl));
                self.data
                    .lock()
                    .unwrap()
                    .insert(key.to_string(), ((*value).clone(), ttl));
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
            assert!(
                log.get_calls.is_empty(),
                "inner.get should not be called on BF miss"
            );
        }

        #[test]
        fn test_bf_backend_sync_get_hit_calls_inner() {
            let inner = MockSyncInner::new();
            let log = inner.log_handle();
            let backend = BloomFilterBackend::new(inner);
            // set via sync writer to populate both BF and inner.
            SyncCacheWriter::set(&backend, Arc::from("k"), Arc::new(b"v".to_vec()), None).unwrap();
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
            SyncCacheWriter::set(&backend, Arc::from("k"), Arc::new(b"v".to_vec()), Some(ttl))
                .unwrap();
            let log = log.lock().unwrap();
            assert_eq!(log.set_calls.len(), 1);
            assert_eq!(log.set_calls[0].2, Some(ttl));
        }
    }
}

#[cfg(test)]
mod prefill_tests {
    use super::*;
    use crate::backend::memory::DashMapMemoryBackend;

    /// 审计 F01 验收：重启后（过滤器空、后端有数据）经 prefill_from_backend
    /// 回灌，get 才能查到已有条目；未回灌的 key 假阴性短路返回 None。
    #[tokio::test]
    async fn prefill_from_backend_aligns_filter_with_inner() {
        let inner = DashMapMemoryBackend::new();
        CacheWriter::set(
            &inner,
            Arc::from("warm-key"),
            Arc::new(b"warm-value".to_vec()),
            None,
        )
        .await
        .unwrap();

        let backend = BloomFilterBackend::new(inner);
        // 未预热：bloom miss 短路 → None（假阴性，连后端都不查）
        let cold = CacheReader::get(&backend, "warm-key").await.unwrap();
        assert_eq!(cold, None, "未预热时假阴性短路应返回 None");

        // 回灌后：命中真实值
        let count = backend.prefill_from_backend("*").await.unwrap();
        assert_eq!(count, 1);
        let warm = CacheReader::get(&backend, "warm-key").await.unwrap();
        assert_eq!(warm, Some(b"warm-value".to_vec()));
    }

    /// prefill_from_keys 对未在 inner 的 key 仅产生假阳性（get 返回 None）而非 panic
    #[tokio::test]
    async fn prefill_from_keys_false_positive_only() {
        let backend = BloomFilterBackend::new(DashMapMemoryBackend::new());
        backend.prefill_from_keys(["ghost-key", "warm-key"]);
        let ghost = CacheReader::get(&backend, "ghost-key").await.unwrap();
        assert_eq!(
            ghost, None,
            "假阳性应退化为后端 miss，不得 panic 或错误命中"
        );
    }
}
