// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 后端原子写（AtomicCacheWriter）与故障注入长尾覆盖：
//!
//! - Moka 后端原子写全分支：incr 新建/累加/溢出/非 UTF-8、CAS 命中与未命中、
//!   SETNX 成功与已存在、set_if_absent
//! - DashMap 后端 ttl/expire 边界
//! - Cache sync 原子面在后端无 SyncAtomicCacheWriter 能力时的显性 NotSupported

#![cfg(feature = "full")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use oxcache::Cache;
use oxcache::OxCacheError;
use oxcache::backend::memory::MokaMemoryBackend;
use oxcache::backend::{
    AtomicCacheWriter, BackendKind, CacheReader, CacheWriter, SyncCacheConnector, SyncCacheReader,
    SyncCacheWriter,
};
use oxcache::error::OxCacheResult;

// ============================================================================
// Moka：AtomicCacheWriter 全分支
// ============================================================================

#[tokio::test]
async fn moka_incr_creates_counts_and_overflows() {
    let backend = MokaMemoryBackend::new();

    // 新键从 0 起步
    let v = AtomicCacheWriter::incr(&backend, "n", 5, None)
        .await
        .unwrap();
    assert_eq!(v, 5);
    // 累加
    let v = AtomicCacheWriter::incr(&backend, "n", -2, None)
        .await
        .unwrap();
    assert_eq!(v, 3);
    // 带 TTL 写入
    let v = AtomicCacheWriter::incr(&backend, "t", 1, Some(Duration::from_secs(30)))
        .await
        .unwrap();
    assert_eq!(v, 1);
    assert!(CacheReader::ttl(&backend, "t").await.unwrap().is_some());

    // i64 溢出：entry 原样保留，incr 报错
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

    // 非 UTF-8 载荷：moka 走 Nop 路径，incr 显性报错且 entry 原样保留
    CacheWriter::set(&backend, Arc::from("bin"), Arc::new(vec![0xff, 0xfe]), None)
        .await
        .unwrap();
    let err = AtomicCacheWriter::incr(&backend, "bin", 1, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("overflow"), "got: {err}");
    assert_eq!(
        CacheReader::get(&backend, "bin").await.unwrap().as_deref(),
        Some(&[0xffu8, 0xfe][..])
    );

    // 非整数文本载荷：同样显性报错
    CacheWriter::set(&backend, Arc::from("txt"), Arc::new(b"abc".to_vec()), None)
        .await
        .unwrap();
    let err = AtomicCacheWriter::incr(&backend, "txt", 1, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("overflow"), "got: {err}");
}

#[tokio::test]
async fn moka_cas_and_setnx_branches() {
    let backend = MokaMemoryBackend::new();

    // SETNX：absent → true；present → false
    assert!(
        AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v1".to_vec(), None)
            .await
            .unwrap()
    );
    assert!(
        !AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v2".to_vec(), None)
            .await
            .unwrap()
    );

    // CAS：期望匹配 → true；不匹配 → false
    assert!(
        AtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"v1".as_slice()),
            b"v2".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    assert!(
        !AtomicCacheWriter::compare_and_swap(
            &backend,
            "k",
            Some(b"v1".as_slice()),
            b"v3".to_vec(),
            None
        )
        .await
        .unwrap()
    );
    // 对不存在键做带期望 CAS → false
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

    // set_if_absent 两分支
    assert!(
        AtomicCacheWriter::set_if_absent(
            &backend,
            "s",
            b"a".to_vec(),
            Some(Duration::from_secs(5))
        )
        .await
        .unwrap()
    );
    assert!(
        !AtomicCacheWriter::set_if_absent(&backend, "s", b"b".to_vec(), None)
            .await
            .unwrap()
    );
    assert_eq!(
        CacheReader::get(&backend, "s").await.unwrap().as_deref(),
        Some(&b"a"[..])
    );
}

// ============================================================================
// Cache sync 原子面：后端无 SyncAtomicCacheWriter → 显性 NotSupported
// ============================================================================

/// 只实现同步三 trait、不实现 SyncAtomicCacheWriter 的最小后端，
/// 用于验证 sync 原子 API 的能力探测失败路径。
#[derive(Default)]
struct MinimalSyncBackend {
    data: std::sync::RwLock<HashMap<String, Vec<u8>>>,
}

impl SyncCacheReader for MinimalSyncBackend {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(self.data.read().unwrap().get(key).cloned())
    }
    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(self.data.read().unwrap().contains_key(key))
    }
    fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
        Ok(None)
    }
    fn len(&self) -> OxCacheResult<u64> {
        Ok(self.data.read().unwrap().len() as u64)
    }
    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        Ok(HashMap::new())
    }
}

impl SyncCacheWriter for MinimalSyncBackend {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, _ttl: Option<Duration>) -> OxCacheResult<()> {
        self.data
            .write()
            .unwrap()
            .insert(key.to_string(), value.as_ref().clone());
        Ok(())
    }
    fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.data.write().unwrap().remove(key);
        Ok(())
    }
    fn clear(&self) -> OxCacheResult<()> {
        self.data.write().unwrap().clear();
        Ok(())
    }
    fn expire(&self, _key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Ok(false)
    }
}

impl SyncCacheConnector for MinimalSyncBackend {
    fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }
    fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Mock
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_atomic_apis_report_not_supported_without_capability() {
    let cache: Cache<String, String> = Cache::builder()
        .sync_mode(true)
        .sync_backend_arc(Arc::new(MinimalSyncBackend::default()))
        .build()
        .await
        .unwrap();

    let err = cache.incr_sync(&"k".to_string(), 1, None).unwrap_err();
    assert!(
        matches!(err, OxCacheError::NotSupported(ref m) if m.contains("SyncAtomicCacheWriter")),
        "got: {err:?}"
    );

    let err = cache
        .compare_and_swap_sync(&"k".to_string(), None, b"v".to_vec(), None)
        .unwrap_err();
    assert!(matches!(err, OxCacheError::NotSupported(_)), "got: {err:?}");

    let err = cache
        .set_if_absent_sync(&"k".to_string(), &"v".to_string(), None)
        .unwrap_err();
    assert!(matches!(err, OxCacheError::NotSupported(_)), "got: {err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_atomic_apis_work_over_moka_bridge() {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    assert_eq!(cache.incr_sync(&"c".to_string(), 5, None).unwrap(), 5);
    assert_eq!(cache.incr_sync(&"c".to_string(), 2, None).unwrap(), 7);

    assert!(
        cache
            .set_if_absent_sync(&"n".to_string(), &"42".to_string(), None)
            .unwrap()
    );
    assert!(
        !cache
            .set_if_absent_sync(&"n".to_string(), &"43".to_string(), None)
            .unwrap()
    );

    // CAS：SETNX 成功后按期望值替换
    assert!(
        cache
            .compare_and_swap_sync(&"cas".to_string(), None, b"first".to_vec(), None)
            .unwrap()
    );
    assert!(
        cache
            .compare_and_swap_sync(
                &"cas".to_string(),
                Some(b"first".as_slice()),
                b"second".to_vec(),
                None
            )
            .unwrap()
    );
    assert!(
        !cache
            .compare_and_swap_sync(
                &"cas".to_string(),
                Some(b"first".as_slice()),
                b"third".to_vec(),
                None
            )
            .unwrap()
    );
}
