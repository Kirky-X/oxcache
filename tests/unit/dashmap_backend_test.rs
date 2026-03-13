// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// DashMap 后端单元测试

use oxcache::backend::client::dashmap::{
    dashmap_memory, dashmap_memory_with_capacity, dashmap_memory_with_capacity_and_ttl, DashMapBackendBuilder,
    DashMapMemoryBackend,
};
use oxcache::backend::interface::CacheBackend;
use oxcache::backend::score::BackendScore;
use std::time::Duration;

#[tokio::test]
async fn test_dashmap_new() {
    let backend = DashMapMemoryBackend::new();
    assert!(backend.capacity() > 0);
    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_dashmap_builder_default() {
    let backend = DashMapBackendBuilder::default().build();
    assert!(backend.capacity() > 0);
}

#[tokio::test]
async fn test_dashmap_builder_with_capacity() {
    let backend = DashMapBackendBuilder::default().capacity(5000).build();
    assert_eq!(backend.capacity(), 5000);
}

#[tokio::test]
async fn test_dashmap_builder_with_ttl() {
    let backend = DashMapBackendBuilder::default()
        .capacity(1000)
        .default_ttl(Duration::from_secs(60))
        .build();
    assert_eq!(backend.capacity(), 1000);
}

#[tokio::test]
async fn test_dashmap_set_and_get() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    let value = backend.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_dashmap_get_nonexistent() {
    let backend = DashMapMemoryBackend::new();
    let value = backend.get("nonexistent").await.unwrap();
    assert!(value.is_none());
}

#[tokio::test]
async fn test_dashmap_delete() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(backend.exists("key1").await.unwrap());

    backend.delete("key1").await.unwrap();
    assert!(!backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_dashmap_exists() {
    let backend = DashMapMemoryBackend::new();

    assert!(!backend.exists("key1").await.unwrap());
    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_dashmap_clear() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.set("key2", b"value2".to_vec(), None).await.unwrap();

    backend.clear().await.unwrap();

    assert!(backend.is_empty().await.unwrap());
    assert!(!backend.exists("key1").await.unwrap());
    assert!(!backend.exists("key2").await.unwrap());
}

#[tokio::test]
async fn test_dashmap_close() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.close().await.unwrap();

    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_dashmap_ttl() {
    let backend = DashMapMemoryBackend::new();

    backend
        .set("key1", b"value1".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let ttl = backend.ttl("key1").await.unwrap();
    assert!(ttl.is_some());
    assert!(ttl.unwrap() <= Duration::from_secs(60));
}

#[tokio::test]
async fn test_dashmap_ttl_nonexistent() {
    let backend = DashMapMemoryBackend::new();
    let ttl = backend.ttl("nonexistent").await.unwrap();
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_dashmap_expire() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    let ttl_before = backend.ttl("key1").await.unwrap();
    assert!(ttl_before.is_none());

    let result = backend.expire("key1", Duration::from_secs(30)).await.unwrap();
    assert!(result);

    let ttl_after = backend.ttl("key1").await.unwrap();
    assert!(ttl_after.is_some());
}

#[tokio::test]
async fn test_dashmap_expire_nonexistent() {
    let backend = DashMapMemoryBackend::new();
    let result = backend.expire("nonexistent", Duration::from_secs(30)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_dashmap_health_check() {
    let backend = DashMapMemoryBackend::new();
    let healthy = backend.health_check().await.unwrap();
    assert!(healthy);
}

#[tokio::test]
async fn test_dashmap_stats() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.get("key1").await.unwrap();
    backend.get("nonexistent").await.unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"dashmap".to_string()));
    assert!(stats.contains_key("capacity"));
    assert!(stats.contains_key("entry_count"));
    assert!(stats.contains_key("hits"));
    assert!(stats.contains_key("misses"));
    assert!(stats.contains_key("hit_rate"));
}

#[tokio::test]
async fn test_dashmap_len() {
    let backend = DashMapMemoryBackend::new();

    assert_eq!(backend.len().await.unwrap(), 0);

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 1);

    backend.set("key2", b"value2".to_vec(), None).await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 2);

    backend.delete("key1").await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 1);
}

#[tokio::test]
async fn test_dashmap_capacity_method() {
    let backend = DashMapMemoryBackend::new();
    let capacity = backend.capacity();
    assert!(capacity > 0);
}

#[test]
fn test_dashmap_hit_rate_empty() {
    let backend = DashMapMemoryBackend::new();
    assert_eq!(backend.hit_rate(), 0.0);
}

#[tokio::test]
async fn test_dashmap_hit_rate_with_hits() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.get("key1").await.unwrap();
    backend.get("key1").await.unwrap();
    backend.get("nonexistent").await.unwrap();

    let hit_rate = backend.hit_rate();
    assert!(hit_rate > 0.0 && hit_rate <= 1.0);
}

#[test]
fn test_dashmap_entry_count() {
    let backend = DashMapMemoryBackend::new();
    assert_eq!(backend.entry_count(), 0);
}

#[test]
fn test_dashmap_backend_score() {
    let backend = DashMapMemoryBackend::new();
    assert!(backend.score() > 0);
    assert!(!backend.is_persistent());
    assert_eq!(backend.backend_name(), "dashmap");
}

#[test]
fn test_dashmap_clone() {
    let backend1 = DashMapMemoryBackend::new();
    let backend2 = backend1.clone();
    assert_eq!(backend1.capacity(), backend2.capacity());
}

#[test]
fn test_dashmap_debug() {
    let backend = DashMapMemoryBackend::new();
    let debug_str = format!("{:?}", backend);
    assert!(debug_str.contains("DashMapMemoryBackend"));
}

#[test]
fn test_convenience_dashmap_memory() {
    let backend = dashmap_memory();
    assert!(backend.capacity() > 0);
}

#[test]
fn test_convenience_dashmap_memory_with_capacity() {
    let backend = dashmap_memory_with_capacity(2000);
    assert_eq!(backend.capacity(), 2000);
}

#[test]
fn test_convenience_dashmap_memory_with_capacity_and_ttl() {
    let backend = dashmap_memory_with_capacity_and_ttl(3000, Duration::from_secs(120));
    assert_eq!(backend.capacity(), 3000);
}

#[tokio::test]
async fn test_dashmap_overwrite() {
    let backend = DashMapMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.set("key1", b"value2".to_vec(), None).await.unwrap();

    let value = backend.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));
}

#[tokio::test]
async fn test_dashmap_large_value() {
    let backend = DashMapMemoryBackend::new();
    let large_value = vec![0u8; 1024 * 1024];

    backend.set("large_key", large_value.clone(), None).await.unwrap();
    let value = backend.get("large_key").await.unwrap();
    assert_eq!(value, Some(large_value));
}

#[tokio::test]
async fn test_dashmap_many_keys() {
    let backend = DashMapMemoryBackend::builder().capacity(1000).build();

    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        backend.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
    }

    assert_eq!(backend.len().await.unwrap(), 100);

    for i in 0..100 {
        let key = format!("key_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }
}

#[tokio::test]
async fn test_dashmap_ttl_expiration() {
    let backend = DashMapMemoryBackend::new();

    backend
        .set("key1", b"value1".to_vec(), Some(Duration::from_millis(50)))
        .await
        .unwrap();

    assert!(backend.get("key1").await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(backend.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_dashmap_default_ttl() {
    let backend = DashMapMemoryBackend::builder()
        .default_ttl(Duration::from_millis(100))
        .build();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();

    assert!(backend.get("key1").await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(backend.get("key1").await.unwrap().is_none());
}
