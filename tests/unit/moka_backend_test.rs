// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Moka 后端单元测试

use oxcache::backend::interface::{CacheConnector, CacheReader, CacheWriter};
use oxcache::backend::memory::moka::{
    MokaMemoryBackend, MokaMemoryBackendBuilder, moka_memory, moka_memory_with_capacity,
    moka_memory_with_capacity_and_ttl,
};
use oxcache::backend::score::BackendScore;
use std::time::Duration;

/// Poll until a condition is true or timeout. Replaces fixed sleep for timing-dependent tests.
/// ponytail: simple polling loop, add exponential backoff if CI flakiness persists.
async fn poll_until<F, Fut>(timeout: Duration, interval: Duration, f: F) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if f().await {
            return true;
        }
        tokio::time::sleep(interval).await;
    }
    false
}

#[tokio::test]
async fn test_moka_new_default_state() {
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
async fn test_moka_builder_with_capacity_custom() {
    let backend = MokaMemoryBackendBuilder::default().capacity(5000).build();
    assert_eq!(backend.capacity(), 5000);
}

#[tokio::test]
async fn test_moka_builder_with_ttl_default_ttl() {
    let backend = MokaMemoryBackendBuilder::default()
        .capacity(1000)
        .ttl(Duration::from_secs(60))
        .build();
    assert_eq!(backend.capacity(), 1000);
}

#[tokio::test]
async fn test_moka_builder_with_time_to_idle_default() {
    let backend = MokaMemoryBackendBuilder::default()
        .capacity(1000)
        .time_to_idle(Duration::from_secs(30))
        .build();
    assert_eq!(backend.capacity(), 1000);
}

#[tokio::test]
async fn test_moka_set_and_get_basic_roundtrip() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap().is_some()
        })
        .await
    );

    let value = backend.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_moka_get_nonexistent_returns_none() {
    let backend = MokaMemoryBackend::new();
    let value = backend.get("nonexistent").await.unwrap();
    assert!(value.is_none());
}

#[tokio::test]
async fn test_moka_delete_removes_key() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.exists("key1").await.unwrap()
        })
        .await
    );

    backend.delete("key1").await.unwrap();
    assert!(!backend.exists("key1").await.unwrap());
}

#[tokio::test]
async fn test_moka_exists_checks_key_presence() {
    let backend = MokaMemoryBackend::new();

    assert!(!backend.exists("key1").await.unwrap());
    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.exists("key1").await.unwrap()
        })
        .await
    );
}

#[tokio::test]
async fn test_moka_clear_empties_all() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.set("key2", b"value2".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.exists("key2").await.unwrap()
        })
        .await
    );

    backend.clear().await.unwrap();

    assert!(backend.is_empty().await.unwrap());
    assert!(!backend.exists("key1").await.unwrap());
    assert!(!backend.exists("key2").await.unwrap());
}

#[tokio::test]
async fn test_moka_close_shutdown_empties() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.shutdown().await;

    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_moka_ttl_returns_remaining_after_set_with_ttl() {
    let backend = MokaMemoryBackend::new();

    backend
        .set("key1", b"value1".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let ttl = backend.ttl("key1").await.unwrap();
    // BREAKING 0.3.0: per-entry TTL 现在真实生效，ttl(key) 返回剩余时间
    assert!(ttl.is_some(), "ttl should be Some after set with TTL");
    let ttl = ttl.unwrap();
    assert!(
        ttl <= Duration::from_secs(60) && ttl > Duration::from_secs(58),
        "ttl should be within (58s, 60s], got {:?}",
        ttl
    );
}

#[tokio::test]
async fn test_moka_ttl_nonexistent_returns_none() {
    let backend = MokaMemoryBackend::new();
    let ttl = backend.ttl("nonexistent").await.unwrap();
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_moka_expire_returns_true_for_existing_key() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();

    let result = backend.expire("key1", Duration::from_secs(30)).await.unwrap();
    // BREAKING 0.3.0: expire 对存在 key 真实更新过期时间并返回 true
    assert!(result, "expire should return true for existing key");
}

#[tokio::test]
async fn test_moka_expire_nonexistent_returns_false() {
    let backend = MokaMemoryBackend::new();
    let result = backend.expire("nonexistent", Duration::from_secs(30)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_moka_health_check_returns_ok() {
    let backend = MokaMemoryBackend::new();
    backend.health_check().await.unwrap();
}

#[tokio::test]
async fn test_moka_stats_returns_metrics() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap().is_some()
        })
        .await
    );
    backend.get("key1").await.unwrap();
    backend.get("nonexistent").await.unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"moka".to_string()));
    assert!(stats.contains_key("capacity"));
    assert!(stats.contains_key("entry_count"));
}

#[tokio::test]
async fn test_moka_len_tracks_count() {
    let backend = MokaMemoryBackend::new();

    assert_eq!(backend.len().await.unwrap(), 0);

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap().is_some()
        })
        .await
    );

    backend.set("key2", b"value2".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key2").await.unwrap().is_some()
        })
        .await
    );

    backend.delete("key1").await.unwrap();
    assert!(!backend.exists("key1").await.unwrap());
    assert!(backend.exists("key2").await.unwrap());
}

#[tokio::test]
async fn test_moka_capacity_method_returns_positive() {
    let backend = MokaMemoryBackend::new();
    let capacity = backend.capacity();
    assert!(capacity > 0);
}

#[test]
fn test_moka_entry_count_initial_zero() {
    let backend = MokaMemoryBackend::new();
    assert_eq!(backend.entry_count(), 0);
}

#[test]
fn test_moka_backend_score_returns_positive() {
    let backend = MokaMemoryBackend::new();
    assert!(backend.score() > 0);
    assert!(!backend.is_persistent());
    assert_eq!(backend.backend_name(), "moka");
}

#[test]
fn test_moka_clone_copies_capacity() {
    let backend1 = MokaMemoryBackend::new();
    let backend2 = backend1.clone();
    assert_eq!(backend1.capacity(), backend2.capacity());
}

#[test]
fn test_moka_debug_includes_name() {
    let backend = MokaMemoryBackend::new();
    let debug_str = format!("{:?}", backend);
    assert!(debug_str.contains("MokaMemoryBackend"));
}

#[test]
fn test_convenience_moka_memory_default_has_capacity() {
    let backend = moka_memory();
    assert!(backend.capacity() > 0);
}

#[test]
fn test_convenience_moka_memory_with_capacity_custom() {
    let backend = moka_memory_with_capacity(2000);
    assert_eq!(backend.capacity(), 2000);
}

#[test]
fn test_convenience_moka_memory_with_capacity_and_ttl_custom() {
    let backend = moka_memory_with_capacity_and_ttl(3000, Duration::from_secs(120));
    assert_eq!(backend.capacity(), 3000);
}

#[tokio::test]
async fn test_moka_overwrite_replaces_value() {
    let backend = MokaMemoryBackend::new();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    backend.set("key1", b"value2".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap() == Some(b"value2".to_vec())
        })
        .await
    );

    let value = backend.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));
}

#[tokio::test]
async fn test_moka_large_value_handles_1mb() {
    let backend = MokaMemoryBackend::new();
    let large_value = vec![0u8; 1024 * 1024];

    backend.set("large_key", large_value.clone(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("large_key").await.unwrap().is_some()
        })
        .await
    );
    let value = backend.get("large_key").await.unwrap();
    assert_eq!(value, Some(large_value));
}

#[tokio::test]
async fn test_moka_many_keys_handles_100() {
    let backend = MokaMemoryBackend::builder().capacity(1000).build();

    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        backend.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
    }

    assert!(
        poll_until(Duration::from_millis(500), Duration::from_millis(10), || async {
            backend.get("key_99").await.unwrap().is_some()
        })
        .await
    );

    for i in 0..100 {
        let key = format!("key_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }
}

#[tokio::test]
async fn test_moka_ttl_expiration_evicts_after_ttl() {
    let backend = MokaMemoryBackend::builder()
        .capacity(1000)
        .ttl(Duration::from_millis(100))
        .build();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap().is_some()
        })
        .await
    );

    assert!(
        poll_until(Duration::from_millis(500), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap().is_none()
        })
        .await
    );
}

#[tokio::test]
async fn test_moka_time_to_idle_evicts_after_idle() {
    let backend = MokaMemoryBackend::builder()
        .capacity(1000)
        .time_to_idle(Duration::from_millis(100))
        .build();

    backend.set("key1", b"value1".to_vec(), None).await.unwrap();
    assert!(
        poll_until(Duration::from_millis(200), Duration::from_millis(10), || async {
            backend.get("key1").await.unwrap().is_some()
        })
        .await
    );

    // ponytail: can't poll for TTI expiration since each get() resets the idle timer
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(backend.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_moka_concurrent_access_handles_parallel() {
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
