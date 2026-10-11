// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 真实场景端到端测试

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Session {
    session_id: String,
    user_id: u64,
    expires_at: i64,
    data: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ApiResponse {
    status: u16,
    data: String,
    cached_at: i64,
}

#[tokio::test]
async fn test_web_application_cache_scenario() {
    println!("=== Web application caching scenario test ===");

    let cache: Cache<String, User> = Cache::memory().await.unwrap();

    // 模拟用户注册
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        created_at: chrono::Utc::now().timestamp(),
    };

    // 缓存用户数据
    let key = "user:1".to_string();
    cache.set(&key, &user).await.unwrap();

    // 模拟多次读取
    for _ in 0..10 {
        let cached: Option<User> = cache.get(&key).await.unwrap();
        assert_eq!(cached, Some(user.clone()));
    }

    // 更新用户
    let updated_user = User {
        name: "Alice Updated".to_string(),
        ..user.clone()
    };
    cache.set(&key, &updated_user).await.unwrap();

    let cached: Option<User> = cache.get(&key).await.unwrap();
    assert_eq!(cached.unwrap().name, "Alice Updated");

    // 删除用户
    cache.delete(&key).await.unwrap();
    assert!(!cache.exists(&key).await.unwrap());

    println!("✓ Web application caching scenario test passed");
}

#[tokio::test]
async fn test_session_storage_scenario() {
    println!("=== Session storage scenario test ===");

    let cache: Cache<String, Session> = Cache::memory().await.unwrap();

    // 创建会话
    let session = Session {
        session_id: "sess_123456".to_string(),
        user_id: 1,
        expires_at: chrono::Utc::now().timestamp() + 3600,
        data: std::collections::HashMap::from([
            ("theme".to_string(), "dark".to_string()),
            ("language".to_string(), "zh-CN".to_string()),
        ]),
    };

    // 存储会话，设置 TTL
    let key = "session:sess_123456".to_string();
    cache
        .set_with_ttl(&key, &session, Some(Duration::from_secs(3600)))
        .await
        .unwrap();

    // 验证会话
    let cached: Option<Session> = cache.get(&key).await.unwrap();
    assert!(cached.is_some());
    let cached_session = cached.unwrap();
    assert_eq!(cached_session.session_id, "sess_123456");
    assert_eq!(cached_session.user_id, 1);

    // 更新会话数据
    let mut updated_session = cached_session;
    updated_session
        .data
        .insert("last_page".to_string(), "/dashboard".to_string());
    cache.set(&key, &updated_session).await.unwrap();

    // 验证更新
    let cached: Option<Session> = cache.get(&key).await.unwrap();
    assert_eq!(
        cached.unwrap().data.get("last_page"),
        Some(&"/dashboard".to_string())
    );

    // 注销会话
    cache.delete(&key).await.unwrap();
    assert!(!cache.exists(&key).await.unwrap());

    println!("✓ Session storage scenario test passed");
}

#[tokio::test]
async fn test_api_response_cache_scenario() {
    println!("=== API response caching scenario test ===");

    let cache: Cache<String, ApiResponse> = Cache::memory().await.unwrap();

    // 模拟 API 响应缓存
    let response = ApiResponse {
        status: 200,
        data: r#"{"users": [{"id": 1, "name": "Alice"}]}"#.to_string(),
        cached_at: chrono::Utc::now().timestamp(),
    };

    // 缓存 API 响应
    let cache_key = "api:/users:list".to_string();
    cache
        .set_with_ttl(&cache_key, &response, Some(Duration::from_secs(300)))
        .await
        .unwrap();

    // 模拟缓存命中
    let cached: Option<ApiResponse> = cache.get(&cache_key).await.unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().status, 200);

    // 使用 get_or 模式
    let products_key = "api:/products:list".to_string();
    let result: ApiResponse = cache
        .get_or(&products_key, || async {
            Ok(ApiResponse {
                status: 200,
                data: r#"{"products": []}"#.to_string(),
                cached_at: chrono::Utc::now().timestamp(),
            })
        })
        .await
        .unwrap();

    assert_eq!(result.status, 200);

    // 验证已缓存
    let cached: Option<ApiResponse> = cache.get(&products_key).await.unwrap();
    assert!(cached.is_some());

    println!("✓ API response caching scenario test passed");
}

#[tokio::test]
async fn test_distributed_lock_scenario() {
    println!("=== Distributed lock scenario test ===");

    let cache: Cache<String, String> = Cache::memory().await.unwrap();

    let lock_key = "lock:resource:123".to_string();
    let lock_value = uuid::Uuid::new_v4().to_string();
    let lock_ttl = Duration::from_secs(10);

    // 尝试获取锁
    let acquired = cache.get(&lock_key).await.unwrap().is_none();

    if acquired {
        // 设置锁
        cache
            .set_with_ttl(&lock_key, &lock_value, Some(lock_ttl))
            .await
            .unwrap();

        // 执行临界区操作
        println!("Executing critical section operation...");

        // 释放锁（验证锁值）
        let current_lock: Option<String> = cache.get(&lock_key).await.unwrap();
        if current_lock == Some(lock_value.clone()) {
            cache.delete(&lock_key).await.unwrap();
            println!("Lock released");
        }
    }

    // 验证锁已释放
    assert!(!cache.exists(&lock_key).await.unwrap());

    println!("✓ Distributed lock scenario test passed");
}

#[tokio::test]
async fn test_rate_limiting_scenario() {
    println!("=== Rate limiting scenario test ===");

    let cache: Cache<String, u64> = Cache::memory().await.unwrap();

    let rate_limit_key = "rate_limit:user:123:api".to_string();
    let max_requests = 10u64;
    let window_secs = 60u64;

    // 模拟请求计数
    for i in 1..=max_requests + 5 {
        let count: u64 = cache
            .get_or(&rate_limit_key, || async { Ok(0u64) })
            .await
            .unwrap();

        if count < max_requests {
            cache
                .set_with_ttl(
                    &rate_limit_key,
                    &(count + 1),
                    Some(Duration::from_secs(window_secs)),
                )
                .await
                .unwrap();
            println!("request {} allowed, current count: {}", i, count + 1);
        } else {
            println!("request {} rate-limited, current count: {}", i, count);
        }
    }

    // 验证最终计数
    let final_count: Option<u64> = cache.get(&rate_limit_key).await.unwrap();
    assert_eq!(final_count, Some(max_requests));

    println!("✓ Rate limiting scenario test passed");
}

#[tokio::test]
async fn test_bulk_operations_scenario() {
    println!("=== Batch operations scenario test ===");

    let cache: Cache<String, User> = Cache::memory().await.unwrap();

    // 批量创建用户
    let users: Vec<(String, User)> = (1..=100)
        .map(|i| {
            (
                format!("user:{}", i),
                User {
                    id: i,
                    name: format!("User {}", i),
                    email: format!("user{}@example.com", i),
                    created_at: chrono::Utc::now().timestamp(),
                },
            )
        })
        .collect();

    // 批量写入
    cache
        .set_many(users.iter().map(|(k, v)| (k, v)))
        .await
        .unwrap();
    println!("batch-wrote {} users", users.len());

    // 批量读取
    let keys: Vec<String> = (1..=100).map(|i| format!("user:{}", i)).collect();
    let results: std::collections::HashMap<String, User> =
        cache.get_many(keys.iter()).await.unwrap();
    println!("batch-read {} users", results.len());

    assert_eq!(results.len(), 100);

    // 验证数据完整性
    for (key, user) in &users {
        let cached = results.get(key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, user.id);
    }

    println!("✓ Batch operations scenario test passed");
}

#[tokio::test]
async fn test_concurrent_access_scenario() {
    println!("=== Concurrent access scenario test ===");

    let cache = Arc::new(Mutex::new(Cache::<String, User>::memory().await.unwrap()));
    let mut handles = Vec::new();

    // 启动多个并发任务
    for thread_id in 0..10 {
        let cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            for i in 0..50 {
                let key = format!("thread_{}_key_{}", thread_id, i);
                let user = User {
                    id: thread_id * 100 + i,
                    name: format!("User {}-{}", thread_id, i),
                    email: format!("user{}-{}@example.com", thread_id, i),
                    created_at: chrono::Utc::now().timestamp(),
                };

                // 写入
                cache.lock().await.set(&key, &user).await.unwrap();

                // 读取
                let cached: Option<User> = cache.lock().await.get(&key).await.unwrap();
                assert_eq!(cached.unwrap().id, user.id);

                // 删除
                cache.lock().await.delete(&key).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }

    println!("✓ Concurrent access scenario test passed");
}

#[tokio::test]
async fn test_cache_warming_scenario() {
    println!("=== Cache warmup scenario test ===");

    let cache: Cache<String, User> = Cache::memory().await.unwrap();

    // 模拟从数据库加载热点数据
    let hot_users: Vec<(String, User)> = vec![
        (
            "user:1".to_string(),
            User {
                id: 1,
                name: "Hot User 1".to_string(),
                email: "hot1@example.com".to_string(),
                created_at: chrono::Utc::now().timestamp(),
            },
        ),
        (
            "user:2".to_string(),
            User {
                id: 2,
                name: "Hot User 2".to_string(),
                email: "hot2@example.com".to_string(),
                created_at: chrono::Utc::now().timestamp(),
            },
        ),
    ];

    // 预热缓存
    cache
        .set_many(hot_users.iter().map(|(k, v)| (k, v)))
        .await
        .unwrap();
    println!(
        "cache warmup complete, warmed {} hot users",
        hot_users.len()
    );

    // 验证预热数据
    for (key, user) in &hot_users {
        let cached: Option<User> = cache.get(key).await.unwrap();
        assert_eq!(cached.unwrap().id, user.id);
    }

    println!("✓ Cache warmup scenario test passed");
}

#[tokio::test]
async fn test_cache_invalidation_scenario() {
    println!("=== Cache invalidation scenario test ===");

    let cache: Cache<String, ApiResponse> = Cache::memory().await.unwrap();

    // 设置多个相关缓存
    let key1 = "api:/users:1".to_string();
    cache
        .set(
            &key1,
            &ApiResponse {
                status: 200,
                data: r#"{"id": 1, "name": "Alice"}"#.to_string(),
                cached_at: chrono::Utc::now().timestamp(),
            },
        )
        .await
        .unwrap();

    let key2 = "api:/users:list".to_string();
    cache
        .set(
            &key2,
            &ApiResponse {
                status: 200,
                data: r#"{"users": [{"id": 1, "name": "Alice"}]}"#.to_string(),
                cached_at: chrono::Utc::now().timestamp(),
            },
        )
        .await
        .unwrap();

    // 模拟用户更新，需要失效相关缓存
    let keys_to_invalidate = vec![key1, key2];

    for key in &keys_to_invalidate {
        cache.delete(key).await.unwrap();
    }

    // 验证缓存已失效
    for key in &keys_to_invalidate {
        assert!(!cache.exists(key).await.unwrap());
    }

    println!("✓ Cache invalidation scenario test passed");
}

#[tokio::test]
async fn test_long_running_stability() {
    println!("=== Long-run stability test ===");

    let cache: Cache<String, User> = Cache::memory().await.unwrap();
    let iterations = 1000;

    for i in 0..iterations {
        let key = format!("stability_key_{}", i % 100);
        let user = User {
            id: i as u64,
            name: format!("User {}", i),
            email: format!("user{}@example.com", i),
            created_at: chrono::Utc::now().timestamp(),
        };

        // 写入
        cache.set(&key, &user).await.unwrap();

        // 读取
        let _: Option<User> = cache.get(&key).await.unwrap();

        // 每 100 次清理一次
        if i % 100 == 99 {
            cache.clear().await.unwrap();
        }
    }

    println!(
        "✓ Long-run stability test passed ({} iterations)",
        iterations
    );
}
