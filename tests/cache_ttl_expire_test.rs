// Copyright (c) 2025-2026, Kirky.X
//
// MIT License

// tests/cache_ttl_expire_test.rs
//
// 验证 Cache<K,V> 暴露的 ttl() / expire() 方法（async + sync）。
//
// 背景：0.3.0 sync API 实现时遗漏了 ttl/expire 方法的暴露，导致下游用户
// 在 update 缓存值时无法保留原 TTL（set 覆盖会清除 per-entry TTL，
// token 永久驻留）。本测试覆盖该修复。

#![cfg(feature = "memory")]

use std::time::Duration;

use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Token {
    id: u64,
    value: String,
}

// ============================================================================
// async API: ttl / expire
// ============================================================================

#[tokio::test]
async fn test_cache_ttl_returns_none_for_no_ttl_key() {
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();
    cache
        .set(
            &"no_ttl".to_string(),
            &Token {
                id: 1,
                value: "v".into(),
            },
        )
        .await
        .unwrap();

    // 无 per-entry TTL → None
    let ttl = cache.ttl(&"no_ttl".to_string()).await.unwrap();
    assert_eq!(ttl, None);
}

#[tokio::test]
async fn test_cache_ttl_returns_remaining_for_ttl_key() {
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();
    cache
        .set_with_ttl(
            &"with_ttl".to_string(),
            &Token {
                id: 2,
                value: "v".into(),
            },
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    let ttl = cache.ttl(&"with_ttl".to_string()).await.unwrap();
    assert!(ttl.is_some(), "ttl must be Some for key with per-entry TTL");
    let remaining = ttl.unwrap();
    assert!(
        remaining <= Duration::from_secs(60) && remaining > Duration::from_secs(50),
        "remaining ttl should be in (50s, 60s], got {:?}",
        remaining
    );
}

#[tokio::test]
async fn test_cache_ttl_returns_none_for_missing_key() {
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();
    let ttl = cache.ttl(&"missing".to_string()).await.unwrap();
    assert_eq!(ttl, None);
}

#[tokio::test]
async fn test_cache_expire_sets_ttl_on_existing_key() {
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();
    cache
        .set(
            &"k".to_string(),
            &Token {
                id: 3,
                value: "v".into(),
            },
        )
        .await
        .unwrap();

    // 初始无 TTL
    assert_eq!(cache.ttl(&"k".to_string()).await.unwrap(), None);

    // expire 设置 TTL
    let ok = cache.expire(&"k".to_string(), Duration::from_secs(30)).await.unwrap();
    assert!(ok, "expire should return true for existing key");

    // 验证 TTL 已设置
    let ttl = cache.ttl(&"k".to_string()).await.unwrap();
    assert!(ttl.is_some());
    let remaining = ttl.unwrap();
    assert!(
        remaining <= Duration::from_secs(30) && remaining > Duration::from_secs(20),
        "remaining ttl should be in (20s, 30s], got {:?}",
        remaining
    );
}

#[tokio::test]
async fn test_cache_expire_returns_false_for_missing_key() {
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();
    let ok = cache
        .expire(&"missing".to_string(), Duration::from_secs(30))
        .await
        .unwrap();
    assert!(!ok, "expire should return false for missing key");
}

#[tokio::test]
async fn test_cache_update_preserves_ttl_via_ttl_query() {
    // 模拟下游 BulwarkDaoOxcache::update 场景：
    // 1. set key with TTL
    // 2. update value but preserve original TTL
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();
    let key = "token:abc".to_string();

    // 1. 初始 set with 60s TTL
    cache
        .set_with_ttl(
            &key,
            &Token {
                id: 1,
                value: "old".into(),
            },
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    // 2. update：先查原 TTL，再 set_with_ttl 保留
    let original_ttl = cache.ttl(&key).await.unwrap();
    assert!(original_ttl.is_some(), "original ttl must be Some");

    cache
        .set_with_ttl(
            &key,
            &Token {
                id: 1,
                value: "new".into(),
            },
            original_ttl,
        )
        .await
        .unwrap();

    // 3. 验证值已更新
    let updated = cache.get(&key).await.unwrap().unwrap();
    assert_eq!(updated.value, "new");

    // 4. 验证 TTL 仍然存在（未被清除）
    let ttl_after = cache.ttl(&key).await.unwrap();
    assert!(ttl_after.is_some(), "TTL must be preserved after update");
    let remaining = ttl_after.unwrap();
    assert!(
        remaining > Duration::from_secs(50),
        "remaining ttl should still be > 50s after quick update, got {:?}",
        remaining
    );
}

// ============================================================================
// sync API: ttl_sync / expire_sync
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_ttl_sync_returns_none_for_no_ttl_key() {
    let cache: Cache<String, Token> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache
        .set_sync(
            &"no_ttl".to_string(),
            &Token {
                id: 1,
                value: "v".into(),
            },
        )
        .unwrap();

    let ttl = cache.ttl_sync(&"no_ttl".to_string()).unwrap();
    assert_eq!(ttl, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_ttl_sync_returns_remaining_for_ttl_key() {
    let cache: Cache<String, Token> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache
        .set_with_ttl_sync(
            &"with_ttl".to_string(),
            &Token {
                id: 2,
                value: "v".into(),
            },
            Some(Duration::from_secs(60)),
        )
        .unwrap();

    let ttl = cache.ttl_sync(&"with_ttl".to_string()).unwrap();
    assert!(ttl.is_some());
    let remaining = ttl.unwrap();
    assert!(
        remaining <= Duration::from_secs(60) && remaining > Duration::from_secs(50),
        "remaining ttl should be in (50s, 60s], got {:?}",
        remaining
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_expire_sync_sets_ttl_on_existing_key() {
    let cache: Cache<String, Token> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache
        .set_sync(
            &"k".to_string(),
            &Token {
                id: 3,
                value: "v".into(),
            },
        )
        .unwrap();

    assert_eq!(cache.ttl_sync(&"k".to_string()).unwrap(), None);

    let ok = cache.expire_sync(&"k".to_string(), Duration::from_secs(30)).unwrap();
    assert!(ok);

    let ttl = cache.ttl_sync(&"k".to_string()).unwrap();
    assert!(ttl.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_expire_sync_returns_false_for_missing_key() {
    let cache: Cache<String, Token> = Cache::builder().sync_mode(true).build().await.unwrap();
    let ok = cache
        .expire_sync(&"missing".to_string(), Duration::from_secs(30))
        .unwrap();
    assert!(!ok);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cache_sync_returns_not_supported_without_sync_mode() {
    // 不启用 sync_mode → ttl_sync/expire_sync 应返回 Err(NotSupported)
    let cache: Cache<String, Token> = Cache::builder().build().await.unwrap();

    let err = cache.ttl_sync(&"k".to_string()).unwrap_err();
    assert!(matches!(err, oxcache::error::CacheError::NotSupported(_)));

    let err = cache
        .expire_sync(&"k".to_string(), Duration::from_secs(30))
        .unwrap_err();
    assert!(matches!(err, oxcache::error::CacheError::NotSupported(_)));
}
