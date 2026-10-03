// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! ChainCache 读取策略与批量读错误路径覆盖：
//!
//! - `Race` / `ParallelFreshest` / `Sequential` 全后端失败时错误传播
//! - Race 与 ParallelFreshest 命中高分以下后端时的 backfill 回填
//! - `get_many` 长度违约（整批失败口径）与整批 Err 的按键拆分上报
//! - `lowest_score_backend` / `persistent_backends` 视图方法

#![cfg(feature = "full")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use oxcache::backend::{
    BackendKind, BackendScore, CacheBackend, CacheConnector, CacheReader, CacheWriter, Scores,
};

fn mem_link(
    backend: Arc<dyn CacheBackend>,
    score: u8,
    persistent: bool,
    name: &'static str,
) -> ChainLink {
    ChainLink::from_arc(backend, score, persistent, name)
}
use oxcache::OxCacheError as Error;
use oxcache::cache::chain::{ChainCache, ChainLink, ChainReadStrategy};
use oxcache::error::{OxCacheError, OxCacheResult};

/// 恒定故障后端：所有读写都返回 Err。
#[derive(Default)]
struct FailingBackend {
    prefix: &'static str,
}

#[async_trait]
impl CacheReader for FailingBackend {
    async fn get(&self, _key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn exists(&self, _key: &str) -> OxCacheResult<bool> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn len(&self) -> OxCacheResult<u64> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn capacity(&self) -> OxCacheResult<u64> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
}

#[async_trait]
impl CacheWriter for FailingBackend {
    async fn set(
        &self,
        _key: Arc<str>,
        _value: Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn delete(&self, _key: &str) -> OxCacheResult<()> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn clear(&self) -> OxCacheResult<()> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
    async fn expire(&self, _key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Err(OxCacheError::Operation(format!("{} down", self.prefix)))
    }
}

#[async_trait]
impl CacheConnector for FailingBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Mock
    }
}

impl BackendScore for FailingBackend {
    fn score(&self) -> u8 {
        10
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        self.prefix
    }
}

/// 批量读违约后端：`get_many` 返回与请求数不符的长度（trait 契约破坏）。
#[derive(Default)]
struct BadBatchBackend;

#[async_trait]
impl CacheReader for BadBatchBackend {
    async fn get(&self, _key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn exists(&self, _key: &str) -> OxCacheResult<bool> {
        Ok(false)
    }
    async fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
        Ok(None)
    }
    async fn len(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        Ok(HashMap::new())
    }
    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        // 契约要求长度一致；这里故意少返回一个，触发整批失败口径
        Ok(vec![None; keys.len().saturating_sub(1)])
    }
}

#[async_trait]
impl CacheWriter for BadBatchBackend {
    async fn set(
        &self,
        _key: Arc<str>,
        _value: Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        Ok(())
    }
    async fn delete(&self, _key: &str) -> OxCacheResult<()> {
        Ok(())
    }
    async fn clear(&self) -> OxCacheResult<()> {
        Ok(())
    }
    async fn expire(&self, _key: &str, _ttl: Duration) -> OxCacheResult<bool> {
        Ok(false)
    }
}

#[async_trait]
impl CacheConnector for BadBatchBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        Ok(())
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Mock
    }
}

impl BackendScore for BadBatchBackend {
    fn score(&self) -> u8 {
        5
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        "bad-batch"
    }
}

fn two_failing_chain(strategy: ChainReadStrategy) -> ChainCache {
    ChainCache::builder()
        .link(ChainLink::from_backend(FailingBackend { prefix: "f1" }))
        .link(ChainLink::from_backend(FailingBackend { prefix: "f2" }))
        .read_strategy(strategy)
        .build()
}

#[tokio::test]
async fn race_read_all_failed_propagates_error() {
    let chain = two_failing_chain(ChainReadStrategy::Race);
    let err = chain.get("k").await.unwrap_err();
    assert!(
        err.to_string()
            .contains("All backends failed during race read"),
        "got: {err}"
    );
}

#[tokio::test]
async fn parallel_freshest_all_failed_propagates_error() {
    let chain = two_failing_chain(ChainReadStrategy::ParallelFreshest);
    let err = chain.get("k").await.unwrap_err();
    assert!(
        err.to_string()
            .contains("All backends failed during parallel freshest read"),
        "got: {err}"
    );
}

#[tokio::test]
async fn sequential_read_all_failed_propagates_error() {
    let chain = two_failing_chain(ChainReadStrategy::Sequential);
    let err = chain.get("k").await.unwrap_err();
    // Sequential 语义：逐层尝试后传播最后一次后端错误原文
    assert!(err.to_string().contains("down"), "got: {err}");
}

#[tokio::test]
async fn race_read_hit_on_lower_link_backfills_to_higher() {
    // 高分位 = Moka(100，空)，低分位 = DashMap(90，有值) → Race 命中低分位后回填高分位
    let high = Arc::new(oxcache::backend::memory::MokaMemoryBackend::new());
    let low = Arc::new(oxcache::backend::DashMapMemoryBackend::new());
    use oxcache::backend::CacheReader as _;
    CacheWriter::set(
        low.as_ref(),
        Arc::from("bk"),
        Arc::new(b"from-low".to_vec()),
        Some(Duration::from_secs(60)),
    )
    .await
    .unwrap();

    let chain = ChainCache::builder()
        .link(mem_link(high.clone(), Scores::MOKA, false, "high"))
        .link(mem_link(low.clone(), Scores::DASHMAP, false, "low"))
        .read_strategy(ChainReadStrategy::Race)
        .enable_backfill()
        .build();

    let hit = chain.get("bk").await.unwrap().expect("hit");
    assert_eq!(hit, b"from-low");

    // 回填是异步旁路；轮询等待高分位可见
    let mut backfilled = false;
    for _ in 0..100 {
        if CacheReader::get(high.as_ref(), "bk")
            .await
            .unwrap()
            .is_some()
        {
            backfilled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        backfilled,
        "race hit on lower link must backfill higher link"
    );
}

#[tokio::test]
async fn parallel_freshest_picks_longest_remaining_ttl() {
    // 两个后端都有值：L1 剩余 TTL 短、L2 长 → 择新读应取 L2 的值
    let l1 = Arc::new(oxcache::backend::DashMapMemoryBackend::new());
    let l2 = Arc::new(oxcache::backend::memory::MokaMemoryBackend::new());
    use oxcache::backend::CacheReader as _;
    CacheWriter::set(
        l1.as_ref(),
        Arc::from("fk"),
        Arc::new(b"short-ttl".to_vec()),
        Some(Duration::from_secs(2)),
    )
    .await
    .unwrap();
    CacheWriter::set(
        l2.as_ref(),
        Arc::from("fk"),
        Arc::new(b"long-ttl".to_vec()),
        Some(Duration::from_secs(600)),
    )
    .await
    .unwrap();

    let chain = ChainCache::builder()
        .link(mem_link(l1.clone(), Scores::DASHMAP, false, "l1"))
        .link(mem_link(l2.clone(), Scores::MOKA, false, "l2"))
        .read_strategy(ChainReadStrategy::ParallelFreshest)
        .build();

    let hit = chain.get("fk").await.unwrap().expect("hit");
    assert_eq!(hit, b"long-ttl");
}

#[tokio::test]
async fn get_many_length_violation_degrades_to_next_layer() {
    // L1 返回长度违约（整批失败口径），L2 正常命中 → 值仍然可达
    let l1 = Arc::new(BadBatchBackend);
    let l2 = Arc::new(oxcache::backend::memory::MokaMemoryBackend::new());
    use oxcache::backend::CacheReader as _;
    CacheWriter::set(
        l2.as_ref(),
        Arc::from("dk"),
        Arc::new(b"deep".to_vec()),
        None,
    )
    .await
    .unwrap();

    let chain = ChainCache::builder()
        .link(mem_link(l1, Scores::MEMCACHED, false, "bad-batch"))
        .link(mem_link(l2, Scores::MOKA, false, "l2"))
        .build();

    let result = chain
        .get_many(&["dk".to_string(), "miss".to_string()])
        .await
        .unwrap();
    assert_eq!(result[0].as_deref(), Some(&b"deep"[..]));
    assert_eq!(result[1], None);
}

#[tokio::test]
async fn get_many_batch_error_maps_per_key() {
    // 整批 Err 也按键拆分，后续层兜底
    let l1 = Arc::new(FailingBackend {
        prefix: "batch-fail",
    });
    let l2 = Arc::new(oxcache::backend::memory::MokaMemoryBackend::new());
    use oxcache::backend::CacheReader as _;
    CacheWriter::set(l2.as_ref(), Arc::from("ek"), Arc::new(b"ok".to_vec()), None)
        .await
        .unwrap();

    let chain = ChainCache::builder()
        .link(mem_link(l1, Scores::MEMCACHED, false, "batch-fail"))
        .link(mem_link(l2, Scores::MOKA, false, "l2"))
        .build();

    let result = chain.get_many(&["ek".to_string()]).await.unwrap();
    assert_eq!(result[0].as_deref(), Some(&b"ok"[..]));
}

#[tokio::test]
async fn chain_views_expose_lowest_and_persistent_links() {
    let l1 = Arc::new(oxcache::backend::DashMapMemoryBackend::new());
    let l2 = Arc::new(oxcache::backend::memory::MokaMemoryBackend::new());
    let chain = ChainCache::builder()
        .link(mem_link(l1.clone(), Scores::DASHMAP, false, "l1"))
        .link(mem_link(l2.clone(), Scores::MOKA, false, "l2"))
        .build();

    // DashMap(90) 低于 Moka(100)，分数排序后最末位即 l1
    let lowest = chain.lowest_score_backend().expect("lowest");
    assert_eq!(lowest.name(), "l1");

    // 两个内存后端均非持久化 → 空集
    assert!(chain.persistent_backends().is_empty());
}

// 抑制未使用导入告警（Error 别名用于文档化错误类型来源）
#[allow(unused)]
fn _type_anchor() -> Option<Error> {
    None
}

// ============================================================================
// 回填失败容忍与 Freshest 回填
// ============================================================================

#[tokio::test]
async fn race_backfill_tolerates_higher_link_write_failure() {
    // 高分位 = 恒故障后端（回填写必失败），低分位 = DashMap 有值。
    // 回填失败必须被容忍（不影响读命中），错误按键上报。
    let high: Arc<dyn CacheBackend> = Arc::new(FailingBackend { prefix: "bf-high" });
    let low = Arc::new(oxcache::backend::DashMapMemoryBackend::new());
    use oxcache::backend::CacheReader as _;
    CacheWriter::set(
        low.as_ref(),
        Arc::from("bk2"),
        Arc::new(b"v".to_vec()),
        Some(Duration::from_secs(60)),
    )
    .await
    .unwrap();

    let chain = ChainCache::builder()
        .link(mem_link(high, Scores::REDIS, false, "bf-high"))
        .link(mem_link(low.clone(), Scores::DASHMAP, false, "bf-low"))
        .read_strategy(ChainReadStrategy::Race)
        .enable_backfill()
        .build();

    let hit = chain.get("bk2").await.unwrap().expect("hit from low link");
    assert_eq!(hit, b"v");
}

#[tokio::test]
async fn parallel_freshest_backfills_picked_value_to_higher_links() {
    // 择新读选中低分位（剩余 TTL 更长）后回填高分位
    let high = Arc::new(oxcache::backend::memory::MokaMemoryBackend::new());
    let low = Arc::new(oxcache::backend::DashMapMemoryBackend::new());
    use oxcache::backend::CacheReader as _;
    CacheWriter::set(
        low.as_ref(),
        Arc::from("fk2"),
        Arc::new(b"fresh".to_vec()),
        Some(Duration::from_secs(900)),
    )
    .await
    .unwrap();
    CacheWriter::set(
        high.as_ref(),
        Arc::from("fk2"),
        Arc::new(b"stale".to_vec()),
        Some(Duration::from_secs(1)),
    )
    .await
    .unwrap();

    let chain = ChainCache::builder()
        .link(mem_link(high.clone(), Scores::MOKA, false, "high"))
        .link(mem_link(low.clone(), Scores::DASHMAP, false, "low"))
        .read_strategy(ChainReadStrategy::ParallelFreshest)
        .enable_backfill()
        .build();

    let hit = chain.get("fk2").await.unwrap().expect("hit");
    assert_eq!(hit, b"fresh");

    // 回填后高分位被低分位的更新值覆盖（保留原始 TTL）
    let mut refreshed = false;
    for _ in 0..100 {
        if let Some(v) = CacheReader::get(high.as_ref(), "fk2").await.unwrap()
            && v.as_slice() == b"fresh"
        {
            refreshed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(refreshed, "freshest pick must backfill higher links");
}
