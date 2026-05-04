//! Integration tests for new_in_memory() factory function
//!
//! This test verifies the Brick Architecture requirement that foundation modules
//! provide a `new_in_memory()` factory function for zero-configuration usage.

use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};

/// Test that new_in_memory() creates a working cache without configuration
#[tokio::test]
async fn test_new_in_memory_basic() {
    let cache = oxcache::new_in_memory();

    // Should be able to set and get values
    cache.set("key1", b"value1".to_vec(), None).await.unwrap();
    let value = cache.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Should return None for non-existent keys
    let missing = cache.get("nonexistent").await.unwrap();
    assert!(missing.is_none());
}

/// Test that new_in_memory() cache can set values with TTL parameter
#[tokio::test]
async fn test_new_in_memory_with_ttl() {
    let cache = oxcache::new_in_memory();

    // Set with TTL - just verify the method works and accepts the parameter
    cache
        .set(
            "ttl_key",
            b"ttl_value".to_vec(),
            Some(std::time::Duration::from_secs(60)),
        )
        .await
        .unwrap();

    // Should be present
    let value = cache.get("ttl_key").await.unwrap();
    assert_eq!(value, Some(b"ttl_value".to_vec()));
}

/// Test that new_in_memory() cache supports delete
#[tokio::test]
async fn test_new_in_memory_delete() {
    let cache = oxcache::new_in_memory();

    cache.set("delete_key", b"delete_value".to_vec(), None).await.unwrap();
    assert!(cache.exists("delete_key").await.unwrap());

    cache.delete("delete_key").await.unwrap();
    assert!(!cache.exists("delete_key").await.unwrap());
}

/// Test that new_in_memory() cache supports clear
#[tokio::test]
async fn test_new_in_memory_clear() {
    let cache = oxcache::new_in_memory();

    cache.set("key1", b"value1".to_vec(), None).await.unwrap();
    cache.set("key2", b"value2".to_vec(), None).await.unwrap();

    // Verify keys exist
    assert!(cache.exists("key1").await.unwrap());
    assert!(cache.exists("key2").await.unwrap());

    // Clear should work
    cache.clear().await.unwrap();

    // Keys should be gone
    assert!(!cache.exists("key1").await.unwrap());
    assert!(!cache.exists("key2").await.unwrap());
}

/// Test that new_in_memory() returns correct BackendKind
#[tokio::test]
async fn test_new_in_memory_backend_kind() {
    use oxcache::backend::interface::BackendKind;

    let cache = oxcache::new_in_memory();
    let kind = cache.backend_kind();

    assert_eq!(kind, BackendKind::Moka);
    assert!(kind.is_memory());
    assert!(!kind.is_distributed());
}

/// Test that new_in_memory() cache is healthy
#[tokio::test]
async fn test_new_in_memory_health_check() {
    let cache = oxcache::new_in_memory();

    // In-memory backends should always be healthy
    cache.health_check().await.unwrap();
}

/// Test batch operations
#[tokio::test]
async fn test_new_in_memory_batch_operations() {
    let cache = oxcache::new_in_memory();

    // Batch set
    let items = vec![
        ("key1".to_string(), b"value1".to_vec(), None),
        ("key2".to_string(), b"value2".to_vec(), None),
        ("key3".to_string(), b"value3".to_vec(), None),
    ];
    cache.set_many(&items).await.unwrap();

    // Batch get
    let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
    let results = cache.get_many(&keys).await.unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Some(b"value1".to_vec()));
    assert_eq!(results[1], Some(b"value2".to_vec()));
    assert_eq!(results[2], Some(b"value3".to_vec()));

    // Batch delete
    cache.delete_many(&keys).await.unwrap();
    let results = cache.get_many(&keys).await.unwrap();
    assert!(results.iter().all(|r| r.is_none()));
}
