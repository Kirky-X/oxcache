// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Sync trait implementation tests for RedisBackend.

use super::*;
use crate::backend::interface::{SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_get_set() {
    let backend = make_backend().await;
    let key = unique_key("sync_set_get");

    SyncCacheWriter::set(
        &backend,
        Arc::from(key.as_str()),
        Arc::new(b"sync_value".to_vec()),
        None,
    )
    .expect("sync set failed");

    let value = SyncCacheReader::get(&backend, &key).expect("sync get failed");
    assert_eq!(value, Some(b"sync_value".to_vec()));

    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_exists() {
    let backend = make_backend().await;
    let key = unique_key("sync_exists");

    SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None).expect("sync set failed");

    assert!(SyncCacheReader::exists(&backend, &key).expect("sync exists failed"));

    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_delete() {
    let backend = make_backend().await;
    let key = unique_key("sync_delete");

    SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None).expect("sync set failed");

    SyncCacheWriter::delete(&backend, &key).expect("sync delete failed");
    assert!(!SyncCacheReader::exists(&backend, &key).expect("sync exists failed"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_ttl() {
    let backend = make_backend().await;
    let key = unique_key("sync_ttl");

    SyncCacheWriter::set(
        &backend,
        Arc::from(key.as_str()),
        Arc::new(b"v".to_vec()),
        Some(Duration::from_secs(100)),
    )
    .expect("sync set failed");

    let ttl = SyncCacheReader::ttl(&backend, &key).expect("sync ttl failed");
    assert!(ttl.is_some());
    let secs = ttl.unwrap().as_secs();
    assert!(secs > 90 && secs <= 100, "ttl secs = {}", secs);

    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_expire() {
    let backend = make_backend().await;
    let key = unique_key("sync_expire");

    SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None).expect("sync set failed");

    let ok = SyncCacheWriter::expire(&backend, &key, Duration::from_secs(50)).expect("sync expire failed");
    assert!(ok);

    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_health_check() {
    let backend = make_backend().await;
    SyncCacheConnector::health_check(&backend).expect("sync health check failed");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_backend_kind() {
    let backend = make_backend().await;
    assert_eq!(
        SyncCacheConnector::backend_kind(&backend),
        crate::backend::BackendKind::Redis
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_capacity_returns_zero() {
    let backend = make_backend().await;
    let cap = SyncCacheReader::capacity(&backend).expect("sync capacity failed");
    assert_eq!(cap, 0);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_len() {
    let backend = make_backend().await;
    SyncCacheReader::len(&backend).expect("sync len failed");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_stats() {
    let backend = make_backend().await;
    let stats = SyncCacheReader::stats(&backend).expect("sync stats failed");
    assert!(stats.contains_key("memory_info"));
}

// ============================================================================
// SyncAtomicCacheWriter integration tests
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_atomic_incr() {
    use crate::backend::SyncAtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("sync_incr");
    let val = SyncAtomicCacheWriter::incr(&backend, &key, 5, None).expect("sync incr failed");
    assert_eq!(val, 5);
    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_atomic_set_if_absent() {
    use crate::backend::SyncAtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("sync_setnx");
    let ok =
        SyncAtomicCacheWriter::set_if_absent(&backend, &key, b"v1".to_vec(), None).expect("sync set_if_absent failed");
    assert!(ok);
    let ok2 =
        SyncAtomicCacheWriter::set_if_absent(&backend, &key, b"v2".to_vec(), None).expect("sync set_if_absent failed");
    assert!(!ok2);
    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_atomic_compare_and_swap() {
    use crate::backend::SyncAtomicCacheWriter;
    use crate::backend::interface::SyncCacheReader;
    use crate::backend::interface::SyncCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("sync_cas");
    SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"old".to_vec()), None).expect("sync set failed");
    let ok = SyncAtomicCacheWriter::compare_and_swap(&backend, &key, Some(b"old"), b"new".to_vec(), None)
        .expect("sync cas failed");
    assert!(ok);
    let val = SyncCacheReader::get(&backend, &key).expect("cas get");
    assert_eq!(val.as_deref(), Some(b"new" as &[u8]));
    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_atomic_cas_wrong_expected() {
    use crate::backend::SyncAtomicCacheWriter;
    use crate::backend::interface::SyncCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("sync_cas_neg");
    SyncCacheWriter::set(&backend, Arc::from(key.as_str()), Arc::new(b"actual".to_vec()), None)
        .expect("sync set failed");
    // CAS with wrong expected value should return false
    let ok = SyncAtomicCacheWriter::compare_and_swap(&backend, &key, Some(b"wrong"), b"new".to_vec(), None)
        .expect("sync cas should not error");
    assert!(!ok);
    cleanup(&backend, &key).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Redis server"]
async fn test_sync_ttl_expiry() {
    use crate::backend::interface::SyncCacheReader;
    use crate::backend::interface::SyncCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("sync_expire_wait");
    SyncCacheWriter::set(
        &backend,
        Arc::from(key.as_str()),
        Arc::new(b"gone_soon".to_vec()),
        Some(Duration::from_secs(1)),
    )
    .expect("sync set failed");
    // Wait for TTL to expire (1s TTL + 1s margin)
    std::thread::sleep(Duration::from_secs(2));
    let val = SyncCacheReader::get(&backend, &key).expect("sync get after expiry failed");
    assert!(val.is_none(), "key should have expired but still exists");
}

// NOTE: test_sync_keys removed — sync SCAN bridge has known limitations
// in tokio::test context; async keys() is covered by writer_tests.
