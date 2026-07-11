// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 链式缓存集成测试

use oxcache::backend::{BackendScore, CacheConnector, CacheReader, CacheWriter, MokaMemoryBackend, Scores};
use oxcache::cache::{ChainCache, ChainLink};
use std::time::Duration;

/// 测试链式缓存基本读写
#[tokio::test]
async fn test_chain_cache_basic_operations() {
    let moka1 = MokaMemoryBackend::builder().capacity(100).build();
    let moka2 = MokaMemoryBackend::builder().capacity(100).build();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(moka1))
        .link(ChainLink::from_backend(moka2))
        .build();

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
    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(low))
        .link(ChainLink::from_backend(high))
        .build();

    // 验证后端数量
    assert_eq!(chain.links().len(), 2);
}

/// 测试链式缓存回填功能
#[tokio::test]
async fn test_chain_cache_backfill() {
    let high = MokaMemoryBackend::builder().capacity(100).build();
    let low = MokaMemoryBackend::builder().capacity(100).build();

    // 只在低分后端设置值
    low.set("key1", b"value1".to_vec(), None).await.unwrap();

    // 构建启用回填的链式缓存
    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(high))
        .link(ChainLink::from_backend(low))
        .enable_backfill()
        .build();

    // 读取应该触发回填（从第一个 link 读取）
    let value = chain.get("key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

/// 测试单后端链式缓存
#[tokio::test]
async fn test_single_backend_chain() {
    let moka = MokaMemoryBackend::new();

    let chain = ChainCache::builder().link(ChainLink::from_backend(moka)).build();

    assert_eq!(chain.links().len(), 1);

    // 测试基本操作
    chain.set("key", b"value".to_vec(), None).await.unwrap();
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

/// 测试链式缓存 TTL（各 backend 使用自己的 TTL）
#[tokio::test]
async fn test_chain_cache_ttl() {
    let moka = MokaMemoryBackend::builder()
        .capacity(100)
        .ttl(Duration::from_secs(3600))
        .build();

    let chain = ChainCache::builder().link(ChainLink::from_backend(moka)).build();

    // 设置值，backend 使用自己的默认 TTL（3600秒）
    chain.set("key", b"value".to_vec(), None).await.unwrap();

    // 验证值存在
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

/// 测试链式缓存健康检查
#[tokio::test]
async fn test_chain_cache_health_check() {
    let moka = MokaMemoryBackend::new();

    let chain = ChainCache::builder().link(ChainLink::from_backend(moka)).build();

    // Moka 后端应该总是健康的
    chain.health_check().await.unwrap();
}

/// 测试链式缓存统计信息
#[tokio::test]
async fn test_chain_cache_stats() {
    let moka = MokaMemoryBackend::new();

    let chain = ChainCache::builder().link(ChainLink::from_backend(moka)).build();

    let stats = chain.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"chain".to_string()));
    assert_eq!(stats.get("backend_count"), Some(&"1".to_string()));
}

/// 测试链式缓存清理
#[tokio::test]
async fn test_chain_cache_clear() {
    let moka = MokaMemoryBackend::new();

    let chain = ChainCache::builder().link(ChainLink::from_backend(moka)).build();

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
        .link(ChainLink::from_backend(moka))
        .enable_backfill()
        .build();

    assert_eq!(chain.links().len(), 1);
}

/// 测试 ChainLink 创建
#[tokio::test]
async fn test_chain_link_creation() {
    let moka = MokaMemoryBackend::new();

    let link = ChainLink::from_backend(moka);

    assert_eq!(link.score(), Scores::MOKA);
    assert!(!link.is_persistent());
    assert_eq!(link.name(), "moka");
}
