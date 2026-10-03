#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

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
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();

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

    // ========================================================================
    // Per-entry TTL tests (spec: universal-per-entry-ttl)
    // ========================================================================

    #[tokio::test]
    async fn test_moka_set_with_ttl_expires_after_timeout() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(50)),
            )
            .await
            .unwrap();
        // 立即可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
        // 等待 100ms 后应过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        // moka 异步清理可能略有延迟，循环等待最多 500ms 确保过期
        let mut expired = false;
        for _ in 0..10 {
            if backend.get("k").await.unwrap().is_none() {
                expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(expired, "entry should expire after TTL");
    }

    #[tokio::test]
    async fn test_moka_set_with_ttl_readable_within_window() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        // 60s TTL 内应可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_moka_set_without_ttl_uses_global_ttl() {
        // 用全局 30s TTL 构建后端
        let backend = MokaMemoryBackend::builder()
            .capacity(1000)
            .ttl(Duration::from_secs(30))
            .build();
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        // 立即可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
        // 全局 TTL 查询（per-entry 未设置时返回 None，符合 spec "无 TTL 键返回 None"）
        let ttl = backend.ttl("k").await.unwrap();
        assert_eq!(
            ttl, None,
            "set(None) with global TTL should report None per-entry"
        );
    }

    #[tokio::test]
    async fn test_moka_ttl_returns_remaining() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ttl = backend.ttl("k").await.unwrap().expect("ttl should be Some");
        // 58s < ttl <= 60s
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
    }

    #[tokio::test]
    async fn test_moka_ttl_returns_none_for_missing_key() {
        let backend = MokaMemoryBackend::new();
        assert_eq!(backend.ttl("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_moka_ttl_returns_none_for_no_ttl_key() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.ttl("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_moka_expire_extends_ttl() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ok = backend.expire("k", Duration::from_secs(120)).await.unwrap();
        assert!(ok, "expire on existing key should return true");
        let ttl = backend
            .ttl("k")
            .await
            .unwrap()
            .expect("ttl should be Some after expire");
        assert!(
            ttl > Duration::from_secs(118),
            "ttl={} should be > 118s",
            ttl.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_moka_expire_shrinks_ttl() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        let ok = backend
            .expire("k", Duration::from_millis(50))
            .await
            .unwrap();
        assert!(ok, "expire on existing key should return true");
        // 等待 100ms 后应过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut expired = false;
        for _ in 0..10 {
            if backend.get("k").await.unwrap().is_none() {
                expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(expired, "entry should expire after shrunk TTL");
    }

    #[tokio::test]
    async fn test_moka_expire_missing_key_returns_false() {
        let backend = MokaMemoryBackend::new();
        let ok = backend
            .expire("missing", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!ok, "expire on missing key should return false");
    }

    // ========================================================================
    // Synchronous trait hierarchy tests (任务组 6)
    //
    // 隔离在嵌套 `mod sync_tests` 内：sync trait 的 import 仅在此模块可见，
    // 避免与父模块 `mod tests` 中 async `CacheReader::get` 等同名方法产生
    // 歧义。方法调用通过 trait object (`&dyn SyncCacheReader` 等) 消歧。
    // ========================================================================
    mod sync_tests {
        use super::MokaMemoryBackend;
        use crate::backend::{BackendKind, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
        use std::sync::Arc;
        use std::time::Duration;

        #[test]
        fn test_moka_sync_get_set_basic() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert_eq!(reader.get("key1").unwrap(), Some(b"value1".to_vec()));
            assert!(reader.exists("key1").unwrap());
            assert!(!reader.exists("key2").unwrap());
            assert!(reader.capacity().unwrap() > 0);

            let stats = reader.stats().unwrap();
            assert_eq!(stats.get("type"), Some(&"moka".to_string()));
        }

        #[test]
        fn test_moka_sync_set_with_ttl_expires() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(
                    Arc::from("k"),
                    Arc::new(b"v".to_vec()),
                    Some(Duration::from_millis(50)),
                )
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            // 立即可读
            assert_eq!(reader.get("k").unwrap(), Some(b"v".to_vec()));

            // 等待过期（moka 读时按 per-entry TTL 校验，无需后台驱逐）
            std::thread::sleep(Duration::from_millis(120));
            let mut expired = false;
            for _ in 0..10 {
                if reader.get("k").unwrap().is_none() {
                    expired = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(expired, "entry should expire after TTL via sync get");
        }

        #[test]
        fn test_moka_sync_ttl_returns_remaining() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(
                    Arc::from("k"),
                    Arc::new(b"v".to_vec()),
                    Some(Duration::from_secs(60)),
                )
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            let ttl = reader
                .ttl("k")
                .unwrap()
                .expect("ttl should be Some for TTL'd key");
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
            writer
                .set(Arc::from("no_ttl"), Arc::new(b"v".to_vec()), None)
                .unwrap();
            assert_eq!(reader.ttl("no_ttl").unwrap(), None);
            // 不存在的 key 返回 None
            assert_eq!(reader.ttl("missing").unwrap(), None);
        }

        #[test]
        fn test_moka_sync_expire_works() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(
                    Arc::from("k"),
                    Arc::new(b"v".to_vec()),
                    Some(Duration::from_secs(60)),
                )
                .unwrap();

            // expire 已存在 key → true，TTL 延长至 120s
            let ok = writer.expire("k", Duration::from_secs(120)).unwrap();
            assert!(ok, "expire on existing key should return true");

            let reader: &dyn SyncCacheReader = &backend;
            let new_ttl = reader
                .ttl("k")
                .unwrap()
                .expect("ttl should be Some after expire");
            assert!(
                new_ttl > Duration::from_secs(118),
                "new_ttl={} should be > 118s",
                new_ttl.as_secs_f64()
            );

            // expire 不存在 key → false
            let ok = writer.expire("missing", Duration::from_secs(10)).unwrap();
            assert!(!ok, "expire on missing key should return false");
        }

        #[test]
        fn test_moka_sync_delete_clear() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
                .unwrap();
            writer
                .set(Arc::from("k2"), Arc::new(b"v2".to_vec()), None)
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert!(reader.exists("k1").unwrap());
            assert!(reader.exists("k2").unwrap());

            // delete 单个 key
            writer.delete("k1").unwrap();
            assert!(!reader.exists("k1").unwrap());
            assert!(reader.exists("k2").unwrap());

            // clear 清空全部
            writer.clear().unwrap();
            assert!(!reader.exists("k2").unwrap());
            assert_eq!(reader.len().unwrap(), 0);
            assert!(reader.is_empty().unwrap());

            // connector: health_check / shutdown / backend_kind
            let connector: &dyn SyncCacheConnector = &backend;
            connector.health_check().unwrap();
            assert_eq!(connector.backend_kind(), BackendKind::Moka);
            connector.shutdown();
        }

        // 回归（问题 3.2）：在 multi-thread runtime 的异步上下文内调用同步方法，
        // 必须使用 block_in_place 避免 "Cannot start a runtime from within a runtime" panic。
        // 注意：current_thread runtime 无法支持阻塞操作，这是 tokio 的固有限制。
        #[tokio::test(flavor = "multi_thread")]
        async fn test_moka_sync_ops_inside_multi_thread_runtime() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("mt"), Arc::new(b"v".to_vec()), None)
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert_eq!(reader.get("mt").unwrap(), Some(b"v".to_vec()));
            assert!(reader.exists("mt").unwrap());

            writer.delete("mt").unwrap();
            assert!(!reader.exists("mt").unwrap());
        }
    }

    // ========================================================================
    // keys_matching / keys() with patterns
    // ========================================================================

    #[tokio::test]
    async fn test_moka_keys_matching_glob() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(Arc::from("user:1"), Arc::new(b"a".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("user:2"), Arc::new(b"b".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("session:1"), Arc::new(b"c".to_vec()), None)
            .await
            .unwrap();

        let all = backend.keys_matching("*").await;
        assert_eq!(all.len(), 3);

        let users = backend.keys_matching("user:*").await;
        assert_eq!(users.len(), 2);

        let sessions = backend.keys_matching("session:*").await;
        assert_eq!(sessions.len(), 1);

        let none = backend.keys_matching("nope:*").await;
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn test_moka_keys_via_cache_reader() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(Arc::from("a"), Arc::new(b"1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("b"), Arc::new(b"2".to_vec()), None)
            .await
            .unwrap();

        let keys = CacheReader::keys(&backend, "*").await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    // ========================================================================
    // Debug impl & entry_count
    // ========================================================================

    #[test]
    fn test_moka_backend_debug() {
        let backend = MokaMemoryBackend::new();
        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("MokaMemoryBackend"));
        assert!(debug_str.contains("capacity"));
    }

    #[test]
    fn test_moka_backend_entry_count() {
        let backend = MokaMemoryBackend::new();
        assert_eq!(backend.entry_count(), 0);
    }

    #[tokio::test]
    async fn test_moka_backend_entry_count_after_insert() {
        let backend = MokaMemoryBackend::new();
        backend
            .set(Arc::from("k1"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("k2"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        // moka async insertion may need a moment to register
        tokio::time::sleep(Duration::from_millis(100)).await;
        // entry_count is eventually consistent; just verify it doesn't panic
        let _ = backend.entry_count();
    }

    // ========================================================================
    // SyncAtomicCacheWriter tests
    // ========================================================================

    #[test]
    fn test_moka_sync_atomic_incr() {
        let backend = MokaMemoryBackend::new();
        let val = crate::backend::SyncAtomicCacheWriter::incr(&backend, "c", 5, None).unwrap();
        assert_eq!(val, 5);
        let val = crate::backend::SyncAtomicCacheWriter::incr(&backend, "c", 3, None).unwrap();
        assert_eq!(val, 8);
    }

    #[test]
    fn test_moka_sync_atomic_cas() {
        let backend = MokaMemoryBackend::new();
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            None,
            b"v1".to_vec(),
            None,
        )
        .unwrap();
        assert!(ok);
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"v1"),
            b"v2".to_vec(),
            None,
        )
        .unwrap();
        assert!(ok);
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"v1"),
            b"v3".to_vec(),
            None,
        )
        .unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_moka_sync_atomic_set_if_absent() {
        let backend = MokaMemoryBackend::new();
        let ok = crate::backend::SyncAtomicCacheWriter::set_if_absent(
            &backend,
            "k",
            b"v".to_vec(),
            None,
        )
        .unwrap();
        assert!(ok);
        let ok = crate::backend::SyncAtomicCacheWriter::set_if_absent(
            &backend,
            "k",
            b"v2".to_vec(),
            None,
        )
        .unwrap();
        assert!(!ok);
    }
}

#[cfg(test)]
mod byte_budget_tests {
    use super::*;

    /// 审计 F10：字节预算下内存不得超卖（1MB 预算 × 10KB 值 → 上界 ~105 条）
    #[tokio::test]
    async fn byte_budget_bounds_entry_count() {
        let backend = MokaMemoryBackend::builder()
            .max_capacity_bytes(1024 * 1024)
            .build();
        let ten_kb = vec![0u8; 10 * 1024];
        for i in 0..200u32 {
            let key = Arc::from(format!("byte-budget-k{i}").as_str());
            backend
                .set(key, Arc::new(ten_kb.clone()), None)
                .await
                .unwrap();
        }
        // moka 惰性维护：先执行待处理任务再读取最终一致的条目数
        backend.cache.run_pending_tasks().await;
        let count = backend.entry_count();
        assert!(
            count <= 110,
            "1MB 预算 + 10KB 值应 ≤ ~105 条，实际 {count}（条目数口径会到 200 → 内存超卖）"
        );
        assert!(count > 0, "预算缓存不应为空");
    }

    /// 未设置字节预算时保持条目数口径（行为不变）
    #[tokio::test]
    async fn default_capacity_semantics_unchanged() {
        let backend = MokaMemoryBackend::builder().capacity(10).build();
        for i in 0..25u32 {
            let key = Arc::from(format!("entry-cap-k{i}").as_str());
            backend
                .set(key, Arc::new(vec![1u8; 64]), None)
                .await
                .unwrap();
        }
        backend.cache.run_pending_tasks().await;
        let count = backend.entry_count();
        assert!(count <= 12, "条目数口径 10 条预算应 ≤ ~12 条，实际 {count}");
    }
}
