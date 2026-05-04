// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Moka 后端单元测试

use oxcache::backend::memory::moka::{
    moka_memory, moka_memory_with_capacity, moka_memory_with_capacity_and_ttl, MokaMemoryBackend,
    MokaMemoryBackendBuilder,
};
use oxcache::backend::interface::{CacheConnector, CacheReader, CacheWriter};
use oxcache::backend::score::BackendScore;
use std::time::Duration;

#[tokio::test]
async fn test_moka_new() {
    let backend = MokaMemoryBackend::new();
    assert!(backend.capacity() > 0);
    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_moka_builder_default() {
    let backend = MokaMemoryBackendBuilder::default().build();
    assert!(backend.capacity() > 0);
}

#[tokio::test]
async fn test_moka_builder_with_capacity() {
    let backend = MokaMemoryBackendBuilder::default().capacity(5000).build();
    assert_eq!(backend.capacity(), 5000);
}

#[tokio::test]
async fn test_moka_builder_with_ttl() {
    let backend = MokaMemoryBackendBuilder::default()
        .capacity(1000)
        .ttl(Duration::from_secs(60))
        .build();
    assert_eq!(backend.capacity(), 1000);
}

#[tokio::test]
async fn test_moka_builder_with_time_to_idle() {
    let backend = MokaMemoryBackendBuilder::default()
        .capacity(1000)
        .time_to_idle(Duration::from_secs(30))
        .build();
    assert_eq!(backend.capacity(), 1000);
}

#[tokio::test]
async fn test_moka_set_and_get() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    let value = backend.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_moka_get_nonexistent() {
    let backend = MokaMemoryBackend::new();
    let value = backend.get("nonexistent").await.unwrap();
    assert!(value.is_none());
}

#[tokio::test]
async fn test_moka_delete() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(backend.exists("key1").await.unwrap());

    backend.delete("key1").await.unwrap();
    assert!(!backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_moka_exists() {
    let backend = MokaMemoryBackend::new();

    assert!(!backend.exists("key1").await.unwrap());
    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_moka_clear() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.set("key2", b"value2".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    backend.clear().await.unwrap();

    assert!(backend.is_empty().await.unwrap());
    assert!(!backend.exists("key1").await.unwrap());
    assert!(!backend.exists("key2").await.unwrap());
}

#[tokio::test]
async fn test_moka_close() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.shutdown().await;

    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_moka_ttl() {
    let backend = MokaMemoryBackend::new();

    backend
        .set("key1", b"value1".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let ttl = backend.ttl("key1").await.unwrap();
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_moka_ttl_nonexistent() {
    let backend = MokaMemoryBackend::new();
    let ttl = backend.ttl("nonexistent").await.unwrap();
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_moka_expire() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();

    let result = backend.expire("key1", Duration::from_secs(30)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_moka_expire_nonexistent() {
    let backend = MokaMemoryBackend::new();
    let result = backend.expire("nonexistent", Duration::from_secs(30)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_moka_health_check() {
    let backend = MokaMemoryBackend::new();
    backend.health_check().await.unwrap();
}

#[tokio::test]
async fn test_moka_stats() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    backend.get("key1").await.unwrap();
    backend.get("nonexistent").await.unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"moka".to_string()));
    assert!(stats.contains_key("capacity"));
    assert!(stats.contains_key("entry_count"));
}

#[tokio::test]
async fn test_moka_len() {
    let backend = MokaMemoryBackend::new();

    assert_eq!(backend.len().await.unwrap(), 0);

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(backend.len().await.unwrap(), 1);

    backend.set("key2", b"value2".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(backend.len().await.unwrap(), 2);

    backend.delete("key1").await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 1);
}

#[tokio::test]
async fn test_moka_capacity_method() {
    let backend = MokaMemoryBackend::new();
    let capacity = backend.capacity().await.unwrap();
    assert!(capacity > 0);
}

#[test]
fn test_moka_entry_count() {
    let backend = MokaMemoryBackend::new();
    assert_eq!(backend.entry_count(), 0);
}

#[test]
fn test_moka_backend_score() {
    let backend = MokaMemoryBackend::new();
    assert!(backend.score() > 0);
    assert!(!backend.is_persistent());
    assert_eq!(backend.backend_name(), "moka");
}

#[test]
fn test_moka_clone() {
    let backend1 = MokaMemoryBackend::new();
    let backend2 = backend1.clone();
    assert_eq!(backend1.capacity(), backend2.capacity());
}

#[test]
fn test_moka_debug() {
    let backend = MokaMemoryBackend::new();
    let debug_str = format!("{:?}", backend);
    assert!(debug_str.contains("MokaMemoryBackend"));
}

#[test]
fn test_convenience_moka_memory() {
    let backend = moka_memory();
    assert!(backend.capacity() > 0);
}

#[test]
fn test_convenience_moka_memory_with_capacity() {
    let backend = moka_memory_with_capacity(2000);
    assert_eq!(backend.capacity(), 2000);
}

#[test]
fn test_convenience_moka_memory_with_capacity_and_ttl() {
    let backend = moka_memory_with_capacity_and_ttl(3000, Duration::from_secs(120));
    assert_eq!(backend.capacity(), 3000);
}

#[tokio::test]
async fn test_moka_overwrite() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.set("key1", b"value2".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    let value = backend.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));
}

#[tokio::test]
async fn test_moka_large_value() {
    let backend = MokaMemoryBackend::new();
    let large_value = vec![0u8; 1024 * 1024];

    backend.set("large_key", large_value.clone(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let value = backend.get("large_key").await.unwrap();
    assert_eq!(value, Some(large_value));
}

#[tokio::test]
async fn test_moka_many_keys() {
    let backend = MokaMemoryBackend::builder().capacity(1000).build();

    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        backend.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(backend.len().await.unwrap(), 100);

    for i in 0..100 {
        let key = format!("key_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }
}

#[tokio::test]
async fn test_moka_ttl_expiration() {
    let backend = MokaMemoryBackend::builder()
        .capacity(1000)
        .ttl(Duration::from_millis(100))
        .build();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(backend.get("key1").await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(backend.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_moka_time_to_idle() {
    let backend = MokaMemoryBackend::builder()
        .capacity(1000)
        .time_to_idle(Duration::from_millis(100))
        .build();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(backend.get("key1").await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(backend.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_moka_concurrent_access() {
    let backend = std::sync::Arc::new(MokaMemoryBackend::new());
    let mut handles = Vec::new();

    for i in 0..10 {
        let backend = backend.clone();
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                let key = format!("concurrent_key_{}_{}", i, j);
                backend.set(&key, b"value".to_vec(), None).await.unwrap();
                backend.get(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
