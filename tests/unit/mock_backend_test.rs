// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// MockBackend 单元测试

#[path = "../common/mock_backend.rs"]
mod mock_backend;

use mock_backend::MockBackend;
use oxcache::backend::{BackendScore, CacheConnector, CacheReader, CacheWriter};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_mock_backend_new() {
    let backend = MockBackend::new("test_backend", 80, true);
    assert_eq!(backend.backend_name(), "test_backend");
    assert_eq!(backend.score(), 80);
    assert!(backend.is_persistent());
}

#[test]
fn test_mock_backend_with_data() {
    let backend = MockBackend::with_data("data_backend", 50, false);
    assert_eq!(backend.backend_name(), "data_backend");
    assert_eq!(backend.score(), 50);
    assert!(!backend.is_persistent());
}

#[test]
fn test_mock_backend_score_edge_cases() {
    let high_score = MockBackend::new("high", 100, true);
    assert_eq!(high_score.score(), 100);

    let low_score = MockBackend::new("low", 0, false);
    assert_eq!(low_score.score(), 0);
}

#[tokio::test]
async fn test_mock_backend_set_and_get() {
    let backend = MockBackend::new("set_get", 80, false);

    backend
        .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
        .await
        .unwrap();
    let result = backend.get("key1").await.unwrap();
    assert_eq!(result, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_mock_backend_get_nonexistent() {
    let backend = MockBackend::new("get_none", 80, false);

    let result = backend.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_mock_backend_delete() {
    let backend = MockBackend::new("delete", 80, false);

    backend
        .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
        .await
        .unwrap();
    assert!(backend.exists("key1").await.unwrap());

    backend.delete("key1").await.unwrap();
    assert!(!backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_mock_backend_exists() {
    let backend = MockBackend::new("exists", 80, false);

    assert!(!backend.exists("key1").await.unwrap());
    backend
        .set(Arc::from("key1"), Arc::new(b"value".to_vec()), None)
        .await
        .unwrap();
    assert!(backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_mock_backend_clear() {
    let backend = MockBackend::new("clear", 80, false);

    backend
        .set(Arc::from("key1"), Arc::new(b"v1".to_vec()), None)
        .await
        .unwrap();
    backend
        .set(Arc::from("key2"), Arc::new(b"v2".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(backend.len().await.unwrap(), 2);

    backend.clear().await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 0);
    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_mock_backend_len_and_is_empty() {
    let backend = MockBackend::new("len", 80, false);

    assert!(backend.is_empty().await.unwrap());
    assert_eq!(backend.len().await.unwrap(), 0);

    backend
        .set(Arc::from("key1"), Arc::new(b"v1".to_vec()), None)
        .await
        .unwrap();
    assert!(!backend.is_empty().await.unwrap());
    assert_eq!(backend.len().await.unwrap(), 1);

    backend
        .set(Arc::from("key2"), Arc::new(b"v2".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(backend.len().await.unwrap(), 2);
}

#[tokio::test]
async fn test_mock_backend_ttl_returns_none() {
    let backend = MockBackend::new("ttl", 80, false);
    backend
        .set(
            Arc::from("key1"),
            Arc::new(b"v1".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    let ttl = backend.ttl("key1").await.unwrap();
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_mock_backend_expire_returns_false() {
    let backend = MockBackend::new("expire", 80, false);
    backend
        .set(Arc::from("key1"), Arc::new(b"v1".to_vec()), None)
        .await
        .unwrap();

    let result = backend.expire("key1", Duration::from_secs(60)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_mock_backend_health_check() {
    let backend = MockBackend::new("health", 80, false);
    backend.health_check().await.unwrap();
}

#[tokio::test]
async fn test_mock_backend_stats() {
    let backend = MockBackend::new("stats_backend", 80, false);

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type").unwrap(), "mock");
    assert_eq!(stats.get("name").unwrap(), "stats_backend");
}

#[tokio::test]
async fn test_mock_backend_capacity() {
    let backend = MockBackend::new("capacity", 80, false);
    let capacity = backend.capacity().await.unwrap();
    assert_eq!(capacity, 1000);
}

#[tokio::test]
async fn test_mock_backend_close() {
    let backend = MockBackend::new("close", 80, false);
    backend.shutdown().await;
}

#[test]
fn test_mock_backend_kind() {
    let backend = MockBackend::new("kind", 80, false);
    let kind = backend.backend_kind();

    assert_eq!(kind, oxcache::backend::interface::BackendKind::Mock);
    assert!(kind.is_memory());
}

#[tokio::test]
async fn test_mock_backend_multiple_operations() {
    let backend = MockBackend::new("multi", 80, false);

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

    assert_eq!(backend.len().await.unwrap(), 3);

    backend.delete("k2").await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 2);
    assert!(!backend.exists("k2").await.unwrap());

    assert_eq!(backend.get("k1").await.unwrap(), Some(b"v1".to_vec()));
    assert_eq!(backend.get("k3").await.unwrap(), Some(b"v3".to_vec()));
}

#[tokio::test]
async fn test_mock_backend_overwrite_key() {
    let backend = MockBackend::new("overwrite", 80, false);

    backend
        .set(Arc::from("key"), Arc::new(b"value1".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(backend.get("key").await.unwrap(), Some(b"value1".to_vec()));

    backend
        .set(Arc::from("key"), Arc::new(b"value2".to_vec()), None)
        .await
        .unwrap();
    assert_eq!(backend.get("key").await.unwrap(), Some(b"value2".to_vec()));
    assert_eq!(backend.len().await.unwrap(), 1);
}

#[tokio::test]
async fn test_mock_backend_empty_value() {
    let backend = MockBackend::new("empty", 80, false);

    backend
        .set(Arc::from("empty_key"), Arc::new(vec![]), None)
        .await
        .unwrap();
    let result = backend.get("empty_key").await.unwrap();
    assert_eq!(result, Some(vec![]));

    backend
        .set(Arc::from("none_key"), Arc::new(b"val".to_vec()), None)
        .await
        .unwrap();
    backend.delete("none_key").await.unwrap();
    let deleted = backend.get("none_key").await.unwrap();
    assert!(deleted.is_none());
}

#[test]
fn test_mock_backend_backend_score_trait() {
    let backend = MockBackend::new("trait_test", 75, true);

    assert_eq!(BackendScore::score(&backend), 75);
    assert!(BackendScore::is_persistent(&backend));
    assert_eq!(BackendScore::backend_name(&backend), "trait_test");
}

#[tokio::test]
async fn test_mock_backend_cache_backend_trait() {
    let backend = MockBackend::new("cache_trait", 80, false);

    CacheWriter::set(&backend, Arc::from("tkey"), Arc::new(b"tval".to_vec()), None)
        .await
        .unwrap();
    let val = CacheReader::get(&backend, "tkey").await.unwrap();
    assert_eq!(val, Some(b"tval".to_vec()));
}
