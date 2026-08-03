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
    let _client: &redis::Client = backend.client();
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
