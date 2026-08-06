// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Penetration guard tests — null sentinel cache via public API.

use oxcache::cache::Cache;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

// ============================================================================
// Null sentinel behavior tests (via get_or_option public API)
// ============================================================================

#[tokio::test]
async fn test_null_sentinel_get_returns_none() {
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_secs(30))
        .build()
        .await
        .unwrap();

    // Cache a null result via get_or_option
    let result = cache
        .get_or_option(&"null-key".to_string(), || async { Ok(None) })
        .await
        .unwrap();
    assert!(result.is_none(), "get_or_option should return None");

    // Subsequent get() should also return None (null sentinel hit)
    let get_result = cache.get(&"null-key".to_string()).await.unwrap();
    assert!(
        get_result.is_none(),
        "get() should return None when null sentinel is cached"
    );
}

#[tokio::test]
async fn test_get_or_option_caches_null_on_none_fallback() {
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_secs(30))
        .build()
        .await
        .unwrap();

    // First call: fallback returns None — should cache null sentinel
    let result = cache
        .get_or_option(&"missing-key".to_string(), || async { Ok(None) })
        .await
        .unwrap();
    assert!(result.is_none(), "get_or_option should return None");

    // Second call: fallback should NOT be called (sentinel hit)
    let call_count = AtomicU32::new(0);
    let call_count_ref = &call_count;
    let result2 = cache
        .get_or_option(&"missing-key".to_string(), || async move {
            call_count_ref.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })
        .await
        .unwrap();
    assert!(result2.is_none());
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "fallback should NOT be called when null sentinel is cached"
    );
}

#[tokio::test]
async fn test_get_or_option_returns_some_on_real_value() {
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_secs(30))
        .build()
        .await
        .unwrap();

    // Fallback returns Some — should cache normally
    let result = cache
        .get_or_option(&"real-key".to_string(), || async { Ok(Some("real-value".to_string())) })
        .await
        .unwrap();
    assert_eq!(result, Some("real-value".to_string()));

    // Subsequent get should return the cached value
    let cached = cache.get(&"real-key".to_string()).await.unwrap();
    assert_eq!(cached, Some("real-value".to_string()));
}

#[tokio::test]
async fn test_get_or_option_no_null_cache_when_disabled() {
    // null_cache_ttl NOT configured — fallback None should NOT write sentinel
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    let result = cache
        .get_or_option(&"no-null-key".to_string(), || async { Ok(None) })
        .await
        .unwrap();
    assert!(result.is_none());

    // Second call: fallback SHOULD be called again (no sentinel cached)
    let call_count = AtomicU32::new(0);
    let call_count_ref = &call_count;
    let _ = cache
        .get_or_option(&"no-null-key".to_string(), || async move {
            call_count_ref.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })
        .await
        .unwrap();
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "fallback should be called again when null_cache_ttl is not configured"
    );
}

#[tokio::test]
async fn test_null_sentinel_expires_after_ttl() {
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_millis(100))
        .build()
        .await
        .unwrap();

    // Cache a null sentinel
    let result = cache
        .get_or_option(&"expiring-null".to_string(), || async { Ok(None) })
        .await
        .unwrap();
    assert!(result.is_none());

    // Immediately: second call should NOT invoke fallback (sentinel active)
    let count1 = AtomicU32::new(0);
    let count1_ref = &count1;
    let _ = cache
        .get_or_option(&"expiring-null".to_string(), || async move {
            count1_ref.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })
        .await
        .unwrap();
    assert_eq!(
        count1.load(Ordering::SeqCst),
        0,
        "sentinel should block fallback immediately"
    );

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(200)).await;

    // After TTL: fallback should be called again (sentinel expired)
    let count2 = AtomicU32::new(0);
    let count2_ref = &count2;
    let _ = cache
        .get_or_option(&"expiring-null".to_string(), || async move {
            count2_ref.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })
        .await
        .unwrap();
    assert_eq!(
        count2.load(Ordering::SeqCst),
        1,
        "fallback should be called again after null sentinel expires"
    );
}

#[tokio::test]
async fn test_get_or_option_error_propagates() {
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_secs(30))
        .build()
        .await
        .unwrap();

    // Fallback returns error — should NOT cache anything
    let result: oxcache::error::OxCacheResult<Option<String>> = cache
        .get_or_option(&"error-key".to_string(), || async {
            Err(oxcache::error::OxCacheError::Operation("db down".to_string()))
        })
        .await;
    assert!(result.is_err());

    // Second call: fallback should be called again (error not cached)
    let call_count = AtomicU32::new(0);
    let call_count_ref = &call_count;
    let _ = cache
        .get_or_option(&"error-key".to_string(), || async move {
            call_count_ref.fetch_add(1, Ordering::SeqCst);
            Ok(Some("recovered".to_string()))
        })
        .await
        .unwrap();
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "fallback should be called again after previous error"
    );
}

// ============================================================================
// Builder integration tests
// ============================================================================

#[tokio::test]
async fn test_builder_null_cache_ttl_chained() {
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_secs(60))
        .ttl(Duration::from_secs(300))
        .capacity(1000)
        .build()
        .await
        .unwrap();

    // Verify the cache works with null_cache_ttl + other settings
    let result = cache
        .get_or_option(&"k".to_string(), || async { Ok(Some("v".to_string())) })
        .await
        .unwrap();
    assert_eq!(result, Some("v".to_string()));
}

#[tokio::test]
async fn test_get_or_option_concurrent_single_flight() {
    let cache = Arc::new(
        Cache::<String, String>::builder()
            .null_cache_ttl(Duration::from_secs(30))
            .build()
            .await
            .unwrap(),
    );

    let call_count = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();

    for _ in 0..16 {
        let cache = cache.clone();
        let call_count = call_count.clone();
        handles.push(tokio::spawn(async move {
            cache
                .get_or_option(&"concurrent-null".to_string(), || {
                    let cc = call_count.clone();
                    async move {
                        cc.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok(None)
                    }
                })
                .await
                .unwrap()
        }));
    }

    for h in handles {
        let result = h.await.unwrap();
        assert!(result.is_none());
    }

    // Single-flight should ensure fallback runs only once
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "single-flight should ensure fallback runs exactly once"
    );
}

// ============================================================================
// TTL Jitter tests
// ============================================================================

#[tokio::test]
async fn test_ttl_jitter_applied() {
    // With a large jitter factor, the actual TTL should vary across calls
    let cache: Cache<String, String> = Cache::builder().ttl_jitter(0.5).build().await.unwrap();

    let base_ttl = Duration::from_secs(100);
    let mut ttls = Vec::new();

    // Set multiple keys and check their TTLs vary
    for i in 0..20 {
        let key = format!("jitter-key-{i}");
        cache
            .set_with_ttl(&key, &"value".to_string(), Some(base_ttl))
            .await
            .unwrap();
        if let Some(ttl) = cache.ttl(&key).await.unwrap() {
            ttls.push(ttl);
        }
    }

    // With 0.5 jitter factor, TTLs should be in range [50s, 150s]
    for ttl in &ttls {
        assert!(
            *ttl >= Duration::from_secs(49) && *ttl <= Duration::from_secs(151),
            "TTL {:?} should be within jitter range [50s, 150s]",
            ttl
        );
    }

    // Not all TTLs should be identical (jitter is working)
    let unique_ttls: std::collections::HashSet<_> = ttls.iter().map(|t| t.as_millis()).collect();
    assert!(
        unique_ttls.len() > 1,
        "jitter should produce varied TTLs, got {} unique values from {} samples",
        unique_ttls.len(),
        ttls.len()
    );
}

#[tokio::test]
async fn test_ttl_jitter_zero_no_change() {
    // factor = 0.0 (default) — TTL should not be modified
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

    let base_ttl = Duration::from_secs(60);
    cache
        .set_with_ttl(&"stable-key".to_string(), &"v".to_string(), Some(base_ttl))
        .await
        .unwrap();

    let remaining = cache
        .ttl(&"stable-key".to_string())
        .await
        .unwrap()
        .expect("key should exist");
    // TTL should be very close to base_ttl (within 1s for processing time)
    assert!(
        remaining > Duration::from_secs(58) && remaining <= Duration::from_secs(60),
        "with zero jitter, TTL should remain close to base: {:?}",
        remaining
    );
}

#[tokio::test]
async fn test_null_sentinel_no_jitter() {
    // Null sentinel TTL should NOT be affected by jitter
    let cache: Cache<String, String> = Cache::builder()
        .null_cache_ttl(Duration::from_millis(200))
        .ttl_jitter(0.5)
        .build()
        .await
        .unwrap();

    // Cache a null sentinel
    let result = cache
        .get_or_option(&"null-no-jitter".to_string(), || async { Ok(None) })
        .await
        .unwrap();
    assert!(result.is_none());

    // Wait slightly more than the null_cache_ttl
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Sentinel should have expired (null_cache_ttl is not affected by jitter)
    let call_count = AtomicU32::new(0);
    let call_count_ref = &call_count;
    let _ = cache
        .get_or_option(&"null-no-jitter".to_string(), || async move {
            call_count_ref.fetch_add(1, Ordering::SeqCst);
            Ok(Some("back".to_string()))
        })
        .await
        .unwrap();
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "null sentinel should expire at fixed TTL (no jitter)"
    );
}

#[tokio::test]
async fn test_builder_ttl_jitter_clamped() {
    // Values > 1.0 should be clamped to 1.0
    let cache: Cache<String, String> = Cache::builder().ttl_jitter(2.0).build().await.unwrap();

    let base_ttl = Duration::from_secs(100);
    cache
        .set_with_ttl(&"clamped".to_string(), &"v".to_string(), Some(base_ttl))
        .await
        .unwrap();

    let remaining = cache
        .ttl(&"clamped".to_string())
        .await
        .unwrap()
        .expect("key should exist");
    // With factor clamped to 1.0, range is [0s, 200s]
    assert!(
        remaining <= Duration::from_secs(201),
        "TTL should be within clamped jitter range: {:?}",
        remaining
    );
}
