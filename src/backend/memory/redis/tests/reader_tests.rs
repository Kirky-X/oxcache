// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! CacheReader tests for RedisBackend.

use super::*;
use crate::backend::CacheReader;
use crate::error::OxCacheError;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_nonexistent_returns_none() {
    let backend = make_backend().await;
    let key = unique_key("no_such_key");
    let result = backend.get(&key).await.expect("get failed");
    assert!(result.is_none());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_then_get() {
    let backend = make_backend().await;
    let key = unique_key("set_get");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"hello world".to_vec()), None)
        .await
        .expect("set failed");
    let value = backend.get(&key).await.expect("get failed");
    assert_eq!(value, Some(b"hello world".to_vec()));
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_empty_value() {
    let backend = make_backend().await;
    let key = unique_key("empty_val");
    backend
        .set(Arc::from(key.as_str()), Arc::new(vec![]), None)
        .await
        .expect("set failed");
    let value = backend.get(&key).await.expect("get failed");
    assert_eq!(value, Some(vec![]));
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_set_binary_value() {
    let backend = make_backend().await;
    let key = unique_key("binary");
    let data: Vec<u8> = (0..=255).collect();
    backend
        .set(Arc::from(key.as_str()), Arc::new(data.clone()), None)
        .await
        .expect("set failed");
    let value = backend.get(&key).await.expect("get failed");
    assert_eq!(value, Some(data));
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_exists_true_after_set() {
    let backend = make_backend().await;
    let key = unique_key("exists_yes");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set failed");
    assert!(backend.exists(&key).await.expect("exists failed"));
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_exists_false_for_missing() {
    let backend = make_backend().await;
    let key = unique_key("exists_no");
    assert!(!backend.exists(&key).await.expect("exists failed"));
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_ttl_returns_none_for_key_without_expiry() {
    let backend = make_backend().await;
    let key = unique_key("no_ttl");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set failed");
    let ttl = backend.ttl(&key).await.expect("ttl failed");
    assert_eq!(ttl, None);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_ttl_returns_none_for_missing_key() {
    let backend = make_backend().await;
    let key = unique_key("missing_ttl");
    let ttl = backend.ttl(&key).await.expect("ttl failed");
    assert_eq!(ttl, None);
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_ttl_returns_some_after_set_with_ttl() {
    let backend = make_backend().await;
    let key = unique_key("with_ttl");
    backend
        .set(
            Arc::from(key.as_str()),
            Arc::new(b"v".to_vec()),
            Some(Duration::from_secs(100)),
        )
        .await
        .expect("set failed");
    let ttl = backend.ttl(&key).await.expect("ttl failed");
    assert!(ttl.is_some());
    let secs = ttl.unwrap().as_secs();
    assert!(secs > 90 && secs <= 100, "ttl secs = {}", secs);
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_stats_returns_memory_info() {
    let backend = make_backend().await;
    let stats = backend.stats().await.expect("stats failed");
    let info = stats.get("memory_info").expect("memory_info key missing");
    assert!(!info.is_empty());
    assert!(info.contains("memory") || info.contains("used_memory"));
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_len_returns_u64() {
    let backend = make_backend().await;
    let _len = backend.len().await.expect("len failed");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_is_empty_returns_bool() {
    let backend = make_backend().await;
    let _ = backend.is_empty().await.expect("is_empty failed");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_capacity_returns_zero() {
    let backend = make_backend().await;
    let cap = backend.capacity().await.expect("capacity failed");
    assert_eq!(cap, 0);
}

// Key validation tests (no Redis required for most)

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_empty_key_rejected() {
    let backend = make_backend().await;
    let result = backend.get("").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        OxCacheError::InvalidInput(_) => {}
        other => panic!("Expected InvalidInput, got {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_exists_empty_key_rejected() {
    let backend = make_backend().await;
    let result = backend.exists("").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_ttl_empty_key_rejected() {
    let backend = make_backend().await;
    let result = backend.ttl("").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_key_with_newline_rejected() {
    let backend = make_backend().await;
    let result = backend.get("key\nwith\nnewline").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_key_with_null_rejected() {
    let backend = make_backend().await;
    let result = backend.get("key\0null").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_key_with_command_injection_char_rejected() {
    let backend = make_backend().await;
    let result = backend.get("key;rm -rf").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_key_with_pipe_rejected() {
    let backend = make_backend().await;
    let result = backend.get("key|pipe").await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_get_key_with_path_traversal_rejected() {
    let backend = make_backend().await;
    let result = backend.get("../etc/passwd").await;
    assert!(result.is_err());
}
