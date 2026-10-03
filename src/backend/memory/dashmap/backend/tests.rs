#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

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
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();
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
            .set(
                Arc::from("key1"),
                Arc::new(b"value1".to_vec()),
                Some(Duration::from_millis(100)),
            )
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
    // Eviction tests (问题 2.1 / 7.2)
    // ========================================================================

    #[tokio::test]
    async fn test_eviction_fifo_oldest_evicted() {
        // capacity=3，插入 4 个无 TTL 条目，最早插入的 key1 应被淘汰
        let backend = dashmap_memory_with_capacity(3);

        backend
            .set(Arc::from("key1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key2"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key3"), Arc::new(b"v3".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key4"), Arc::new(b"v4".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.entry_count(), 3);
        assert_eq!(
            backend.get("key1").await.unwrap(),
            None,
            "最旧的 key1 应被淘汰"
        );
        assert_eq!(backend.get("key4").await.unwrap(), Some(b"v4".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_no_ttl_entries_are_evictable() {
        // 无 TTL 条目现在也必须能被淘汰（旧实现中被 filter_map 过滤掉）
        let backend = dashmap_memory_with_capacity(2);

        backend
            .set(Arc::from("a"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("b"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("c"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.entry_count(), 2);
        assert_eq!(backend.get("a").await.unwrap(), None);
        assert_eq!(backend.get("b").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_reset_key_stays_fresh() {
        // 满容量后 re-set 已存在的 key 不应立即被淘汰（seq 更新）
        let backend = dashmap_memory_with_capacity(3);

        backend
            .set(Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("k2"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("k3"), Arc::new(b"v3".to_vec()), None)
            .await
            .unwrap();

        // re-set k1：其 FIFO 旧条目作废，新条目在队尾
        backend
            .set(Arc::from("k1"), Arc::new(b"v1b".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.get("k1").await.unwrap(), Some(b"v1b".to_vec()));
        assert_eq!(backend.entry_count(), 3);

        // 再插入新 key，队头的 k2 应被淘汰而非刚 re-set 的 k1
        backend
            .set(Arc::from("k4"), Arc::new(b"v4".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.get("k2").await.unwrap(), None, "应淘汰最早的 k2");
        assert_eq!(backend.get("k1").await.unwrap(), Some(b"v1b".to_vec()));
        assert_eq!(backend.entry_count(), 3);
    }

    #[tokio::test]
    async fn test_eviction_batch_evicts_multiple() {
        // capacity=10，一次性插入 25 条，应批量淘汰到容量内
        let backend = dashmap_memory_with_capacity(10);

        for i in 0..25 {
            backend
                .set(
                    Arc::from(format!("key{i}")),
                    Arc::new(format!("v{i}").into_bytes()),
                    None,
                )
                .await
                .unwrap();
        }

        assert_eq!(backend.entry_count(), 10);
        // 最早插入的 15 条应被淘汰
        assert_eq!(backend.get("key0").await.unwrap(), None);
        assert_eq!(backend.get("key14").await.unwrap(), None);
        assert_eq!(backend.get("key15").await.unwrap(), Some(b"v15".to_vec()));
        assert_eq!(backend.get("key24").await.unwrap(), Some(b"v24".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_expired_entries_evicted() {
        // 有 TTL 的条目过期后，容量检查应将其淘汰
        let backend = dashmap_memory_with_capacity(3);

        backend
            .set(
                Arc::from("short"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();
        backend
            .set(Arc::from("a"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("b"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("c"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        assert_eq!(backend.get("short").await.unwrap(), None, "过期条目不可读");
        assert_eq!(backend.entry_count(), 3);
        assert_eq!(backend.get("a").await.unwrap(), Some(b"v".to_vec()));
    }

    // ========================================================================
    // 过期条目清理测试 (修复验证)
    // ========================================================================

    #[tokio::test]
    async fn test_exists_removes_expired_entry() {
        // exists 检测到过期条目时应原子删除，释放内存
        let backend = dashmap_memory_with_capacity(100);

        backend
            .set(
                Arc::from("expire_me"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        // exists 应返回 false 并从 cache 中移除条目
        assert!(!backend.exists("expire_me").await.unwrap());
        // 验证条目已从底层 DashMap 中删除
        assert_eq!(backend.cache.len(), 0, "过期条目应从 cache 中物理删除");
    }

    #[tokio::test]
    async fn test_ttl_removes_expired_entry() {
        // ttl 检测到过期条目时应原子删除
        let backend = dashmap_memory_with_capacity(100);

        backend
            .set(
                Arc::from("expire_me"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        // ttl 应返回 None 并从 cache 中移除条目
        assert_eq!(backend.ttl("expire_me").await.unwrap(), None);
        assert_eq!(backend.cache.len(), 0, "过期条目应从 cache 中物理删除");
    }

    #[tokio::test]
    async fn test_get_removes_expired_entry() {
        // get 读到过期条目时应物理删除，而非仅返回 None（防条目滞留内存）
        let backend = dashmap_memory_with_capacity(100);

        backend
            .set(
                Arc::from("expire_me"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        assert_eq!(backend.get("expire_me").await.unwrap(), None);
        assert_eq!(backend.cache.len(), 0, "过期条目应从 cache 中物理删除");
    }

    #[tokio::test]
    async fn test_keys_glob_matching() {
        let backend = dashmap_memory_with_capacity(100);

        for key in ["user:1", "user:2", "order:1"] {
            backend
                .set(Arc::from(key), Arc::new(b"v".to_vec()), None)
                .await
                .unwrap();
        }

        let mut users = backend.keys("user:*").await.unwrap();
        users.sort();
        assert_eq!(users, vec!["user:1".to_string(), "user:2".to_string()]);
        assert_eq!(
            backend.keys("missing:*").await.unwrap(),
            Vec::<String>::new()
        );
    }

    #[tokio::test]
    async fn test_delete_compacts_stale_fifo() {
        // 大量删除后 FIFO 中陈旧条目（含被删 key 字符串）应被紧缩回收；
        // 紧缩阈值为 max(4 × 存活条目数, 1024)，需超过阈值才能观察到重建
        let backend = dashmap_memory_with_capacity(4000);

        for i in 0..3000u64 {
            backend
                .set(
                    Arc::from(format!("k{i}").as_str()),
                    Arc::new(b"v".to_vec()),
                    None,
                )
                .await
                .unwrap();
        }
        // 删到只剩 100 条：FIFO 曾达 3000（> 1024），删除路径的紧缩必须生效
        for i in 0..2900u64 {
            backend.delete(format!("k{i}").as_str()).await.unwrap();
        }

        let cache_len = backend.cache.len();
        let fifo_len = backend.fifo.lock().unwrap().len();
        assert_eq!(cache_len, 100);
        assert!(
            fifo_len < 1500,
            "删除后 FIFO 陈旧条目应被紧缩: fifo_len={fifo_len}"
        );
        assert!(
            fifo_len <= 1024.max(cache_len * 4),
            "FIFO 应满足紧缩不变量: fifo_len={fifo_len}, cache_len={cache_len}"
        );
    }

    #[tokio::test]
    async fn test_eviction_expired_entry_with_stale_seq() {
        // 过期条目即使 seq 不匹配（被 re-set 过）也应被淘汰
        let backend = dashmap_memory_with_capacity(2);

        // 插入短 TTL 条目
        backend
            .set(
                Arc::from("ttl_key"),
                Arc::new(b"v1".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();
        backend
            .set(Arc::from("other"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();

        // re-set ttl_key，使旧 FIFO 条目 seq 失效，但新条目仍然有短 TTL
        backend
            .set(
                Arc::from("ttl_key"),
                Arc::new(b"v1b".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        // 插入新条目触发淘汰，过期的 ttl_key 应被淘汰（即使 seq 不匹配旧 FIFO 条目）
        backend
            .set(Arc::from("new_key"), Arc::new(b"v3".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(
            backend.get("ttl_key").await.unwrap(),
            None,
            "过期条目应被淘汰"
        );
        assert_eq!(backend.get("other").await.unwrap(), Some(b"v2".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_stress_100_writers() {
        // 100 并发 writer + capacity=1000，验证高并发下容量不超限
        let backend = dashmap_memory_with_capacity(1000);

        let mut handles = Vec::new();
        for w in 0..100u64 {
            let backend = backend.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..200u64 {
                    let key = format!("w{w}_k{i}");
                    backend
                        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
                        .await
                        .unwrap();
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert!(
            backend.entry_count() <= 1000,
            "capacity exceeded: {}",
            backend.entry_count()
        );
        // 总数 20000 条 > capacity 1000，必然发生过淘汰
        assert_eq!(backend.entry_count(), 1000);
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
        use crate::backend::{BackendKind, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
        use std::sync::Arc;
        use std::time::Duration;

        #[test]
        fn test_dashmap_sync_get_set_basic() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
                .unwrap();

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

            // DashMap 在访问时按 expires_at 校验，无后台驱逐；等待过期后读应返回 None
            std::thread::sleep(Duration::from_millis(120));
            assert_eq!(reader.get("k").unwrap(), None);
            assert!(!reader.exists("k").unwrap());
        }

        #[test]
        fn test_dashmap_sync_expire() {
            let backend = DashMapMemoryBackend::new();

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

            // connector: health_check / shutdown / backend_kind
            let connector: &dyn SyncCacheConnector = &backend;
            connector.health_check().unwrap();
            assert_eq!(connector.backend_kind(), BackendKind::DashMap);
            connector.shutdown();
        }
    }
}

#[cfg(test)]
mod byte_budget_tests {
    use super::*;

    fn budget_backend(max_bytes: u64) -> DashMapMemoryBackend {
        DashMapMemoryBackend::builder()
            .capacity(10_000)
            .max_capacity_bytes(max_bytes)
            .build()
    }

    /// 审计 F10：字节预算下 16 × 8KB 写入 64KB 预算必须触发淘汰（≤ 10 条）
    #[tokio::test]
    async fn byte_budget_bounds_entry_count() {
        let backend = budget_backend(64 * 1024);
        let eight_kb = vec![0u8; 8 * 1024];
        for i in 0..16u32 {
            let key = Arc::from(format!("dashmap-budget-k{i}").as_str());
            backend
                .set(key, Arc::new(eight_kb.clone()), None)
                .await
                .unwrap();
        }
        let count = backend.entry_count();
        assert!(count <= 10, "64KB 预算 + 8KB 值应 ≤ ~9 条，实际 {count}");
        let used = backend.bytes_used.load(Ordering::Relaxed);
        assert!(used <= 72 * 1024, "bytes_used {used} 不得超过预算+单批余量");
    }

    /// delete 精确递减字节记账；clear 归零
    #[tokio::test]
    async fn byte_accounting_on_delete_and_clear() {
        let backend = budget_backend(64 * 1024);
        let eight_kb = vec![0u8; 8 * 1024];
        for i in 0..4u32 {
            let key = Arc::from(format!("dashmap-acc-k{i}").as_str());
            backend
                .set(key, Arc::new(eight_kb.clone()), None)
                .await
                .unwrap();
        }
        assert_eq!(backend.bytes_used.load(Ordering::Relaxed), 32 * 1024);

        backend.delete("dashmap-acc-k0").await.unwrap();
        assert_eq!(backend.bytes_used.load(Ordering::Relaxed), 24 * 1024);

        backend.clear().await.unwrap();
        assert_eq!(backend.bytes_used.load(Ordering::Relaxed), 0);
    }

    /// 未设置字节预算时条目数口径不变（行为回归）
    #[tokio::test]
    async fn default_entry_capacity_unchanged() {
        let backend = DashMapMemoryBackend::builder().capacity(10).build();
        for i in 0..25u32 {
            let key = Arc::from(format!("dashmap-entry-cap-k{i}").as_str());
            backend
                .set(key, Arc::new(vec![1u8; 64]), None)
                .await
                .unwrap();
        }
        assert!(
            backend.entry_count() <= 12,
            "条目数口径 10 条预算应 ≤ ~12 条"
        );
    }

    /// sync 路径与 async 路径共用同一记账
    #[test]
    fn sync_set_delete_tracks_bytes() {
        let backend = budget_backend(64 * 1024);
        use crate::backend::SyncCacheWriter;
        SyncCacheWriter::set(
            &backend,
            Arc::from(String::from("sync-acc-k")),
            Arc::new(vec![0u8; 8 * 1024]),
            None,
        )
        .unwrap();
        assert_eq!(backend.bytes_used.load(Ordering::Relaxed), 8 * 1024);
        SyncCacheWriter::delete(&backend, "sync-acc-k").unwrap();
        assert_eq!(backend.bytes_used.load(Ordering::Relaxed), 0);
    }
}
