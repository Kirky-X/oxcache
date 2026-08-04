// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! CacheWriter tests for RedisBackend.

use super::*;
use crate::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::backend::BackendScore;
use crate::error::OxCacheError;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_empty_key_rejected() {
    let backend = make_backend().await;
    let result = backend.set(Arc::from(""), Arc::new(b"v".to_vec()), None).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_delete_empty_key_rejected() {
    let backend = make_backend().await;
    let result = backend.delete("").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_expire_empty_key_rejected() {
    let backend = make_backend().await;
    let result = backend.expire("", Duration::from_secs(10)).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_delete_removes_key() {
    let backend = make_backend().await;
    let key = unique_key("del");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set failed");
    assert!(backend.exists(&key).await.unwrap());
    backend.delete(&key).await.expect("delete failed");
    assert!(!backend.exists(&key).await.unwrap());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_delete_nonexistent_is_ok() {
    let backend = make_backend().await;
    let key = unique_key("del_missing");
    backend.delete(&key).await.expect("delete missing key should be ok");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_expire_sets_ttl_on_existing_key() {
    let backend = make_backend().await;
    let key = unique_key("expire_ok");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set failed");
    let ok = backend
        .expire(&key, Duration::from_secs(50))
        .await
        .expect("expire failed");
    assert!(ok, "expire should return true for existing key");
    let ttl = backend.ttl(&key).await.unwrap();
    assert!(ttl.is_some());
    let secs = ttl.unwrap().as_secs();
    assert!(secs > 40 && secs <= 50, "ttl secs = {}", secs);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_expire_returns_false_for_missing_key() {
    let backend = make_backend().await;
    let key = unique_key("expire_missing");
    let ok = backend
        .expire(&key, Duration::from_secs(50))
        .await
        .expect("expire call failed");
    assert!(!ok, "expire should return false for missing key");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_with_ttl_expires() {
    let backend = make_backend().await;
    let key = unique_key("short_ttl");
    backend
        .set(
            Arc::from(key.as_str()),
            Arc::new(b"v".to_vec()),
            Some(Duration::from_secs(1)),
        )
        .await
        .expect("set failed");
    assert!(backend.exists(&key).await.unwrap());
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(!backend.exists(&key).await.unwrap());
}

// Batch operations

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_and_get_many() {
    let backend = make_backend().await;
    let k1 = unique_key("m1");
    let k2 = unique_key("m2");
    let k3 = unique_key("m3");
    let items = vec![
        (Arc::from(k1.clone()), Arc::new(b"v1".to_vec()), None),
        (Arc::from(k2.clone()), Arc::new(b"v2".to_vec()), None),
        (Arc::from(k3.clone()), Arc::new(b"v3".to_vec()), None),
    ];
    backend.set_many(&items).await.expect("set_many failed");

    let keys = vec![k1.clone(), k2.clone(), k3.clone()];
    let values = backend.get_many(&keys).await.expect("get_many failed");
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], Some(b"v1".to_vec()));
    assert_eq!(values[1], Some(b"v2".to_vec()));
    assert_eq!(values[2], Some(b"v3".to_vec()));

    backend.delete_many(&keys).await.expect("delete_many failed");
    for k in &keys {
        assert!(!backend.exists(k).await.unwrap());
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_empty_is_ok() {
    let backend = make_backend().await;
    backend.set_many(&[]).await.expect("set_many empty should be ok");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_many_empty_returns_empty() {
    let backend = make_backend().await;
    let result = backend.get_many(&[]).await.expect("get_many empty failed");
    assert!(result.is_empty());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_delete_many_empty_is_ok() {
    let backend = make_backend().await;
    backend.delete_many(&[]).await.expect("delete_many empty should be ok");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_many_with_missing_keys() {
    let backend = make_backend().await;
    let k1 = unique_key("gm_present");
    let k2 = unique_key("gm_absent");
    backend
        .set(Arc::from(k1.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .unwrap();
    let keys = vec![k1.clone(), k2.clone()];
    let values = backend.get_many(&keys).await.expect("get_many failed");
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Some(b"v".to_vec()));
    assert_eq!(values[1], None);
    cleanup(&backend, &k1).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_with_ttl() {
    let backend = make_backend().await;
    let k1 = unique_key("mttl1");
    let k2 = unique_key("mttl2");
    let items = vec![
        (Arc::from(k1.clone()), Arc::new(b"v1".to_vec()), Some(Duration::from_secs(100))),
        (Arc::from(k2.clone()), Arc::new(b"v2".to_vec()), Some(Duration::from_secs(100))),
    ];
    backend.set_many(&items).await.expect("set_many failed");
    let ttl1 = backend.ttl(&k1).await.unwrap();
    let ttl2 = backend.ttl(&k2).await.unwrap();
    assert!(ttl1.is_some() && ttl1.unwrap().as_secs() > 90);
    assert!(ttl2.is_some() && ttl2.unwrap().as_secs() > 90);
    cleanup(&backend, &k1).await;
    cleanup(&backend, &k2).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_many_with_invalid_key_rejected() {
    let backend = make_backend().await;
    let items = vec![
        (Arc::from("valid_key".to_string()), Arc::new(b"v".to_vec()), None),
        (Arc::from("bad;key".to_string()), Arc::new(b"v".to_vec()), None),
    ];
    let result = backend.set_many(&items).await;
    assert!(result.is_err());
}

// Accessor / metadata tests

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_mode_accessor() {
    let backend = make_backend().await;
    assert_eq!(backend.mode(), crate::core::RedisModeType::Standalone);
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_client_accessor() {
    let backend = make_backend().await;
    let client: &redis::Client = backend.client();
    // Verify the client reference is valid by checking its connection string
    let _ = format!("{:?}", client);
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_backend_kind_is_redis() {
    let backend = make_backend().await;
    assert_eq!(backend.backend_kind(), BackendKind::Redis);
    assert!(backend.backend_kind().is_distributed());
    assert!(!backend.backend_kind().is_memory());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_backend_score() {
    let backend = make_backend().await;
    assert_eq!(backend.score(), 50);
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_is_persistent_true() {
    let backend = make_backend().await;
    assert!(backend.is_persistent());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_backend_name() {
    let backend = make_backend().await;
    assert_eq!(backend.backend_name(), "redis");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_shutdown_is_noop() {
    let backend = make_backend().await;
    backend.shutdown().await;
    backend.health_check().await.expect("health check after shutdown failed");
}

// clear test (uses separate DB)

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_clear_removes_all_keys() {
    // clear requires dangerous_clear_enabled
    // For this test, we use the builder with the flag enabled
    set_allow_insecure_env();
    let backend = RedisBackend::builder()
        .connection_string(REDIS_URL_DB1)
        .dangerous_clear_enabled(true)
        .build()
        .await
        .expect("Failed to connect");

    let key = unique_key("clear_target");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set failed");
    assert!(backend.exists(&key).await.unwrap());

    backend.clear().await.expect("clear failed");
    assert!(!backend.exists(&key).await.unwrap());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_clear_disabled_by_default() {
    set_allow_insecure_env();
    // Build without dangerous_clear_enabled (default false)
    let backend = RedisBackend::new(REDIS_URL).await;
    // If Redis is unavailable, skip
    if backend.is_err() {
        return;
    }
    let backend = backend.unwrap();
    let result = backend.clear().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        OxCacheError::NotSupported(msg) => {
            assert!(msg.contains("disabled") || msg.contains("clear"));
        }
        other => panic!("Expected NotSupported, got {:?}", other),
    }
}

// Blanket impl test

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_redis_backend_implements_all_traits() {
    use crate::backend::CacheBackend;
    let backend = make_backend().await;
    let _: &dyn CacheBackend = &backend;
    let _: &dyn CacheReader = &backend;
    let _: &dyn CacheWriter = &backend;
    let _: &dyn CacheConnector = &backend;
}

// ============================================================================
// AtomicCacheWriter integration tests
// ============================================================================

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_incr_from_zero() {
    use crate::backend::AtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("incr0");
    let val = backend.incr(&key, 1, None).await.expect("incr failed");
    assert_eq!(val, 1);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_incr_accumulates() {
    use crate::backend::AtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("incrac");
    backend.incr(&key, 10, None).await.unwrap();
    let val = backend.incr(&key, 5, None).await.unwrap();
    assert_eq!(val, 15);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_incr_negative_delta() {
    use crate::backend::AtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("incrneg");
    backend.incr(&key, 10, None).await.unwrap();
    let val = backend.incr(&key, -3, None).await.unwrap();
    assert_eq!(val, 7);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_set_if_absent_success() {
    use crate::backend::AtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("setnx");
    let ok = backend.set_if_absent(&key, b"v1".to_vec(), None).await.unwrap();
    assert!(ok);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_set_if_absent_already_exists() {
    use crate::backend::AtomicCacheWriter;
    let backend = make_backend().await;
    let key = unique_key("setnxe");
    backend.set_if_absent(&key, b"v1".to_vec(), None).await.unwrap();
    let ok = backend.set_if_absent(&key, b"v2".to_vec(), None).await.unwrap();
    assert!(!ok);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_compare_and_swap_success() {
    use crate::backend::AtomicCacheWriter;
    use crate::backend::CacheReader;
    let backend = make_backend().await;
    let key = unique_key("cas");
    backend.set(Arc::from(key.as_str()), Arc::new(b"old".to_vec()), None).await.unwrap();
    let ok = backend.compare_and_swap(&key, Some(b"old"), b"new".to_vec(), None).await.unwrap();
    assert!(ok);
    let val = backend.get(&key).await.unwrap().unwrap();
    assert_eq!(val, b"new");
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_atomic_compare_and_swap_wrong_expected() {
    use crate::backend::AtomicCacheWriter;
    use crate::backend::CacheReader;
    let backend = make_backend().await;
    let key = unique_key("casw");
    backend.set(Arc::from(key.as_str()), Arc::new(b"actual".to_vec()), None).await.unwrap();
    let ok = backend.compare_and_swap(&key, Some(b"wrong"), b"new".to_vec(), None).await.unwrap();
    assert!(!ok);
    // Value should be unchanged
    let val = backend.get(&key).await.unwrap().unwrap();
    assert_eq!(val, b"actual");
    cleanup(&backend, &key).await;
}

// ============================================================================
// keys() SCAN integration tests
// ============================================================================

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_keys_scan_returns_matching_keys() {
    use crate::backend::CacheReader;
    let backend = make_backend().await;
    let prefix = unique_key("scan");
    let k1 = format!("{}_1", prefix);
    let k2 = format!("{}_2", prefix);
    backend.set(Arc::from(k1.as_str()), Arc::new(b"v1".to_vec()), None).await.unwrap();
    backend.set(Arc::from(k2.as_str()), Arc::new(b"v2".to_vec()), None).await.unwrap();
    let pattern = format!("{}*", prefix);
    let keys = backend.keys(&pattern).await.unwrap();
    assert!(keys.len() >= 2);
    assert!(keys.iter().any(|k| k == &k1));
    assert!(keys.iter().any(|k| k == &k2));
    cleanup(&backend, &k1).await;
    cleanup(&backend, &k2).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_keys_scan_no_match_returns_empty() {
    use crate::backend::CacheReader;
    let backend = make_backend().await;
    let keys = backend.keys("nonexistent_prefix_xyz_*").await.unwrap();
    assert!(keys.is_empty());
}

// ============================================================================
// clear() with dangerous_clear_enabled
// ============================================================================

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_clear_disabled_by_default_returns_error() {
    let backend = make_backend().await;
    let result = backend.clear().await;
    match result {
        Err(OxCacheError::NotSupported(msg)) => {
            assert!(msg.contains("disabled") || msg.contains("clear"));
        }
        Ok(()) => panic!("Expected clear() to fail with NotSupported"),
        Err(other) => panic!("Expected NotSupported, got {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_clear_namespace_prefix() {
    use crate::backend::CacheReader;
    let backend = make_backend().await;
    let prefix = unique_key("ns");
    let k1 = format!("{}_a", prefix);
    let k2 = format!("{}_b", prefix);
    let other = unique_key("other");
    backend.set(Arc::from(k1.as_str()), Arc::new(b"v1".to_vec()), None).await.unwrap();
    backend.set(Arc::from(k2.as_str()), Arc::new(b"v2".to_vec()), None).await.unwrap();
    backend.set(Arc::from(other.as_str()), Arc::new(b"v3".to_vec()), None).await.unwrap();
    backend.clear_namespace(&prefix).await.unwrap();
    assert!(!backend.exists(&k1).await.unwrap());
    assert!(!backend.exists(&k2).await.unwrap());
    assert!(backend.exists(&other).await.unwrap());
    cleanup(&backend, &other).await;
}

// ============================================================================
// stats() with INFO clients
// ============================================================================

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_stats_includes_clients_info() {
    use crate::backend::CacheReader;
    let backend = make_backend().await;
    let stats = backend.stats().await.unwrap();
    // Should contain connected_clients from INFO clients
    assert!(stats.contains_key("connected_clients"));
}

// ============================================================================
// TTL validation
// ============================================================================

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_ttl_zero_rejected() {
    let backend = make_backend().await;
    let key = unique_key("ttl_zero");
    let result = backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), Some(Duration::from_secs(0)))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_ttl_subsecond_rejected() {
    let backend = make_backend().await;
    let key = unique_key("ttl_subsecond");
    let result = backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), Some(Duration::from_millis(500)))
        .await;
    assert!(result.is_err());
}
