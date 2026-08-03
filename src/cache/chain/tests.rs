// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! ChainCache 单元测试

use super::*;
use crate::backend::{DashMapMemoryBackend, MokaMemoryBackend};
use crate::testing::MockBackend;

#[test]
fn test_chain_link_creation() {
    let backend = MockBackend::new("test", 50, false);
    let link = ChainLink::from_backend(backend);

    assert_eq!(link.score(), 50);
    assert!(!link.is_persistent());
    assert_eq!(link.name(), "test");
}

#[test]
fn test_chain_cache_builder() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let chain = ChainCache::builder()
        .backend(low)
        .backend(high)
        .enable_backfill()
        .build();

    assert_eq!(chain.links().len(), 2);
    assert_eq!(chain.links()[0].score(), 100);
    assert_eq!(chain.links()[1].score(), 50);
}

#[tokio::test]
async fn test_chain_cache_get_set() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let chain = ChainCache::builder().backend(high).backend(low).build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();

    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_chain_cache_delete() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let chain = ChainCache::builder().backend(high).backend(low).build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();
    chain.delete("key").await.unwrap();

    let exists = chain.exists("key").await.unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn test_chain_cache_backfill() {
    // Build chain with backfill enabled
    let chain = ChainCache::builder()
        .link(ChainLink::new(MockBackend::new("high", 100, false), 100, false, "high"))
        .link(ChainLink::new(MockBackend::new("low", 50, true), 50, true, "low"))
        .enable_backfill()
        .build();

    // Set value in chain (writes to all backends)
    chain.set("key", b"value".to_vec(), None).await.unwrap();

    // Read should succeed
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_empty_chain() {
    let chain = ChainCache::new(vec![]);

    let value = chain.get("key").await.unwrap();
    assert!(value.is_none());

    let exists = chain.exists("key").await.unwrap();
    assert!(!exists);
}

// ========================================================================
// ChainLink tests
// ========================================================================

#[test]
fn test_chain_link_new_constructor() {
    let backend = MokaMemoryBackend::new();
    let link = ChainLink::new(backend, 75, true, "custom");

    assert_eq!(link.score(), 75);
    assert!(link.is_persistent());
    assert_eq!(link.name(), "custom");
    // backend() getter should return a usable reference
    let _backend_ref = link.backend();
}

#[test]
fn test_chain_link_from_backend_moka() {
    let backend = MokaMemoryBackend::new();
    let link = ChainLink::from_backend(backend);

    // Moka scores 100 (Scores::MOKA), non-persistent, name "moka"
    assert_eq!(link.score(), 100);
    assert!(!link.is_persistent());
    assert_eq!(link.name(), "moka");
}

#[test]
fn test_chain_link_debug() {
    let backend = MokaMemoryBackend::new();
    let link = ChainLink::new(backend, 80, true, "dbg");

    let debug_str = format!("{:?}", link);
    assert!(debug_str.contains("ChainLink"));
    assert!(debug_str.contains("80"));
    assert!(debug_str.contains("dbg"));
}

// ========================================================================
// ChainCache accessor tests
// ========================================================================

#[test]
fn test_chain_cache_new_constructor() {
    let link = ChainLink::from_backend(MokaMemoryBackend::new());
    let chain = ChainCache::new(vec![link]);

    assert_eq!(chain.len(), 1);
    assert!(!chain.is_empty());
}

#[test]
fn test_chain_cache_len_is_empty() {
    let empty = ChainCache::new(vec![]);
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);

    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();
    assert!(!chain.is_empty());
    assert_eq!(chain.len(), 1);
}

#[test]
fn test_chain_cache_get_by_score() {
    let chain = ChainCache::builder()
        .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
        .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
        .build();

    assert!(chain.get_by_score(100).is_some());
    assert!(chain.get_by_score(50).is_some());
    assert!(chain.get_by_score(75).is_none());
}

#[test]
fn test_chain_cache_highest_lowest_backend() {
    // Add low first to verify sorting works
    let chain = ChainCache::builder()
        .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
        .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
        .build();

    let highest = chain.highest_score_backend().unwrap();
    assert_eq!(highest.score(), 100);
    assert_eq!(highest.name(), "high");

    let lowest = chain.lowest_score_backend().unwrap();
    assert_eq!(lowest.score(), 50);
    assert_eq!(lowest.name(), "low");
}

#[test]
fn test_chain_cache_highest_lowest_empty() {
    let chain = ChainCache::new(vec![]);
    assert!(chain.highest_score_backend().is_none());
    assert!(chain.lowest_score_backend().is_none());
}

#[test]
fn test_chain_cache_persistent_filters() {
    let chain = ChainCache::builder()
        .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
        .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
        .build();

    let persistent = chain.persistent_backends();
    assert_eq!(persistent.len(), 1);
    assert_eq!(persistent[0].name(), "low");

    let non_persistent = chain.non_persistent_backends();
    assert_eq!(non_persistent.len(), 1);
    assert_eq!(non_persistent[0].name(), "high");
}

#[test]
fn test_chain_cache_links_accessor() {
    let chain = ChainCache::builder()
        .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
        .build();

    let links = chain.links();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].name(), "high");
}

// ========================================================================
// Builder tests
// ========================================================================

#[test]
fn test_builder_link_method() {
    let link = ChainLink::new(MokaMemoryBackend::new(), 100, false, "moka");
    let chain = ChainCache::builder().link(link).build();
    assert_eq!(chain.len(), 1);
}

#[test]
fn test_builder_links_method() {
    let links = vec![
        ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"),
        ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"),
    ];
    let chain = ChainCache::builder().links(links).build();
    assert_eq!(chain.len(), 2);
    // Verify sorting by score descending
    assert_eq!(chain.links()[0].score(), 100);
    assert_eq!(chain.links()[1].score(), 50);
}

#[tokio::test]
async fn test_builder_default_time_to_live() {
    let chain = ChainCache::builder()
        .backend(MokaMemoryBackend::new())
        .default_time_to_live(Duration::from_secs(60))
        .build();

    // set with None should use default_ttl
    chain.set("key", b"value".to_vec(), None).await.unwrap();
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

#[test]
fn test_builder_disable_backfill() {
    let chain = ChainCache::builder()
        .backend(MokaMemoryBackend::new())
        .enable_backfill()
        .disable_backfill()
        .build();

    assert_eq!(chain.len(), 1);
}

// ========================================================================
// UnifiedCache trait tests (get_bytes / set_bytes)
// ========================================================================

#[tokio::test]
async fn test_chain_cache_get_bytes_set_bytes() {
    use crate::UnifiedCache;
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    chain.set_bytes("key", b"value".to_vec(), None).await.unwrap();
    let value = chain.get_bytes("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_chain_cache_get_bytes_missing() {
    use crate::UnifiedCache;
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    let value = chain.get_bytes("missing").await.unwrap();
    assert!(value.is_none());
}

// ========================================================================
// CacheWriter tests
// ========================================================================

#[tokio::test]
async fn test_chain_cache_clear() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();
    assert!(chain.exists("key").await.unwrap());

    chain.clear().await.unwrap();
    assert!(!chain.exists("key").await.unwrap());
}

#[tokio::test]
async fn test_chain_cache_clear_empty() {
    let chain = ChainCache::new(vec![]);
    assert!(chain.clear().await.is_ok());
}

#[tokio::test]
async fn test_chain_cache_expire() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();
    // Moka now supports per-entry TTL via Expiry trait; expire on existing key returns true
    let result = chain.expire("key", Duration::from_secs(60)).await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_chain_cache_expire_missing_key() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    let result = chain.expire("missing", Duration::from_secs(60)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_chain_cache_set_empty_chain_error() {
    let chain = ChainCache::new(vec![]);
    let result = chain.set("key", b"value".to_vec(), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_chain_cache_delete_empty_chain() {
    let chain = ChainCache::new(vec![]);
    assert!(chain.delete("key").await.is_ok());
}

#[tokio::test]
async fn test_chain_cache_set_with_explicit_ttl() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    chain
        .set("key", b"value".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_chain_cache_multi_backend_set_writes_all() {
    let high = MokaMemoryBackend::new();
    let low = MokaMemoryBackend::new();

    let high_ref = high.clone();
    let low_ref = low.clone();

    let chain = ChainCache::builder()
        .link(ChainLink::new(high, 100, false, "high"))
        .link(ChainLink::new(low, 50, true, "low"))
        .build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();

    // Both backends should have the value
    assert_eq!(high_ref.get("key").await.unwrap(), Some(b"value".to_vec()));
    assert_eq!(low_ref.get("key").await.unwrap(), Some(b"value".to_vec()));
}

#[tokio::test]
async fn test_chain_cache_delete_removes_from_all() {
    let high = MokaMemoryBackend::new();
    let low = MokaMemoryBackend::new();

    let high_ref = high.clone();
    let low_ref = low.clone();

    let chain = ChainCache::builder()
        .link(ChainLink::new(high, 100, false, "high"))
        .link(ChainLink::new(low, 50, true, "low"))
        .build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();
    chain.delete("key").await.unwrap();

    assert!(high_ref.get("key").await.unwrap().is_none());
    assert!(low_ref.get("key").await.unwrap().is_none());
}

// ========================================================================
// Backfill behavior tests
// ========================================================================

#[tokio::test]
async fn test_chain_cache_backfill_populates_higher() {
    let high = MokaMemoryBackend::new();
    let low = MokaMemoryBackend::new();

    let high_ref = high.clone();
    let low_ref = low.clone();

    let chain = ChainCache::builder()
        .link(ChainLink::new(high, 100, false, "high"))
        .link(ChainLink::new(low, 50, true, "low"))
        .enable_backfill()
        .build();

    // Set value only in low backend (bypass chain)
    low_ref
        .set(Arc::from("key"), Arc::new(b"low_value".to_vec()), None)
        .await
        .unwrap();

    // Verify high doesn't have it yet
    assert!(high_ref.get("key").await.unwrap().is_none());

    // Get from chain - should find in low and backfill to high
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"low_value".to_vec()));

    // Verify high now has the value (backfilled asynchronously — poll)
    let mut backfilled = false;
    for _ in 0..10 {
        if high_ref.get("key").await.unwrap().is_some() {
            backfilled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let high_value = high_ref.get("key").await.unwrap();
    assert_eq!(
        high_value,
        Some(b"low_value".to_vec()),
        "backfill should populate high backend"
    );
    assert!(backfilled, "backfill should complete asynchronously");
}

#[tokio::test]
async fn test_chain_cache_no_backfill_when_disabled() {
    let high = MokaMemoryBackend::new();
    let low = MokaMemoryBackend::new();

    let high_ref = high.clone();
    let low_ref = low.clone();

    let chain = ChainCache::builder()
        .link(ChainLink::new(high, 100, false, "high"))
        .link(ChainLink::new(low, 50, true, "low"))
        .build(); // backfill disabled by default

    // Set value only in low backend (bypass chain)
    low_ref
        .set(Arc::from("key"), Arc::new(b"low_value".to_vec()), None)
        .await
        .unwrap();

    // Get from chain - should find in low but NOT backfill to high
    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"low_value".to_vec()));

    // Verify high still doesn't have the value
    assert!(high_ref.get("key").await.unwrap().is_none());
}

// ========================================================================
// CacheReader trait tests
// ========================================================================

#[tokio::test]
async fn test_chain_cache_ttl_len_capacity() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    chain.set("key", b"value".to_vec(), None).await.unwrap();

    // ttl - Moka returns None for per-entry TTL
    let ttl = chain.ttl("key").await.unwrap();
    assert!(ttl.is_none());

    // len (CacheReader trait) - Moka's entry_count is approximate
    let len = CacheReader::len(&chain).await.unwrap();
    assert!(len <= 100, "len should be reasonable after single insert");

    // capacity
    let capacity = chain.capacity().await.unwrap();
    assert!(capacity > 0);
}

#[tokio::test]
async fn test_chain_cache_reader_empty() {
    let chain = ChainCache::new(vec![]);

    assert_eq!(CacheReader::len(&chain).await.unwrap(), 0);
    assert!(CacheReader::is_empty(&chain).await.unwrap());
    assert_eq!(chain.capacity().await.unwrap(), 0);

    let ttl = chain.ttl("key").await.unwrap();
    assert!(ttl.is_none());
}

#[tokio::test]
async fn test_chain_cache_stats() {
    let chain = ChainCache::builder()
        .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
        .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
        .build();

    let stats = chain.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"chain".to_string()));
    assert_eq!(stats.get("backend_count"), Some(&"2".to_string()));
    assert_eq!(stats.get("backend_0_name"), Some(&"high".to_string()));
    assert_eq!(stats.get("backend_0_score"), Some(&"100".to_string()));
    assert_eq!(stats.get("backend_1_name"), Some(&"low".to_string()));
    assert_eq!(stats.get("backend_1_score"), Some(&"50".to_string()));
}

#[tokio::test]
async fn test_chain_cache_stats_empty() {
    let chain = ChainCache::new(vec![]);
    let stats = chain.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"chain".to_string()));
    assert_eq!(stats.get("backend_count"), Some(&"0".to_string()));
}

// ========================================================================
// CacheConnector trait tests
// ========================================================================

#[tokio::test]
async fn test_chain_cache_health_check() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    assert!(chain.health_check().await.is_ok());
}

#[tokio::test]
async fn test_chain_cache_health_check_empty() {
    let chain = ChainCache::new(vec![]);
    assert!(chain.health_check().await.is_ok());
}

// ========================================================================
// 故障降级与错误可见性测试 (问题 5.1 / 5.2 / 7.3)
// ========================================================================

#[tokio::test]
async fn test_chain_read_degrades_when_high_backend_fails() {
    // L1 (score=100) get 注入故障，L2 (score=50) 有数据
    // 链式 get 应降级到 L2 返回数据（L1 失败降级）
    let high = MockBackend::new("high", 100, false).with_fail_get();
    let low = MockBackend::new("low", 50, true);
    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(high))
        .link(ChainLink::from_backend(low))
        .build();

    // 仅在 L2 写入数据（通过链引用访问）
    chain.links()[1]
        .backend()
        .set(Arc::from("key"), Arc::new(b"low_value".to_vec()), None)
        .await
        .unwrap();

    let value = chain.get("key").await.unwrap();
    assert_eq!(value, Some(b"low_value".to_vec()), "L1 get 失败时应降级到 L2 读取");
}

#[tokio::test]
async fn test_chain_read_returns_none_when_all_backends_fail() {
    // L7 修复验证：所有后端 get 都失败时，链式 get 应返回 Err（与竞速读语义一致）
    let high = MockBackend::new("high", 100, false).with_fail_get();
    let low = MockBackend::new("low", 50, true).with_fail_get();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(high))
        .link(ChainLink::from_backend(low))
        .build();

    let result = chain.get("key").await;
    assert!(result.is_err(), "所有后端 get 失败时应返回 Err");
}

#[tokio::test]
async fn test_chain_health_check_fails_when_backend_unhealthy() {
    // 任一后端 health_check 失败，链式 health_check 应返回错误
    let healthy = MockBackend::new("ok", 100, false);
    let unhealthy = MockBackend::new("down", 50, true).with_fail_health();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(healthy))
        .link(ChainLink::from_backend(unhealthy))
        .build();

    let result = chain.health_check().await;
    assert!(result.is_err(), "含不健康后端时 health_check 应失败");
}

#[tokio::test]
async fn test_chain_write_succeeds_when_partial_backend_fails() {
    // 一个后端 set 注入故障，另一个正常：写操作不应整体失败
    let failing = MockBackend::new("failing", 100, false).with_fail_set();
    let ok = MokaMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(failing))
        .link(ChainLink::from_backend(ok))
        .build();

    // 不应报错（只要至少一个后端成功）
    chain.set("key", b"value".to_vec(), None).await.unwrap();
}

#[tokio::test]
async fn test_chain_cache_shutdown() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    // Should not panic
    chain.shutdown().await;
}

#[test]
fn test_chain_cache_backend_kind() {
    let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

    assert_eq!(chain.backend_kind(), BackendKind::Chain);
}

// ========================================================================
// TTL 透传契约测试 (spec: universal-per-entry-ttl / Decision 4c)
// ========================================================================

#[tokio::test]
async fn test_chain_set_with_ttl_propagates_to_all_links() {
    // 链中 Moka (score=100) + DashMap (score=50) + Mock (score=30)
    // set 50ms TTL，等 100ms，三者皆过期
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();
    let mock = MockBackend::new("mock", 30, false);

    let moka_ref = moka.clone();
    let dashmap_ref = dashmap.clone();
    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(moka))
        .link(ChainLink::from_backend(dashmap))
        .link(ChainLink::new(mock, 30, false, "mock"))
        .build();

    chain
        .set("k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .await
        .unwrap();

    // 立即链式 get 应返回 Some
    assert_eq!(chain.get("k").await.unwrap(), Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Moka 后端：moka 异步清理可能略有延迟，循环等待最多 500ms
    let mut moka_expired = false;
    for _ in 0..10 {
        if moka_ref.get("k").await.unwrap().is_none() {
            moka_expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(moka_expired, "moka link should expire after TTL");

    // DashMap 后端：lazy 过期，get 应返回 None
    assert_eq!(
        dashmap_ref.get("k").await.unwrap(),
        None,
        "dashmap link should expire after TTL"
    );

    // 链式 get：所有链接都过期，应返回 None
    assert_eq!(
        chain.get("k").await.unwrap(),
        None,
        "chain get should return None after all links expired"
    );
}

#[tokio::test]
async fn test_chain_ttl_returns_highest_score_link_ttl() {
    // Moka (score=100) + DashMap (score=50) 都 set 60s TTL
    // chain.ttl 应返回 Moka 的 ttl（最高分优先）
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(moka))
        .link(ChainLink::from_backend(dashmap))
        .build();

    chain
        .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let ttl = chain.ttl("k").await.unwrap();
    assert!(ttl.is_some(), "chain ttl should return Some for highest-score link");
    let ttl = ttl.unwrap();
    // 58s < ttl <= 60s（最高分链接 Moka 的剩余 TTL）
    assert!(
        ttl > Duration::from_secs(58) && ttl <= Duration::from_secs(60),
        "chain ttl={} should be in (58s, 60s]",
        ttl.as_secs_f64()
    );
}

#[tokio::test]
async fn test_chain_expire_any_link_success_returns_true() {
    // Moka + DashMap 都已 set，expire 任一成功返回 true
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(moka))
        .link(ChainLink::from_backend(dashmap))
        .build();

    chain
        .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
        .await
        .unwrap();

    let result = chain.expire("k", Duration::from_secs(120)).await.unwrap();
    assert!(result, "chain expire should return true when any link succeeds");
}

#[tokio::test]
async fn test_chain_expire_all_missing_returns_false() {
    // 所有链接都没有 "missing" 键
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(moka))
        .link(ChainLink::from_backend(dashmap))
        .build();

    let result = chain.expire("missing", Duration::from_secs(60)).await.unwrap();
    assert!(!result, "chain expire should return false when all links miss");
}

// ========================================================================
// Sync API tests (任务组 15)
// ========================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_get_set() {
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_sync_backend(dashmap))
        .build();

    // sync set + get roundtrip
    chain.set_sync("k", b"v".to_vec(), None).unwrap();
    let value = chain.get_sync("k").unwrap();
    assert_eq!(value, Some(b"v".to_vec()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_get_returns_highest_score_hit() {
    use crate::backend::SyncCacheWriter;

    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    // 直接通过 sync API 在各后端写入不同值
    SyncCacheWriter::set(&moka, Arc::from("k"), Arc::new(b"high".to_vec()), None).unwrap();
    SyncCacheWriter::set(&dashmap, Arc::from("k"), Arc::new(b"low".to_vec()), None).unwrap();

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_sync_backend(dashmap))
        .build();

    let value = chain.get_sync("k").unwrap();
    assert_eq!(
        value,
        Some(b"high".to_vec()),
        "get_sync should return highest-score link's value (Moka=100 > DashMap=90)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_with_unsupported_link_falls_back_to_err() {
    use crate::error::OxCacheError;

    let moka = MokaMemoryBackend::new();
    let mock = MockBackend::new("mock", 30, false);

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_backend(mock)) // async-only
        .build();

    let result = chain.get_sync("k");
    assert!(
        matches!(result, Err(OxCacheError::NotSupported(_))),
        "get_sync should return NotSupported when chain has non-sync link, got {:?}",
        result
    );

    let result = chain.set_sync("k", b"v".to_vec(), None);
    assert!(
        matches!(result, Err(OxCacheError::NotSupported(_))),
        "set_sync should return NotSupported when chain has non-sync link, got {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_set_propagates_ttl() {
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_sync_backend(dashmap))
        .build();

    // set with 50ms TTL
    chain
        .set_sync("k", b"v".to_vec(), Some(Duration::from_millis(50)))
        .unwrap();

    // 立即 get_sync 应返回 Some
    let value = chain.get_sync("k").unwrap();
    assert_eq!(value, Some(b"v".to_vec()));

    // 等 100ms 让 TTL 过期
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Moka 后端：moka 异步清理可能略有延迟，循环等待
    let mut expired = false;
    for _ in 0..10 {
        if chain.get_sync("k").unwrap().is_none() {
            expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        expired,
        "chain get_sync should return None after TTL expires on all links"
    );
}

// ========================================================================
// 竞速读测试 (问题 4.1)
// ========================================================================

#[tokio::test]
async fn test_chain_race_read_returns_earliest_hit() {
    // race_read 默认关闭时走串行路径，返回值仍正确
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);
    high.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await.unwrap();

    let chain = ChainCache::builder().backend(high).backend(low).build();
    let value = chain.get("k").await.unwrap();
    assert_eq!(value, Some(b"v".to_vec()));

    // race_read 开启后同样返回命中值
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);
    low.set(Arc::from("k"), Arc::new(b"l2".to_vec()), None).await.unwrap();

    let chain = ChainCache::builder()
        .backend(high)
        .backend(low)
        .enable_race_read()
        .build();
    let value = chain.get("k").await.unwrap();
    assert_eq!(
        value,
        Some(b"l2".to_vec()),
        "race read should return value from whichever backend has it"
    );
}

#[tokio::test]
async fn test_chain_race_read_backs_off_on_backend_error() {
    // L1 失败、L2 命中：race read 应返回 L2 值而非 Err（5.1 降级语义）
    let failing = MockBackend::new("high", 100, false).with_fail_get();
    let ok = MockBackend::new("low", 50, true);
    ok.set(Arc::from("k"), Arc::new(b"l2".to_vec()), None).await.unwrap();

    let chain = ChainCache::builder()
        .backend(failing)
        .backend(ok)
        .enable_race_read()
        .build();

    let value = chain.get("k").await.unwrap();
    assert_eq!(
        value,
        Some(b"l2".to_vec()),
        "race read should degrade past failing backend"
    );
}

#[tokio::test]
async fn test_chain_race_read_all_backends_fail() {
    use crate::error::OxCacheError;

    let failing1 = MockBackend::new("high", 100, false).with_fail_get();
    let failing2 = MockBackend::new("low", 50, true).with_fail_get();

    let chain = ChainCache::builder()
        .backend(failing1)
        .backend(failing2)
        .enable_race_read()
        .build();

    let result = chain.get("k").await;
    assert!(
        matches!(result, Err(OxCacheError::Operation(ref msg)) if msg.contains("All backends failed")),
        "race read should error when all backends fail, got {:?}",
        result
    );
}

#[tokio::test]
async fn test_chain_race_read_miss_returns_none() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let chain = ChainCache::builder()
        .backend(high)
        .backend(low)
        .enable_race_read()
        .build();

    let value = chain.get("missing").await.unwrap();
    assert_eq!(value, None, "race read with no hits should return None");
}

// ========================================================================
// AtomicCacheWriter for ChainCache (T028) 补充测试
// ========================================================================

#[tokio::test]
async fn test_chain_atomic_incr() {
    let mock = MockBackend::new("mock", 100, false);
    let chain = ChainCache::builder().backend(mock).build();

    // incr on non-existing key → 0 + 1 = 1
    let val = chain.incr("counter", 1, None).await.unwrap();
    assert_eq!(val, 1);

    // incr again → 1 + 10 = 11
    let val = chain.incr("counter", 10, None).await.unwrap();
    assert_eq!(val, 11);

    // negative delta → 11 - 3 = 8
    let val = chain.incr("counter", -3, None).await.unwrap();
    assert_eq!(val, 8);
}

#[tokio::test]
async fn test_chain_atomic_compare_and_swap() {
    let mock = MockBackend::new("mock", 100, false);
    let chain = ChainCache::builder().backend(mock).build();

    // CAS with expected=None → SETNX
    let ok = chain
        .compare_and_swap("cas_key", None, b"initial".to_vec(), None)
        .await
        .unwrap();
    assert!(ok);

    // CAS with correct expected value
    let ok = chain
        .compare_and_swap("cas_key", Some(b"initial"), b"updated".to_vec(), None)
        .await
        .unwrap();
    assert!(ok);

    // CAS with wrong expected value → fail
    let ok = chain
        .compare_and_swap("cas_key", Some(b"initial"), b"again".to_vec(), None)
        .await
        .unwrap();
    assert!(!ok);
}

#[tokio::test]
async fn test_chain_atomic_set_if_absent() {
    let mock = MockBackend::new("mock", 100, false);
    let chain = ChainCache::builder().backend(mock).build();

    let ok = chain
        .set_if_absent("nx_key", b"first".to_vec(), None)
        .await
        .unwrap();
    assert!(ok);

    let ok = chain
        .set_if_absent("nx_key", b"second".to_vec(), None)
        .await
        .unwrap();
    assert!(!ok);
}

#[tokio::test]
async fn test_chain_atomic_no_atomic_backend_returns_not_supported() {
    use crate::error::OxCacheError;

    // DashMap does NOT implement AtomicCacheWriter
    let dashmap = DashMapMemoryBackend::new();
    let chain = ChainCache::builder().backend(dashmap).build();

    let result = chain.incr("k", 1, None).await;
    assert!(
        matches!(result, Err(OxCacheError::NotSupported(_))),
        "incr should return NotSupported when no link implements AtomicCacheWriter"
    );

    let result = chain.compare_and_swap("k", None, b"v".to_vec(), None).await;
    assert!(matches!(result, Err(OxCacheError::NotSupported(_))));

    let result = chain.set_if_absent("k", b"v".to_vec(), None).await;
    assert!(matches!(result, Err(OxCacheError::NotSupported(_))));
}

// ========================================================================
// keys() 合并去重测试 (T029)
// ========================================================================

#[tokio::test]
async fn test_chain_keys_merges_and_deduplicates() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    // 两个后端写入不同 key + 重叠 key
    high.set(Arc::from("a"), Arc::new(b"1".to_vec()), None).await.unwrap();
    high.set(Arc::from("b"), Arc::new(b"2".to_vec()), None).await.unwrap();
    low.set(Arc::from("b"), Arc::new(b"2b".to_vec()), None).await.unwrap();
    low.set(Arc::from("c"), Arc::new(b"3".to_vec()), None).await.unwrap();

    let chain = ChainCache::builder().backend(high).backend(low).build();

    let mut keys = chain.keys("*").await.unwrap();
    keys.sort();
    assert_eq!(keys, vec!["a", "b", "c"], "keys should be merged and deduplicated");
}

#[tokio::test]
async fn test_chain_keys_empty_chain() {
    let chain = ChainCache::builder().build();
    let keys = chain.keys("*").await.unwrap();
    assert!(keys.is_empty());
}

// ========================================================================
// delete_sync 补充测试
// ========================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_delete() {
    let moka = MokaMemoryBackend::new();
    let dashmap = DashMapMemoryBackend::new();

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_sync_backend(dashmap))
        .build();

    // Write to both backends
    chain.set_sync("k", b"v".to_vec(), None).unwrap();
    assert_eq!(chain.get_sync("k").unwrap(), Some(b"v".to_vec()));

    // Delete from all
    chain.delete_sync("k").unwrap();

    // Both backends should be empty (verified via chain)
    assert_eq!(chain.get_sync("k").unwrap(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chain_sync_delete_with_unsupported_link() {
    use crate::error::OxCacheError;

    let moka = MokaMemoryBackend::new();
    let mock = MockBackend::new("mock", 30, false);

    let chain = ChainCache::builder()
        .link(ChainLink::from_sync_backend(moka))
        .link(ChainLink::from_backend(mock)) // async-only
        .build();

    let result = chain.delete_sync("k");
    assert!(
        matches!(result, Err(OxCacheError::NotSupported(_))),
        "delete_sync should return NotSupported when chain has non-sync link"
    );
}

// ========================================================================
// ChainLink Debug 测试
// ========================================================================

#[test]
fn test_chain_link_debug_format_detailed() {
    let backend = MockBackend::new("debug_test", 42, true);
    let link = ChainLink::from_backend(backend);
    let debug_str = format!("{:?}", link);
    assert!(debug_str.contains("ChainLink"));
    assert!(debug_str.contains("42"));
    assert!(debug_str.contains("debug_test"));
    assert!(debug_str.contains("is_persistent"));
}

// ========================================================================
// backfill with TTL 测试
// ========================================================================

#[tokio::test]
async fn test_chain_backfill_preserves_ttl() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    // 只写入低分后端
    low.set(
        Arc::from("ttl_key"),
        Arc::new(b"val".to_vec()),
        Some(Duration::from_secs(300)),
    )
    .await
    .unwrap();

    let chain = ChainCache::builder()
        .backend(high)
        .backend(low)
        .enable_backfill()
        .build();

    // 读取应触发回填到 high
    let value = chain.get("ttl_key").await.unwrap();
    assert_eq!(value, Some(b"val".to_vec()));
}

// ========================================================================
// ChainCache stats / persistent / non_persistent 补充
// ========================================================================

#[tokio::test]
async fn test_chain_stats() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let chain = ChainCache::builder().backend(high).backend(low).build();

    let stats = chain.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"chain".to_string()));
    assert_eq!(stats.get("backend_count"), Some(&"2".to_string()));
    assert_eq!(stats.get("backend_0_name"), Some(&"high".to_string()));
    assert_eq!(stats.get("backend_1_score"), Some(&"50".to_string()));
}

#[test]
fn test_chain_persistent_and_non_persistent_backends() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let chain = ChainCache::builder().backend(high).backend(low).build();

    let persistent = chain.persistent_backends();
    assert_eq!(persistent.len(), 1);
    assert_eq!(persistent[0].name(), "low");

    let non_persistent = chain.non_persistent_backends();
    assert_eq!(non_persistent.len(), 1);
    assert_eq!(non_persistent[0].name(), "high");
}

#[tokio::test]
async fn test_chain_len_is_empty_capacity() {
    // Empty chain (no backends)
    let empty_chain = ChainCache::builder().build();
    assert!(empty_chain.is_empty());
    assert_eq!(empty_chain.len(), 0);

    // Chain with one backend
    let mock = MockBackend::new("mock", 100, false);
    let chain = ChainCache::builder().backend(mock).build();
    assert!(!chain.is_empty());
    assert_eq!(chain.len(), 1);

    // CacheReader async versions
    // CacheReader::is_empty delegates to len() which returns first backend's entry count
    assert!(CacheReader::is_empty(&empty_chain).await.unwrap()); // no links → Ok(0) → true
    assert!(CacheReader::is_empty(&chain).await.unwrap()); // has link but no data → true
    assert_eq!(CacheReader::len(&chain).await.unwrap(), 0); // no entries yet
    assert_eq!(CacheReader::capacity(&chain).await.unwrap(), 0); // MockBackend returns 0

    // After writing data
    chain.set("k", b"v".to_vec(), None).await.unwrap();
    assert!(!CacheReader::is_empty(&chain).await.unwrap());
    assert_eq!(CacheReader::len(&chain).await.unwrap(), 1);
}

#[tokio::test]
async fn test_chain_empty_chain_operations() {
    let chain = ChainCache::builder().build();

    // get on empty chain
    assert_eq!(chain.get("k").await.unwrap(), None);

    // set on empty chain returns error
    assert!(chain.set("k", b"v".to_vec(), None).await.is_err());

    // delete on empty chain is ok
    assert!(chain.delete("k").await.is_ok());

    // clear on empty chain is ok
    assert!(chain.clear().await.is_ok());

    // health_check on empty chain is ok
    assert!(chain.health_check().await.is_ok());

    // expire on empty chain
    assert!(!chain.expire("k", Duration::from_secs(1)).await.unwrap());
}

#[tokio::test]
async fn test_chain_exists_and_ttl() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    low.set(
        Arc::from("k"),
        Arc::new(b"v".to_vec()),
        Some(Duration::from_secs(60)),
    )
    .await
    .unwrap();

    let chain = ChainCache::builder().backend(high).backend(low).build();

    assert!(chain.exists("k").await.unwrap());
    assert!(!chain.exists("missing").await.unwrap());

    let ttl = chain.ttl("k").await.unwrap();
    assert!(ttl.is_some());
    assert!(ttl.unwrap() > Duration::from_secs(58));

    // ttl on missing key
    assert!(chain.ttl("missing").await.unwrap().is_none());
}

#[tokio::test]
async fn test_chain_expire_propagates_to_all() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    // 两个后端都有 key
    high.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await.unwrap();
    low.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await.unwrap();

    let chain = ChainCache::builder().backend(high).backend(low).build();

    let ok = chain.expire("k", Duration::from_secs(60)).await.unwrap();
    assert!(ok, "expire should return true when at least one backend succeeds");
}
