// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis 单机集成测试

use crate::common::{get_redis_url, is_redis_available, setup_redis_container, wait_for_redis};
use oxcache::backend::client::redis::RedisBackend;
use oxcache::backend::interface::CacheBackend;
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[path = "../common/mod.rs"]
mod common;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestUser {
    id: u64,
    name: String,
}

impl oxcache::traits::Cacheable for TestUser {}

#[tokio::test]
async fn test_redis_backend_creation() {
    println!("测试 RedisBackend 创建...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let result = RedisBackend::new(&redis_url).await;

    assert!(result.is_ok(), "应该能成功创建 RedisBackend: {:?}", result.err());
    println!("✓ RedisBackend 创建成功");
}

#[tokio::test]
async fn test_redis_backend_ping() {
    println!("测试 RedisBackend PING...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    let ping_result = backend.ping().await;
    assert!(ping_result.is_ok(), "PING 应该成功");
    assert_eq!(ping_result.unwrap(), "PONG", "PING 应该返回 PONG");

    println!("✓ RedisBackend PING 测试成功");
}

#[tokio::test]
async fn test_redis_backend_basic_operations() {
    println!("测试 RedisBackend 基本操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // SET
    backend
        .set("test_key_1", b"test_value_1".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    // GET
    let value = backend.get("test_key_1").await.unwrap();
    assert_eq!(value, Some(b"test_value_1".to_vec()));

    // EXISTS
    let exists = backend.exists("test_key_1").await.unwrap();
    assert!(exists);

    // DELETE
    backend.delete("test_key_1").await.unwrap();
    let exists_after = backend.exists("test_key_1").await.unwrap();
    assert!(!exists_after);

    println!("✓ RedisBackend 基本操作测试成功");
}

#[tokio::test]
async fn test_redis_backend_ttl() {
    println!("测试 RedisBackend TTL...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 设置带 TTL 的键
    backend
        .set("ttl_key", b"ttl_value".to_vec(), Some(Duration::from_secs(2)))
        .await
        .unwrap();

    // 验证键存在
    let exists = backend.exists("ttl_key").await.unwrap();
    assert!(exists);

    // 获取 TTL
    let ttl = backend.ttl("ttl_key").await.unwrap();
    assert!(ttl.is_some());
    assert!(ttl.unwrap() <= Duration::from_secs(2));

    // 等待过期
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 验证键已过期
    let exists_after = backend.exists("ttl_key").await.unwrap();
    assert!(!exists_after);

    println!("✓ RedisBackend TTL 测试成功");
}

#[tokio::test]
async fn test_redis_backend_expire() {
    println!("测试 RedisBackend EXPIRE...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 设置不带 TTL 的键
    backend.set("expire_key", b"expire_value".to_vec(), None).await.unwrap();

    // 验证 TTL 为 None
    let ttl_before = backend.ttl("expire_key").await.unwrap();
    assert!(ttl_before.is_none() || ttl_before.unwrap() == Duration::from_secs(-1i32 as u64));

    // 设置过期时间
    let result = backend.expire("expire_key", Duration::from_secs(60)).await.unwrap();
    assert!(result);

    // 验证 TTL 已设置
    let ttl_after = backend.ttl("expire_key").await.unwrap();
    assert!(ttl_after.is_some());

    println!("✓ RedisBackend EXPIRE 测试成功");
}

#[tokio::test]
async fn test_redis_backend_clear() {
    println!("测试 RedisBackend CLEAR...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 设置多个键
    backend.set("clear_key_1", b"value1".to_vec(), None).await.unwrap();
    backend.set("clear_key_2", b"value2".to_vec(), None).await.unwrap();

    // 清空
    backend.clear().await.unwrap();

    println!("✓ RedisBackend CLEAR 测试成功");
}

#[tokio::test]
async fn test_redis_backend_stats() {
    println!("测试 RedisBackend STATS...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    backend.set("stats_key", b"stats_value".to_vec(), None).await.unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"redis".to_string()));

    println!("✓ RedisBackend STATS 测试成功");
}

#[tokio::test]
async fn test_redis_backend_health_check() {
    println!("测试 RedisBackend HEALTH CHECK...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    let healthy = backend.health_check().await.unwrap();
    assert!(healthy);

    println!("✓ RedisBackend HEALTH CHECK 测试成功");
}

#[tokio::test]
async fn test_cache_with_redis_backend() {
    println!("测试 Cache with Redis Backend...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let cache: Cache<String, TestUser> = match Cache::redis(&redis_url).await {
        Ok(c) => c,
        Err(e) => {
            println!("跳过测试: 创建 Redis Cache 失败 - {}", e);
            return;
        }
    };

    let user = TestUser {
        id: 1,
        name: "Redis Test User".to_string(),
    };

    // SET
    cache.set("redis_test_user", &user).await.unwrap();

    // GET
    let retrieved: Option<TestUser> = cache.get("redis_test_user").await.unwrap();
    assert_eq!(retrieved, Some(user.clone()));

    // DELETE
    cache.delete("redis_test_user").await.unwrap();
    let retrieved_after: Option<TestUser> = cache.get("redis_test_user").await.unwrap();
    assert!(retrieved_after.is_none());

    println!("✓ Cache with Redis Backend 测试成功");
}

#[tokio::test]
async fn test_redis_backend_large_value() {
    println!("测试 RedisBackend 大值...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 1MB 数据
    let large_value = vec![0u8; 1024 * 1024];

    backend.set("large_key", large_value.clone(), None).await.unwrap();
    let retrieved = backend.get("large_key").await.unwrap();
    assert_eq!(retrieved, Some(large_value));

    backend.delete("large_key").await.unwrap();

    println!("✓ RedisBackend 大值测试成功");
}

#[tokio::test]
async fn test_redis_backend_many_keys() {
    println!("测试 RedisBackend 多键操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis 不可用");
        return;
    }

    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 设置 100 个键
    for i in 0..100 {
        let key = format!("many_keys_{}", i);
        let value = format!("value_{}", i);
        backend.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
    }

    // 验证所有键
    for i in 0..100 {
        let key = format!("many_keys_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }

    // 清理
    for i in 0..100 {
        let key = format!("many_keys_{}", i);
        backend.delete(&key).await.unwrap();
    }

    println!("✓ RedisBackend 多键操作测试成功");
}

#[tokio::test]
async fn test_redis_backend_with_testcontainers() {
    println!("测试 RedisBackend with Testcontainers...");

    let (container, redis_url) = match setup_redis_container().await {
        Ok(result) => result,
        Err(e) => {
            println!("跳过测试: 无法启动 Redis 容器 - {}", e);
            return;
        }
    };

    // 等待 Redis 就绪
    if !wait_for_redis(&redis_url, 30).await {
        println!("跳过测试: Redis 容器未就绪");
        return;
    }

    let backend = RedisBackend::new(&redis_url).await.unwrap();

    // 基本操作测试
    backend.set("container_key", b"container_value".to_vec(), None).await.unwrap();
    let value = backend.get("container_key").await.unwrap();
    assert_eq!(value, Some(b"container_value".to_vec()));

    println!("✓ RedisBackend with Testcontainers 测试成功");

    // 容器在函数结束时自动清理
    drop(container);
}
