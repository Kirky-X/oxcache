// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 链式缓存集成测试

use oxcache::backend::{BackendScore, CacheBackend, MokaMemoryBackend, Scores};
use oxcache::builder::OxCacheBuilder;
use oxcache::chain::{ChainCache, ChainLink};
use std::time::Duration;

/// 测试链式缓存基本读写
#[tokio::test]
async fn test_chain_cache_basic_operations() {
    let moka1 = MokaMemoryBackend::builder().capacity(100).build();
    let moka2 = MokaMemoryBackend::builder().capacity(100).build();

    let chain = OxCacheBuilder::new().backend(moka1).backend(moka2).build().unwrap();

    // 测试设置值
    chain.set("key1", b"value1".to_vec(), None).await.unwrap();

    // 测试获取值
    let value = chain.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // 测试存在性检查
    let exists = chain.exists("key1").await.unwrap();
    assert!(exists);

    // 测试删除
    chain.delete("key1").await.unwrap();
    let exists = chain.exists("key1").await.unwrap();
    assert!(!exists);
}

/// 测试链式缓存自动排序
#[tokio::test]
async fn test_chain_cache_auto_sort() {
    let high = MokaMemoryBackend::builder().capacity(100).build();
    let low = MokaMemoryBackend::builder().capacity(100).build();

    // 验证 Moka 后端分数
    assert_eq!(high.score(), Scores::MOKA);

    // 构建链式缓存（后端会被自动排序）
    let chain = OxCacheBuilder::new().backend(low).backend(high).build().unwrap();

    // 验证后端数量
    assert_eq!(chain.len(), 2);
}

/// 测试链式缓存回填功能
#[tokio::test]
async fn test_chain_cache_backfill() {
    let high = MokaMemoryBackend::builder().capacity(100).build();
    let low = MokaMemoryBackend::builder().capacity(100).build();

    // 只在低分后端设置值
    low.set("key1", b"value1".to_vec(), None).await.unwrap();

    // 构建启用回填的链式缓存
    let chain = OxCacheBuilder::new()
        .backend(high.clone())
        .backend(low.clone())
        .enable_backfill()
        .build()
        .unwrap();

    // 读取应该触发回填
    let value = chain.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // 高分后端现在应该有值了（回填）
    let high_value = high.get("key1").await.unwrap();
    assert_eq!(high_value, Some(b"value1".to_vec()));
}

/// 测试空链式缓存
#[tokio::test]
async fn test_empty_chain_cache() {
    let result = OxCacheBuilder::new().build();
    assert!(result.is_err());
}

/// 测试单后端链式缓存
#[tokio::test]
async fn test_single_backend_chain() {
    let moka = MokaMemoryBackend::new();

    let chain = OxCacheBuilder::new().backend(moka).build().unwrap();

    assert_eq!(chain.len(), 1);

    // 测试基本操作
    chain.set("key", b"value".to_vec(), None).await.unwrap();
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

/// 测试链式缓存 TTL
#[tokio::test]
async fn test_chain_cache_ttl() {
    let moka = MokaMemoryBackend::builder()
        .capacity(100)
        .ttl(Duration::from_secs(3600))
        .build();

    let chain = OxCacheBuilder::new()
        .backend(moka)
        .default_ttl(Duration::from_secs(60))
        .build()
        .unwrap();

    // 设置带 TTL 的值
    chain
        .set("key", b"value".to_vec(), Some(Duration::from_secs(10)))
        .await
        .unwrap();

    // 验证值存在
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

/// 测试链式缓存健康检查
#[tokio::test]
async fn test_chain_cache_health_check() {
    let moka = MokaMemoryBackend::new();

    let chain = OxCacheBuilder::new().backend(moka).build().unwrap();

    // Moka 后端应该总是健康的
    let healthy = chain.health_check().await.unwrap();
    assert!(healthy);
}

/// 测试链式缓存统计信息
#[tokio::test]
async fn test_chain_cache_stats() {
    let moka = MokaMemoryBackend::new();

    let chain = OxCacheBuilder::new().backend(moka).build().unwrap();

    let stats = chain.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"chain".to_string()));
    assert_eq!(stats.get("backend_count"), Some(&"1".to_string()));
}

/// 测试链式缓存清理
#[tokio::test]
async fn test_chain_cache_clear() {
    let moka = MokaMemoryBackend::new();

    let chain = OxCacheBuilder::new().backend(moka).build().unwrap();

    // 设置多个值
    for i in 0..10 {
        chain.set(&format!("key{}", i), b"value".to_vec(), None).await.unwrap();
    }

    // 清理
    chain.clear().await.unwrap();

    // 验证所有键都被删除
    for i in 0..10 {
        let exists = chain.exists(&format!("key{}", i)).await.unwrap();
        assert!(!exists, "key{} should not exist after clear", i);
    }
}

/// 测试 ChainCacheBuilder 直接使用
#[tokio::test]
async fn test_chain_cache_builder_direct() {
    let moka = MokaMemoryBackend::new();

    let chain = ChainCache::builder()
        .backend(moka)
        .default_ttl(Duration::from_secs(300))
        .enable_backfill()
        .build();

    assert_eq!(chain.links().len(), 1);
}

/// 测试 ChainLink 创建
#[tokio::test]
async fn test_chain_link_creation() {
    let moka = MokaMemoryBackend::new();

    let link = ChainLink::from_backend(moka);

    assert_eq!(link.score, Scores::MOKA);
    assert!(!link.is_persistent);
    assert_eq!(link.name, "moka");
}
