// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// tests/sync_api_integration.rs
//
// Sync API 端到端集成测试 (spec: sync-cache-api)
//
// 验证 Cache<K,V> 的 sync API（sync_mode）与 ChainCache 的 sync API
// （from_sync_backend）在 Moka / DashMap 后端上的端到端行为。
//
// 注意：所有涉及 Moka sync 的测试使用 `multi_thread` flavor，因为
// Moka 的 sync_block_on 在 current-thread runtime 上会 panic（TG10 经验）。

#![cfg(feature = "memory")]

use std::time::Duration;

use oxcache::Cache;
use oxcache::backend::{DashMapMemoryBackend, MokaMemoryBackend};
use oxcache::cache::{ChainCache, ChainLink};

// ============================================================================
// Cache<K,V> sync API（sync_mode + Moka 后端）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_sync_full_lifecycle() {
    // sync_mode(true) 启用 Moka 后端的 sync API
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    // set_sync + get_sync roundtrip
    cache.set_sync(&"k1".to_string(), &"v1".to_string()).unwrap();
    let value = cache.get_sync(&"k1".to_string()).unwrap();
    assert_eq!(value, Some("v1".to_string()));

    // exists_sync
    assert!(cache.exists_sync(&"k1".to_string()).unwrap());
    assert!(!cache.exists_sync(&"missing".to_string()).unwrap());

    // delete_sync + verify gone
    cache.delete_sync(&"k1".to_string()).unwrap();
    assert_eq!(cache.get_sync(&"k1".to_string()).unwrap(), None);
    assert!(!cache.exists_sync(&"k1".to_string()).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_sync_with_ttl_expires() {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    // set with 50ms TTL
    cache
        .set_with_ttl_sync(&"k".to_string(), &"v".to_string(), Some(Duration::from_millis(50)))
        .unwrap();

    // 立即 get_sync 应返回 Some
    let value = cache.get_sync(&"k".to_string()).unwrap();
    assert_eq!(value, Some("v".to_string()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Moka 异步清理可能略有延迟，循环等待
    let mut expired = false;
    for _ in 0..10 {
        if cache.get_sync(&"k".to_string()).unwrap().is_none() {
            expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(expired, "sync get should return None after TTL expires");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_get_or_sync_hit_and_miss() {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    // Cache miss: fallback 被调用
    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let call_count_clone = call_count.clone();
    let value = cache
        .get_or_sync(&"user:1".to_string(), || {
            call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok("Alice".to_string())
        })
        .unwrap();
    assert_eq!(value, "Alice");
    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Cache hit: fallback 不被调用
    let value = cache
        .get_or_sync(&"user:1".to_string(), || Ok("Should not be called".to_string()))
        .unwrap();
    assert_eq!(value, "Alice");
    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}

// ============================================================================
// ChainCache sync API（from_sync_backend + Moka / DashMap）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_multi_backend_roundtrip() {
    // Moka (score=100) + DashMap (score=90)，都通过 from_sync_backend
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    // 保留引用以验证写入传播
    let moka_ref = moka.clone();
    let dashmap_ref = dashmap.clone();

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_sync_backend(dashmap))
        .build();

    // sync set 写入所有链接
    chain.set_sync("k", b"v".to_vec(), None).unwrap();

    // 两个后端都应有值（通过 sync API 直接查询）
    use oxcache::backend::SyncCacheReader;
    assert_eq!(SyncCacheReader::get(&moka_ref, "k").unwrap(), Some(b"v".to_vec()));
    assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), Some(b"v".to_vec()));

    // chain get_sync 返回最高分链接的值
    let value = chain.get_sync("k").unwrap();
    assert_eq!(value, Some(b"v".to_vec()));

    // sync delete 从所有链接删除
    chain.delete_sync("k").unwrap();
    assert_eq!(SyncCacheReader::get(&moka_ref, "k").unwrap(), None);
    assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), None);
    assert_eq!(chain.get_sync("k").unwrap(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_ttl_propagates_to_all_links() {
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let moka_ref = moka.clone();
    let dashmap_ref = dashmap.clone();

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_sync_backend(dashmap))
        .build();

    // set_sync with 50ms TTL，透传到所有链接
    chain
        .set_sync("k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .unwrap();

    // 立即两个后端都应返回 Some
    use oxcache::backend::SyncCacheReader;
    assert_eq!(SyncCacheReader::get(&moka_ref, "k").unwrap(), Some(b"v".to_vec()));
    assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // DashMap lazy 过期，立即查询返回 None
    assert_eq!(SyncCacheReader::get(&dashmap_ref, "k").unwrap(), None);

    // Moka 异步清理可能略有延迟，循环等待
    let mut moka_expired = false;
    for _ in 0..10 {
        if SyncCacheReader::get(&moka_ref, "k").unwrap().is_none() {
            moka_expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(moka_expired, "moka link should expire after TTL");

    // chain get_sync 应返回 None（所有链接都过期）
    assert_eq!(chain.get_sync("k").unwrap(), None);
}

// ============================================================================
// sync_mode 与 async API 混用
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_and_async_coexist() {
    // sync_mode(true) 后，async API 仍然可用
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    // async set + sync get
    cache
        .set(&"async_key".to_string(), &"async_value".to_string())
        .await
        .unwrap();
    let value = cache.get_sync(&"async_key".to_string()).unwrap();
    assert_eq!(value, Some("async_value".to_string()));

    // sync set + async get
    cache
        .set_sync(&"sync_key".to_string(), &"sync_value".to_string())
        .unwrap();
    let value = cache.get(&"sync_key".to_string()).await.unwrap();
    assert_eq!(value, Some("sync_value".to_string()));
}
