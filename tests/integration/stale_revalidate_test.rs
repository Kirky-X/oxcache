// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Integration tests for SWR three-state expiry + StalePolicy wiring
//
// Verifies, at the `Cache` API level:
//   1. `Return` policy: stale hit serves the old value, fallback not run
//   2. `Revalidate` policy: stale hit re-sources synchronously via single-flight
//   3. `OffloadRevalidate` policy: immediate old value + deduplicated background
//      refresh via `get_or_refresh`
//   4. No stale config → zero behavior change

#![cfg(feature = "stale")]

use oxcache::Cache;
use oxcache::features::stale::StalePolicy;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const TTL: Duration = Duration::from_millis(50);
const STALE: Duration = Duration::from_millis(10_000);

/// 等待 key 进入 stale 窗口（TTL 50ms < 等待 120ms < stale 10s）
async fn wait_until_stale() {
    tokio::time::sleep(Duration::from_millis(120)).await;
}

// ============================================================================
// Return 策略
// ============================================================================

static RETURN_CALLS: AtomicUsize = AtomicUsize::new(0);

#[tokio::test]
#[serial]
async fn return_policy_serves_old_value_without_fallback() {
    let cache: Cache<String, String> = Cache::builder()
        .stale_ttl(STALE)
        .stale_policy(StalePolicy::Return)
        .build()
        .await
        .unwrap();

    cache
        .set_with_ttl(&"k".to_string(), &"old".to_string(), Some(TTL))
        .await
        .unwrap();
    wait_until_stale().await;

    RETURN_CALLS.store(0, Ordering::SeqCst);
    let value = cache
        .get_or(&"k".to_string(), || async {
            RETURN_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok::<_, oxcache::OxCacheError>("new".to_string())
        })
        .await
        .unwrap();
    assert_eq!(value, "old", "Return policy must serve the stale value");
    assert_eq!(
        RETURN_CALLS.load(Ordering::SeqCst),
        0,
        "Return policy must not execute the fallback"
    );

    // 裸 get 一律 Return 语义
    let plain = cache.get(&"k".to_string()).await.unwrap();
    assert_eq!(plain, Some("old".to_string()));
}

// ============================================================================
// Revalidate 策略
// ============================================================================

static REVAL_CALLS: AtomicUsize = AtomicUsize::new(0);

#[tokio::test]
#[serial]
async fn revalidate_policy_refreshes_synchronously() {
    let cache: Cache<String, String> = Cache::builder()
        .stale_ttl(STALE)
        .stale_policy(StalePolicy::Revalidate)
        .build()
        .await
        .unwrap();

    cache
        .set_with_ttl(&"k".to_string(), &"old".to_string(), Some(TTL))
        .await
        .unwrap();
    wait_until_stale().await;

    REVAL_CALLS.store(0, Ordering::SeqCst);
    let value = cache
        .get_or(&"k".to_string(), || async {
            REVAL_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok::<_, oxcache::OxCacheError>("new".to_string())
        })
        .await
        .unwrap();
    assert_eq!(value, "new", "Revalidate must return the refreshed value");
    assert_eq!(REVAL_CALLS.load(Ordering::SeqCst), 1);

    // 回写后的新值处于 fresh 窗口，后续 get_or 命中缓存
    let again = cache
        .get_or(&"k".to_string(), || async {
            REVAL_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok::<_, oxcache::OxCacheError>("newer".to_string())
        })
        .await
        .unwrap();
    assert_eq!(again, "new");
    assert_eq!(
        REVAL_CALLS.load(Ordering::SeqCst),
        1,
        "fresh hit must not re-execute"
    );
}

// ============================================================================
// OffloadRevalidate 策略（get_or_refresh）
// ============================================================================

static OFFLOAD_CALLS: AtomicUsize = AtomicUsize::new(0);

#[tokio::test]
#[serial]
async fn offload_policy_returns_old_then_background_refreshes() {
    let cache: Cache<String, String> = Cache::builder()
        .stale_ttl(STALE)
        .stale_policy(StalePolicy::OffloadRevalidate)
        .build()
        .await
        .unwrap();

    cache
        .set_with_ttl(&"k".to_string(), &"old".to_string(), Some(TTL))
        .await
        .unwrap();
    wait_until_stale().await;

    OFFLOAD_CALLS.store(0, Ordering::SeqCst);
    let value = cache
        .get_or_refresh(&"k".to_string(), Some(Duration::from_secs(60)), || async {
            OFFLOAD_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok::<_, oxcache::OxCacheError>("new".to_string())
        })
        .await
        .unwrap();
    assert_eq!(
        value, "old",
        "OffloadRevalidate must return the stale value immediately"
    );

    // 等待后台刷新收敛（fallback 极快时任务可能先于 wait_all 完成）
    let manager = cache.offload_manager().expect("offload manager present");
    let _ = manager.wait_all(Duration::from_secs(2)).await;
    assert!(
        !manager.is_in_flight("k"),
        "background refresh must have completed"
    );
    assert_eq!(
        OFFLOAD_CALLS.load(Ordering::SeqCst),
        1,
        "background refresh must execute the fallback exactly once"
    );
    let refreshed = cache.get(&"k".to_string()).await.unwrap();
    assert_eq!(
        refreshed,
        Some("new".to_string()),
        "refresh must write back"
    );
}

#[tokio::test]
#[serial]
async fn offload_policy_deduplicates_concurrent_refreshes() {
    let cache: Cache<String, String> = Cache::builder()
        .stale_ttl(STALE)
        .stale_policy(StalePolicy::OffloadRevalidate)
        .build()
        .await
        .unwrap();

    cache
        .set_with_ttl(&"k".to_string(), &"old".to_string(), Some(TTL))
        .await
        .unwrap();
    wait_until_stale().await;

    OFFLOAD_CALLS.store(0, Ordering::SeqCst);
    let cache = std::sync::Arc::new(cache);
    let mut handles = Vec::new();
    for _ in 0..5 {
        let cache = cache.clone();
        handles.push(tokio::spawn(async move {
            cache
                .get_or_refresh(&"k".to_string(), Some(Duration::from_secs(60)), || async {
                    OFFLOAD_CALLS.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok::<_, oxcache::OxCacheError>("new".to_string())
                })
                .await
        }));
    }
    for h in handles {
        let v = h.await.unwrap().unwrap();
        assert_eq!(v, "old", "every concurrent caller gets the stale value");
    }

    let manager = cache.offload_manager().unwrap();
    manager.wait_all(Duration::from_secs(2)).await;
    assert!(
        OFFLOAD_CALLS.load(Ordering::SeqCst) <= 2,
        "concurrent stale hits must deduplicate background refreshes, got {}",
        OFFLOAD_CALLS.load(Ordering::SeqCst)
    );
}

// ============================================================================
// 未配置 stale → 行为不变（回归保护）
// ============================================================================

#[tokio::test]
#[serial]
async fn without_stale_config_behavior_unchanged() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
    cache
        .set_with_ttl(&"k".to_string(), &"v".to_string(), Some(TTL))
        .await
        .unwrap();
    wait_until_stale().await;
    // TTL 已过且无 stale 窗口：miss
    assert_eq!(cache.get(&"k".to_string()).await.unwrap(), None);
    assert!(cache.offload_manager().is_none());
}
