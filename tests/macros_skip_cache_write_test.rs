// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Integration tests for `#[cached(skip_cache_write)]` — the T003 extension.
//
// These tests verify:
//   1. `#[cached(skip_cache_write)]` compiles (smoke test)
//   2. With `skip_cache_write` set, an `Ok` result does NOT write to the cache
//      (second call re-executes the fn body — cache miss every time)
//   3. Without `skip_cache_write` (default), an `Ok` result writes to the cache
//      (second call hits the cache — fn body not re-executed)
//   4. Both sync and async branches respect `skip_cache_write`
//
// The counter-based approach distinguishes "cached" from "re-executed":
// each fn body bumps an `AtomicUsize`; if the counter stays at 1 after
// two calls with the same args, the second call hit the cache.

#![cfg(feature = "macros")]

use oxcache::Cache;
use oxcache::cached;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

// Per-fn counters so the two test fns don't interfere.
static ASYNC_SKIP_CALLS: AtomicUsize = AtomicUsize::new(0);
static ASYNC_DEFAULT_CALLS: AtomicUsize = AtomicUsize::new(0);
static SYNC_SKIP_CALLS: AtomicUsize = AtomicUsize::new(0);
static SYNC_DEFAULT_CALLS: AtomicUsize = AtomicUsize::new(0);

// ============================================================================
// Async branch — `#[cached(skip_cache_write)]` vs `#[cached]` (default)
// ============================================================================

#[cached(service = "skip_cache_write_async_skip_svc", skip_cache_write)]
async fn cached_async_skip(id: u64) -> Result<u64, String> {
    ASYNC_SKIP_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(id * 2)
}

#[cached(service = "skip_cache_write_async_default_svc")]
async fn cached_async_default(id: u64) -> Result<u64, String> {
    ASYNC_DEFAULT_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(id * 2)
}

/// `#[cached(skip_cache_write)]` on an async fn must compile (T003 smoke test)
/// and the generated code must still produce the correct return value.
#[tokio::test]
#[serial]
async fn skip_cache_write_async_compiles_and_returns_value() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache
        .register_for_macro("skip_cache_write_async_skip_svc")
        .await
        .unwrap();

    ASYNC_SKIP_CALLS.store(0, Ordering::SeqCst);
    let r = cached_async_skip(21).await.unwrap();
    assert_eq!(r, 42);
}

/// With `skip_cache_write`, an `Ok` result must NOT be written to the cache.
/// Both calls execute the fn body (cache miss every time).
#[tokio::test]
#[serial]
async fn skip_cache_write_async_does_not_cache_ok_result() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache
        .register_for_macro("skip_cache_write_async_skip_svc")
        .await
        .unwrap();

    ASYNC_SKIP_CALLS.store(0, Ordering::SeqCst);

    let r1 = cached_async_skip(7).await.unwrap();
    assert_eq!(r1, 14);
    assert_eq!(
        ASYNC_SKIP_CALLS.load(Ordering::SeqCst),
        1,
        "first call must execute fn body"
    );

    let r2 = cached_async_skip(7).await.unwrap();
    assert_eq!(r2, 14);
    assert_eq!(
        ASYNC_SKIP_CALLS.load(Ordering::SeqCst),
        2,
        "skip_cache_write=true must skip cache write, so second call re-executes fn body"
    );
}

/// Without `skip_cache_write` (default behavior), an `Ok` result MUST be cached.
/// First call executes fn body; second call hits the cache.
#[tokio::test]
#[serial]
async fn default_async_caches_ok_result() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache
        .register_for_macro("skip_cache_write_async_default_svc")
        .await
        .unwrap();

    ASYNC_DEFAULT_CALLS.store(0, Ordering::SeqCst);

    let r1 = cached_async_default(9).await.unwrap();
    assert_eq!(r1, 18);
    assert_eq!(
        ASYNC_DEFAULT_CALLS.load(Ordering::SeqCst),
        1,
        "first call must execute fn body"
    );

    let r2 = cached_async_default(9).await.unwrap();
    assert_eq!(r2, 18);
    assert_eq!(
        ASYNC_DEFAULT_CALLS.load(Ordering::SeqCst),
        1,
        "default behavior must cache Ok, second call hits cache (counter stays at 1)"
    );
}

// ============================================================================
// Sync branch — `#[cached(sync, skip_cache_write)]` vs `#[cached(sync)]` (default)
// ============================================================================

#[cached(service = "skip_cache_write_sync_skip_svc", sync, skip_cache_write)]
fn cached_sync_skip(id: u64) -> Result<u64, String> {
    SYNC_SKIP_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(id * 3)
}

#[cached(service = "skip_cache_write_sync_default_svc", sync)]
fn cached_sync_default(id: u64) -> Result<u64, String> {
    SYNC_DEFAULT_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(id * 3)
}

/// `#[cached(sync, skip_cache_write)]` must compile (sync-branch T003 smoke test).
// Multi-thread flavor required: MokaMemoryBackend sync ops rely on
// `block_in_place`, which panics on current_thread runtimes.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn skip_cache_write_sync_compiles_and_returns_value() {
    let cache: Cache<String, Vec<u8>> = Cache::builder()
        .sync_mode(true)
        .build()
        .await
        .unwrap();
    cache
        .register_for_macro("skip_cache_write_sync_skip_svc")
        .await
        .unwrap();

    SYNC_SKIP_CALLS.store(0, Ordering::SeqCst);
    let r = cached_sync_skip(14);
    assert_eq!(r.unwrap(), 42);
}

/// With `skip_cache_write` on a sync fn, an `Ok` result must NOT be cached.
/// Both calls execute the fn body.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn skip_cache_write_sync_does_not_cache_ok_result() {
    let cache: Cache<String, Vec<u8>> = Cache::builder()
        .sync_mode(true)
        .build()
        .await
        .unwrap();
    cache
        .register_for_macro("skip_cache_write_sync_skip_svc")
        .await
        .unwrap();

    SYNC_SKIP_CALLS.store(0, Ordering::SeqCst);

    let r1 = cached_sync_skip(5).unwrap();
    assert_eq!(r1, 15);
    assert_eq!(
        SYNC_SKIP_CALLS.load(Ordering::SeqCst),
        1,
        "first call must execute fn body"
    );

    let r2 = cached_sync_skip(5).unwrap();
    assert_eq!(r2, 15);
    assert_eq!(
        SYNC_SKIP_CALLS.load(Ordering::SeqCst),
        2,
        "skip_cache_write=true must skip cache write, so second call re-executes fn body"
    );
}

/// Without `skip_cache_write`, a sync `#[cached]` fn must cache `Ok` results.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn default_sync_caches_ok_result() {
    let cache: Cache<String, Vec<u8>> = Cache::builder()
        .sync_mode(true)
        .build()
        .await
        .unwrap();
    cache
        .register_for_macro("skip_cache_write_sync_default_svc")
        .await
        .unwrap();

    SYNC_DEFAULT_CALLS.store(0, Ordering::SeqCst);

    let r1 = cached_sync_default(6).unwrap();
    assert_eq!(r1, 18);
    assert_eq!(
        SYNC_DEFAULT_CALLS.load(Ordering::SeqCst),
        1,
        "first call must execute fn body"
    );

    let r2 = cached_sync_default(6).unwrap();
    assert_eq!(r2, 18);
    assert_eq!(
        SYNC_DEFAULT_CALLS.load(Ordering::SeqCst),
        1,
        "default behavior must cache Ok, second call hits cache (counter stays at 1)"
    );
}
