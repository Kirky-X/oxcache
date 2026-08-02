// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Redis Sentinel 集成测试

use crate::common::{is_redis_available, wait_for_sentinel};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::CacheBackend;
use std::sync::Arc;
use std::time::Duration;

#[path = "../common/mod.rs"]
mod common;

fn get_sentinel_urls() -> Vec<String> {
    vec![
        "redis://127.0.0.1:26379".to_string(),
        "redis://127.0.0.1:26380".to_string(),
        "redis://127.0.0.1:26381".to_string(),
    ]
}

fn get_master_url() -> String {
    std::env::var("REDIS_SENTINEL_MASTER_URL").unwrap_or_else(|_| "redis://127.0.0.1:16379".to_string())
}

fn is_sentinel_available() -> bool {
    std::env::var("REDIS_SENTINEL_AVAILABLE").is_ok()
}

#[tokio::test]
async fn test_sentinel_connection() {
    println!("测试 Redis Sentinel 连接...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用 (设置 REDIS_SENTINEL_AVAILABLE=1 启用)");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    // 测试连接到 Sentinel 节点
    let urls = get_sentinel_urls();
    for url in &urls {
        let backend = RedisBackend::new(url).await;
        if backend.is_ok() {
            println!("✓ 成功连接到 Sentinel 节点: {}", url);
            return;
        }
    }

    panic!("无法连接到任何 Sentinel 节点");
}

#[tokio::test]
async fn test_sentinel_master_operations() {
    println!("测试 Redis Sentinel 主节点操作...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 测试基本操作
    backend
        .set(
            "sentinel_key_1",
            b"sentinel_value_1".to_vec(),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    let value = backend.get("sentinel_key_1").await.unwrap();
    assert_eq!(value, Some(b"sentinel_value_1".to_vec()));

    backend.delete("sentinel_key_1").await.unwrap();

    println!("✓ Redis Sentinel 主节点操作测试成功");
}

#[tokio::test]
async fn test_sentinel_ttl() {
    println!("测试 Redis Sentinel TTL...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 设置带 TTL 的键
    backend
        .set(Arc::from("sentinel_ttl_key"), Arc::new(b"ttl_value".to_vec()), Some(Duration::from_secs(2)))
        .await
        .unwrap();

    // 验证键存在
    assert!(backend.exists("sentinel_ttl_key").await.unwrap());

    // 获取 TTL
    let ttl = backend.ttl("sentinel_ttl_key").await.unwrap();
    assert!(ttl.is_some());

    // 等待过期
    // ponytail: requires Docker, polling not feasible without container
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 验证键已过期
    assert!(!backend.exists("sentinel_ttl_key").await.unwrap());

    println!("✓ Redis Sentinel TTL 测试成功");
}

#[tokio::test]
async fn test_sentinel_expire() {
    println!("测试 Redis Sentinel EXPIRE...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 设置不带 TTL 的键
    backend
        .set(Arc::from("sentinel_expire_key"), Arc::new(b"expire_value".to_vec()), None)
        .await
        .unwrap();

    // 设置过期时间
    let result = backend
        .expire("sentinel_expire_key", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(result);

    // 验证 TTL 已设置
    let ttl = backend.ttl("sentinel_expire_key").await.unwrap();
    assert!(ttl.is_some());

    backend.delete("sentinel_expire_key").await.unwrap();

    println!("✓ Redis Sentinel EXPIRE 测试成功");
}

#[tokio::test]
async fn test_sentinel_health_check() {
    println!("测试 Redis Sentinel 健康检查...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    backend.health_check().await.unwrap();

    println!("✓ Redis Sentinel 健康检查测试成功");
}

#[tokio::test]
async fn test_sentinel_stats() {
    println!("测试 Redis Sentinel 统计信息...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    backend
        .set(Arc::from("sentinel_stats_key"), Arc::new(b"stats_value".to_vec()), None)
        .await
        .unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"redis".to_string()));

    backend.delete("sentinel_stats_key").await.unwrap();

    println!("✓ Redis Sentinel 统计信息测试成功");
}

#[tokio::test]
async fn test_sentinel_many_keys() {
    println!("测试 Redis Sentinel 多键操作...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 设置 50 个键
    for i in 0..50 {
        let key = format!("sentinel_many_{}", i);
        let value = format!("value_{}", i);
        backend.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
    }

    // 验证所有键
    for i in 0..50 {
        let key = format!("sentinel_many_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }

    // 清理
    for i in 0..50 {
        let key = format!("sentinel_many_{}", i);
        backend.delete(&key).await.unwrap();
    }

    println!("✓ Redis Sentinel 多键操作测试成功");
}

#[tokio::test]
async fn test_sentinel_large_value() {
    println!("测试 Redis Sentinel 大值...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 1MB 数据
    let large_value = vec![0u8; 1024 * 1024];

    backend
        .set(Arc::from("sentinel_large_key"), Arc::new(large_value.clone()), None)
        .await
        .unwrap();
    let retrieved = backend.get("sentinel_large_key").await.unwrap();
    assert_eq!(retrieved, Some(large_value));

    backend.delete("sentinel_large_key").await.unwrap();

    println!("✓ Redis Sentinel 大值测试成功");
}
