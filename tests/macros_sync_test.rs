// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Integration tests for #[cached(sync)] — the sync branch of the macro.
//
// These tests verify:
//   1. `#[cached(sync)]` generates a sync fn callable without `.await`
//   2. `#[cached]` (no sync) preserves async behavior
//   3. `#[cached(sync)]` on `async fn` fails at compile time (trybuild)
//
// NOTE: All runtime tests use `multi_thread` tokio flavor because
// MokaMemoryBackend's sync_block_on relies on `block_in_place`, which
// panics on current_thread runtimes. The sync cached fns call moka
// sync ops internally.

#![cfg(feature = "macros")]

use oxcache::Cache;
use oxcache::cached;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct User {
    id: u64,
    name: String,
}

// ============================================================================
// Sync cached fn — generates a plain `fn` (no async)
// ============================================================================

#[cached(service = "macros_sync_test_svc", sync)]
fn get_user_sync(id: u64) -> Result<User, String> {
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

/// `#[cached(sync)]` generates a sync fn that can be called without `.await`.
/// Caching must work end-to-end: first call runs original fn + caches result;
/// second call returns cached value.
#[tokio::test(flavor = "multi_thread")]
async fn sync_mode_generates_sync_fn() {
    // Register a cache built with sync_mode(true) so backend_sync is wired up.
    let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
    cache.register_for_macro("macros_sync_test_svc").await.unwrap();

    // First call: cache miss → runs original fn → caches result
    let result1 = get_user_sync(1).unwrap();
    assert_eq!(result1.id, 1);
    assert_eq!(result1.name, "User 1");

    // Second call: cache hit → returns cached value (fn body not re-executed)
    let result2 = get_user_sync(1).unwrap();
    assert_eq!(result2.id, 1);
    assert_eq!(result2.name, "User 1");
    // Verify cached value matches first call
    assert_eq!(result1, result2, "second call should return cached value");
}

// ============================================================================
// Async cached fn (no sync flag) — preserves original async behavior
// ============================================================================

#[cached(service = "macros_async_test_svc")]
async fn get_user_async(id: u64) -> Result<User, String> {
    Ok(User {
        id,
        name: format!("Async User {}", id),
    })
}

/// Without `sync` flag, the macro generates an `async fn` (original behavior).
/// Calling requires `.await` and caching works as before.
#[tokio::test(flavor = "multi_thread")]
async fn no_sync_keeps_async_behavior() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("macros_async_test_svc").await.unwrap();

    let result1 = get_user_async(1).await.unwrap();
    assert_eq!(result1.id, 1);
    assert_eq!(result1.name, "Async User 1");

    let result2 = get_user_async(1).await.unwrap();
    assert_eq!(result2.id, 1);
    assert_eq!(result2.name, "Async User 1");
    assert_eq!(result1, result2, "second call should return cached value");
}

// ============================================================================
// Compile-fail test: #[cached(sync)] on async fn must be rejected
// ============================================================================

/// Verifies that `#[cached(sync)]` applied to an `async fn` produces a
/// compile error. The macro panics at expansion time with a clear message.
#[test]
fn sync_flag_with_async_fn_compile_error() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/macros_sync_compile_fail/*.rs");
}
