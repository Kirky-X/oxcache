// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 后端故障注入（FailingBackend）混沌测试
//
// 变更记录（production-mock-purge T027）：本文件原为 tests/e2e/advanced_scenarios_test.rs
// 中的 FailingBackend 故障注入用例（P0 D-007 / P1 D-001 / P1 N-004 / D-007 单后端失败），
// 按"集成/e2e 禁止 mock（含故障注入替身）"原则下沉至 tests/chaos/——故障注入即混沌测试。
//
// 断言语义与原始版本保持一致：仅将用例中的"正常"后端由 MockBackend 替换为真实
// 内存后端（DashMapMemoryBackend）。FailingBackend 本身是意图显式的错误注入替身，
// 只出现在 chaos 测试中，不进入集成/e2e 面。

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::{CacheReader, CacheWriter};

// ============================================================================
// FailingBackend — error-injecting backend for degradation / failure scenarios
// (D-007, D-001, N-004).  All read/write operations return
// `Err(Connection(...))`; health_check returns Err; shutdown is no-op.
// ============================================================================

struct FailingBackend {
    score_val: u8,
    name_str: &'static str,
}

impl FailingBackend {
    fn new(score: u8) -> Self {
        Self {
            score_val: score,
            name_str: "failing",
        }
    }
}

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

/// P0 D-007: All backends fail simultaneously → ChainCache must return
/// `Operation("All backends failed to write")`.
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

/// P1 D-001: L2 (lower-score backend) unavailable, L1 (higher-score) still
/// serves reads. ChainCache must NOT fail the read when a lower-priority
/// backend errors.
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

/// D-007 (partial): One backend fails, chain still succeeds (partial failure
/// is tolerated; only ALL-backends-fail returns error).
#[tokio::test]
async fn d007_partial_backend_failure_chain_succeeds() {
    use oxcache::{ChainCache, ChainLink, DashMapMemoryBackend};

    let good = DashMapMemoryBackend::new(); // 真实内存后端，写入成功
    let bad = FailingBackend::new(50); // 故障注入，所有写入失败

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(good))
        .link(ChainLink::from_backend(bad))
        .build();

    // set: one backend fails, one succeeds → overall Ok.
    let result = chain.set("partial", b"v".to_vec(), None).await;
    assert!(result.is_ok(), "partial failure should not fail the chain: {result:?}");
}
