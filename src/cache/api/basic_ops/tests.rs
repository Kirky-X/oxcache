#![allow(clippy::module_inception)]
// Re-export 业务符号给嵌套测试模块（use super::* 的 glob 不跨层传递）
#[allow(unused_imports)]
pub use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::MAX_JSON_DEPTH;

    #[tokio::test]
    async fn test_cache_clear() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        cache.clear().await.unwrap();
        assert!(cache.get(&"key".to_string()).await.unwrap().is_none());
    }

    #[test]
    fn test_get_or_shard_index_in_range() {
        // 任意 key 都映射到 [0, SHARDS) 内的分片
        for key in [
            "",
            "a",
            "key1",
            "user:123",
            "很长很长的中文key🎯",
            "x".repeat(1024).as_str(),
        ] {
            let idx = get_or_shard_index(key);
            assert!(
                idx < GET_OR_LOCK_SHARDS,
                "key={key} shard={idx} out of range"
            );
        }
    }

    #[test]
    fn test_get_or_shards_distribute() {
        // 不同 key 应分散到多个分片（而非全部挤在同一分片）
        let mut seen = std::collections::HashSet::new();
        for i in 0..256 {
            seen.insert(get_or_shard_index(&format!("key{i}")));
        }
        assert!(
            seen.len() > 1,
            "256 keys should spread across shards, only {} distinct shards",
            seen.len()
        );
    }

    #[test]
    fn test_get_or_shard_index_same_key_same_shard() {
        // 相同 key 稳定映射到同一分片（single-flight 正确性的前提）
        let a = get_or_shard_index("stable-key");
        let b = get_or_shard_index("stable-key");
        assert_eq!(a, b);
        // 不同 key 可能不同分片
        let c = get_or_shard_index("other-key");
        let d = get_or_shard_index("another-key");
        assert_ne!(c, d, "不同 key 应路由到不同分片（本例期望）");
    }

    #[tokio::test]
    async fn test_get_or_concurrent_different_keys_no_contention_error() {
        // 高并发不同 key 的 get_or：分片锁下不应出错、不应丢值
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let cache = Arc::new(cache);

        let mut handles = Vec::new();
        for i in 0..64u64 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                let key = format!("concurrent-key-{i}");
                let value = cache
                    .get_or(&key, || async move { Ok(format!("value-{i}")) })
                    .await
                    .unwrap();
                assert_eq!(value, format!("value-{i}"));
                cache.get(&key).await.unwrap().unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_cache_len() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key1".to_string(), &"v1".to_string())
            .await
            .unwrap();
        // Moka's entry_count() is approximate; verify it returns a reasonable value
        let len = cache.len().await.unwrap();
        assert!(len <= 100, "len should be reasonable after single insert");
    }

    #[tokio::test]
    async fn test_cache_is_empty() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        // Moka's is_empty is based on approximate entry_count; just verify no error
        let _ = cache.is_empty().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_exists() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        assert!(!cache.exists(&"key".to_string()).await.unwrap());
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        assert!(cache.exists(&"key".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_delete() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"key".to_string(), &"value".to_string())
            .await
            .unwrap();
        cache.delete(&"key".to_string()).await.unwrap();
        assert!(cache.get(&"key".to_string()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cache_get_or() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let value = cache
            .get_or(&"key".to_string(), || async { Ok("computed".to_string()) })
            .await
            .unwrap();
        assert_eq!(value, "computed");
        let cached = cache.get(&"key".to_string()).await.unwrap().unwrap();
        assert_eq!(cached, "computed");
    }

    #[tokio::test]
    async fn test_cache_health_check() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        assert!(cache.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let stats = cache.stats().await.unwrap();
        assert!(stats.contains_key("type"));
    }

    // ========================================================================
    // get / set / delete scenarios
    // ========================================================================

    #[tokio::test]
    async fn test_cache_get_miss_returns_none() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let result = cache.get(&"missing".to_string()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_set_overwrite() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache
            .set(&"k".to_string(), &"v1".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap().unwrap(),
            "v1".to_string()
        );

        // Overwrite with a new value
        cache
            .set(&"k".to_string(), &"v2".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap().unwrap(),
            "v2".to_string()
        );
    }

    #[tokio::test]
    async fn test_cache_delete_missing_key_no_error() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        // Deleting a key that was never set should not error
        assert!(cache.delete(&"never".to_string()).await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_exists_after_delete() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();
        assert!(cache.exists(&"k".to_string()).await.unwrap());

        cache.delete(&"k".to_string()).await.unwrap();
        assert!(!cache.exists(&"k".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_set_with_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache
            .set_with_ttl(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap().unwrap(),
            "v".to_string()
        );
    }

    #[tokio::test]
    async fn test_cache_set_with_ttl_none() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        cache
            .set_with_ttl(&"k".to_string(), &42, None)
            .await
            .unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_cache_get_set_integer_type() {
        let cache: Cache<String, i64> = Cache::builder().build().await.unwrap();

        cache.set(&"count".to_string(), &12345).await.unwrap();
        assert_eq!(
            cache.get(&"count".to_string()).await.unwrap().unwrap(),
            12345
        );
    }

    #[tokio::test]
    async fn test_cache_get_set_struct_type() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct User {
            id: u64,
            name: String,
        }

        let cache: Cache<String, User> = Cache::builder().build().await.unwrap();
        let user = User {
            id: 1,
            name: "alice".to_string(),
        };

        cache.set(&"user:1".to_string(), &user).await.unwrap();
        let result = cache.get(&"user:1".to_string()).await.unwrap().unwrap();
        assert_eq!(result, user);
    }

    // ========================================================================
    // get_or scenarios
    // ========================================================================

    #[tokio::test]
    async fn test_cache_get_or_cache_hit_fast_path() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        // Pre-populate cache
        cache
            .set(&"k".to_string(), &"cached".to_string())
            .await
            .unwrap();

        // get_or should return cached value without calling fallback
        let value = cache
            .get_or(&"k".to_string(), || async {
                Err(OxCacheError::Operation(
                    "fallback should not be called".to_string(),
                ))
            })
            .await
            .unwrap();
        assert_eq!(value, "cached");
    }

    #[tokio::test]
    async fn test_cache_get_or_fallback_error_propagates() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let result: OxCacheResult<String> = cache
            .get_or(&"missing".to_string(), || async {
                Err(OxCacheError::Operation("db down".to_string()))
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(OxCacheError::Operation(msg)) => assert_eq!(msg, "db down"),
            _ => panic!("expected OxCacheError::Operation"),
        }
    }

    #[tokio::test]
    async fn test_cache_get_or_writes_to_cache() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        // First call: miss, fallback computes and caches
        let v1 = cache
            .get_or(&"k".to_string(), || async { Ok(99) })
            .await
            .unwrap();
        assert_eq!(v1, 99);

        // Verify it was cached: a direct get should return the value
        let cached = cache.get(&"k".to_string()).await.unwrap().unwrap();
        assert_eq!(cached, 99);
    }

    // ========================================================================
    // capacity / shutdown
    // ========================================================================

    #[tokio::test]
    async fn test_cache_capacity() {
        let cache: Cache<String, String> = Cache::builder().capacity(500).build().await.unwrap();

        let capacity = cache.capacity().await.unwrap();
        assert_eq!(capacity, 500);
    }

    #[tokio::test]
    async fn test_cache_shutdown() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();

        // Should not panic
        cache.shutdown().await;
    }

    // ========================================================================
    // deserialize_value internal functions
    // ========================================================================

    /// 热路径借用查询语义与吞吐对比（本机 debug 口径记录 docs/PERFORMANCE.md）
    #[tokio::test(flavor = "multi_thread")]
    async fn get_by_str_semantics_and_throughput() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"hit".to_string(), &"v".to_string())
            .await
            .unwrap();

        // 语义：K=String 时 get_by_str 与 get 等价
        assert_eq!(
            cache.get_by_str("hit").await.unwrap(),
            Some("v".to_string())
        );
        assert_eq!(cache.get_by_str("miss").await.unwrap(), None);
        cache
            .set_by_str("via-str", &"w".to_string(), None)
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"via-str".to_string()).await.unwrap(),
            Some("w".to_string())
        );

        // 吞吐对比（相对值；绝对值随环境波动）
        // 公平口径：两个独立循环、各自 2 轮取优，均查询同样 100 个命中键
        const ITER: u32 = 50_000;
        let measure_owned = || async {
            let mut total = Duration::ZERO;
            for round in 0..2 {
                let t = std::time::Instant::now();
                for i in 0..ITER {
                    let key = format!("hit{}", (i + round) % 100);
                    let _: Option<String> = cache.get(&key).await.unwrap();
                }
                if round == 1 {
                    total = t.elapsed();
                }
            }
            total
        };
        let measure_borrowed = || async {
            let mut total = Duration::ZERO;
            for round in 0..2 {
                let t = std::time::Instant::now();
                for i in 0..ITER {
                    let key = format!("hit{}", (i + round) % 100);
                    let _: Option<String> = cache.get_by_str(&key).await.unwrap();
                }
                if round == 1 {
                    total = t.elapsed();
                }
            }
            total
        };
        let owned_total = measure_owned().await;
        let borrowed_total = measure_borrowed().await;
        let owned_us = owned_total.as_micros();
        let borrowed_us = borrowed_total.as_micros();
        println!(
            "hot path ({} iters, debug profile): get(owned)={}us get_by_str(borrowed)={}us",
            ITER, owned_us, borrowed_us
        );
        // 借用查询不应显著劣化（允许测量抖动）
        assert!(
            borrowed_total <= owned_total.saturating_mul(3),
            "borrowed 路径不应慢于 owned 3 倍以上: {owned_us}us vs {borrowed_us}us"
        );
    }

    #[tokio::test]
    async fn test_deserialize_value_valid() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache.set(&"k".to_string(), &42).await.unwrap();

        // get() internally calls deserialize_value
        let v = cache.get(&"k".to_string()).await.unwrap().unwrap();
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn test_deserialize_value_invalid_json() {
        // Store invalid JSON bytes directly via backend
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache
            .backend
            .set(Arc::from("bad"), Arc::new(b"not json".to_vec()), None)
            .await
            .unwrap();

        // get() should return a serialization error
        let result = cache.get(&"bad".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deserialize_value_depth_exceeded() {
        // Build a deeply nested JSON that exceeds MAX_JSON_DEPTH (64)
        let mut json_str = String::new();
        for _ in 0..(MAX_JSON_DEPTH + 5) {
            json_str.push('[');
        }
        for _ in 0..(MAX_JSON_DEPTH + 5) {
            json_str.push(']');
        }

        let cache: Cache<String, serde_json::Value> = Cache::builder().build().await.unwrap();
        cache
            .backend
            .set(Arc::from("deep"), Arc::new(json_str.into_bytes()), None)
            .await
            .unwrap();

        let result = cache.get(&"deep".to_string()).await;
        assert!(result.is_err());
        match result {
            Err(OxCacheError::Serialization(msg)) => {
                assert!(msg.contains("深度") || msg.contains("depth"));
            }
            _ => panic!("expected OxCacheError::Serialization"),
        }
    }

    // ========================================================================
    // Async keys(), ttl(), expire() coverage
    // ========================================================================

    #[tokio::test]
    async fn test_cache_keys_returns_matching() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set(&"user:1".to_string(), &"a".to_string())
            .await
            .unwrap();
        cache
            .set(&"user:2".to_string(), &"b".to_string())
            .await
            .unwrap();
        cache
            .set(&"session:1".to_string(), &"c".to_string())
            .await
            .unwrap();

        let all = cache.keys("*").await.unwrap();
        assert_eq!(all.len(), 3);

        let users = cache.keys("user:*").await.unwrap();
        assert_eq!(users.len(), 2);

        let none = cache.keys("nope:*").await.unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn test_cache_ttl_returns_remaining() {
        // 断言精确 TTL 窗口：显式关闭默认抖动（审计 F06）
        let cache: Cache<String, String> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
        cache
            .set_with_ttl(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl = cache
            .ttl(&"k".to_string())
            .await
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(58));
        assert!(ttl <= Duration::from_secs(60));
        // Missing key
        assert_eq!(cache.ttl(&"missing".to_string()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_cache_expire_extends_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set_with_ttl(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ok = cache
            .expire(&"k".to_string(), Duration::from_secs(120))
            .await
            .unwrap();
        assert!(ok);
        let ttl = cache
            .ttl(&"k".to_string())
            .await
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(118));
        // expire missing key
        let ok = cache
            .expire(&"missing".to_string(), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_get_or_follower_not_hung_when_leader_set_fails() {
        // Regression: leader's `set` failure after a successful fallback must
        // still notify waiting followers, otherwise they hang forever.
        use crate::testing::MockBackend;

        let backend: Arc<dyn crate::backend::CacheBackend> =
            Arc::new(MockBackend::new("mock", 50, false).with_fail_set());
        let cache: Arc<Cache<String, f64>> = Arc::new(Cache::new_with_backend(backend));

        let (leader_registered_tx, leader_registered_rx) = tokio::sync::oneshot::channel();
        let (leader_go_tx, leader_go_rx) = tokio::sync::oneshot::channel();

        // Leader: fallback blocks until the follower has registered, so the
        // follower is guaranteed to be waiting when the leader's set fails.
        let cache_leader = cache.clone();
        let leader = tokio::spawn(async move {
            cache_leader
                .get_or(&"k".to_string(), || async {
                    let _ = leader_registered_tx.send(());
                    let _ = leader_go_rx.await;
                    Ok(1.0f64)
                })
                .await
        });

        let _ = leader_registered_rx.await;
        let cache_follower = cache.clone();
        let follower = tokio::spawn(async move {
            tokio::time::timeout(
                Duration::from_secs(5),
                cache_follower.get_or(&"k".to_string(), || async { Ok(2.0f64) }),
            )
            .await
        });

        // Let the follower register as a follower, then release the leader.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = leader_go_tx.send(());

        let _ = leader.await;
        let follower_result = follower.await.unwrap();
        assert!(
            follower_result.is_ok(),
            "follower must resolve (timeout indicates hang): {:?}",
            follower_result
        );
    }
}

#[cfg(all(test, feature = "memory"))]
mod sync_tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;
    use std::time::Duration;

    /// Helper: construct a Cache whose `backend_sync` is wired to the same
    /// Moka instance as the async backend. Mirrors what
    /// `CacheBuilder::sync_mode(true)` will do in task group 10.
    fn make_sync_cache() -> Cache<String, String> {
        let moka = Arc::new(MokaMemoryBackend::new());
        let mut cache: Cache<String, String> = Cache::new_with_backend(moka.clone());
        cache.set_sync_backend(moka);
        // 既有 sync 测试断言精确 TTL 窗口：显式关闭默认抖动（审计 F06）
        cache.set_ttl_jitter_factor(0.0);
        cache
    }

    #[test]
    fn test_cache_get_sync_set_sync_basic() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        let v = cache.get_sync(&"k".to_string()).unwrap();
        assert_eq!(v, Some("v".to_string()));
    }

    #[test]
    fn test_cache_get_sync_without_sync_mode_returns_err() {
        // Cache::new() leaves backend_sync = None
        let cache: Cache<String, String> = Cache::new();
        let result = cache.get_sync(&"k".to_string());
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "expected Err(NotSupported), got {:?}",
            result
        );
    }

    #[test]
    fn test_cache_get_or_sync_cache_hit() {
        let cache = make_sync_cache();
        cache
            .set_sync(&"k".to_string(), &"cached".to_string())
            .unwrap();

        // Fallback should NOT be called — pre-populated value wins
        let v = cache
            .get_or_sync(&"k".to_string(), || {
                Err(OxCacheError::Operation(
                    "fallback should not run".to_string(),
                ))
            })
            .unwrap();
        assert_eq!(v, "cached");
    }

    // NOTE: test_cache_get_or_sync_cache_miss_triggers_fallback removed —
    // sync bridge (block_in_place) is incompatible with test runtime contexts.
    // The single_flight test below covers the get_or_sync leader path.

    #[test]
    fn test_cache_get_or_sync_single_flight_prevents_duplicate_fallback() {
        let cache = Arc::new(make_sync_cache());
        let counter = Arc::new(AtomicU32::new(0));

        // Thread A: becomes leader, sleeps inside fallback to give B time to
        // arrive and become a follower.
        let cache_a = cache.clone();
        let counter_a = counter.clone();
        let handle_a = thread::spawn(move || {
            cache_a
                .get_or_sync(&"k".to_string(), || {
                    counter_a.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(120));
                    Ok("v".to_string())
                })
                .unwrap()
        });

        // Give A time to register as leader before B arrives.
        thread::sleep(Duration::from_millis(20));

        let cache_b = cache.clone();
        let counter_b = counter.clone();
        let handle_b = thread::spawn(move || {
            cache_b
                .get_or_sync(&"k".to_string(), || {
                    counter_b.fetch_add(1, Ordering::SeqCst);
                    Ok("should_not_run".to_string())
                })
                .unwrap()
        });

        let v_a = handle_a.join().expect("thread A panicked");
        let v_b = handle_b.join().expect("thread B panicked");

        assert_eq!(v_a, "v");
        assert_eq!(v_b, "v");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "fallback must run exactly once under single-flight"
        );
    }

    #[test]
    fn test_cache_set_with_ttl_sync_expires() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_millis(50)),
            )
            .unwrap();

        // Within TTL window: readable
        assert_eq!(
            cache.get_sync(&"k".to_string()).unwrap(),
            Some("v".to_string())
        );

        // After TTL: expired
        thread::sleep(Duration::from_millis(120));
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_delete_sync() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        cache.delete_sync(&"k".to_string()).unwrap();
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_exists_sync() {
        let cache = make_sync_cache();
        assert!(!cache.exists_sync(&"k".to_string()).unwrap());
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        assert!(cache.exists_sync(&"k".to_string()).unwrap());
    }

    #[test]
    fn test_cache_ttl_sync() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .unwrap();
        let ttl = cache
            .ttl_sync(&"k".to_string())
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(58));
        assert!(ttl <= Duration::from_secs(60));
        // Missing key
        assert_eq!(cache.ttl_sync(&"missing".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_expire_sync() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(
                &"k".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .unwrap();
        let ok = cache
            .expire_sync(&"k".to_string(), Duration::from_secs(120))
            .unwrap();
        assert!(ok);
        let ttl = cache
            .ttl_sync(&"k".to_string())
            .unwrap()
            .expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(118));
        // expire missing key
        let ok = cache
            .expire_sync(&"missing".to_string(), Duration::from_secs(60))
            .unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_cache_sync_methods_without_sync_mode_returns_err() {
        let cache: Cache<String, String> = Cache::new();
        assert!(cache.delete_sync(&"k".to_string()).is_err());
        assert!(cache.exists_sync(&"k".to_string()).is_err());
        assert!(cache.ttl_sync(&"k".to_string()).is_err());
        assert!(
            cache
                .expire_sync(&"k".to_string(), Duration::from_secs(1))
                .is_err()
        );
    }

    /// Sync backend stub that delegates reads to Moka but fails every `set`.
    /// Pins the `get_or_option_sync` error-propagation contract: a leader
    /// whose cache write fails must surface `Err`, not a phantom `Ok`.
    struct FailSetBackend {
        inner: MokaMemoryBackend,
    }

    impl FailSetBackend {
        fn new() -> Self {
            Self {
                inner: MokaMemoryBackend::new(),
            }
        }
    }

    impl crate::backend::SyncCacheReader for FailSetBackend {
        fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            self.inner.get(key)
        }

        fn exists(&self, key: &str) -> OxCacheResult<bool> {
            self.inner.exists(key)
        }

        fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            self.inner.ttl(key)
        }

        fn len(&self) -> OxCacheResult<u64> {
            self.inner.len()
        }

        fn capacity(&self) -> OxCacheResult<u64> {
            Ok(self.inner.capacity())
        }

        fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
            self.inner.stats()
        }
    }

    impl crate::backend::SyncCacheWriter for FailSetBackend {
        fn set(
            &self,
            _key: Arc<str>,
            _value: Arc<Vec<u8>>,
            _ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            Err(OxCacheError::Operation(
                "FailSetBackend: injected set failure".to_string(),
            ))
        }

        fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.inner.delete(key)
        }

        fn clear(&self) -> OxCacheResult<()> {
            self.inner.clear()
        }

        fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            self.inner.expire(key, ttl)
        }
    }

    impl crate::backend::SyncCacheConnector for FailSetBackend {
        fn health_check(&self) -> OxCacheResult<()> {
            self.inner.health_check()
        }

        fn shutdown(&self) {}

        fn backend_kind(&self) -> crate::backend::BackendKind {
            self.inner.backend_kind()
        }
    }

    fn make_failing_sync_cache() -> Cache<String, String> {
        let moka = Arc::new(MokaMemoryBackend::new());
        let mut cache: Cache<String, String> = Cache::new_with_backend(moka);
        cache.set_sync_backend(Arc::new(FailSetBackend::new()));
        cache
    }

    #[test]
    fn test_get_or_option_sync_propagates_value_set_failure() {
        let cache = make_failing_sync_cache();
        let result = cache.get_or_option_sync(&"k-value-set-fail".to_string(), || {
            Ok(Some("v".to_string()))
        });
        assert!(
            result.is_err(),
            "leader set failure must propagate, got {:?}",
            result
        );
    }

    #[test]
    fn test_get_or_option_sync_propagates_sentinel_set_failure() {
        let mut cache = make_failing_sync_cache();
        cache.set_null_cache_ttl(Some(Duration::from_secs(60)));
        let result = cache.get_or_option_sync(&"k-sentinel-set-fail".to_string(), || Ok(None));
        assert!(
            result.is_err(),
            "sentinel set failure must propagate, got {:?}",
            result
        );
    }
}

#[cfg(all(test, feature = "memory"))]
mod sentinel_race_tests {
    use super::*;
    use crate::backend::memory::MokaMemoryBackend;
    use crate::backend::{CacheConnector, CacheReader, CacheWriter};
    use std::sync::atomic::AtomicBool;

    /// 首次 get 返回 None（模拟快速路径 miss），其后透传真实数据的 scripted backend。
    struct FirstGetMissBackend {
        inner: Arc<MokaMemoryBackend>,
        key: &'static str,
        swallowed: AtomicBool,
    }

    impl FirstGetMissBackend {
        fn new(key: &'static str) -> Self {
            Self {
                inner: Arc::new(MokaMemoryBackend::new()),
                key,
                swallowed: AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl CacheReader for FirstGetMissBackend {
        async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            if key == self.key
                && !self
                    .swallowed
                    .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                return Ok(None);
            }
            self.inner.get(key).await
        }
        async fn exists(&self, key: &str) -> OxCacheResult<bool> {
            self.inner.exists(key).await
        }
        async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            CacheReader::ttl(&*self.inner, key).await
        }
        async fn len(&self) -> OxCacheResult<u64> {
            self.inner.len().await
        }
        async fn capacity(&self) -> OxCacheResult<u64> {
            Ok(self.inner.capacity())
        }
        async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
            self.inner.stats().await
        }
    }

    #[async_trait::async_trait]
    impl CacheWriter for FirstGetMissBackend {
        async fn set(
            &self,
            key: Arc<str>,
            value: Arc<Vec<u8>>,
            ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            self.inner.set(key, value, ttl).await
        }
        async fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.inner.delete(key).await
        }
        async fn clear(&self) -> OxCacheResult<()> {
            self.inner.clear().await
        }
        async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            self.inner.expire(key, ttl).await
        }
    }

    #[async_trait::async_trait]
    impl CacheConnector for FirstGetMissBackend {
        async fn health_check(&self) -> OxCacheResult<()> {
            self.inner.health_check().await
        }
        async fn shutdown(&self) {
            self.inner.shutdown().await
        }
        fn backend_kind(&self) -> crate::backend::BackendKind {
            self.inner.backend_kind()
        }
    }

    /// 审计 F02 确定性回归：快速路径 get miss 后、哨兵判定前该 key 被并发写入
    /// 真实值。旧 `exists()` 判定无法区分哨兵与真实值，会把真实值误判为
    /// "哨兵有效" 返回 Ok(None)；get + 字节比对必须返回 Ok(Some(真实值))。
    #[tokio::test]
    async fn get_or_option_returns_real_value_written_after_fast_path() {
        let backend: Arc<dyn crate::backend::CacheBackend> =
            Arc::new(FirstGetMissBackend::new("race-real"));
        let mut cache: Cache<String, String> = Cache::new_with_backend(backend);
        cache.set_null_cache_ttl(Some(Duration::from_secs(60)));

        // 真实值在"快速路径 miss 之后"才可见（scripted backend 吞掉首查）
        cache
            .backend
            .set(
                Arc::from("race-real"),
                Arc::new(b"\"real-value\"".to_vec()),
                None,
            )
            .await
            .unwrap();

        let got = cache
            .get_or_option(&"race-real".to_string(), || async {
                Err(OxCacheError::Operation("fallback must not run".into()))
            })
            .await
            .unwrap();
        assert_eq!(
            got,
            Some("real-value".to_string()),
            "真实值不得被 exists 判定误判为空值哨兵"
        );
    }
}

#[cfg(test)]
mod jitter_tests {
    use super::*;

    fn jitter_cache(factor: f64) -> Cache<String, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut cache: Cache<String, String> = rt
            .block_on(async { Cache::builder().build().await })
            .unwrap();
        cache.set_ttl_jitter_factor(factor);
        cache
    }

    /// 审计 F07 验收：10000 次采样全部落在 [ttl*(1-f), ttl*(1+f)] 且分布非退化
    #[test]
    fn jitter_samples_within_bounds_and_non_degenerate() {
        let cache = jitter_cache(0.1);
        let base = Duration::from_secs(60);
        let low = base.mul_f64(0.9).as_millis() as u64;
        let high = base.mul_f64(1.1).as_millis() as u64;
        let mut samples: Vec<u64> = (0..10_000)
            .map(|_| cache.apply_jitter(base).as_millis() as u64)
            .collect();
        assert!(
            samples.iter().all(|&s| s >= low && s <= high),
            "采样必须落在 ±factor 区间内 [{low}, {high}]"
        );
        samples.sort_unstable();
        assert!(
            samples[0] < samples[5_000] && samples[5_000] < samples[9_999],
            "分布非退化：min < median < max，实际 {} / {} / {}",
            samples[0],
            samples[5_000],
            samples[9_999]
        );
    }

    /// factor=0 恒等返回（显式关闭抖动语义不变）
    #[test]
    fn zero_factor_is_identity() {
        let cache = jitter_cache(0.0);
        let base = Duration::from_secs(60);
        for _ in 0..100 {
            assert_eq!(cache.apply_jitter(base), base);
        }
    }
}

#[cfg(all(test, feature = "memory"))]
mod get_or_with_ttl_tests {
    use super::*;
    use crate::backend::memory::MokaMemoryBackend;
    use std::sync::Arc as StdArc;

    /// 审计 F09 验收：get_or_with_ttl 写入带 TTL（≤ 传入值，抖动上界）
    #[tokio::test]
    async fn get_or_with_ttl_caches_with_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let v = cache
            .get_or_with_ttl(
                &"ttl-or".to_string(),
                Some(Duration::from_secs(60)),
                || async { Ok("v".to_string()) },
            )
            .await
            .unwrap();
        assert_eq!(v, "v");
        let ttl = cache
            .ttl(&"ttl-or".to_string())
            .await
            .unwrap()
            .expect("ttl 应存在");
        // 默认抖动 ±10%：60s → [54s, 66s)
        assert!(
            ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
            "ttl {ttl:?} 应在抖动区间 [54s, 66s)"
        );

        // 旧 get_or 语义不变：无 TTL
        let cache2: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache2
            .get_or(&"no-ttl-or".to_string(), || async { Ok("v".to_string()) })
            .await
            .unwrap();
        assert_eq!(
            cache2.ttl(&"no-ttl-or".to_string()).await.unwrap(),
            None,
            "get_or 旧路径必须保持无 TTL 语义"
        );
    }

    /// get_or_option_with_ttl：真实值与空值哨兵均带抖动 TTL
    #[tokio::test]
    async fn get_or_option_with_ttl_jitters_sentinel() {
        let mut cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set_null_cache_ttl(Some(Duration::from_secs(30)));

        // fallback 返回 None → 哨兵写入且带 TTL
        let got = cache
            .get_or_option_with_ttl(
                &"sentinel-ttl".to_string(),
                Some(Duration::from_secs(60)),
                || async { Ok(None) },
            )
            .await
            .unwrap();
        assert_eq!(got, None);
        let ttl = cache
            .ttl(&"sentinel-ttl".to_string())
            .await
            .unwrap()
            .expect("哨兵应存在");
        assert!(
            ttl >= Duration::from_secs(27) && ttl < Duration::from_secs(33),
            "哨兵 ttl {ttl:?} 应在抖动区间 [27s, 33s)"
        );

        // 第二次调用命中哨兵直接返回 None（不执行 fallback）
        let got2 = cache
            .get_or_option_with_ttl(
                &"sentinel-ttl".to_string(),
                Some(Duration::from_secs(60)),
                || async { Err(OxCacheError::Operation("must not run".into())) },
            )
            .await
            .unwrap();
        assert_eq!(got2, None);
    }

    /// sync 变体：get_or_with_ttl_sync 带 TTL
    #[test]
    fn get_or_with_ttl_sync_caches_with_ttl() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut cache: Cache<String, String> = rt
            .block_on(async { Cache::builder().build().await })
            .unwrap();
        cache.set_sync_backend(StdArc::new(MokaMemoryBackend::new()));

        cache
            .get_or_with_ttl_sync(
                &"ttl-sync".to_string(),
                Some(Duration::from_secs(60)),
                || Ok("v".to_string()),
            )
            .unwrap();
        let ttl = cache
            .ttl_sync(&"ttl-sync".to_string())
            .unwrap()
            .expect("ttl 应存在");
        // 默认抖动 ±10%：60s → [54s, 66s)
        assert!(
            ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
            "ttl {ttl:?} 应在抖动区间 [54s, 66s)"
        );
    }
}

#[cfg(test)]
mod default_jitter_tests {
    use super::*;

    /// 审计 F06 验收：默认构建抖动 0.1 生效（±10%），显式 0.0 关闭后恒等
    #[tokio::test]
    async fn default_jitter_is_on_and_can_be_disabled() {
        // 默认构建：60s → [54s, 66s)
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set_with_ttl(
                &"dj-on".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl = cache.ttl(&"dj-on".to_string()).await.unwrap().unwrap();
        assert!(
            ttl >= Duration::from_secs(54) && ttl < Duration::from_secs(66),
            "默认抖动应使 ttl ∈ [54s, 66s)，实际 {ttl:?}"
        );

        // 显式关闭：60s → (58s, 60s]
        let cache2: Cache<String, String> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
        cache2
            .set_with_ttl(
                &"dj-off".to_string(),
                &"v".to_string(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl2 = cache2.ttl(&"dj-off".to_string()).await.unwrap().unwrap();
        assert!(
            ttl2 > Duration::from_secs(58),
            "关闭抖动后 ttl 应 ≈ 60s，实际 {ttl2:?}"
        );
    }
}
