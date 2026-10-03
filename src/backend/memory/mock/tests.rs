#[allow(unused_imports)]
pub use super::*;

#[cfg(test)]
mod mock_tests {
    use super::*;
    use crate::backend::{
        AtomicCacheWriter, BackendScore, CacheConnector, CacheReader, CacheWriter,
    };

    #[tokio::test]
    async fn test_mock_backend_new() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(BackendScore::score(&backend), 50);
        assert_eq!(BackendScore::backend_name(&backend), "test");
        assert!(!BackendScore::is_persistent(&backend));
    }

    #[tokio::test]
    async fn test_mock_backend_set_get() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(
            &backend,
            Arc::from("key"),
            Arc::new(b"value".to_vec()),
            None,
        )
        .await
        .unwrap();
        let result = CacheReader::get(&backend, "key").await.unwrap();
        assert_eq!(result, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_mock_backend_delete() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(
            &backend,
            Arc::from("key"),
            Arc::new(b"value".to_vec()),
            None,
        )
        .await
        .unwrap();
        CacheWriter::delete(&backend, "key").await.unwrap();
        assert!(CacheReader::get(&backend, "key").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mock_backend_clear() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(&backend, Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        CacheWriter::clear(&backend).await.unwrap();
        assert!(CacheReader::is_empty(&backend).await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_backend_exists() {
        let backend = MockBackend::new("test", 50, false);
        assert!(!CacheReader::exists(&backend, "key").await.unwrap());
        CacheWriter::set(
            &backend,
            Arc::from("key"),
            Arc::new(b"value".to_vec()),
            None,
        )
        .await
        .unwrap();
        assert!(CacheReader::exists(&backend, "key").await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_backend_len() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(CacheReader::len(&backend).await.unwrap(), 0);
        CacheWriter::set(&backend, Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(CacheReader::len(&backend).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_mock_backend_stats() {
        let backend = MockBackend::new("test", 50, false);
        let stats = CacheReader::stats(&backend).await.unwrap();
        assert_eq!(stats.get("type"), Some(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_mock_backend_health_check() {
        let backend = MockBackend::new("test", 50, false);
        assert!(CacheConnector::health_check(&backend).await.is_ok());
    }

    #[tokio::test]
    async fn test_mock_backend_shutdown() {
        let backend = MockBackend::new("test", 50, false);
        CacheConnector::shutdown(&backend).await;
    }

    #[test]
    fn test_mock_backend_kind() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(
            CacheConnector::backend_kind(&backend),
            crate::backend::interface::BackendKind::Mock
        );
    }

    // ========================================================================
    // 故障注入测试 (问题 5.1 / 7.3)
    // ========================================================================

    #[tokio::test]
    async fn test_mock_backend_fault_injected_get_returns_err() {
        let backend = MockBackend::new("failing", 50, false).with_fail_get();
        assert!(backend.fails_get());
        let result = CacheReader::get(&backend, "key").await;
        assert!(result.is_err(), "fail_get 注入后 get 应返回错误");
    }

    #[tokio::test]
    async fn test_mock_backend_fault_injected_set_returns_err() {
        let backend = MockBackend::new("failing", 50, false).with_fail_set();
        let result =
            CacheWriter::set(&backend, Arc::from("key"), Arc::new(b"v".to_vec()), None).await;
        assert!(result.is_err(), "fail_set 注入后 set 应返回错误");
    }

    #[tokio::test]
    async fn test_mock_backend_fault_injected_health_returns_err() {
        let backend = MockBackend::new("failing", 50, false).with_fail_health();
        let result = CacheConnector::health_check(&backend).await;
        assert!(
            result.is_err(),
            "fail_health 注入后 health_check 应返回错误"
        );
    }

    #[tokio::test]
    async fn test_mock_backend_persistent() {
        let backend = MockBackend::new("test", 50, true);
        assert!(BackendScore::is_persistent(&backend));
    }

    // ========================================================================
    // Per-entry TTL tests (spec: universal-per-entry-ttl)
    // ========================================================================

    #[tokio::test]
    async fn test_mock_set_with_ttl_expires_after_timeout() {
        let backend = MockBackend::new("test", 50, false);
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
        assert_eq!(backend.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_set_without_ttl_never_expires() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_mock_ttl_returns_remaining() {
        let backend = MockBackend::new("test", 50, false);
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
    async fn test_mock_ttl_returns_none_for_missing_key() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(backend.ttl("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_ttl_returns_none_for_no_ttl_key() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.ttl("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_expire_extends_ttl() {
        let backend = MockBackend::new("test", 50, false);
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
    async fn test_mock_expire_missing_key_returns_false() {
        let backend = MockBackend::new("test", 50, false);
        let ok = backend
            .expire("missing", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!ok, "expire on missing key should return false");
    }

    #[tokio::test]
    async fn test_mock_lazy_cleanup_removes_expired_entry() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(50)),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // 触发 lazy 过期清理
        let _ = backend.get("k").await.unwrap();
        // 内部 HashMap 中 "k" 应已删除
        let data = backend.data.read().await;
        assert!(
            !data.contains_key("k"),
            "expired entry should be lazily removed"
        );
    }

    // ========================================================================
    // keys() with glob patterns
    // ========================================================================

    #[tokio::test]
    async fn test_mock_keys_returns_matching_keys() {
        let backend = MockBackend::new("test", 50, false);
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

        let all_keys = backend.keys("*").await.unwrap();
        assert_eq!(all_keys.len(), 3);

        let user_keys = backend.keys("user:*").await.unwrap();
        assert_eq!(user_keys.len(), 2);

        let session_keys = backend.keys("session:*").await.unwrap();
        assert_eq!(session_keys.len(), 1);

        let no_match = backend.keys("nope:*").await.unwrap();
        assert!(no_match.is_empty());
    }

    #[tokio::test]
    async fn test_mock_keys_lazy_expires_during_scan() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("alive"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(
                Arc::from("dying"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;

        let keys = backend.keys("*").await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "alive");
    }

    // ========================================================================
    // capacity() and as_atomic_writer()
    // ========================================================================

    #[tokio::test]
    async fn test_mock_capacity_returns_zero() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(backend.capacity().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mock_as_atomic_writer_returns_some() {
        let backend = MockBackend::new("test", 50, false);
        let writer = CacheConnector::as_atomic_writer(&backend);
        assert!(
            writer.is_some(),
            "MockBackend should implement AtomicCacheWriter"
        );
    }

    // ========================================================================
    // AtomicCacheWriter direct tests (async)
    // ========================================================================

    #[tokio::test]
    async fn test_mock_atomic_incr_from_zero() {
        let backend = MockBackend::new("test", 50, false);
        let val = AtomicCacheWriter::incr(&backend, "counter", 1, None)
            .await
            .unwrap();
        assert_eq!(val, 1);
    }

    #[tokio::test]
    async fn test_mock_atomic_incr_accumulates() {
        let backend = MockBackend::new("test", 50, false);
        AtomicCacheWriter::incr(&backend, "c", 10, None)
            .await
            .unwrap();
        let val = AtomicCacheWriter::incr(&backend, "c", 5, None)
            .await
            .unwrap();
        assert_eq!(val, 15);
    }

    #[tokio::test]
    async fn test_mock_atomic_incr_negative_delta() {
        let backend = MockBackend::new("test", 50, false);
        AtomicCacheWriter::incr(&backend, "c", 10, None)
            .await
            .unwrap();
        let val = AtomicCacheWriter::incr(&backend, "c", -3, None)
            .await
            .unwrap();
        assert_eq!(val, 7);
    }

    #[tokio::test]
    async fn test_mock_atomic_cas_setnx() {
        let backend = MockBackend::new("test", 50, false);
        // CAS with None = SETNX
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v1".to_vec(), None)
            .await
            .unwrap();
        assert!(ok);
        // Second SETNX should fail
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v2".to_vec(), None)
            .await
            .unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_mock_atomic_cas_with_expected() {
        let backend = MockBackend::new("test", 50, false);
        AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v1".to_vec(), None)
            .await
            .unwrap();
        // CAS with correct expected → success
        let ok =
            AtomicCacheWriter::compare_and_swap(&backend, "k", Some(b"v1"), b"v2".to_vec(), None)
                .await
                .unwrap();
        assert!(ok);
        // CAS with wrong expected → fail
        let ok =
            AtomicCacheWriter::compare_and_swap(&backend, "k", Some(b"v1"), b"v3".to_vec(), None)
                .await
                .unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_mock_atomic_cas_missing_key_with_expected() {
        let backend = MockBackend::new("test", 50, false);
        let ok = AtomicCacheWriter::compare_and_swap(
            &backend,
            "missing",
            Some(b"v1"),
            b"v2".to_vec(),
            None,
        )
        .await
        .unwrap();
        assert!(!ok, "CAS on missing key with expected should fail");
    }

    #[tokio::test]
    async fn test_mock_atomic_set_if_absent() {
        let backend = MockBackend::new("test", 50, false);
        let ok = AtomicCacheWriter::set_if_absent(&backend, "k", b"v".to_vec(), None)
            .await
            .unwrap();
        assert!(ok);
        let ok = AtomicCacheWriter::set_if_absent(&backend, "k", b"v2".to_vec(), None)
            .await
            .unwrap();
        assert!(!ok);
    }

    // ========================================================================
    // SyncAtomicCacheWriter direct tests
    // ========================================================================

    #[test]
    fn test_mock_sync_atomic_incr() {
        let backend = MockBackend::new("test", 50, false);
        let val = crate::backend::SyncAtomicCacheWriter::incr(&backend, "c", 5, None).unwrap();
        assert_eq!(val, 5);
        let val = crate::backend::SyncAtomicCacheWriter::incr(&backend, "c", 3, None).unwrap();
        assert_eq!(val, 8);
    }

    #[test]
    fn test_mock_sync_atomic_incr_error_paths_match_async() {
        use crate::backend::SyncAtomicCacheWriter;

        let backend = MockBackend::new("test", 50, false);
        // Non-numeric stored value → Err (not silently treated as 0)
        backend
            .data
            .blocking_write()
            .insert("bad".to_string(), (b"not-a-number".to_vec(), None));
        let err = SyncAtomicCacheWriter::incr(&backend, "bad", 1, None).unwrap_err();
        assert!(
            err.to_string().contains("invalid integer"),
            "unexpected error: {err}"
        );

        // Overflow → Err
        backend
            .data
            .blocking_write()
            .insert("big".to_string(), (i64::MAX.to_string().into_bytes(), None));
        let err = SyncAtomicCacheWriter::incr(&backend, "big", 1, None).unwrap_err();
        assert!(
            err.to_string().contains("overflow"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_mock_sync_atomic_cas() {
        let backend = MockBackend::new("test", 50, false);
        // SETNX
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            None,
            b"v1".to_vec(),
            None,
        )
        .unwrap();
        assert!(ok);
        // CAS correct expected
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"v1"),
            b"v2".to_vec(),
            None,
        )
        .unwrap();
        assert!(ok);
        // CAS wrong expected
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
    fn test_mock_sync_atomic_set_if_absent() {
        let backend = MockBackend::new("test", 50, false);
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

    // ========================================================================
    // exists() with expired entry
    // ========================================================================

    #[tokio::test]
    async fn test_mock_exists_lazy_expires() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(
                Arc::from("k"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();
        assert!(backend.exists("k").await.unwrap());
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(
            !backend.exists("k").await.unwrap(),
            "expired entry should report not exists"
        );
    }
}

// ============================================================================
// 原子写（AtomicCacheWriter / SyncAtomicCacheWriter）语义驱动
// ============================================================================

#[tokio::test]
async fn mock_async_incr_counts_overflow_and_bad_payload() {
    let backend = MockBackend::new("atomic", 50, false);
    use crate::backend::{AtomicCacheWriter, CacheReader, CacheWriter};

    assert_eq!(
        AtomicCacheWriter::incr(&backend, "n", 3, None)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        AtomicCacheWriter::incr(&backend, "n", 4, None)
            .await
            .unwrap(),
        7
    );
    // 带 TTL 计数后可读到 TTL
    assert!(
        AtomicCacheWriter::incr(&backend, "t", 1, Some(Duration::from_secs(30)))
            .await
            .is_ok()
    );
    assert!(CacheReader::ttl(&backend, "t").await.unwrap().is_some());

    // i64 溢出：显性报错
    CacheWriter::set(
        &backend,
        Arc::from("big"),
        Arc::new(i64::MAX.to_string().into_bytes()),
        None,
    )
    .await
    .unwrap();
    let err = AtomicCacheWriter::incr(&backend, "big", 1, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("overflow"), "got: {err}");

    // 非 UTF-8：显性报错（不静默清零）
    CacheWriter::set(&backend, Arc::from("bin"), Arc::new(vec![0xff]), None)
        .await
        .unwrap();
    let err = AtomicCacheWriter::incr(&backend, "bin", 1, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("UTF-8"), "got: {err}");

    // 非整数文本：显性报错
    CacheWriter::set(&backend, Arc::from("txt"), Arc::new(b"abc".to_vec()), None)
        .await
        .unwrap();
    let err = AtomicCacheWriter::incr(&backend, "txt", 1, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("integer"), "got: {err}");
}

#[tokio::test]
async fn mock_async_cas_setnx_and_set_if_absent_branches() {
    let backend = MockBackend::new("atomic", 50, false);
    use crate::backend::AtomicCacheWriter;

    // SETNX 两分支
    assert!(
        AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"a".to_vec(), None)
            .await
            .unwrap()
    );
    assert!(
        !AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"b".to_vec(), None)
            .await
            .unwrap()
    );
    // CAS 命中 / 未命中 / 键不存在
    assert!(
        AtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"a".as_slice()),
            b"b".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    assert!(
        !AtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"a".as_slice()),
            b"c".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    assert!(
        !AtomicCacheWriter::compare_and_swap(
            &backend,
            "ghost",
            Some(b"x".as_slice()),
            b"y".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    // set_if_absent 两分支（带 TTL）
    assert!(
        AtomicCacheWriter::set_if_absent(
            &backend,
            "s",
            b"x".to_vec(),
            Some(Duration::from_secs(5))
        )
        .await
        .unwrap()
    );
    assert!(
        !AtomicCacheWriter::set_if_absent(&backend, "s", b"y".to_vec(), None)
            .await
            .unwrap()
    );
}

#[test]
fn mock_sync_atomic_writer_mirrors_async_semantics() {
    let backend = MockBackend::new("sync-atomic", 50, false);
    use crate::backend::SyncAtomicCacheWriter;

    assert_eq!(
        SyncAtomicCacheWriter::incr(&backend, "n", 5, None).unwrap(),
        5
    );
    assert_eq!(
        SyncAtomicCacheWriter::incr(&backend, "n", -2, None).unwrap(),
        3
    );
    // 溢出：先以 SETNX 预置 i64::MAX，再自增必报错
    assert!(
        SyncAtomicCacheWriter::set_if_absent(
            &backend,
            "big",
            i64::MAX.to_string().into_bytes(),
            None
        )
        .unwrap()
    );
    let err = SyncAtomicCacheWriter::incr(&backend, "big", 1, None).unwrap_err();
    assert!(err.to_string().contains("overflow"), "got: {err}");
    // SETNX / CAS
    assert!(
        SyncAtomicCacheWriter::compare_and_swap(&backend, "k", None, b"a".to_vec(), None).unwrap()
    );
    assert!(
        !SyncAtomicCacheWriter::compare_and_swap(&backend, "k", None, b"b".to_vec(), None).unwrap()
    );
    assert!(
        SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"a".as_slice()),
            b"b".to_vec(),
            None
        )
        .unwrap()
    );
    assert!(
        !SyncAtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"a".as_slice()),
            b"c".to_vec(),
            None
        )
        .unwrap()
    );
    assert!(SyncAtomicCacheWriter::set_if_absent(&backend, "s", b"x".to_vec(), None).unwrap());
    assert!(!SyncAtomicCacheWriter::set_if_absent(&backend, "s", b"y".to_vec(), None).unwrap());
}
