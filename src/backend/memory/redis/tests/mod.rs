// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Split test modules for Redis backend.

pub(crate) mod builder_tests;
pub(crate) mod error_tests;
#[cfg(feature = "lua")]
pub(crate) mod lua_tests;
pub(crate) mod pipeline_tests;
pub(crate) mod reader_tests;
pub(crate) mod sync_tests;
pub(crate) mod writer_tests;

use super::client::RedisBackend;
use crate::backend::{CacheConnector, CacheWriter};
use std::sync::atomic::{AtomicU64, Ordering};

/// Redis test connection URL (Docker Redis)
pub(crate) const REDIS_URL: &str = "redis://127.0.0.1:6379";
/// Separate DB for clear tests to avoid interfering with parallel tests
pub(crate) const REDIS_URL_DB1: &str = "redis://127.0.0.1:6379/1";
/// Test key prefix to avoid conflicts
pub(crate) const KEY_PREFIX: &str = "test_client:";

/// Global unique ID generator for test keys
static UID: AtomicU64 = AtomicU64::new(0);

/// Generate a unique test key
pub(crate) fn unique_key(suffix: &str) -> String {
    let id = UID.fetch_add(1, Ordering::SeqCst);
    format!("{}{}_{}", KEY_PREFIX, id, suffix)
}

/// Set `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`.
///
/// Note: Tests that remove this env var use `#[serial]` to avoid races.
/// Setting the same value from multiple threads is safe (idempotent).
/// nosem: rust.lang.security.unsafe-usage.unsafe-usage
pub(crate) fn set_allow_insecure_env() {
    set_insecure_env("I_UNDERSTAND_THE_RISKS");
}

/// Set `OXCACHE_ALLOW_INSECURE_REDIS=<value>` (parameterized).
///
/// Note: Tests that mutate this env var use `#[serial]` where needed.
pub(crate) fn set_insecure_env(value: &str) {
    // SAFETY: Rust 2024 edition — set_var is unsafe; idempotent sets need no serialisation
    unsafe {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", value);
    }
}

/// Remove `OXCACHE_ALLOW_INSECURE_REDIS` env var.
///
/// Note: All callers use `#[serial]` to avoid concurrent remove/set races.
pub(crate) fn remove_allow_insecure_env() {
    // SAFETY: Rust 2024 edition — remove_var is unsafe; callers serialize via #[serial]
    unsafe {
        std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
    }
}

/// Create a RedisBackend for testing (allows insecure connection)
pub(crate) async fn make_backend() -> RedisBackend {
    set_allow_insecure_env();
    RedisBackend::new(REDIS_URL)
        .await
        .unwrap_or_else(|e| panic!("Redis connection failed ({}): {}", REDIS_URL, e))
}

/// Create a RedisBackend connected to a specific URL
#[allow(dead_code)]
pub(crate) async fn make_backend_with_url(url: &str) -> RedisBackend {
    set_allow_insecure_env();
    RedisBackend::new(url)
        .await
        .unwrap_or_else(|e| panic!("Redis connection failed ({}): {}", url, e))
}

/// Clean up a test key
pub(crate) async fn cleanup(backend: &RedisBackend, key: &str) {
    // Intentionally discard errors — test teardown should not fail the test
    #[allow(let_underscore_drop)]
    let _ = backend.delete(key).await;
}
