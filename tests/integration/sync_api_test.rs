// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Sync API 集成测试
//
// 合并自原 tests/redis_sync.rs 和 tests/sync_api_integration.rs：
//   - memory_sync_tests：Cache<K,V> sync API + ChainCache sync API（Moka/DashMap）
//   - redis_sync_tests：RedisBackend sync API（需 Redis server，默认 #[ignore]）
//
// 注意：所有涉及 Moka sync 的测试使用 `multi_thread` flavor，因为
// Moka 的 sync_block_on 在 current-thread runtime 上会 panic。

// ============================================================================
// Memory 后端 sync API（Moka / DashMap / ChainCache）
// ============================================================================

#[cfg(feature = "memory")]
mod memory_sync_tests {
    use std::time::Duration;

    use oxcache::Cache;
    use oxcache::backend::{DashMapMemoryBackend, MokaMemoryBackend};
    use oxcache::cache::{ChainCache, ChainLink};

    // ------------------------------------------------------------------------
    // Cache<K,V> sync API（sync_mode + Moka 后端）
    // ------------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cache_sync_full_lifecycle() {
        let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

        // set_sync + get_sync roundtrip
        cache.set_sync(&"k1".to_string(), &"v1".to_string()).unwrap();
        let value = cache.get_sync(&"k1".to_string()).unwrap();
        assert_eq!(value, Some("v1".to_string()));

        // exists_sync
        assert!(cache.exists_sync(&"k1".to_string()).unwrap());
        assert!(!cache.exists_sync(&"missing".to_string()).unwrap());

        // delete_sync + verify gone
        cache.delete_sync(&"k1".to_string()).unwrap();
        assert_eq!(cache.get_sync(&"k1".to_string()).unwrap(), None);
        assert!(!cache.exists_sync(&"k1".to_string()).unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cache_sync_with_ttl_expires() {
        let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

        cache
            .set_with_ttl_sync(&"k".to_string(), &"v".to_string(), Some(Duration::from_millis(50)))
            .unwrap();

        let value = cache.get_sync(&"k".to_string()).unwrap();
        assert_eq!(value, Some("v".to_string()));

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut expired = false;
        for _ in 0..10 {
            if cache.get_sync(&"k".to_string()).unwrap().is_none() {
                expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(expired, "sync get should return None after TTL expires");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cache_get_or_sync_hit_and_miss() {
        let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();
        let value = cache
            .get_or_sync(&"user:1".to_string(), || {
                call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok("Alice".to_string())
            })
            .unwrap();
        assert_eq!(value, "Alice");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        let value = cache
            .get_or_sync(&"user:1".to_string(), || Ok("Should not be called".to_string()))
            .unwrap();
        assert_eq!(value, "Alice");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    // ------------------------------------------------------------------------
    // ChainCache sync API（from_sync_backend + Moka / DashMap）
    // ------------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chain_sync_multi_backend_roundtrip() {
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let moka_ref = moka.clone();
        let dashmap_ref = dashmap.clone();

        let chain = ChainCache::builder()
            .link(ChainLink::from_sync_backend(moka))
            .link(ChainLink::from_sync_backend(dashmap))
            .build();

        chain.set_sync("k", b"v".to_vec(), None).unwrap();

        use oxcache::backend::SyncCacheReader;
        assert_eq!(SyncCacheReader::get(&moka_ref, "k").unwrap(), Some(b"v".to_vec()));
        assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), Some(b"v".to_vec()));

        let value = chain.get_sync("k").unwrap();
        assert_eq!(value, Some(b"v".to_vec()));

        chain.delete_sync("k").unwrap();
        assert_eq!(SyncCacheReader::get(&moka_ref, "k").unwrap(), None);
        assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), None);
        assert_eq!(chain.get_sync("k").unwrap(), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chain_sync_ttl_propagates_to_all_links() {
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let moka_ref = moka.clone();
        let dashmap_ref = dashmap.clone();

        let chain = ChainCache::builder()
            .link(ChainLink::from_sync_backend(moka))
            .link(ChainLink::from_sync_backend(dashmap))
            .build();

        chain
            .set_sync("k", b"v".to_vec(), Some(Duration::from_millis(50)))
            .unwrap();

        use oxcache::backend::SyncCacheReader;
        assert_eq!(SyncCacheReader::get(&moka_ref, "k").unwrap(), Some(b"v".to_vec()));
        assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), Some(b"v".to_vec()));

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), None);

        let mut moka_expired = false;
        for _ in 0..10 {
            if SyncCacheReader::get(&moka_ref, "k").unwrap().is_none() {
                moka_expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(moka_expired, "moka link should expire after TTL");

        assert_eq!(chain.get_sync("k").unwrap(), None);
    }

    // ------------------------------------------------------------------------
    // sync_mode 与 async API 混用
    // ------------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sync_and_async_coexist() {
        let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

        // async set + sync get
        cache
            .set(&"async_key".to_string(), &"async_value".to_string())
            .await
            .unwrap();
        let value = cache.get_sync(&"async_key".to_string()).unwrap();
        assert_eq!(value, Some("async_value".to_string()));

        // sync set + async get
        cache
            .set_sync(&"sync_key".to_string(), &"sync_value".to_string())
            .unwrap();
        let value = cache.get(&"sync_key".to_string()).await.unwrap();
        assert_eq!(value, Some("sync_value".to_string()));
    }
}

// ============================================================================
// Redis 后端 sync API（需 Redis server，默认 ignored）
//
// 运行方式：cargo test --features redis --test integration -- --ignored
// ============================================================================

#[cfg(feature = "redis")]
mod redis_sync_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use oxcache::backend::{RedisBackend, SyncCacheReader, SyncCacheWriter};
    use oxcache::error::OxCacheError;

    const REDIS_URL: &str = "redis://127.0.0.1:6379";
    const KEY_PREFIX: &str = "test_redis_sync:";

    static UID: AtomicU64 = AtomicU64::new(0);

    fn unique_key(suffix: &str) -> String {
        let id = UID.fetch_add(1, Ordering::SeqCst);
        format!("{}{}_{}", KEY_PREFIX, id, suffix)
    }

    async fn make_backend() -> RedisBackend {
        unsafe {
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        };
        RedisBackend::new(REDIS_URL)
            .await
            .expect("Failed to connect to Redis — start a Redis server before running this test")
    }

    #[test]
    #[ignore = "requires Redis server; run with: cargo test --features redis --test integration -- --ignored"]
    fn test_redis_sync_get_set_multi_thread_runtime() {
        let rt = tokio::runtime::Runtime::new().expect("failed to build multi-thread runtime");
        rt.block_on(async {
            let backend = make_backend().await;
            let key = unique_key("sync_get_set");

            SyncCacheWriter::set(
                &backend,
                Arc::from(key.as_str()),
                Arc::new(b"hello sync".to_vec()),
                None,
            )
            .expect("sync set failed");

            let val = SyncCacheReader::get(&backend, &key).expect("sync get failed");
            assert_eq!(val, Some(b"hello sync".to_vec()));

            assert!(SyncCacheReader::exists(&backend, &key).expect("sync exists failed"));

            SyncCacheWriter::delete(&backend, &key).expect("sync delete failed");
            assert!(!SyncCacheReader::exists(&backend, &key).expect("sync exists after delete failed"));
        });
    }

    #[test]
    #[ignore = "requires Redis server; run with: cargo test --features redis --test integration -- --ignored"]
    fn test_redis_sync_get_current_thread_fails() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build current-thread runtime");
        rt.block_on(async {
            let backend = make_backend().await;
            let key = unique_key("sync_current_thread");

            let result = SyncCacheReader::get(&backend, &key);
            assert!(
                matches!(result, Err(OxCacheError::NotSupported(_))),
                "expected Err(NotSupported) on current-thread runtime, got {:?}",
                result
            );

            let result = SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None);
            assert!(
                matches!(result, Err(OxCacheError::NotSupported(_))),
                "expected Err(NotSupported) for sync set on current-thread runtime, got {:?}",
                result
            );
        });
    }

    #[test]
    #[ignore = "requires Redis server; run with: cargo test --features redis --test integration -- --ignored"]
    fn test_redis_sync_set_with_ttl_expires() {
        let rt = tokio::runtime::Runtime::new().expect("failed to build multi-thread runtime");
        rt.block_on(async {
            let backend = make_backend().await;
            let key = unique_key("sync_ttl_expires");

            SyncCacheWriter::set(
                &backend,
                Arc::from(key.as_str()),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_secs(1)),
            )
            .expect("sync set with ttl failed");
            assert!(SyncCacheReader::exists(&backend, &key).expect("sync exists failed"));

            tokio::time::sleep(Duration::from_millis(1100)).await;

            assert!(
                !SyncCacheReader::exists(&backend, &key).expect("sync exists after expiry failed"),
                "key should have expired"
            );
        });
    }

    #[test]
    #[ignore = "requires Redis server; run with: cargo test --features redis --test integration -- --ignored"]
    fn test_redis_sync_expire() {
        let rt = tokio::runtime::Runtime::new().expect("failed to build multi-thread runtime");
        rt.block_on(async {
            let backend = make_backend().await;
            let key = unique_key("sync_expire");

            SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
                .expect("sync set failed");

            let ok = SyncCacheWriter::expire(&backend, &key, Duration::from_secs(50)).expect("sync expire failed");
            assert!(ok, "expire should return true for existing key");

            let ttl = SyncCacheReader::ttl(&backend, &key).expect("sync ttl failed");
            assert!(ttl.is_some(), "ttl should be Some after expire");
            let secs = ttl.unwrap().as_secs();
            assert!(secs > 40 && secs <= 50, "ttl secs should be in (40, 50], got {}", secs);

            let missing = unique_key("sync_expire_missing");
            let ok =
                SyncCacheWriter::expire(&backend, &missing, Duration::from_secs(10)).expect("sync expire call failed");
            assert!(!ok, "expire should return false for missing key");

            SyncCacheWriter::delete(&backend, &key).expect("sync delete failed");
        });
    }
}
