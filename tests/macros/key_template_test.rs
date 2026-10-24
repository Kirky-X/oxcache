// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Integration tests for the `#[cached(key = "...")]` template:
//   - the template may reference parameters and `self` field paths;
//   - on a method, the template must carry the ownership dimension
//     (otherwise all tenants share one cache entry);
//   - placeholder values are segment-escaped (no forged segment boundary).

#![cfg(feature = "macros")]

use oxcache::Cache;
use oxcache::cached;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

static CALLS: AtomicUsize = AtomicUsize::new(0);

struct Tenant {
    tenant_id: String,
}

impl Tenant {
    /// The key template carries the ownership dimension: different tenants
    /// must not share one cache entry.
    #[cached(service = "tenant_key_svc", key = "user_{self.tenant_id}_{id}")]
    async fn profile(&self, id: u64) -> Result<u64, String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(id * 2)
    }
}

#[cached(service = "escape_key_svc", key = "user_{id}")]
async fn escaped_fn(id: &str) -> Result<String, String> {
    CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(id.to_string())
}

/// 自由函数上的常量键模板仍然可用（无租户维度，无需归属维度）
#[cached(service = "constant_key_svc", key = "global_profile")]
async fn constant_key_fn() -> Result<u64, String> {
    CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(7)
}

#[tokio::test]
#[serial]
async fn constant_key_template_on_free_function_works() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("constant_key_svc").await.unwrap();

    CALLS.store(0, Ordering::SeqCst);
    assert_eq!(constant_key_fn().await.unwrap(), 7);
    assert_eq!(constant_key_fn().await.unwrap(), 7);
    assert_eq!(
        CALLS.load(Ordering::SeqCst),
        1,
        "second call must hit the cache"
    );
}

/// Tenants must not share a cache entry: the template references
/// `self.tenant_id`, so each tenant gets its own key.
#[tokio::test]
#[serial]
async fn key_template_separates_tenants() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("tenant_key_svc").await.unwrap();

    CALLS.store(0, Ordering::SeqCst);
    let tenant_a = Tenant {
        tenant_id: "t1".into(),
    };
    let tenant_b = Tenant {
        tenant_id: "t2".into(),
    };

    assert_eq!(tenant_a.profile(1).await.unwrap(), 2);
    assert_eq!(tenant_b.profile(1).await.unwrap(), 2);
    assert_eq!(
        CALLS.load(Ordering::SeqCst),
        2,
        "different tenants must not hit the same cache entry"
    );

    // Same tenant → cache hit (no extra source call)
    assert_eq!(tenant_a.profile(1).await.unwrap(), 2);
    assert_eq!(CALLS.load(Ordering::SeqCst), 2);
}

/// Placeholder values are segment-escaped: a value carrying the segment
/// separator must not forge a segment boundary in the key.
#[tokio::test]
#[serial]
async fn key_template_escapes_placeholder_value() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("escape_key_svc").await.unwrap();

    let _ = escaped_fn("a:b").await.unwrap();
    let keys = cache.keys("user*").await.unwrap();
    assert_eq!(keys.len(), 1, "one entry expected: {keys:?}");
    assert!(
        keys[0].contains("%3A"),
        "segment separator must be escaped in the key: {keys:?}"
    );
}
