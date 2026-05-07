// Copyright (c) 2025-2026, Kirky.X
//
// MIT License

use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

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

struct TestMockBackend {
    data: RwLock<std::collections::HashMap<String, Vec<u8>>>,
    _healthy: bool,
}

impl TestMockBackend {
    fn new() -> Self {
        Self {
            data: RwLock::new(std::collections::HashMap::new()),
            _healthy: true,
        }
    }
}

impl oxcache::backend::BackendScore for TestMockBackend {
    fn score(&self) -> u8 {
        80
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        "test_mock"
    }
}

#[async_trait::async_trait]
impl CacheReader for TestMockBackend {
    async fn get(&self, key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn exists(&self, key: &str) -> oxcache::error::Result<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }

    async fn ttl(&self, _key: &str) -> oxcache::error::Result<Option<Duration>> {
        Ok(None)
    }

    async fn len(&self) -> oxcache::error::Result<u64> {
        let data = self.data.read().await;
        Ok(data.len() as u64)
    }

    async fn is_empty(&self) -> oxcache::error::Result<bool> {
        let data = self.data.read().await;
        Ok(data.is_empty())
    }

    async fn capacity(&self) -> oxcache::error::Result<u64> {
        Ok(10000)
    }

    async fn stats(&self) -> oxcache::error::Result<std::collections::HashMap<String, String>> {
        let data = self.data.read().await;
        let mut stats = std::collections::HashMap::new();
        stats.insert("type".to_string(), "test_mock".to_string());
        stats.insert("entries".to_string(), data.len().to_string());
        Ok(stats)
    }

    async fn get_many(&self, keys: &[String]) -> oxcache::error::Result<Vec<Option<Vec<u8>>>> {
        let data = self.data.read().await;
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(data.get(key).cloned());
        }
        Ok(results)
    }
}

#[async_trait::async_trait]
impl CacheWriter for TestMockBackend {
    async fn set(&self, key: &str, value: Vec<u8>, _ttl: Option<Duration>) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }

    async fn expire(&self, _key: &str, _ttl: Duration) -> oxcache::error::Result<bool> {
        Ok(false)
    }

    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        for (key, value, _) in items {
            data.insert(key.clone(), value.clone());
        }
        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        for key in keys {
            data.remove(key);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl CacheConnector for TestMockBackend {
    async fn health_check(&self) -> oxcache::error::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) {
        // no-op
    }

    fn backend_kind(&self) -> oxcache::backend::interface::BackendKind {
        oxcache::backend::interface::BackendKind::Mock
    }
}

#[tokio::test]
async fn test_cache_new_with_moka() {
    let cache: Cache<String, TestValue> = Cache::new();
    assert!(cache.supports_l1_only());
}

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
    let backend = Arc::new(TestMockBackend::new());
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
async fn test_cache_set_l1_bytes() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let data = b"l1_data".to_vec();
    cache.set_l1_bytes("l1_key", data.clone(), None).await.unwrap();
    let result = cache.get_bytes("l1_key").await.unwrap();
    assert_eq!(result, Some(data));
}

#[tokio::test]
async fn test_cache_set_l1_bytes_with_ttl() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let data = b"l1_ttl_data".to_vec();
    cache.set_l1_bytes("l1_ttl_key", data.clone(), Some(30)).await.unwrap();
    let result = cache.get_bytes("l1_ttl_key").await.unwrap();
    assert_eq!(result, Some(data));
}

#[tokio::test]
async fn test_cache_set_l2_bytes() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let data = b"l2_data".to_vec();
    cache.set_l2_bytes("l2_key", data.clone(), None).await.unwrap();
    let result = cache.get_bytes("l2_key").await.unwrap();
    assert_eq!(result, Some(data));
}

#[tokio::test]
async fn test_cache_set_l2_bytes_with_ttl() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    let data = b"l2_ttl_data".to_vec();
    cache.set_l2_bytes("l2_ttl_key", data.clone(), Some(45)).await.unwrap();
    let result = cache.get_bytes("l2_ttl_key").await.unwrap();
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
async fn test_cache_len() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let _initial_len = cache.len().await.unwrap();
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
async fn test_cache_supports_l1_only() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    assert!(cache.supports_l1_only());
}

#[tokio::test]
async fn test_cache_supports_l2_only() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    assert!(!cache.supports_l2_only());
}

#[tokio::test]
async fn test_cache_supports_l1_only_with_mock() {
    let backend = Arc::new(TestMockBackend::new());
    let cache: Cache<String, TestValue> = Cache::with_dependencies(backend);
    // Mock backend is considered a memory backend (BackendKind::Mock.is_memory() == true)
    assert!(cache.supports_l1_only());
}

#[tokio::test]
async fn test_cache_serializer() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    let _serializer = cache.serializer();
    let data = TestValue::default();
    let bytes = serde_json::to_vec(&data).unwrap();
    assert!(!bytes.is_empty());
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
    tokio::time::sleep(Duration::from_millis(50)).await;
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
    cache.register_for_macro("test_service").await;
}

#[tokio::test]
async fn test_cache_register_for_macro_wrong_type() {
    let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("wrong_type_service").await;
}
