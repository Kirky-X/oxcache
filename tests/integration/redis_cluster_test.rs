// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Redis Cluster 集成测试

use crate::common::{is_redis_available, wait_for_redis_cluster};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::CacheBackend;
use std::sync::Arc;
use std::time::Duration;

#[path = "../common/mod.rs"]
mod common;

fn get_cluster_urls() -> Vec<String> {
    vec![
        "redis://127.0.0.1:7000".to_string(),
        "redis://127.0.0.1:7001".to_string(),
        "redis://127.0.0.1:7002".to_string(),
        "redis://127.0.0.1:7003".to_string(),
        "redis://127.0.0.1:7004".to_string(),
        "redis://127.0.0.1:7005".to_string(),
    ]
}

fn is_cluster_available() -> bool {
    std::env::var("REDIS_CLUSTER_AVAILABLE").is_ok()
}

#[tokio::test]
async fn test_redis_cluster_connection() {
    println!("测试 Redis Cluster 连接...");

    if !is_cluster_available() {
        println!("跳过测试: Redis Cluster 不可用 (设置 REDIS_CLUSTER_AVAILABLE=1 启用)");
        return;
    }

    let urls = get_cluster_urls();
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();

    if !wait_for_redis_cluster(&url_refs).await {
        println!("跳过测试: Redis Cluster 未就绪");
        return;
    }

    // 测试连接到第一个节点
    let backend = RedisBackend::new(&urls[0]).await;
    assert!(backend.is_ok(), "应该能连接到 Cluster 节点");

    println!("✓ Redis Cluster 连接测试成功");
}

#[tokio::test]
async fn test_redis_cluster_basic_operations() {
    println!("测试 Redis Cluster 基本操作...");

    if !is_cluster_available() {
        println!("跳过测试: Redis Cluster 不可用");
        return;
    }

    let urls = get_cluster_urls();
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();

    if !wait_for_redis_cluster(&url_refs).await {
        println!("跳过测试: Redis Cluster 未就绪");
        return;
    }

    let backend = RedisBackend::new(&urls[0]).await.unwrap();

    // 测试基本操作
    backend
        .set(
            "cluster_key_1",
            b"cluster_value_1".to_vec(),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    let value = backend.get("cluster_key_1").await.unwrap();
    assert_eq!(value, Some(b"cluster_value_1".to_vec()));

    backend.delete("cluster_key_1").await.unwrap();

    println!("✓ Redis Cluster 基本操作测试成功");
}

#[tokio::test]
async fn test_redis_cluster_data_distribution() {
    println!("测试 Redis Cluster 数据分布...");

    if !is_cluster_available() {
        println!("跳过测试: Redis Cluster 不可用");
        return;
    }

    let urls = get_cluster_urls();
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();

    if !wait_for_redis_cluster(&url_refs).await {
        println!("跳过测试: Redis Cluster 未就绪");
        return;
    }

    let backend = RedisBackend::new(&urls[0]).await.unwrap();

    // 写入多个键，测试数据分布
    for i in 0..50 {
        let key = format!("distributed_key_{}", i);
        let value = format!("value_{}", i);
        backend.set(&key, value.as_bytes().to_vec(), None).await.unwrap();
    }

    // 验证所有键都能正确读取
    for i in 0..50 {
        let key = format!("distributed_key_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }

    // 清理
    for i in 0..50 {
        let key = format!("distributed_key_{}", i);
        backend.delete(&key).await.unwrap();
    }

    println!("✓ Redis Cluster 数据分布测试成功");
}

#[tokio::test]
async fn test_redis_cluster_ttl() {
    println!("测试 Redis Cluster TTL...");

    if !is_cluster_available() {
        println!("跳过测试: Redis Cluster 不可用");
        return;
    }

    let urls = get_cluster_urls();
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();

    if !wait_for_redis_cluster(&url_refs).await {
        println!("跳过测试: Redis Cluster 未就绪");
        return;
    }

    let backend = RedisBackend::new(&urls[0]).await.unwrap();

    // 设置带 TTL 的键
    backend
        .set(Arc::from("cluster_ttl_key"), Arc::new(b"ttl_value".to_vec()), Some(Duration::from_secs(2)))
        .await
        .unwrap();

    // 验证键存在
    assert!(backend.exists("cluster_ttl_key").await.unwrap());

    // 获取 TTL
    let ttl = backend.ttl("cluster_ttl_key").await.unwrap();
    assert!(ttl.is_some());

    // 等待过期
    // ponytail: requires Docker, polling not feasible without container
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 验证键已过期
    assert!(!backend.exists("cluster_ttl_key").await.unwrap());

    println!("✓ Redis Cluster TTL 测试成功");
}

#[tokio::test]
async fn test_redis_cluster_health_check() {
    println!("测试 Redis Cluster 健康检查...");

    if !is_cluster_available() {
        println!("跳过测试: Redis Cluster 不可用");
        return;
    }

    let urls = get_cluster_urls();
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();

    if !wait_for_redis_cluster(&url_refs).await {
        println!("跳过测试: Redis Cluster 未就绪");
        return;
    }

    let backend = RedisBackend::new(&urls[0]).await.unwrap();

    backend.health_check().await.unwrap();

    println!("✓ Redis Cluster 健康检查测试成功");
}

#[tokio::test]
async fn test_redis_cluster_stats() {
    println!("测试 Redis Cluster 统计信息...");

    if !is_cluster_available() {
        println!("跳过测试: Redis Cluster 不可用");
        return;
    }

    let urls = get_cluster_urls();
    let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();

    if !wait_for_redis_cluster(&url_refs).await {
        println!("跳过测试: Redis Cluster 未就绪");
        return;
    }

    let backend = RedisBackend::new(&urls[0]).await.unwrap();

    backend
        .set(Arc::from("stats_test_key"), Arc::new(b"stats_value".to_vec()), None)
        .await
        .unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"redis".to_string()));

    backend.delete("stats_test_key").await.unwrap();

    println!("✓ Redis Cluster 统计信息测试成功");
}
