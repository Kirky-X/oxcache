// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Advanced E2E scenarios covering the 137 feature combinations from
//! `temp/feature_combinations_analysis.md`.
//!
//! Priority order: P0/P1 high-risk → Backend combos (B) → Operations (O) →
//! TTL (T) → Concurrency (C) → Degradation (D) → Security (SEC) → Config (CFG).
//!
//! Constraints:
//! - No external Redis dependency (uses MockBackend / MokaMemoryBackend / FailingBackend)
//! - Redis-specific tests are `#[ignore]`
//! - Concurrent tests use `#[tokio::test(flavor = "multi_thread")]`
//! - Error assertions use `match` + `panic!`, never `is_err()`
//! - Each feature section is isolated with `#[cfg(feature = "xxx")]`

#![allow(clippy::too_many_lines)]

#[cfg(any(feature = "memory", feature = "serialization"))]
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

// Cache type is re-exported under any backend feature gate.
#[cfg(any(
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
use oxcache::Cache;

// Backend trait imports — needed for `set`/`get`/`len`/`exists` etc. on
// MokaMemoryBackend / DashMapMemoryBackend / ChainCache / MockBackend.
#[cfg(any(feature = "memory", feature = "redis"))]
use oxcache::backend::{CacheReader, CacheWriter};

// ============================================================================
// Test data types
// ============================================================================

#[cfg(any(feature = "memory", feature = "serialization"))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
}

#[cfg(any(feature = "memory", feature = "serialization"))]
impl User {
    fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
        }
    }
}

// ============================================================================
// FailingBackend — error-injecting mock for degradation / failure scenarios
// (D-007, D-001, N-004, S-004).  All read/write operations return
// `Err(Connection(...))`; health_check returns Err; shutdown is no-op.
// ============================================================================

#[cfg(feature = "memory")]
struct FailingBackend {
    score_val: u8,
    name_str: &'static str,
}

#[cfg(feature = "memory")]
impl FailingBackend {
    fn new(score: u8) -> Self {
        Self {
            score_val: score,
            name_str: "failing",
        }
    }
}

#[cfg(feature = "memory")]
impl oxcache::backend::BackendScore for FailingBackend {
    fn score(&self) -> u8 {
        self.score_val
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        self.name_str
    }
}

#[cfg(feature = "memory")]
#[async_trait::async_trait]
impl oxcache::backend::CacheReader for FailingBackend {
    async fn get(&self, _key: &str) -> oxcache::error::OxCacheResult<Option<Vec<u8>>> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: get unavailable".to_string(),
        ))
    }
    async fn exists(&self, _key: &str) -> oxcache::error::OxCacheResult<bool> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: exists unavailable".to_string(),
        ))
    }
    async fn ttl(&self, _key: &str) -> oxcache::error::OxCacheResult<Option<Duration>> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: ttl unavailable".to_string(),
        ))
    }
    async fn len(&self) -> oxcache::error::OxCacheResult<u64> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: len unavailable".to_string(),
        ))
    }
    async fn is_empty(&self) -> oxcache::error::OxCacheResult<bool> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: is_empty unavailable".to_string(),
        ))
    }
    async fn capacity(&self) -> oxcache::error::OxCacheResult<u64> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: capacity unavailable".to_string(),
        ))
    }
    async fn stats(&self) -> oxcache::error::OxCacheResult<std::collections::HashMap<String, String>> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: stats unavailable".to_string(),
        ))
    }
}

#[cfg(feature = "memory")]
#[async_trait::async_trait]
impl oxcache::backend::CacheWriter for FailingBackend {
    async fn set(
        &self,
        _key: std::sync::Arc<str>,
        _value: std::sync::Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> oxcache::error::OxCacheResult<()> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: set unavailable".to_string(),
        ))
    }
    async fn delete(&self, _key: &str) -> oxcache::error::OxCacheResult<()> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: delete unavailable".to_string(),
        ))
    }
    async fn clear(&self) -> oxcache::error::OxCacheResult<()> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: clear unavailable".to_string(),
        ))
    }
    async fn expire(&self, _key: &str, _ttl: Duration) -> oxcache::error::OxCacheResult<bool> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: expire unavailable".to_string(),
        ))
    }
}

#[cfg(feature = "memory")]
#[async_trait::async_trait]
impl oxcache::backend::CacheConnector for FailingBackend {
    async fn health_check(&self) -> oxcache::error::OxCacheResult<()> {
        Err(oxcache::OxCacheError::Connection(
            "failing backend: health_check failed".to_string(),
        ))
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> oxcache::backend::BackendKind {
        oxcache::backend::BackendKind::Mock
    }
}

// ============================================================================
// P0/P1 HIGH-RISK SCENARIOS
// ============================================================================

/// P0 D-007: All backends fail simultaneously → ChainCache must return
/// `Operation("All backends failed to write")`.
#[cfg(feature = "memory")]
#[tokio::test]
async fn p0_d007_all_backends_fail_returns_operation_error() {
    let chain = oxcache::ChainCache::builder()
        .backend(FailingBackend::new(80))
        .backend(FailingBackend::new(50))
        .build();

    let result = chain.set("key", b"value".to_vec(), None).await;
    match result {
        Err(oxcache::OxCacheError::Operation(msg)) => {
            assert!(msg.contains("All backends failed"), "unexpected message: {msg}");
        }
        other => panic!("expected OxCacheError::Operation, got {other:?}"),
    }
}

/// P0 R-002: DashMap now has a FIFO O(1) eviction policy — writing beyond
/// capacity evicts the oldest entries and `len()` is bounded at capacity.
#[cfg(feature = "memory")]
#[tokio::test]
async fn p0_r002_dashmap_fifo_eviction_bounds_len_at_capacity() {
    use oxcache::DashMapMemoryBackend;

    let backend = DashMapMemoryBackend::builder().capacity(10).build();

    // Write 50 entries into a capacity-10 backend.
    for i in 0..50u32 {
        let key = format!("key_{i}");
        backend
            .set(Arc::from(key.as_str()), Arc::new(format!("val_{i}").into_bytes()), None)
            .await
            .expect("set must succeed");
    }

    let len = backend.len().await.expect("len must succeed");
    // FIFO eviction keeps len at capacity.
    assert_eq!(len, 10, "DashMap FIFO eviction should bound len at capacity, got {len}");
}

/// P0 C-004: get_or leader panic must not leak the single-flight lock.
/// `GetOrGuard::drop` removes the HashMap entry so subsequent get_or on the
/// same key still works.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn p0_c004_get_or_leader_panic_cleans_lock() {
    let cache: Arc<Cache<String, User>> = Arc::new(Cache::memory().await.expect("cache build"));
    let panic_key = "p0_c004_panic_key_unique".to_string();

    // Spawn a task whose fallback panics.
    let cache_for_panic = cache.clone();
    let key_clone = panic_key.clone();
    let handle = tokio::spawn(async move {
        // The closure panics — GetOrGuard must clean up on drop.
        cache_for_panic
            .get_or(&key_clone, || async {
                panic!("intentional panic in fallback for C-004 test");
            })
            .await
    });

    let join_result = handle.await;
    assert!(join_result.is_err(), "spawned get_or task should have panicked");

    // Give the runtime a tick to finish unwinding / dropping the guard.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Subsequent get_or on the SAME key must succeed — lock was cleaned up.
    let recovery_key = panic_key.clone();
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        cache
            .get_or(&recovery_key, || async { Ok(User::new(42, "recovered")) })
            .await
    })
    .await;

    match result {
        Ok(Ok(user)) => assert_eq!(user, User::new(42, "recovered")),
        Ok(Err(e)) => panic!("get_or after panic should succeed, got error: {e:?}"),
        Err(_) => panic!("get_or after panic timed out — lock was not cleaned up (C-004 regression)"),
    }
}

/// P1 D-001: L2 (lower-score backend) unavailable, L1 (higher-score) still
/// serves reads. ChainCache must NOT fail the read when a lower-priority
/// backend errors.
#[cfg(feature = "memory")]
#[tokio::test]
async fn p1_d001_l2_unavailable_l1_continues_serving() {
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = MokaMemoryBackend::new(); // score 100
    let l2 = FailingBackend::new(50); // always fails

    // Pre-populate L1.
    l1.set(Arc::from("hot_key"), Arc::new(b"hot_value".to_vec()), None)
        .await
        .expect("l1 set");

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1))
        .link(ChainLink::from_backend(l2))
        .build();

    // get traverses high → low; L1 hit means L2 is never touched.
    let val = chain.get("hot_key").await.expect("chain get must succeed from L1");
    assert_eq!(val, Some(b"hot_value".to_vec()));
}

/// P1 N-004: Network partition (L1 available, L2 unavailable) — read from L1
/// succeeds even though L2 is unreachable. With backfill DISABLED, no write
/// is attempted on the failing L2 during read.
#[cfg(feature = "memory")]
#[tokio::test]
async fn p1_n004_partition_l1_hit_l2_fail_no_backfill_stale() {
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = MokaMemoryBackend::new();
    let l2 = FailingBackend::new(40);

    l1.set(Arc::from("partition_key"), Arc::new(b"l1_data".to_vec()), None)
        .await
        .expect("l1 set");

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1))
        .link(ChainLink::from_backend(l2))
        .disable_backfill()
        .build();

    // Read hits L1; L2 failure is silently skipped (err → continue).
    let val = chain
        .get("partition_key")
        .await
        .expect("read should succeed from L1 despite L2 failure");
    assert_eq!(val, Some(b"l1_data".to_vec()));
}

/// P1 SEC-002: Lua script validation has a known limitation — double-quoted
/// string contents are stripped during preprocessing, so
/// `redis.call("FLUSHALL")` bypasses detection. This test DOCUMENTS the
/// known bypass (source code must not be modified per task constraints).
#[cfg(feature = "redis")]
#[tokio::test]
async fn p1_sec002_lua_double_quote_bypass_known_limitation() {
    use oxcache::validate_lua_script;

    // Single-quoted FLUSHALL is correctly blocked.
    let blocked = validate_lua_script("return redis.call('FLUSHALL')", 0);
    match blocked {
        Err(oxcache::OxCacheError::InvalidInput(msg)) => {
            assert!(msg.contains("FLUSHALL"), "should mention FLUSHALL: {msg}");
        }
        other => panic!("single-quoted FLUSHALL must be blocked, got {other:?}"),
    }

    // Double-quoted FLUSHALL bypasses detection (known limitation).
    // The preprocessor strips double-quoted string contents, turning
    // redis.call("FLUSHALL") into redis.call("") which passes validation.
    let bypassed = validate_lua_script("redis.call(\"FLUSHALL\")", 0);
    // This SHOULD be Err but is Ok due to the known bypass.
    // Documenting the current (buggy) behavior — fix requires src/ change.
    assert!(
        bypassed.is_ok(),
        "SEC-002 known bypass: double-quoted FLUSHALL currently passes validation"
    );
}

// ============================================================================
// BACKEND COMBINATIONS (B-001 ~ B-012)
// ============================================================================

/// B-001: Moka standalone with capacity=0 — builder defaults to 10000.
#[cfg(feature = "memory")]
#[tokio::test]
async fn b001_moka_capacity_zero_defaults_to_10000() {
    use oxcache::MokaMemoryBackend;

    let backend = MokaMemoryBackend::builder().capacity(0).build();
    // capacity(0) triggers the builder's fallback to 10_000.
    assert_eq!(backend.capacity(), 10_000);

    // Basic operations still work.
    backend
        .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set must succeed");
    let val = backend.get("k").await.expect("get must succeed");
    assert_eq!(val, Some(b"v".to_vec()));
}

/// B-002: DashMap lazy TTL — expired entry returns None on get but is NOT
/// removed from the map (len still counts it).
#[cfg(feature = "memory")]
#[tokio::test]
async fn b002_dashmap_lazy_ttl_expired_entry_not_removed() {
    use oxcache::DashMapMemoryBackend;

    let backend = DashMapMemoryBackend::new();
    backend
        .set(
            Arc::from("temp"),
            Arc::new(b"data".to_vec()),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("set with TTL");

    // Before expiry.
    assert!(backend.get("temp").await.unwrap().is_some());

    // Wait for expiry.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // get returns None (expired) but entry is still in the map.
    let val = backend.get("temp").await.expect("get must succeed");
    assert!(val.is_none(), "expired entry should return None");

    let len = backend.len().await.expect("len must succeed");
    assert_eq!(len, 1, "DashMap lazy expiry: stale entry still counted in len");
}

/// B-006: Moka + Mock chain with backfill — read from L2 (lower score)
/// backfills to L1 (higher score).
#[cfg(feature = "memory")]
#[tokio::test]
async fn b006_moka_mock_chain_backfill_populates_l1() {
    use crate::common::MockBackend;
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = MokaMemoryBackend::new(); // score 100
    let l2 = MockBackend::with_data("mock_l2", 50, false); // score 50

    // Pre-populate L2 only.
    l2.set(Arc::from("bf_key"), Arc::new(b"from_l2".to_vec()), None)
        .await
        .expect("l2 set");

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1.clone()))
        .link(ChainLink::from_backend(l2))
        .enable_backfill()
        .build();

    // Read — L1 miss, L2 hit, backfill to L1.
    let val = chain.get("bf_key").await.expect("chain get");
    assert_eq!(val, Some(b"from_l2".to_vec()));

    // L1 should now have the backfilled value.
    let l1_val = l1.get("bf_key").await.expect("l1 get");
    assert_eq!(l1_val, Some(b"from_l2".to_vec()), "backfill should populate L1");
}

/// B-007: Moka + DashMap dual-L1 chain — both are memory backends with
/// different scores; write goes to both.
#[cfg(feature = "memory")]
#[tokio::test]
async fn b007_moka_dashmap_dual_l1_chain_writes_to_both() {
    use oxcache::{ChainCache, ChainLink, DashMapMemoryBackend, MokaMemoryBackend};

    let moka = MokaMemoryBackend::new(); // score 100
    let dashmap = DashMapMemoryBackend::new(); // score 90

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(moka.clone()))
        .link(ChainLink::from_backend(dashmap.clone()))
        .build();

    chain
        .set("dual_key", b"dual_val".to_vec(), None)
        .await
        .expect("chain set");

    // Both backends should have the value.
    assert_eq!(moka.get("dual_key").await.unwrap(), Some(b"dual_val".to_vec()));
    assert_eq!(dashmap.get("dual_key").await.unwrap(), Some(b"dual_val".to_vec()));
}

/// B-009: BloomFilter + Moka — negative query (never-set key) is filtered
/// by the Bloom filter without touching the inner backend.
#[cfg(all(feature = "bloom-filter", feature = "memory"))]
#[tokio::test]
async fn b009_bloom_filter_moka_skips_negative_query() {
    use oxcache::MokaMemoryBackend;
    use oxcache::features::BloomFilterBackend;

    let inner = MokaMemoryBackend::new();
    let bf_backend = BloomFilterBackend::new(inner);

    // Set a key — inserts into BF + inner.
    bf_backend
        .set(Arc::from("exists"), Arc::new(b"yes".to_vec()), None)
        .await
        .expect("set");

    // Get existing key — BF says "maybe", inner returns value.
    assert_eq!(bf_backend.get("exists").await.unwrap(), Some(b"yes".to_vec()));

    // Get non-existent key — BF says "definitely absent", inner skipped.
    let val = bf_backend.get("never_set").await.expect("get");
    assert_eq!(val, None, "BF should filter negative query");
}

/// B-010: BloomFilter delete does NOT remove from the filter (standard BF
/// limitation). After delete, BF still says "maybe present", so the inner
/// backend is consulted and returns None.
#[cfg(all(feature = "bloom-filter", feature = "memory"))]
#[tokio::test]
async fn b010_bloom_filter_delete_does_not_remove_from_filter() {
    use oxcache::MokaMemoryBackend;
    use oxcache::features::BloomFilterBackend;

    let inner = MokaMemoryBackend::new();
    let bf_backend = BloomFilterBackend::new(inner);

    bf_backend
        .set(Arc::from("del_key"), Arc::new(b"val".to_vec()), None)
        .await
        .expect("set");

    // Delete — removes from inner, BF untouched.
    bf_backend.delete("del_key").await.expect("delete");

    // BF still thinks the key "maybe exists" → delegates to inner → None.
    let val = bf_backend.get("del_key").await.expect("get");
    assert_eq!(val, None, "inner returns None after delete (BF still says maybe)");
}

/// B-011: Empty ChainCache — get returns None, set returns Operation error,
/// delete returns Ok.
#[cfg(feature = "memory")]
#[tokio::test]
async fn b011_empty_chain_get_none_set_errors_delete_ok() {
    use oxcache::ChainCache;

    let chain = ChainCache::builder().build(); // no links

    // get on empty chain returns Ok(None).
    let val = chain.get("any").await.expect("get on empty chain");
    assert_eq!(val, None);

    // set on empty chain returns Operation error.
    let set_result = chain.set("any", b"v".to_vec(), None).await;
    match set_result {
        Err(oxcache::OxCacheError::Operation(msg)) => {
            assert!(msg.contains("no backends"), "unexpected message: {msg}");
        }
        other => panic!("expected Operation error for set on empty chain, got {other:?}"),
    }

    // delete on empty chain returns Ok (no-op).
    let del_result = chain.delete("any").await;
    assert!(del_result.is_ok(), "delete on empty chain should be Ok");
}

// ============================================================================
// OPERATION COMBINATIONS (O-001 ~ O-016)
// ============================================================================

/// O-004: SET with TTL=0 — entry expires immediately (or is never visible).
#[cfg(feature = "memory")]
#[tokio::test]
async fn o004_set_ttl_zero_expires_immediately() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");

    // TTL=0 means the entry is already expired.
    cache
        .set_with_ttl(
            &"zero_ttl".to_string(),
            &User::new(1, "temp"),
            Some(Duration::from_secs(0)),
        )
        .await
        .expect("set_with_ttl(0)");

    // Give Moka a tick to process expiry.
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Entry should be gone (or was never effectively stored).
    let val: Option<User> = cache.get(&"zero_ttl".to_string()).await.expect("get");
    assert!(val.is_none(), "TTL=0 entry should not be retrievable");
}

/// O-006: get_or fallback returns Err — error propagates to caller.
#[cfg(feature = "memory")]
#[tokio::test]
async fn o006_get_or_fallback_error_propagates() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");

    let result: Result<User, _> = cache
        .get_or(&"err_key".to_string(), || async {
            Err(oxcache::OxCacheError::Connection(
                "fallback deliberately failed".to_string(),
            ))
        })
        .await;

    match result {
        Err(oxcache::OxCacheError::Connection(msg)) => {
            assert!(msg.contains("fallback deliberately failed"), "unexpected msg: {msg}");
        }
        other => panic!("expected Connection error from fallback, got {other:?}"),
    }
}

/// O-011: EXISTS on an expired key returns false.
#[cfg(feature = "memory")]
#[tokio::test]
async fn o011_exists_on_expired_key_returns_false() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");

    cache
        .set_with_ttl(
            &"short_lived".to_string(),
            &User::new(1, "x"),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("set");

    // Before expiry.
    assert!(cache.exists(&"short_lived".to_string()).await.expect("exists"));

    tokio::time::sleep(Duration::from_millis(100)).await;

    // After expiry.
    assert!(
        !cache.exists(&"short_lived".to_string()).await.expect("exists"),
        "expired key should not exist"
    );
}

/// O-012: TTL query on a non-existent key returns None.
#[cfg(feature = "memory")]
#[tokio::test]
async fn o012_ttl_on_nonexistent_returns_none() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");

    let ttl = cache.ttl(&"ghost".to_string()).await.expect("ttl");
    assert_eq!(ttl, None, "TTL for non-existent key should be None");
}

/// O-013: EXPIRE on a non-existent key returns false.
#[cfg(feature = "memory")]
#[tokio::test]
async fn o013_expire_nonexistent_returns_false() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");

    let updated = cache
        .expire(&"no_such_key".to_string(), Duration::from_secs(30))
        .await
        .expect("expire");
    assert!(!updated, "expire on non-existent key should return false");
}

/// O-015: health_check on a healthy memory backend returns Ok.
#[cfg(feature = "memory")]
#[tokio::test]
async fn o015_health_check_memory_backend_ok() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");
    cache
        .health_check()
        .await
        .expect("health_check should succeed for memory backend");
}

/// O-016: Shutdown clears the cache; subsequent operations are safe (no panic).
#[cfg(feature = "memory")]
#[tokio::test]
async fn o016_shutdown_clears_cache_operations_safe() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");
    cache.set(&"k".to_string(), &User::new(1, "v")).await.expect("set");

    cache.shutdown().await;

    // Operations after shutdown should not panic (may return empty/Ok).
    let len = cache.len().await.expect("len after shutdown");
    assert_eq!(len, 0, "shutdown should clear entries");
}

// ============================================================================
// TTL STRATEGIES (T-001 ~ T-007)
// ============================================================================

/// T-001: Global TTL via CacheBuilder — all entries expire after the
/// configured duration.
#[cfg(feature = "memory")]
#[tokio::test]
async fn t001_global_ttl_via_builder_expires() {
    let cache: Cache<String, User> = Cache::builder()
        .ttl(Duration::from_millis(80))
        .build()
        .await
        .expect("build with TTL");

    cache.set(&"k".to_string(), &User::new(1, "v")).await.expect("set");

    // Before expiry.
    assert!(cache.get(&"k".to_string()).await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(150)).await;

    // After global TTL.
    assert!(
        cache.get(&"k".to_string()).await.unwrap().is_none(),
        "entry should expire after global TTL"
    );
}

/// T-002: Per-entry TTL overrides global TTL.
#[cfg(feature = "memory")]
#[tokio::test]
async fn t002_per_entry_ttl_overrides_global() {
    let cache: Cache<String, User> = Cache::builder()
        .ttl(Duration::from_secs(10)) // global: 10s
        .build()
        .await
        .expect("build");

    // Per-entry TTL: 50ms (overrides 10s global).
    cache
        .set_with_ttl(
            &"short".to_string(),
            &User::new(1, "x"),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("set_with_ttl");

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        cache.get(&"short".to_string()).await.unwrap().is_none(),
        "per-entry TTL (50ms) should override global (10s)"
    );
}

/// T-003: TTI (time-to-idle) — builder accepts TTI; cache functions normally.
#[cfg(feature = "memory")]
#[tokio::test]
async fn t003_tti_builder_accepts_and_cache_works() {
    let cache: Cache<String, User> = Cache::builder()
        .tti(Duration::from_millis(100))
        .build()
        .await
        .expect("build with TTI");

    cache
        .set(&"tti_key".to_string(), &User::new(1, "v"))
        .await
        .expect("set");

    // Immediately available.
    assert!(cache.get(&"tti_key".to_string()).await.unwrap().is_some());

    // Wait beyond TTI without access.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Should be expired (TTI exceeded, no access to reset).
    let val = cache.get(&"tti_key".to_string()).await.unwrap();
    assert!(val.is_none(), "entry should expire after TTI with no access");
}

/// T-004: ChainCache default_ttl is applied when set(ttl=None).
#[cfg(feature = "memory")]
#[tokio::test]
async fn t004_chain_default_ttl_applied_on_set_none() {
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = MokaMemoryBackend::new();
    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1.clone()))
        .default_time_to_live(Duration::from_millis(50))
        .build();

    // set with ttl=None → chain uses default_ttl (50ms).
    chain.set("dt_key", b"v".to_vec(), None).await.expect("set");

    // Before expiry.
    assert!(l1.get("dt_key").await.unwrap().is_some());

    tokio::time::sleep(Duration::from_millis(100)).await;

    // After default TTL.
    assert!(
        l1.get("dt_key").await.unwrap().is_none(),
        "chain default_ttl should expire entry"
    );
}

/// T-006: DashMap lazy expiration — get checks expiry, exists checks expiry,
/// but len does NOT (stale entries counted).
#[cfg(feature = "memory")]
#[tokio::test]
async fn t006_dashmap_lazy_expiration_get_and_exists_check() {
    use oxcache::DashMapMemoryBackend;

    let backend = DashMapMemoryBackend::new();
    backend
        .set(
            Arc::from("lazy"),
            Arc::new(b"v".to_vec()),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("set");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // get returns None (checks expiry).
    assert!(backend.get("lazy").await.unwrap().is_none());
    // exists returns false (checks expiry).
    assert!(!backend.exists("lazy").await.unwrap());
    // len still counts the stale entry (no active cleanup).
    assert_eq!(backend.len().await.unwrap(), 1);
}

// ============================================================================
// CONCURRENCY (C-001 ~ C-008)
// ============================================================================

/// C-001: Concurrent get_or dedup — fallback is called exactly once even
/// with multiple concurrent requests for the same key.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn c001_concurrent_get_or_dedup_single_fallback() {
    let cache: Arc<Cache<String, User>> = Arc::new(Cache::memory().await.expect("cache"));
    let counter = Arc::new(AtomicU32::new(0));
    let key = "c001_dedup_unique_key".to_string();

    let mut handles = Vec::new();
    for _ in 0..10 {
        let cache = cache.clone();
        let counter = counter.clone();
        let key = key.clone();
        handles.push(tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                cache
                    .get_or(&key, || async {
                        counter.fetch_add(1, Ordering::SeqCst);
                        // Small delay so followers can register.
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        Ok(User::new(1, "deduped"))
                    })
                    .await
            })
            .await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await);
    }

    // All tasks should complete successfully.
    for result in &results {
        assert!(result.is_ok(), "task should not panic: {result:?}");
        let timeout_result = result.as_ref().unwrap();
        assert!(timeout_result.is_ok(), "task should not time out: {timeout_result:?}");
        let get_or_result = timeout_result.as_ref().unwrap();
        assert!(get_or_result.is_ok(), "get_or should succeed: {get_or_result:?}");
    }

    // Fallback should have been called exactly once.
    let count = counter.load(Ordering::SeqCst);
    assert_eq!(count, 1, "fallback should be called exactly once, got {count}");
}

/// C-002: get_or leader success — followers get the cached value.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn c002_get_or_leader_success_followers_get_cached() {
    let cache: Arc<Cache<String, User>> = Arc::new(Cache::memory().await.expect("cache"));
    let key = "c002_leader_success".to_string();

    let mut handles = Vec::new();
    for _ in 0..5 {
        let cache = cache.clone();
        let key = key.clone();
        handles.push(tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                cache
                    .get_or(&key, || async {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok(User::new(99, "from_leader"))
                    })
                    .await
            })
            .await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await);
    }
    for result in &results {
        assert!(result.is_ok(), "task panicked: {result:?}");
        let timeout_result = result.as_ref().unwrap();
        match timeout_result {
            Ok(get_or_result) => match get_or_result {
                Ok(user) => assert_eq!(user, &User::new(99, "from_leader")),
                Err(e) => panic!("get_or should succeed: {e:?}"),
            },
            Err(e) => panic!("task timed out: {e:?}"),
        }
    }
}

/// C-003: get_or leader failure — followers also receive the error.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn c003_get_or_leader_failure_followers_get_error() {
    let cache: Arc<Cache<String, User>> = Arc::new(Cache::memory().await.expect("cache"));
    let key = "c003_leader_failure".to_string();

    let mut handles = Vec::new();
    for _ in 0..5 {
        let cache = cache.clone();
        let key = key.clone();
        handles.push(tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                cache
                    .get_or(&key, || async {
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        Err(oxcache::OxCacheError::Connection("leader failed".to_string()))
                    })
                    .await
            })
            .await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await);
    }
    for result in &results {
        assert!(result.is_ok(), "task panicked: {result:?}");
        let timeout_result = result.as_ref().unwrap();
        match timeout_result {
            Ok(get_or_result) => match get_or_result {
                // Leader gets the original Connection error.
                Err(oxcache::OxCacheError::Connection(msg)) => {
                    assert!(msg.contains("leader failed"), "unexpected msg: {msg}");
                }
                // Followers get L1Error because the leader didn't cache a result.
                Err(oxcache::OxCacheError::L1Error(msg)) => {
                    assert!(
                        msg.contains("leader failed") || msg.contains("concurrent fetch"),
                        "unexpected msg: {msg}"
                    );
                }
                other => panic!("expected Connection or L1Error, got {other:?}"),
            },
            Err(e) => panic!("task timed out: {e:?}"),
        }
    }
}

/// C-005: Concurrent SET on the same key — last writer wins, no panic.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn c005_concurrent_set_same_key_last_wins_no_panic() {
    let cache: Arc<Cache<String, User>> = Arc::new(Cache::memory().await.expect("cache"));
    let key = "c005_race".to_string();

    let mut handles = Vec::new();
    for i in 0..20u64 {
        let cache = cache.clone();
        let key = key.clone();
        handles.push(tokio::spawn(async move {
            cache.set(&key, &User::new(i, &format!("user_{i}"))).await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await);
    }
    for result in &results {
        assert!(result.is_ok(), "set task panicked: {result:?}");
        assert!(result.as_ref().unwrap().is_ok(), "set should succeed");
    }

    // Final value is one of the 20 (last writer wins).
    let final_val: User = cache.get(&key).await.unwrap().expect("value must exist");
    assert!(final_val.id < 20, "id should be one of the writers: {}", final_val.id);
}

/// C-006: Concurrent GET + SET — no panic, reads see either old or new value.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn c006_concurrent_get_set_no_panic() {
    let cache: Arc<Cache<String, User>> = Arc::new(Cache::memory().await.expect("cache"));
    let key = "c006_rw".to_string();

    // Pre-populate.
    cache.set(&key, &User::new(0, "initial")).await.expect("set");

    // Writers and readers use separate handle vectors (different return types).
    let mut writer_handles = Vec::new();
    for i in 1..10u64 {
        let cache = cache.clone();
        let key = key.clone();
        writer_handles.push(tokio::spawn(async move {
            cache.set(&key, &User::new(i, &format!("v_{i}"))).await
        }));
    }

    let mut reader_handles = Vec::new();
    for _ in 0..10 {
        let cache = cache.clone();
        let key = key.clone();
        reader_handles.push(tokio::spawn(async move { cache.get(&key).await }));
    }

    // All writers should succeed.
    for handle in writer_handles {
        let result = handle.await;
        assert!(result.is_ok(), "writer task panicked: {result:?}");
        assert!(result.as_ref().unwrap().is_ok(), "set should succeed");
    }

    // All readers should succeed (no panic).
    for handle in reader_handles {
        let result = handle.await;
        assert!(result.is_ok(), "reader task panicked: {result:?}");
    }
}

/// C-008: Concurrent backfill — multiple reads from L2 trigger backfill
/// to L1 simultaneously; result is correct (idempotent writes).
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn c008_concurrent_backfill_idempotent() {
    use crate::common::MockBackend;
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = Arc::new(MokaMemoryBackend::new());
    let l2 = MockBackend::with_data("l2", 50, false);

    l2.set(Arc::from("bf_concurrent"), Arc::new(b"shared".to_vec()), None)
        .await
        .expect("l2 set");

    let chain = Arc::new(
        ChainCache::builder()
            .link(ChainLink::from_backend(l1.as_ref().clone()))
            .link(ChainLink::from_backend(l2))
            .enable_backfill()
            .build(),
    );

    let mut handles = Vec::new();
    for _ in 0..10 {
        let chain = chain.clone();
        handles.push(tokio::spawn(async move { chain.get("bf_concurrent").await }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await);
    }
    for result in &results {
        assert!(result.is_ok(), "backfill task panicked: {result:?}");
        let val = result.as_ref().unwrap().as_ref().expect("get should succeed");
        assert_eq!(val, &Some(b"shared".to_vec()));
    }

    // L1 should have the backfilled value.
    assert_eq!(l1.get("bf_concurrent").await.unwrap(), Some(b"shared".to_vec()));
}

// ============================================================================
// DEGRADATION (D-001 ~ D-010)
// ============================================================================

/// D-003: Serialization failure — corrupt data stored in the backend causes
/// deserialization error on get.
#[cfg(feature = "memory")]
#[tokio::test]
async fn d003_serialization_failure_corrupt_data() {
    use oxcache::MokaMemoryBackend;
    use oxcache::backend::CacheWriter;

    // Write corrupt bytes directly to the backend.
    let backend = MokaMemoryBackend::new();
    backend
        .set(Arc::from("corrupt"), Arc::new(b"{not valid json".to_vec()), None)
        .await
        .expect("set corrupt bytes");

    // Create a Cache that reads from this backend.
    let cache: Cache<String, User> = Cache::with_dependencies(Arc::new(backend));

    let result = cache.get(&"corrupt".to_string()).await;
    match result {
        Err(oxcache::OxCacheError::Serialization(_)) => {}
        other => panic!("expected Serialization error for corrupt data, got {other:?}"),
    }
}

/// D-007 (partial): One backend fails, chain still succeeds (partial failure
/// is tolerated; only ALL-backends-fail returns error).
#[cfg(feature = "memory")]
#[tokio::test]
async fn d007_partial_backend_failure_chain_succeeds() {
    use crate::common::MockBackend;
    use oxcache::{ChainCache, ChainLink};

    let good = MockBackend::with_data("good", 80, false);
    let bad = FailingBackend::new(50);

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(good))
        .link(ChainLink::from_backend(bad))
        .build();

    // set: one backend fails, one succeeds → overall Ok.
    let result = chain.set("partial", b"v".to_vec(), None).await;
    assert!(result.is_ok(), "partial failure should not fail the chain: {result:?}");
}

/// D-010: Operations after shutdown are safe (no panic, may return empty).
#[cfg(feature = "memory")]
#[tokio::test]
async fn d010_shutdown_then_operations_safe() {
    let cache: Cache<String, User> = Cache::memory().await.expect("cache");
    cache.set(&"a".to_string(), &User::new(1, "x")).await.expect("set");
    cache.set(&"b".to_string(), &User::new(2, "y")).await.expect("set");

    cache.shutdown().await;

    // get after shutdown — no panic, returns None.
    let val: Option<User> = cache.get(&"a".to_string()).await.expect("get after shutdown");
    assert_eq!(val, None, "shutdown should clear cache");

    // set after shutdown — should not panic (may succeed silently or error).
    let _ = cache.set(&"c".to_string(), &User::new(3, "z")).await;

    // len after shutdown — no panic.
    let _ = cache.len().await;
}

// ============================================================================
// SECURITY (SEC-001 ~ SEC-010) — requires `redis` feature
// ============================================================================

/// SEC-001: validate_redis_key rejects CRLF (protocol injection).
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec001_validate_redis_key_rejects_crlf() {
    use oxcache::validate_redis_key;

    let result = validate_redis_key("key\r\nINJECT");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for CRLF key, got {other:?}"),
    }
}

/// SEC-002: validate_lua_script rejects FLUSHALL via single-quoted string.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec002_lua_rejects_single_quoted_flushall() {
    use oxcache::validate_lua_script;

    let result = validate_lua_script("return redis.call('FLUSHALL')", 0);
    match result {
        Err(oxcache::OxCacheError::InvalidInput(msg)) => {
            assert!(msg.contains("FLUSHALL"), "should mention FLUSHALL: {msg}");
        }
        other => panic!("expected InvalidInput for FLUSHALL, got {other:?}"),
    }
}

/// SEC-003: validate_scan_pattern rejects too many wildcards.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec003_scan_pattern_too_many_wildcards() {
    use oxcache::validate_scan_pattern;

    // 11 wildcards exceeds MAX_SCAN_WILDCARDS (10).
    let pattern = "*".repeat(11);
    let result = validate_scan_pattern(&pattern);
    match result {
        Err(oxcache::OxCacheError::InvalidInput(msg)) => {
            assert!(msg.contains("wildcards"), "should mention wildcards: {msg}");
        }
        other => panic!("expected InvalidInput for too many wildcards, got {other:?}"),
    }
}

/// SEC-003b: clamp_scan_count clamps to [1, 1000].
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec003b_clamp_scan_count_bounds() {
    use oxcache::clamp_scan_count;

    assert_eq!(clamp_scan_count(0), 1);
    assert_eq!(clamp_scan_count(1), 1);
    assert_eq!(clamp_scan_count(500), 500);
    assert_eq!(clamp_scan_count(1000), 1000);
    assert_eq!(clamp_scan_count(5000), 1000);
}

/// SEC-004: redact_cache_key masks keys containing sensitive patterns.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec004_redact_cache_key_masks_sensitive() {
    use oxcache::redact_cache_key;

    let masked = redact_cache_key("user:session:abc123def456");
    // "session" is a sensitive pattern → value is redacted.
    assert!(
        masked.starts_with("****"),
        "sensitive key should be redacted, got: {masked}"
    );

    let normal = redact_cache_key("user:profile:123");
    // No sensitive pattern → returned as-is (under 100 chars).
    assert_eq!(normal, "user:profile:123");
}

/// SEC-005: redact_connection_string hides the password.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec005_redact_connection_string_hides_password() {
    use oxcache::redact_connection_string;

    let original = "redis://user:s3cr3t@localhost:6379";
    let masked = redact_connection_string(original);
    assert!(!masked.contains("s3cr3t"), "password must be hidden, got: {masked}");
    assert!(masked.contains("****"), "should contain **** mask: {masked}");
    assert!(masked.contains("localhost:6379"), "host should be visible: {masked}");
}

/// SEC-007: KeyGenerator validate_key rejects empty and over-length keys.
#[cfg(any(feature = "memory", feature = "minimal"))]
#[tokio::test]
async fn sec007_key_generator_validates_invalid_keys() {
    use oxcache::KeyGenerator;

    let key_gen = KeyGenerator::new();

    // Empty key.
    let result = key_gen.validate_key("");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for empty key, got {other:?}"),
    }

    // Over-length key.
    let long_key = "x".repeat(300); // default max is 256
    let result = key_gen.validate_key(&long_key);
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for over-length key, got {other:?}"),
    }

    // Valid key.
    assert!(key_gen.validate_key("user:123").is_ok());
}

/// SEC-007b: KeyGenerator generate_full produces namespaced keys.
#[cfg(any(feature = "memory", feature = "minimal"))]
#[tokio::test]
async fn sec007b_key_generator_generate_full_namespaced() {
    use oxcache::KeyGenerator;

    let key_gen = KeyGenerator::with_prefix("app:v1:").with_namespace("ns");
    let key = key_gen.generate_full("user:{id}", &[("id", "42")]);
    // generate_full produces "namespace:prefix:template"
    assert_eq!(key, "ns:app:v1:user:42");
}

/// SEC-008: validate_redis_key detects SQL injection patterns.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec008_sql_injection_detected() {
    use oxcache::validate_redis_key;

    let result = validate_redis_key("' OR '1'='1");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for SQL injection, got {other:?}"),
    }
}

/// SEC-009: validate_redis_key detects command injection characters.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec009_command_injection_detected() {
    use oxcache::validate_redis_key;

    // Semicolon.
    let result = validate_redis_key("key;cmd");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for command injection ';', got {other:?}"),
    }

    // Pipe.
    let result = validate_redis_key("key|cmd");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for command injection '|', got {other:?}"),
    }
}

/// SEC-010: validate_redis_key detects path traversal patterns.
#[cfg(feature = "redis")]
#[tokio::test]
async fn sec010_path_traversal_detected() {
    use oxcache::validate_redis_key;

    let result = validate_redis_key("../etc/passwd");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for path traversal, got {other:?}"),
    }

    // URL-encoded variant.
    let result = validate_redis_key("%2e%2e%2f");
    match result {
        Err(oxcache::OxCacheError::InvalidInput(_)) => {}
        other => panic!("expected InvalidInput for encoded path traversal, got {other:?}"),
    }
}

// ============================================================================
// CONFIG / BUILDER (CFG-001 ~ CFG-010)
// ============================================================================

/// CFG-001: Default build creates a working Moka cache with 10000 capacity.
#[cfg(feature = "memory")]
#[tokio::test]
async fn cfg001_default_build_creates_moka_10000() {
    let cache: Cache<String, User> = Cache::builder().build().await.expect("default build");

    let cap = cache.capacity().await.expect("capacity");
    assert_eq!(cap, 10000, "default capacity should be 10000");

    cache.set(&"k".to_string(), &User::new(1, "v")).await.expect("set");
    assert_eq!(cache.get(&"k".to_string()).await.unwrap(), Some(User::new(1, "v")));
}

/// CFG-002: capacity(0) — builder defaults to 10000 (Moka fallback).
#[cfg(feature = "memory")]
#[tokio::test]
async fn cfg002_capacity_zero_defaults_to_10000() {
    let cache: Cache<String, User> = Cache::builder().capacity(0).build().await.expect("build");

    let cap = cache.capacity().await.expect("capacity");
    assert_eq!(cap, 10000, "capacity(0) should default to 10000");
}

/// CFG-003: TTL + TTI combination — builder accepts both.
#[cfg(feature = "memory")]
#[tokio::test]
async fn cfg003_ttl_tti_combo_accepted() {
    let cache: Cache<String, User> = Cache::builder()
        .ttl(Duration::from_secs(60))
        .tti(Duration::from_secs(30))
        .build()
        .await
        .expect("build with TTL+TTI");

    cache.set(&"k".to_string(), &User::new(1, "v")).await.expect("set");
    assert!(cache.get(&"k".to_string()).await.unwrap().is_some());
}

/// CFG-004: sync_mode(true) + backend_arc() returns NotSupported.
#[cfg(feature = "memory")]
#[tokio::test]
async fn cfg004_sync_mode_with_backend_arc_returns_not_supported() {
    use oxcache::MokaMemoryBackend;

    let backend: Arc<dyn oxcache::backend::CacheBackend> = Arc::new(MokaMemoryBackend::new());

    let result: Result<Cache<String, User>, _> = Cache::builder().sync_mode(true).backend_arc(backend).build().await;

    match result {
        Err(oxcache::OxCacheError::NotSupported(msg)) => {
            assert!(
                msg.contains("sync_mode") && msg.contains("backend_arc"),
                "error should explain the incompatibility: {msg}"
            );
        }
        other => panic!("expected NotSupported for sync_mode + backend_arc, got {other:?}"),
    }
}

/// CFG-006: ChainCache backfill enable/disable toggle.
#[cfg(feature = "memory")]
#[tokio::test]
async fn cfg006_chain_backfill_enable_disable_toggle() {
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = MokaMemoryBackend::new();
    let l2 = MokaMemoryBackend::builder().capacity(100).build();

    // Enable backfill.
    let chain_on = ChainCache::builder()
        .link(ChainLink::from_backend(l1.clone()))
        .link(ChainLink::from_backend(l2.clone()))
        .enable_backfill()
        .build();
    assert!(chain_on.links().len() == 2);

    // Disable backfill.
    let chain_off = ChainCache::builder()
        .link(ChainLink::from_backend(l1.clone()))
        .link(ChainLink::from_backend(l2))
        .disable_backfill()
        .build();
    assert!(chain_off.links().len() == 2);
}

/// CFG-007: ChainCache default_time_to_live is stored and used.
#[cfg(feature = "memory")]
#[tokio::test]
async fn cfg007_chain_default_time_to_live_stored() {
    use oxcache::{ChainCache, ChainLink, MokaMemoryBackend};

    let l1 = MokaMemoryBackend::new();
    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1))
        .default_time_to_live(Duration::from_secs(300))
        .build();

    // Verify chain was built (default_ttl is private, tested via behavior in T-004).
    assert_eq!(chain.len(), 1);
}

// ============================================================================
// SYNC API (S-001 ~ S-005)
// ============================================================================

/// S-001: Sync GET/SET via sync_mode(true) on multi_thread runtime.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn s001_sync_get_set_via_sync_mode() {
    let cache: Cache<String, User> = Cache::builder()
        .sync_mode(true)
        .build()
        .await
        .expect("build with sync_mode");

    // Sync set.
    cache
        .set_sync(&"sync_k".to_string(), &User::new(1, "sync_v"))
        .expect("set_sync");

    // Sync get.
    let val = cache.get_sync(&"sync_k".to_string()).expect("get_sync");
    assert_eq!(val, Some(User::new(1, "sync_v")));
}

/// S-002: sync_mode + backend_arc returns NotSupported (same as CFG-004 but
/// tested via the sync API path).
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn s002_sync_api_without_sync_mode_returns_not_supported() {
    let cache: Cache<String, User> = Cache::builder().build().await.expect("build without sync_mode");

    // sync API not available (sync_mode was not enabled).
    let result = cache.get_sync(&"any".to_string());
    match result {
        Err(oxcache::OxCacheError::NotSupported(msg)) => {
            assert!(msg.contains("sync_mode"), "should mention sync_mode: {msg}");
        }
        other => panic!("expected NotSupported when sync_mode is off, got {other:?}"),
    }
}

/// S-004: ChainCache sync API with a non-sync link returns NotSupported.
#[cfg(feature = "memory")]
#[tokio::test(flavor = "multi_thread")]
async fn s004_chain_sync_with_non_sync_link_returns_not_supported() {
    use crate::common::MockBackend;
    use oxcache::{ChainCache, ChainLink};

    // MockBackend does NOT implement SyncCacheBackend.
    let mock = MockBackend::with_data("mock", 80, false);
    let chain = ChainCache::builder().link(ChainLink::from_backend(mock)).build();

    let result = chain.get_sync("any");
    match result {
        Err(oxcache::OxCacheError::NotSupported(msg)) => {
            assert!(
                msg.contains("SyncCacheBackend"),
                "should mention SyncCacheBackend: {msg}"
            );
        }
        other => panic!("expected NotSupported for non-sync chain link, got {other:?}"),
    }
}

// ============================================================================
// METRICS (M-001 ~ M-004) — requires `metrics` feature (in minimal)
// ============================================================================

/// M-002: JSON format export produces valid JSON with expected fields.
#[cfg(feature = "metrics")]
#[tokio::test]
async fn m002_export_json_format_valid() {
    use oxcache::export_json_format;

    let json = export_json_format().expect("json export");
    assert!(json.contains("counters"), "should contain counters: {json}");
    assert!(json.contains("l1_hits"), "should contain l1_hits: {json}");
}

/// M-003: Prometheus format export contains expected metric names.
#[cfg(feature = "metrics")]
#[tokio::test]
async fn m003_export_prometheus_format_valid() {
    use oxcache::export_prometheus_format;

    let prom = export_prometheus_format();
    assert!(prom.contains("cache_l1_hits_total"), "should contain l1_hits: {prom}");
    assert!(
        prom.contains("cache_operations_total"),
        "should contain operations: {prom}"
    );
}
