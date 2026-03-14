// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Cache Builder 单元测试

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestUser {
    id: u64,
    name: String,
    email: String,
}

impl oxcache::traits::Cacheable for TestUser {}

#[tokio::test]
async fn test_cache_builder_default() {
    let cache: Cache<String, TestUser> = Cache::builder().build().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    cache.set("user:1", &user).await.unwrap();
    let retrieved: Option<TestUser> = cache.get("user:1").await.unwrap();

    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_cache_builder_with_capacity() {
    let cache: Cache<String, TestUser> = Cache::builder()
        .capacity(1000)
        .build()
        .await
        .unwrap();

    let user = TestUser {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    cache.set("user:1", &user).await.unwrap();
    let retrieved: Option<TestUser> = cache.get("user:1").await.unwrap();

    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_cache_builder_with_ttl() {
    let cache: Cache<String, TestUser> = Cache::builder()
        .ttl(Duration::from_secs(60))
        .build()
        .await
        .unwrap();

    let user = TestUser {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    cache.set("user:1", &user).await.unwrap();
    let retrieved: Option<TestUser> = cache.get("user:1").await.unwrap();

    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_cache_memory_static() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    cache.set("user:2", &user).await.unwrap();
    let retrieved: Option<TestUser> = cache.get("user:2").await.unwrap();

    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_cache_new() {
    let cache: Cache<String, TestUser> = Cache::new();

    let user = TestUser {
        id: 1,
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };

    cache.set("user:3", &user).await.unwrap();
    let retrieved: Option<TestUser> = cache.get("user:3").await.unwrap();

    assert_eq!(retrieved, Some(user));
}

#[tokio::test]
async fn test_cache_set_and_get() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 42,
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
    };

    cache.set("test_key", &user).await.unwrap();

    let retrieved: Option<TestUser> = cache.get("test_key").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, 42);
}

#[tokio::test]
async fn test_cache_get_nonexistent() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let retrieved: Option<TestUser> = cache.get("nonexistent_key").await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_cache_delete() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "Delete Test".to_string(),
        email: "delete@example.com".to_string(),
    };

    cache.set("delete_key", &user).await.unwrap();
    assert!(cache.exists("delete_key").await.unwrap());

    cache.delete("delete_key").await.unwrap();
    assert!(!cache.exists("delete_key").await.unwrap());
}

#[tokio::test]
async fn test_cache_exists() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    assert!(!cache.exists("exists_key").await.unwrap());

    let user = TestUser {
        id: 1,
        name: "Exists Test".to_string(),
        email: "exists@example.com".to_string(),
    };
    cache.set("exists_key", &user).await.unwrap();

    assert!(cache.exists("exists_key").await.unwrap());
}

#[tokio::test]
async fn test_cache_clear() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "Clear Test".to_string(),
        email: "clear@example.com".to_string(),
    };

    cache.set("key1", &user).await.unwrap();
    cache.set("key2", &user).await.unwrap();

    assert!(cache.exists("key1").await.unwrap());
    assert!(cache.exists("key2").await.unwrap());

    cache.clear().await.unwrap();

    assert!(!cache.exists("key1").await.unwrap());
    assert!(!cache.exists("key2").await.unwrap());
}

#[tokio::test]
async fn test_cache_set_with_ttl() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "TTL Test".to_string(),
        email: "ttl@example.com".to_string(),
    };

    cache
        .set_with_ttl("ttl_key", &user, Some(Duration::from_millis(100)))
        .await
        .unwrap();

    let retrieved: Option<TestUser> = cache.get("ttl_key").await.unwrap();
    assert!(retrieved.is_some());

    tokio::time::sleep(Duration::from_millis(200)).await;

    let retrieved_after: Option<TestUser> = cache.get("ttl_key").await.unwrap();
    assert!(retrieved_after.is_none());
}

#[tokio::test]
async fn test_cache_get_or_with_fallback() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = cache
        .get_or("fallback_key", || async {
            Ok(TestUser {
                id: 999,
                name: "Fallback User".to_string(),
                email: "fallback@example.com".to_string(),
            })
        })
        .await
        .unwrap();

    assert_eq!(user.id, 999);
    assert_eq!(user.name, "Fallback User");

    let cached: Option<TestUser> = cache.get("fallback_key").await.unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().id, 999);
}

#[tokio::test]
async fn test_cache_get_or_returns_cached() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let original = TestUser {
        id: 1,
        name: "Original".to_string(),
        email: "original@example.com".to_string(),
    };
    cache.set("cached_key", &original).await.unwrap();

    let user = cache
        .get_or("cached_key", || async {
            Ok(TestUser {
                id: 999,
                name: "Should Not Be Called".to_string(),
                email: "no@example.com".to_string(),
            })
        })
        .await
        .unwrap();

    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Original");
}

#[tokio::test]
async fn test_cache_set_many() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let users = vec![
        (
            "user:1".to_string(),
            TestUser {
                id: 1,
                name: "User 1".to_string(),
                email: "user1@example.com".to_string(),
            },
        ),
        (
            "user:2".to_string(),
            TestUser {
                id: 2,
                name: "User 2".to_string(),
                email: "user2@example.com".to_string(),
            },
        ),
        (
            "user:3".to_string(),
            TestUser {
                id: 3,
                name: "User 3".to_string(),
                email: "user3@example.com".to_string(),
            },
        ),
    ];

    cache.set_many(users).await.unwrap();

    assert!(cache.exists("user:1").await.unwrap());
    assert!(cache.exists("user:2").await.unwrap());
    assert!(cache.exists("user:3").await.unwrap());
}

#[tokio::test]
async fn test_cache_get_many() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user1 = TestUser {
        id: 1,
        name: "User 1".to_string(),
        email: "user1@example.com".to_string(),
    };
    let user2 = TestUser {
        id: 2,
        name: "User 2".to_string(),
        email: "user2@example.com".to_string(),
    };

    cache.set("user:1", &user1).await.unwrap();
    cache.set("user:2", &user2).await.unwrap();

    let keys = vec!["user:1".to_string(), "user:2".to_string(), "user:3".to_string()];
    let results: std::collections::HashMap<String, TestUser> = cache.get_many(keys).await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.contains_key("user:1"));
    assert!(results.contains_key("user:2"));
    assert!(!results.contains_key("user:3"));
}

#[tokio::test]
async fn test_cache_len() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    assert_eq!(cache.len().await.unwrap(), 0);

    let user = TestUser {
        id: 1,
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
    };

    cache.set("key1", &user).await.unwrap();
    assert_eq!(cache.len().await.unwrap(), 1);

    cache.set("key2", &user).await.unwrap();
    assert_eq!(cache.len().await.unwrap(), 2);

    cache.delete("key1").await.unwrap();
    assert_eq!(cache.len().await.unwrap(), 1);
}

#[tokio::test]
async fn test_cache_stats() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "Stats Test".to_string(),
        email: "stats@example.com".to_string(),
    };

    cache.set("stats_key", &user).await.unwrap();
    cache.get("stats_key").await.unwrap();
    cache.get("nonexistent").await.unwrap();

    let stats = cache.stats().await.unwrap();
    assert!(!stats.is_empty());
}

#[tokio::test]
async fn test_cache_overwrite() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user1 = TestUser {
        id: 1,
        name: "Original".to_string(),
        email: "original@example.com".to_string(),
    };
    let user2 = TestUser {
        id: 2,
        name: "Updated".to_string(),
        email: "updated@example.com".to_string(),
    };

    cache.set("overwrite_key", &user1).await.unwrap();
    cache.set("overwrite_key", &user2).await.unwrap();

    let retrieved: Option<TestUser> = cache.get("overwrite_key").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, 2);
    assert_eq!(retrieved.unwrap().name, "Updated");
}

#[tokio::test]
async fn test_cache_concurrent_access() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let cache = Arc::new(Mutex::new(Cache::<String, TestUser>::memory().await.unwrap()));

    let mut handles = Vec::new();

    for i in 0..10 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("concurrent_{}_{}", i, j);
                let user = TestUser {
                    id: i * 10 + j,
                    name: format!("User {}-{}", i, j),
                    email: format!("user{}-{}@example.com", i, j),
                };
                cache.lock().await.set(&key, &user).await.unwrap();
                cache.lock().await.get(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_cache_shutdown() {
    let cache: Cache<String, TestUser> = Cache::memory().await.unwrap();

    let user = TestUser {
        id: 1,
        name: "Shutdown Test".to_string(),
        email: "shutdown@example.com".to_string(),
    };

    cache.set("shutdown_key", &user).await.unwrap();

    let result = cache.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cache_with_different_types() {
    let string_cache: Cache<String, String> = Cache::memory().await.unwrap();
    string_cache.set("string_key", &"test_value".to_string()).await.unwrap();
    let retrieved: Option<String> = string_cache.get("string_key").await.unwrap();
    assert_eq!(retrieved, Some("test_value".to_string()));

    let int_cache: Cache<String, i64> = Cache::memory().await.unwrap();
    int_cache.set("int_key", &42).await.unwrap();
    let retrieved: Option<i64> = int_cache.get("int_key").await.unwrap();
    assert_eq!(retrieved, Some(42));

    let vec_cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
    vec_cache.set("vec_key", &vec![1, 2, 3, 4, 5]).await.unwrap();
    let retrieved: Option<Vec<u8>> = vec_cache.get("vec_key").await.unwrap();
    assert_eq!(retrieved, Some(vec![1, 2, 3, 4, 5]));
}
