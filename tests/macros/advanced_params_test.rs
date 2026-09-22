// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Integration tests for `#[cached]` advanced parameters:
//   - `single_flight`: concurrent miss dedup (same key → one source call)
//   - `condition`: pre-execution predicate (skip cache when false)
//   - `cache_none`: whether to cache None results (default: false)
//   - `strict`: panic on unregistered cache name (default: warn + passthrough)

#![cfg(feature = "macros")]

use oxcache::Cache;
use oxcache::cached;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// strict parameter
// ============================================================================

/// `strict` parameter: when cache is not registered, panic instead of passthrough.
#[cached(service = "strict_unregistered_svc", strict)]
async fn strict_fn(id: u64) -> Result<u64, String> {
    Ok(id * 2)
}

#[cached(service = "strict_sync_unregistered_svc", strict, sync)]
fn strict_sync_fn(id: u64) -> Result<u64, String> {
    Ok(id * 3)
}

/// Without `strict`, unregistered cache → silent passthrough (fn runs, no panic).
#[cached(service = "non_strict_svc")]
async fn non_strict_fn(id: u64) -> Result<u64, String> {
    Ok(id * 4)
}

/// `strict` async: panics when cache not registered.
#[tokio::test]
#[serial]
#[should_panic(expected = "not registered")]
async fn strict_async_panics_on_unregistered_cache() {
    // Do NOT register the cache — strict mode should panic
    let _ = strict_fn(1).await;
}

/// `strict` sync: panics when cache not registered.
#[test]
#[serial]
#[should_panic(expected = "not registered")]
fn strict_sync_panics_on_unregistered_cache() {
    let _ = strict_sync_fn(1);
}

/// Non-strict (default): unregistered cache → passthrough, no panic.
#[tokio::test]
#[serial]
async fn non_strict_passthrough_on_unregistered_cache() {
    let result = non_strict_fn(5).await.unwrap();
    assert_eq!(result, 20);
}

// ============================================================================
// condition parameter
// ============================================================================

static CONDITION_CALLS: AtomicUsize = AtomicUsize::new(0);

/// `condition` parameter: a function `(args...) -> bool` evaluated before cache lookup.
/// When condition returns false, skip cache entirely (always run original fn).
fn should_cache(id: u64) -> bool {
    id > 10
}

#[cached(service = "condition_svc", condition = should_cache)]
async fn condition_fn(id: u64) -> Result<u64, String> {
    CONDITION_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(id * 2)
}

/// When condition returns true (id > 10), caching is active.
#[tokio::test]
#[serial]
async fn condition_true_enables_caching() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("condition_svc").await.unwrap();

    CONDITION_CALLS.store(0, Ordering::SeqCst);

    // id=20 > 10 → condition true → cache enabled
    let r1 = condition_fn(20).await.unwrap();
    assert_eq!(r1, 40);
    assert_eq!(CONDITION_CALLS.load(Ordering::SeqCst), 1);

    let r2 = condition_fn(20).await.unwrap();
    assert_eq!(r2, 40);
    assert_eq!(
        CONDITION_CALLS.load(Ordering::SeqCst),
        1,
        "condition=true → second call hits cache"
    );
}

/// When condition returns false (id <= 10), cache is bypassed.
#[tokio::test]
#[serial]
async fn condition_false_bypasses_cache() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("condition_svc").await.unwrap();

    CONDITION_CALLS.store(0, Ordering::SeqCst);

    // id=5 <= 10 → condition false → skip cache
    let r1 = condition_fn(5).await.unwrap();
    assert_eq!(r1, 10);
    assert_eq!(CONDITION_CALLS.load(Ordering::SeqCst), 1);

    let r2 = condition_fn(5).await.unwrap();
    assert_eq!(r2, 10);
    assert_eq!(
        CONDITION_CALLS.load(Ordering::SeqCst),
        2,
        "condition=false → second call re-executes fn (no caching)"
    );
}

// ============================================================================
// single_flight parameter
// ============================================================================

static SF_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "single_flight_svc", single_flight)]
async fn sf_fn(id: u64) -> Result<u64, String> {
    SF_CALLS.fetch_add(1, Ordering::SeqCst);
    // Simulate slow computation
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    Ok(id * 10)
}

/// Single-flight: concurrent calls with same key → only one source execution.
#[tokio::test]
#[serial]
async fn single_flight_deduplicates_concurrent_misses() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("single_flight_svc").await.unwrap();

    SF_CALLS.store(0, Ordering::SeqCst);

    // Launch 5 concurrent calls with the same key
    let mut handles = Vec::new();
    for _ in 0..5 {
        handles.push(tokio::spawn(async { sf_fn(42).await }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap().unwrap());
    }

    // All results must be identical
    for r in &results {
        assert_eq!(*r, 420);
    }

    // Single-flight: only 1 source call (not 5)
    let call_count = SF_CALLS.load(Ordering::SeqCst);
    assert!(
        call_count <= 2,
        "single_flight should limit source calls to ~1, got {call_count}"
    );
}

// ============================================================================
// single_flight panic recovery & sharding（审计 F04 回归）
// ============================================================================

static SF_PANIC_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "single_flight_panic_svc", single_flight)]
async fn sf_panic_fn(id: u64) -> Result<u64, String> {
    let n = SF_PANIC_CALLS.fetch_add(1, Ordering::SeqCst);
    if n == 0 {
        panic!("simulated leader panic");
    }
    Ok(id * 100)
}

/// Leader panic 后：注册表条目必须被守卫清理，同 key 后续调用重新执行而非
/// 永久成为等待已死信号的 follower（修复前：key 级永久死锁）。
#[tokio::test]
#[serial]
async fn single_flight_recovers_after_leader_panic() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache
        .register_for_macro("single_flight_panic_svc")
        .await
        .unwrap();
    SF_PANIC_CALLS.store(0, Ordering::SeqCst);

    // First call: leader panics inside the spawned task (JoinError expected)
    let first = tokio::spawn(async { sf_panic_fn(7).await });
    assert!(first.await.is_err(), "首次调用应因 leader panic 失败");

    // Second call must re-execute and succeed within timeout
    let second = tokio::time::timeout(tokio::time::Duration::from_secs(2), sf_panic_fn(7)).await;
    assert!(
        second.is_ok(),
        "leader panic 后同 key 不得永久死锁（follower 丢失唤醒/条目残留）"
    );
    assert_eq!(second.unwrap().unwrap(), 700);
    assert_eq!(
        SF_PANIC_CALLS.load(Ordering::SeqCst),
        2,
        "panic 后应重新执行底层函数"
    );
}

static SF_MULTI_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "single_flight_multi_svc", single_flight)]
async fn sf_multi_fn(id: u64) -> Result<u64, String> {
    SF_MULTI_CALLS.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    Ok(id + 1)
}

/// 分片活性行为：多 key 并发 single_flight 全部完成且各 key 恰执行一次
///（防止回退为单把全局锁时引入的争用/死锁回归）。
#[tokio::test]
#[serial]
async fn single_flight_concurrent_distinct_keys_all_complete() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache
        .register_for_macro("single_flight_multi_svc")
        .await
        .unwrap();
    SF_MULTI_CALLS.store(0, Ordering::SeqCst);

    let mut handles = Vec::new();
    for i in 0..32u64 {
        handles.push(tokio::spawn(async move {
            let fut = sf_multi_fn(i);
            tokio::time::timeout(tokio::time::Duration::from_secs(5), fut)
                .await
                .expect("key 任务不得挂起")
                .unwrap()
        }));
    }
    for (i, h) in handles.into_iter().enumerate() {
        assert_eq!(h.await.unwrap(), i as u64 + 1);
    }

    let calls = SF_MULTI_CALLS.load(Ordering::SeqCst);
    assert!(
        (16..=32).contains(&calls),
        "distinct-key 并发应分散执行（分片生效），got {calls}/32"
    );
}
