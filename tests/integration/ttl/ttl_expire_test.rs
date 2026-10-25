// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
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

// async API: ttl / expire

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
    // 默认 10% TTL 抖动(防雪崩)会使剩余时长落在 [54s,66s];
    // 本测试断言精确窗口,显式关闭抖动
    let cache: Cache<String, Token> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
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
    let ok = cache
        .expire(&"k".to_string(), Duration::from_secs(30))
        .await
        .unwrap();
    assert!(ok, "expire should return true for existing key");

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
    // 模拟下游 BulwarkDaoOxcache::update 场景：set 带 TTL 后更新值并保留原 TTL
    // 本用例断言的是「TTL 被保留」，不是「TTL 的抖动区间」——抖动因子默认
    // 0.1（base_ttl * (1 ± factor)），且每次写入都会重算，复合后剩余量可
    // 偏离基准 ±21%，用固定阈值断言会偶发红。此处显式关闭抖动，让断言
    // 精确可判：更新后的剩余量必须与更新前读到的剩余量一致（仅允许流逝）。
    let cache: Cache<String, Token> = Cache::builder().ttl_jitter(0.0).build().await.unwrap();
    let key = "token:abc".to_string();

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

    let original_ttl = cache.ttl(&key).await.unwrap();
    assert!(original_ttl.is_some(), "original ttl must be Some");
    let original_remaining = original_ttl.unwrap();

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

    let updated = cache.get(&key).await.unwrap().unwrap();
    assert_eq!(updated.value, "new");

    let ttl_after = cache.ttl(&key).await.unwrap();
    assert!(ttl_after.is_some(), "TTL must be preserved after update");
    let remaining = ttl_after.unwrap();
    // TTL 必须被原样保留：既不能被重置为完整 TTL，也不能被丢弃。
    let drift = original_remaining.abs_diff(remaining);
    assert!(
        drift < Duration::from_millis(500),
        "ttl must be preserved across update: before={:?} after={:?} drift={:?}",
        original_remaining,
        remaining,
        drift
    );
}

// sync API: ttl_sync / expire_sync

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
    // 断言精确 TTL 窗口：显式关闭默认抖动
    let cache: Cache<String, Token> = Cache::builder()
        .sync_mode(true)
        .ttl_jitter(0.0)
        .build()
        .await
        .unwrap();
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

    let ok = cache
        .expire_sync(&"k".to_string(), Duration::from_secs(30))
        .unwrap();
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
    assert!(matches!(err, oxcache::error::OxCacheError::NotSupported(_)));

    let err = cache
        .expire_sync(&"k".to_string(), Duration::from_secs(30))
        .unwrap_err();
    assert!(matches!(err, oxcache::error::OxCacheError::NotSupported(_)));
}
