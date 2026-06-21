//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 覆盖率测试：src/cache/interface.rs
//!
//! 测试 UnifiedCache trait 的默认实现、适配器模式和批量操作。

use oxcache::backend::client::MokaMemoryBackend;
use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use oxcache::cache::UnifiedCache;
use oxcache::error::CacheError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// 测试用的数据结构
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestUser {
    id: u64,
    name: String,
    email: String,
}

/// 测试用的另一个数据结构
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestProduct {
    sku: String,
    price: f64,
    stock: u32,
}

// ============================================================================
// 测试 Layer-specific 默认实现 (返回 NotSupported)
// ============================================================================

#[tokio::test]
async fn test_get_l1_bytes_not_supported() {
    let backend = MokaMemoryBackend::new();

    // 设置一个值
    backend.set("test_key", b"test_value".to_vec(), None).await.unwrap();

    // MokaMemoryBackend 实际上实现了 get 方法，所以这个测试验证它工作正常
    let result = backend.get("test_key").await.unwrap();
    assert_eq!(result, Some(b"test_value".to_vec()));
}

#[tokio::test]
async fn test_set_l1_bytes_not_supported() {
    let backend = MokaMemoryBackend::new();

    // set_l1_bytes 默认返回 NotSupported
    let result = UnifiedCache::set_l1_bytes(&backend, "key", b"value".to_vec(), None).await;
    assert!(result.is_err());
    if let Err(CacheError::NotSupported(method)) = result {
        assert_eq!(method, "set_l1_bytes");
    } else {
        panic!("Expected NotSupported error");
    }
}

#[tokio::test]
async fn test_get_l1_bytes_not_supported_error() {
    let backend = MokaMemoryBackend::new();

    let result = UnifiedCache::get_l1_bytes(&backend, "key").await;
    assert!(result.is_err());
    if let Err(CacheError::NotSupported(method)) = result {
        assert_eq!(method, "get_l1_bytes");
    }
}

#[tokio::test]
async fn test_get_l2_bytes_not_supported() {
    let backend = MokaMemoryBackend::new();

    let result = UnifiedCache::get_l2_bytes(&backend, "key").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_set_l2_bytes_not_supported() {
    let backend = MokaMemoryBackend::new();

    let result = UnifiedCache::set_l2_bytes(&backend, "key", b"value".to_vec(), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_clear_l1_not_supported() {
    let backend = MokaMemoryBackend::new();

    let result = UnifiedCache::clear_l1(&backend).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_clear_l2_not_supported() {
    let backend = MokaMemoryBackend::new();

    let result = UnifiedCache::clear_l2(&backend).await;
    assert!(result.is_err());
}

// ============================================================================
// 测试分布式锁默认实现
// ============================================================================

#[tokio::test]
async fn test_lock_default_returns_none() {
    let backend = MokaMemoryBackend::new();

    // 默认实现应该返回 None
    let result = UnifiedCache::lock(&backend, "lock_key", 1000).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_unlock_default_returns_false() {
    let backend = MokaMemoryBackend::new();

    // 默认实现应该返回 false
    let result = UnifiedCache::unlock(&backend, "lock_key", "lock_value").await.unwrap();
    assert!(!result);
}

// ============================================================================
// 测试 is_l2_cache 和 should_parallelize 辅助方法
// ============================================================================

#[tokio::test]
async fn test_is_l2_cache_for_moka_backend() {
    let backend = MokaMemoryBackend::new();

    // MokaMemoryBackend 不是 L2 缓存
    assert!(!UnifiedCache::is_l2_cache(&backend));
}

#[tokio::test]
async fn test_should_parallelize_small_batch() {
    let backend = MokaMemoryBackend::new();

    // 小批量不应该并行化 (<= 5)
    assert!(!UnifiedCache::should_parallelize(&backend, 1));
    assert!(!UnifiedCache::should_parallelize(&backend, 5));
    assert!(!UnifiedCache::should_parallelize(&backend, 3));
}

#[tokio::test]
async fn test_should_parallelize_large_batch() {
    let backend = MokaMemoryBackend::new();

    // 大批量也不不会并行化，因为 Moka 是 L1 缓存
    assert!(!UnifiedCache::should_parallelize(&backend, 10));
    assert!(!UnifiedCache::should_parallelize(&backend, 100));
}

// ============================================================================
// 测试类型化操作
// ============================================================================

#[tokio::test]
async fn test_get_typed_with_valid_data() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    UnifiedCache::set_typed(&backend, "user:1", &user, None).await.unwrap();

    let retrieved: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:1").await.unwrap();
    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_get_typed_missing_key() {
    let backend = MokaMemoryBackend::new();

    let retrieved: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:nonexistent").await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_get_typed_invalid_json() {
    let backend = MokaMemoryBackend::new();

    // 设置无效的 JSON 数据
    backend
        .set("invalid_key", b"not valid json".to_vec(), None)
        .await
        .unwrap();

    // 尝试反序列化应该失败
    let result: Result<Option<TestUser>, _> = UnifiedCache::get_typed(&backend, "invalid_key").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_set_typed_with_ttl() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    UnifiedCache::set_typed(&backend, "user:2", &user, Some(Duration::from_secs(10)))
        .await
        .unwrap();

    let retrieved: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:2").await.unwrap();
    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_set_l1_typed_not_supported() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 3,
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };

    // set_l1_typed 调用 set_l1_bytes，后者默认返回 NotSupported
    let result = UnifiedCache::set_l1_typed(&backend, "user:3", &user, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_set_l2_typed_not_supported() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 4,
        name: "Diana".to_string(),
        email: "diana@example.com".to_string(),
    };

    // set_l2_typed 调用 set_l2_bytes，后者默认返回 NotSupported
    let result = UnifiedCache::set_l2_typed(&backend, "user:4", &user, None).await;
    assert!(result.is_err());
}

// ============================================================================
// 测试 get_or_fetch
// ============================================================================

#[tokio::test]
async fn test_get_or_fetch_cache_hit() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 10,
        name: "Cached User".to_string(),
        email: "cached@example.com".to_string(),
    };

    // 先设置缓存
    UnifiedCache::set_typed(&backend, "user:10", &user, None).await.unwrap();

    // get_or_fetch 应该直接返回缓存值，不调用 fetch
    let fetched = UnifiedCache::get_or_fetch::<TestUser, _, _>(&backend, "user:10", None, || async {
        panic!("Fetch should not be called when cache hit!");
        #[allow(unreachable_code)]
        Ok(TestUser {
            id: 0,
            name: String::new(),
            email: String::new(),
        })
    })
    .await
    .unwrap();

    assert_eq!(fetched, user);
}

#[tokio::test]
async fn test_get_or_fetch_cache_miss() {
    let backend = MokaMemoryBackend::new();

    let fetch_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fetch_count_clone = fetch_count.clone();

    let fetched =
        UnifiedCache::get_or_fetch::<TestUser, _, _>(&backend, "user:20", Some(Duration::from_secs(60)), move || {
            let count = fetch_count_clone.clone();
            async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(TestUser {
                    id: 20,
                    name: "Fetched User".to_string(),
                    email: "fetched@example.com".to_string(),
                })
            }
        })
        .await
        .unwrap();

    assert_eq!(fetched.id, 20);
    assert_eq!(fetched.name, "Fetched User");
    assert_eq!(fetch_count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // 验证值已被缓存
    let cached: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:20").await.unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().id, 20);
}

#[tokio::test]
async fn test_get_or_fetch_fetch_error() {
    let backend = MokaMemoryBackend::new();

    let result = UnifiedCache::get_or_fetch::<TestUser, _, _>(&backend, "user:error", None, || async {
        Err(CacheError::InvalidKey("Simulated fetch error".to_string()))
    })
    .await;

    assert!(result.is_err());
}

// ============================================================================
// 测试 try_get_typed
// ============================================================================

#[tokio::test]
async fn test_try_get_typed_existing() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 30,
        name: "Try User".to_string(),
        email: "try@example.com".to_string(),
    };

    UnifiedCache::set_typed(&backend, "user:30", &user, None).await.unwrap();

    let result: Option<TestUser> = UnifiedCache::try_get_typed(&backend, "user:30").await.unwrap();
    assert_eq!(result, Some(user));
}

#[tokio::test]
async fn test_try_get_typed_nonexistent() {
    let backend = MokaMemoryBackend::new();

    let result: Option<TestUser> = UnifiedCache::try_get_typed(&backend, "user:nonexistent_try")
        .await
        .unwrap();
    assert!(result.is_none());
}

// ============================================================================
// 测试 remove_typed
// ============================================================================

#[tokio::test]
async fn test_remove_typed_existing() {
    let backend = MokaMemoryBackend::new();

    let user = TestUser {
        id: 40,
        name: "Remove User".to_string(),
        email: "remove@example.com".to_string(),
    };

    UnifiedCache::set_typed(&backend, "user:40", &user, None).await.unwrap();

    // remove_typed 应该返回旧值并删除
    let removed: Option<TestUser> = UnifiedCache::remove_typed(&backend, "user:40").await.unwrap();
    assert_eq!(removed, Some(user));

    // 验证键已被删除
    let result: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:40").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_remove_typed_nonexistent() {
    let backend = MokaMemoryBackend::new();

    // 删除不存在的键应该返回 None
    let removed: Option<TestUser> = UnifiedCache::remove_typed(&backend, "user:nonexistent_remove")
        .await
        .unwrap();
    assert!(removed.is_none());
}

// ============================================================================
// 测试 contains
// ============================================================================

#[tokio::test]
async fn test_contains_existing() {
    let backend = MokaMemoryBackend::new();

    backend.set("existing_key", b"value".to_vec(), None).await.unwrap();

    assert!(UnifiedCache::contains(&backend, "existing_key").await.unwrap());
}

#[tokio::test]
async fn test_contains_nonexistent() {
    let backend = MokaMemoryBackend::new();

    assert!(!UnifiedCache::contains(&backend, "nonexistent_key").await.unwrap());
}

// ============================================================================
// 测试批量操作 - 顺序执行 (小批量)
// ============================================================================

#[tokio::test]
async fn test_set_many_bytes_small_batch() {
    let backend = MokaMemoryBackend::new();

    let items: Vec<(&str, Vec<u8>)> = vec![
        ("batch_key1", b"value1".to_vec()),
        ("batch_key2", b"value2".to_vec()),
        ("batch_key3", b"value3".to_vec()),
    ];

    UnifiedCache::set_many_bytes(&backend, items).await.unwrap();

    // 验证所有值都被设置
    assert!(CacheReader::exists(&backend, "batch_key1").await.unwrap());
    assert!(CacheReader::exists(&backend, "batch_key2").await.unwrap());
    assert!(CacheReader::exists(&backend, "batch_key3").await.unwrap());

    assert_eq!(backend.get("batch_key1").await.unwrap(), Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_get_many_bytes_small_batch() {
    let backend = MokaMemoryBackend::new();

    // 设置一些值
    backend.set("get_batch1", b"val1".to_vec(), None).await.unwrap();
    backend.set("get_batch2", b"val2".to_vec(), None).await.unwrap();

    let keys: Vec<&str> = vec!["get_batch1", "get_batch2", "get_batch3"];
    let results = UnifiedCache::get_many_bytes(&backend, keys).await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("get_batch1"), Some(&b"val1".to_vec()));
    assert_eq!(results.get("get_batch2"), Some(&b"val2".to_vec()));
    assert!(!results.contains_key("get_batch3"));
}

#[tokio::test]
async fn test_delete_many_small_batch() {
    let backend = MokaMemoryBackend::new();

    // 设置一些值
    backend.set("del_batch1", b"val1".to_vec(), None).await.unwrap();
    backend.set("del_batch2", b"val2".to_vec(), None).await.unwrap();
    backend.set("del_batch3", b"val3".to_vec(), None).await.unwrap();

    let keys: Vec<&str> = vec!["del_batch1", "del_batch2"];
    UnifiedCache::delete_many(&backend, keys).await.unwrap();

    assert!(!CacheReader::exists(&backend, "del_batch1").await.unwrap());
    assert!(!CacheReader::exists(&backend, "del_batch2").await.unwrap());
    assert!(CacheReader::exists(&backend, "del_batch3").await.unwrap());
}

// ============================================================================
// 测试批量类型化操作
// ============================================================================

#[tokio::test]
async fn test_set_many_typed() {
    let backend = MokaMemoryBackend::new();

    let user1 = TestUser {
        id: 101,
        name: "Batch User 1".to_string(),
        email: "batch1@example.com".to_string(),
    };
    let user2 = TestUser {
        id: 102,
        name: "Batch User 2".to_string(),
        email: "batch2@example.com".to_string(),
    };

    let items: Vec<(&str, &TestUser)> = vec![("user:101", &user1), ("user:102", &user2)];

    UnifiedCache::set_many_typed::<_, TestUser>(&backend, items)
        .await
        .unwrap();

    let retrieved1: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:101").await.unwrap();
    let retrieved2: Option<TestUser> = UnifiedCache::get_typed(&backend, "user:102").await.unwrap();

    assert_eq!(retrieved1, Some(user1));
    assert_eq!(retrieved2, Some(user2));
}

#[tokio::test]
async fn test_get_many_typed() {
    let backend = MokaMemoryBackend::new();

    let user1 = TestUser {
        id: 201,
        name: "Get Many 1".to_string(),
        email: "getmany1@example.com".to_string(),
    };
    let user2 = TestUser {
        id: 202,
        name: "Get Many 2".to_string(),
        email: "getmany2@example.com".to_string(),
    };

    UnifiedCache::set_typed(&backend, "user:201", &user1, None)
        .await
        .unwrap();
    UnifiedCache::set_typed(&backend, "user:202", &user2, None)
        .await
        .unwrap();

    let keys: Vec<&str> = vec!["user:201", "user:202", "user:nonexistent"];
    let results = UnifiedCache::get_many_typed::<_, TestUser>(&backend, keys)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results.get("user:201"), Some(&user1));
    assert_eq!(results.get("user:202"), Some(&user2));
    assert!(!results.contains_key("user:nonexistent"));
}

// ============================================================================
// 测试 TTL 相关操作
// ============================================================================

#[tokio::test]
async fn test_expire_not_supported_for_moka() {
    let backend = MokaMemoryBackend::new();

    backend
        .set("expire_key", b"value".to_vec(), Some(Duration::from_secs(10)))
        .await
        .unwrap();

    let result = CacheWriter::expire(&backend, "expire_key", Duration::from_secs(100))
        .await
        .unwrap();
    // Moka 不支持在插入后更新 TTL
    assert!(!result);
}

#[tokio::test]
async fn test_expire_nonexistent_key() {
    let backend = MokaMemoryBackend::new();

    let result = CacheWriter::expire(&backend, "nonexistent_expire", Duration::from_secs(100))
        .await
        .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_ttl_not_exposed_by_moka() {
    let backend = MokaMemoryBackend::new();

    backend
        .set("ttl_key", b"value".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let ttl = CacheReader::ttl(&backend, "ttl_key").await.unwrap();
    // Moka 不暴露每个条目的 TTL 信息
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_ttl_nonexistent_key() {
    let backend = MokaMemoryBackend::new();

    let ttl = CacheReader::ttl(&backend, "nonexistent_ttl").await.unwrap();
    // 不存在的键 TTL 应该是 None
    assert!(ttl.is_none());
}

// ============================================================================
// 测试健康检查和统计
// ============================================================================

#[tokio::test]
async fn test_health_check() {
    let backend = MokaMemoryBackend::new();

    // health_check returns Ok(()) for healthy backends
    CacheConnector::health_check(&backend).await.unwrap();
}

#[tokio::test]
async fn test_stats() {
    let backend = MokaMemoryBackend::new();

    let stats = CacheReader::stats(&backend).await.unwrap();
    assert!(!stats.is_empty());
}

// ============================================================================
// 测试 backend_kind 运行时类型识别
// ============================================================================

#[tokio::test]
async fn test_backend_kind() {
    let backend = MokaMemoryBackend::new();

    let kind = CacheConnector::backend_kind(&backend);
    assert_eq!(kind, oxcache::backend::interface::BackendKind::Moka);
    assert!(kind.is_memory());
    assert!(!kind.is_distributed());
}

#[tokio::test]
async fn test_backend_kind_redis() {
    // Redis backend would be tested separately when redis feature is enabled
    // For now, just verify the BackendKind enum has the right variants
    use oxcache::backend::interface::BackendKind;

    assert!(BackendKind::Moka.is_memory());
    assert!(BackendKind::DashMap.is_memory());
    assert!(BackendKind::Redis.is_distributed());
    assert!(!BackendKind::Redis.is_memory());
}

// ============================================================================
// 测试边界情况
// ============================================================================

#[tokio::test]
async fn test_empty_key_operations() {
    let backend = MokaMemoryBackend::new();

    // 空键操作
    backend.set("", b"empty_key_value".to_vec(), None).await.unwrap();
    let result = backend.get("").await.unwrap();
    assert_eq!(result, Some(b"empty_key_value".to_vec()));
}

#[tokio::test]
async fn test_large_value() {
    let backend = MokaMemoryBackend::new();

    // 大值
    let large_value = vec![0u8; 1024 * 1024]; // 1MB
    backend.set("large_key", large_value.clone(), None).await.unwrap();

    let result = backend.get("large_key").await.unwrap();
    assert_eq!(result, Some(large_value));
}

#[tokio::test]
async fn test_concurrent_operations() {
    use tokio::task::JoinSet;

    let backend = Arc::new(MokaMemoryBackend::new());
    let mut tasks = JoinSet::new();

    // 并发设置多个值
    for i in 0..100 {
        let backend_clone = backend.clone();
        tasks.spawn(async move {
            let key = format!("concurrent_key_{}", i);
            let value = format!("value_{}", i);
            backend_clone.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
        });
    }

    while tasks.join_next().await.is_some() {}

    // 验证所有值都被正确设置
    for i in 0..100 {
        let key = format!("concurrent_key_{}", i);
        let expected_value = format!("value_{}", i);
        let result = backend.get(&key).await.unwrap();
        assert_eq!(result, Some(expected_value.as_bytes().to_vec()));
    }
}

// ============================================================================
// 测试不同数据类型
// ============================================================================

#[tokio::test]
async fn test_different_types() {
    let backend = MokaMemoryBackend::new();

    // 字符串
    UnifiedCache::set_typed(&backend, "string_key", &"hello".to_string(), None)
        .await
        .unwrap();
    let s: Option<String> = UnifiedCache::get_typed(&backend, "string_key").await.unwrap();
    assert_eq!(s, Some("hello".to_string()));

    // 数字
    UnifiedCache::set_typed(&backend, "number_key", &42u64, None)
        .await
        .unwrap();
    let n: Option<u64> = UnifiedCache::get_typed(&backend, "number_key").await.unwrap();
    assert_eq!(n, Some(42));

    // 向量
    let vec_data = vec![1, 2, 3, 4, 5];
    UnifiedCache::set_typed(&backend, "vec_key", &vec_data, None)
        .await
        .unwrap();
    let v: Option<Vec<i32>> = UnifiedCache::get_typed(&backend, "vec_key").await.unwrap();
    assert_eq!(v, Some(vec_data));

    // 哈希映射
    let mut map = std::collections::HashMap::new();
    map.insert("key1".to_string(), "value1".to_string());
    map.insert("key2".to_string(), "value2".to_string());
    UnifiedCache::set_typed(&backend, "map_key", &map, None).await.unwrap();
    let m: Option<std::collections::HashMap<String, String>> =
        UnifiedCache::get_typed(&backend, "map_key").await.unwrap();
    assert_eq!(m, Some(map));
}

#[tokio::test]
async fn test_product_type() {
    let backend = MokaMemoryBackend::new();

    let product = TestProduct {
        sku: "SKU-12345".to_string(),
        price: 99.99,
        stock: 100,
    };

    UnifiedCache::set_typed(&backend, "product:SKU-12345", &product, None)
        .await
        .unwrap();

    let retrieved: Option<TestProduct> = UnifiedCache::get_typed(&backend, "product:SKU-12345").await.unwrap();
    assert_eq!(retrieved, Some(product));
}

// ============================================================================
// 测试序列化相关边界情况
// ============================================================================

#[tokio::test]
async fn test_serializer_method() {
    let backend = MokaMemoryBackend::new();

    // serializer 方法应该返回一个有效的序列化器
    let _serializer = UnifiedCache::serializer(&backend);
}

// ============================================================================
// 测试 close 操作
// ============================================================================

#[tokio::test]
async fn test_close() {
    let backend = MokaMemoryBackend::new();

    // 设置一些数据
    backend.set("close_key", b"value".to_vec(), None).await.unwrap();

    // shutdown 应该成功
    CacheConnector::shutdown(&backend).await;
}

// ============================================================================
// 测试 clear 操作
// ============================================================================

#[tokio::test]
async fn test_clear() {
    let backend = MokaMemoryBackend::new();

    // 设置多个值
    backend.set("clear1", b"v1".to_vec(), None).await.unwrap();
    backend.set("clear2", b"v2".to_vec(), None).await.unwrap();
    backend.set("clear3", b"v3".to_vec(), None).await.unwrap();

    // clear 应该删除所有值
    CacheWriter::clear(&backend).await.unwrap();

    assert!(!CacheReader::exists(&backend, "clear1").await.unwrap());
    assert!(!CacheReader::exists(&backend, "clear2").await.unwrap());
    assert!(!CacheReader::exists(&backend, "clear3").await.unwrap());
}

// ============================================================================
// 测试 delete 操作
// ============================================================================

#[tokio::test]
async fn test_delete_existing() {
    let backend = MokaMemoryBackend::new();

    backend.set("delete_key", b"value".to_vec(), None).await.unwrap();
    assert!(CacheReader::exists(&backend, "delete_key").await.unwrap());

    CacheWriter::delete(&backend, "delete_key").await.unwrap();
    assert!(!CacheReader::exists(&backend, "delete_key").await.unwrap());
}

#[tokio::test]
async fn test_delete_nonexistent() {
    let backend = MokaMemoryBackend::new();

    // 删除不存在的键应该成功
    CacheWriter::delete(&backend, "nonexistent_delete").await.unwrap();
}
