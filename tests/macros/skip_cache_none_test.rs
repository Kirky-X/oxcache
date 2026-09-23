// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Integration tests for `#[cached]` `skip(...)` and `cache_none`:
//   - `skip(a, b)`: excluded parameters do not participate in the default
//     cache key (same remaining args → same entry)
//   - `cache_none`: absent (`Ok(None)`) results are only cached when the
//     flag is set — default is documented as "do not cache None"

#![cfg(feature = "macros")]

use oxcache::Cache;
use oxcache::cached;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// skip(...) — parameter excluded from the default cache key
// ============================================================================

static SKIP_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "skip_svc", skip(password))]
async fn skip_fn(user: u64, password: String) -> Result<String, String> {
    SKIP_CALLS.fetch_add(1, Ordering::SeqCst);
    let _ = password;
    Ok(format!("user-{user}"))
}

/// Same non-skipped arg + different skipped arg → same cache entry.
#[tokio::test]
#[serial]
async fn skip_excludes_param_from_cache_key() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("skip_svc").await.unwrap();
    SKIP_CALLS.store(0, Ordering::SeqCst);

    let r1 = skip_fn(1, "secret-a".into()).await.unwrap();
    let r2 = skip_fn(1, "secret-b".into()).await.unwrap();
    assert_eq!(r1, "user-1");
    assert_eq!(r2, "user-1");
    assert_eq!(
        SKIP_CALLS.load(Ordering::SeqCst),
        1,
        "same user, different (skipped) password → second call must hit cache"
    );

    // Different non-skipped arg → different entry
    let r3 = skip_fn(2, "secret-a".into()).await.unwrap();
    assert_eq!(r3, "user-2");
    assert_eq!(SKIP_CALLS.load(Ordering::SeqCst), 2);
}

static SKIP_PREFIX_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "skip_prefix_svc", key_prefix = "ns", skip(token))]
async fn skip_prefix_fn(id: u64, token: String) -> Result<String, String> {
    SKIP_PREFIX_CALLS.fetch_add(1, Ordering::SeqCst);
    let _ = token;
    Ok(format!("id-{id}"))
}

/// `skip` also applies on the `key_prefix` default-key path.
#[tokio::test]
#[serial]
async fn skip_applies_with_key_prefix() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("skip_prefix_svc").await.unwrap();
    SKIP_PREFIX_CALLS.store(0, Ordering::SeqCst);

    let _ = skip_prefix_fn(9, "t1".into()).await.unwrap();
    let _ = skip_prefix_fn(9, "t2".into()).await.unwrap();
    assert_eq!(
        SKIP_PREFIX_CALLS.load(Ordering::SeqCst),
        1,
        "skipped token must not split the key_prefix key"
    );
}

static SKIP_SYNC_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "skip_sync_svc", sync, skip(secret))]
fn skip_sync_fn(id: u64, secret: String) -> Result<String, String> {
    SKIP_SYNC_CALLS.fetch_add(1, Ordering::SeqCst);
    let _ = secret;
    Ok(format!("sync-{id}"))
}

/// `skip` on the sync branch behaves identically.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn skip_works_on_sync_branch() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache.register_for_macro("skip_sync_svc").await.unwrap();
    SKIP_SYNC_CALLS.store(0, Ordering::SeqCst);

    let _ = skip_sync_fn(3, "s-a".into()).unwrap();
    let _ = skip_sync_fn(3, "s-b".into()).unwrap();
    assert_eq!(SKIP_SYNC_CALLS.load(Ordering::SeqCst), 1);
}

// ============================================================================
// cache_none — Option-valued results
// ============================================================================

static NONE_OFF_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "none_off_svc")]
async fn none_off_fn() -> Result<Option<u32>, String> {
    NONE_OFF_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(None)
}

/// Default (no `cache_none`): `Ok(None)` is not cached — each call re-executes.
#[tokio::test]
#[serial]
async fn none_not_cached_by_default() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("none_off_svc").await.unwrap();
    NONE_OFF_CALLS.store(0, Ordering::SeqCst);

    let r1 = none_off_fn().await.unwrap();
    let r2 = none_off_fn().await.unwrap();
    assert_eq!(r1, None);
    assert_eq!(r2, None);
    assert_eq!(
        NONE_OFF_CALLS.load(Ordering::SeqCst),
        2,
        "Ok(None) must not be cached without cache_none"
    );
}

static SOME_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "some_svc")]
async fn some_fn() -> Result<Option<u32>, String> {
    SOME_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Some(7))
}

/// `Ok(Some(v))` is cached regardless of `cache_none` (round-trips as Option).
#[tokio::test]
#[serial]
async fn some_value_cached_and_round_trips() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("some_svc").await.unwrap();
    SOME_CALLS.store(0, Ordering::SeqCst);

    let r1 = some_fn().await.unwrap();
    let r2 = some_fn().await.unwrap();
    assert_eq!(r1, Some(7));
    assert_eq!(r2, Some(7));
    assert_eq!(SOME_CALLS.load(Ordering::SeqCst), 1);
}

static NONE_ON_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "none_on_svc", cache_none)]
async fn none_on_fn() -> Result<Option<u32>, String> {
    NONE_ON_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(None)
}

/// `cache_none`: `Ok(None)` is cached (serialized `null`) and restored as
/// `Ok(None)` on the cache-hit path.
#[tokio::test]
#[serial]
async fn none_cached_with_cache_none_flag() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("none_on_svc").await.unwrap();
    NONE_ON_CALLS.store(0, Ordering::SeqCst);

    let r1 = none_on_fn().await.unwrap();
    let r2 = none_on_fn().await.unwrap();
    assert_eq!(r1, None);
    assert_eq!(r2, None);
    assert_eq!(
        NONE_ON_CALLS.load(Ordering::SeqCst),
        1,
        "cache_none → Ok(None) must be cached"
    );
}

static NONE_SF_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "none_sf_svc", single_flight)]
async fn none_sf_fn() -> Result<Option<u32>, String> {
    NONE_SF_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(None)
}

/// Single-flight leader obeys the same filter: leader's `Ok(None)` is not
/// written, so the follower's re-check misses and runs locally.
#[tokio::test]
#[serial]
async fn single_flight_leader_does_not_cache_none() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("none_sf_svc").await.unwrap();
    NONE_SF_CALLS.store(0, Ordering::SeqCst);

    let _ = none_sf_fn().await.unwrap();
    let _ = none_sf_fn().await.unwrap();
    assert_eq!(
        NONE_SF_CALLS.load(Ordering::SeqCst),
        2,
        "leader None write-back must be filtered; second call re-executes"
    );
}

static NONE_ON_SYNC_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "none_on_sync_svc", sync, cache_none)]
fn none_on_sync_fn() -> Result<Option<u32>, String> {
    NONE_ON_SYNC_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(None)
}

/// `cache_none` on the sync branch caches `Ok(None)`.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn none_cached_on_sync_branch_with_flag() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache.register_for_macro("none_on_sync_svc").await.unwrap();
    NONE_ON_SYNC_CALLS.store(0, Ordering::SeqCst);

    let _ = none_on_sync_fn().unwrap();
    let _ = none_on_sync_fn().unwrap();
    assert_eq!(NONE_ON_SYNC_CALLS.load(Ordering::SeqCst), 1);
}

static NONE_OFF_SYNC_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cached(service = "none_off_sync_svc", sync)]
fn none_off_sync_fn() -> Result<Option<u32>, String> {
    NONE_OFF_SYNC_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(None)
}

/// Default sync branch: `Ok(None)` not cached.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn none_not_cached_on_sync_branch_by_default() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache.register_for_macro("none_off_sync_svc").await.unwrap();
    NONE_OFF_SYNC_CALLS.store(0, Ordering::SeqCst);

    let _ = none_off_sync_fn().unwrap();
    let _ = none_off_sync_fn().unwrap();
    assert_eq!(NONE_OFF_SYNC_CALLS.load(Ordering::SeqCst), 2);
}
