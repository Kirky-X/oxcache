// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct TestValue {
    id: u64,
    name: String,
}

impl Default for TestValue {
    fn default() -> Self {
        Self {
            id: 1,
            name: "test".to_string(),
        }
    }
}

use crate::common::MockBackend;

#[tokio::test]
async fn test_cache_memory_constructor() {
    let cache: Cache<String, TestValue> = Cache::memory().await.unwrap();
    let value = TestValue::default();
    cache.set(&"test_key".to_string(), &value).await.unwrap();
    let result: Option<TestValue> = cache.get(&"test_key".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_with_dependencies() {
    let backend = Arc::new(MockBackend::new("test_mock", 80, false));
    let cache: Cache<String, TestValue> = Cache::with_dependencies(backend);
    let value = TestValue::default();
    cache.set(&"di_key".to_string(), &value).await.unwrap();
    let result: Option<TestValue> = cache.get(&"di_key".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_builder_constructor() {
    let cache: Cache<String, TestValue> = Cache::builder().ttl(Duration::from_secs(300)).build().await.unwrap();
    let value = TestValue {
        id: 42,
        name: "builder_test".to_string(),
    };
    cache.set(&"builder_key".to_string(), &value).await.unwrap();
    let result: Option<TestValue> = cache.get(&"builder_key".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_get_bytes() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let data = b"hello world".to_vec();
    cache.set_bytes("raw_key", data.clone(), None).await.unwrap();
    let result = cache.get_bytes("raw_key").await.unwrap();
    assert_eq!(result, Some(data));
}

#[tokio::test]
async fn test_cache_get_bytes_not_found() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let result = cache.get_bytes("nonexistent_key").await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_cache_set_bytes_with_ttl() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let data = b"ttl_data".to_vec();
    cache.set_bytes("ttl_key", data.clone(), Some(60)).await.unwrap();
    let result = cache.get_bytes("ttl_key").await.unwrap();
    assert_eq!(result, Some(data));
}

#[tokio::test]
async fn test_cache_delete_many() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let v1 = TestValue {
        id: 1,
        name: "one".to_string(),
    };
    let v2 = TestValue {
        id: 2,
        name: "two".to_string(),
    };
    let v3 = TestValue {
        id: 3,
        name: "three".to_string(),
    };
    cache.set(&"key1".to_string(), &v1).await.unwrap();
    cache.set(&"key2".to_string(), &v2).await.unwrap();
    cache.set(&"key3".to_string(), &v3).await.unwrap();
    assert!(cache.exists(&"key1".to_string()).await.unwrap());
    assert!(cache.exists(&"key2".to_string()).await.unwrap());
    assert!(cache.exists(&"key3".to_string()).await.unwrap());
    cache
        .delete_many(vec![&"key1".to_string(), &"key2".to_string()])
        .await
        .unwrap();
    assert!(!cache.exists(&"key1".to_string()).await.unwrap());
    assert!(!cache.exists(&"key2".to_string()).await.unwrap());
    assert!(cache.exists(&"key3".to_string()).await.unwrap());
}

#[tokio::test]
async fn test_cache_delete_many_empty() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache.delete_many(vec![]).await.unwrap();
}

#[tokio::test]
async fn test_cache_delete_many_nonexistent_keys() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache
        .delete_many(vec![&"nonexistent1".to_string(), &"nonexistent2".to_string()])
        .await
        .unwrap();
}

#[tokio::test]
async fn test_cache_get_many_partial_results() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let v1 = TestValue {
        id: 1,
        name: "one".to_string(),
    };
    let v2 = TestValue {
        id: 2,
        name: "two".to_string(),
    };
    cache.set(&"key1".to_string(), &v1).await.unwrap();
    cache.set(&"key2".to_string(), &v2).await.unwrap();
    let results: std::collections::HashMap<String, TestValue> = cache
        .get_many(vec![
            &"key1".to_string(),
            &"key2".to_string(),
            &"key_missing".to_string(),
        ])
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results.get("key1"), Some(&v1));
    assert_eq!(results.get("key2"), Some(&v2));
    assert_eq!(results.get("key_missing"), None);
}

#[tokio::test]
async fn test_cache_health_check() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_stats() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let stats = cache.stats().await.unwrap();
    assert!(stats.contains_key("type"));
}

#[tokio::test]
async fn test_set_multiple_get_returns_all() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache.set(&"key1".to_string(), &TestValue::default()).await.unwrap();
    cache.set(&"key2".to_string(), &TestValue::default()).await.unwrap();
    assert!(cache.get(&"key1".to_string()).await.unwrap().is_some());
    assert!(cache.get(&"key2".to_string()).await.unwrap().is_some());
}

#[tokio::test]
async fn test_cache_capacity() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let capacity = cache.capacity().await.unwrap();
    assert!(capacity > 0);
}

#[tokio::test]
async fn test_cache_serializer_roundtrip() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let serializer = cache.serializer();
    let data = serde_json::to_vec(&TestValue::default()).unwrap();
    let bytes = serializer.serialize("test", &data).unwrap();
    let deserialized = serializer.deserialize("test", &bytes).unwrap();
    assert_eq!(deserialized, data);
}

#[tokio::test]
async fn test_cache_unified_serializer() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let unified = cache.unified_serializer();
    let data = TestValue::default();
    let bytes = unified.serialize(&data).unwrap();
    assert!(!bytes.is_empty());
}

#[tokio::test]
async fn test_cache_set_with_ttl() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let value = TestValue {
        id: 99,
        name: "ttl_test".to_string(),
    };
    cache
        .set_with_ttl(&"ttl_key".to_string(), &value, Some(Duration::from_secs(60)))
        .await
        .unwrap();
    let result: Option<TestValue> = cache.get(&"ttl_key".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_set_without_ttl() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let value = TestValue::default();
    cache
        .set_with_ttl(&"no_ttl_key".to_string(), &value, None)
        .await
        .unwrap();
    let result: Option<TestValue> = cache.get(&"no_ttl_key".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_shutdown() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache
        .set(&"shutdown_key".to_string(), &TestValue::default())
        .await
        .unwrap();
    cache.shutdown().await;
}

#[tokio::test]
async fn test_cache_shutdown_empty() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache.shutdown().await;
}

#[tokio::test]
async fn test_cache_clear_removes_all() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    for i in 1..=5 {
        cache
            .set(
                &format!("clear_key_{}", i).to_string(),
                &TestValue {
                    id: i,
                    name: format!("val{}", i),
                },
            )
            .await
            .unwrap();
    }
    assert!(
        poll_until(Duration::from_millis(500), Duration::from_millis(10), || async {
            let mut all_exist = true;
            for i in 1..=5u64 {
                if !cache.exists(&format!("clear_key_{}", i).to_string()).await.unwrap() {
                    all_exist = false;
                    break;
                }
            }
            all_exist
        })
        .await
    );
    for i in 1..=5 {
        assert!(cache.exists(&format!("clear_key_{}", i).to_string()).await.unwrap());
    }
    cache.clear().await.unwrap();
    for i in 1..=5 {
        assert!(!cache.exists(&format!("clear_key_{}", i).to_string()).await.unwrap());
    }
}

#[tokio::test]
async fn test_cache_get_or_with_fallback() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let result = cache
        .get_or(&"fallback_key".to_string(), || async {
            Ok(TestValue {
                id: 100,
                name: "fallback_value".to_string(),
            })
        })
        .await
        .unwrap();
    assert_eq!(result.id, 100);
    assert_eq!(result.name, "fallback_value");
    let cached = cache
        .get_or(&"fallback_key".to_string(), || async {
            Err(oxcache::error::CacheError::NotFound("should not be called".to_string()))
        })
        .await
        .unwrap();
    assert_eq!(cached.id, 100);
}

#[tokio::test]
async fn test_cache_get_or_existing_value() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let value = TestValue {
        id: 42,
        name: "existing".to_string(),
    };
    cache.set(&"existing_key".to_string(), &value).await.unwrap();
    let result = cache
        .get_or(&"existing_key".to_string(), || async {
            Err(oxcache::error::CacheError::NotFound("should not be called".to_string()))
        })
        .await
        .unwrap();
    assert_eq!(result, value);
}

#[tokio::test]
async fn test_cache_debug_impl() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let debug_str = format!("{:?}", cache);
    assert!(debug_str.contains("Cache"));
}

#[tokio::test]
async fn test_cache_default_impl() {
    let cache: Cache<String, TestValue> = Cache::default();
    let value = TestValue::default();
    cache.set(&"default_key".to_string(), &value).await.unwrap();
    let result: Option<TestValue> = cache.get(&"default_key".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_empty_key_handling() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let value = TestValue::default();
    cache.set(&"".to_string(), &value).await.unwrap();
    let result: Option<TestValue> = cache.get(&"".to_string()).await.unwrap();
    assert_eq!(result, Some(value));
}

#[tokio::test]
async fn test_cache_large_value() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let large_data: Vec<u8> = (0..10000).map(|i| i as u8).collect();
    cache.set(&"large_key".to_string(), &large_data).await.unwrap();
    let result: Option<Vec<u8>> = cache.get(&"large_key".to_string()).await.unwrap();
    assert_eq!(result, Some(large_data));
}

#[tokio::test]
async fn test_cache_special_characters_in_key() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let special_keys = vec![
        "key:with:colon",
        "key-with-dash",
        "key_with_underscore",
        "key.with.dot",
        "key with space",
    ];
    for key in special_keys {
        let value = TestValue::default();
        cache.set(&key.to_string(), &value).await.unwrap();
        let result: Option<TestValue> = cache.get(&key.to_string()).await.unwrap();
        assert_eq!(result, Some(value));
    }
}

#[tokio::test]
async fn test_cache_overwrite_value() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache
        .set(
            &"overwrite_key".to_string(),
            &TestValue {
                id: 1,
                name: "original".to_string(),
            },
        )
        .await
        .unwrap();
    cache
        .set(
            &"overwrite_key".to_string(),
            &TestValue {
                id: 2,
                name: "overwritten".to_string(),
            },
        )
        .await
        .unwrap();
    let result: Option<TestValue> = cache.get(&"overwrite_key".to_string()).await.unwrap();
    assert_eq!(
        result,
        Some(TestValue {
            id: 2,
            name: "overwritten".to_string()
        })
    );
}

#[tokio::test]
async fn test_cache_register_for_macro_string_bytes() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let result = cache.register_for_macro("test_service").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cache_register_for_macro_empty_name() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let result = cache.register_for_macro("").await;
    assert!(result.is_err());
}
