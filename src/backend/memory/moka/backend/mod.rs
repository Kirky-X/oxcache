// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Moka-based memory backend implementation

use crate::backend::interface::{AtomicCacheWriter, CachePatternOps, keys_limit_exceeded};
use crate::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::core::constants::MAX_KEYS_RETURN;
// Sync trait 实现使用全限定路径（`crate::backend::SyncCacheReader`），
// 避免将 sync trait 名导入本模块作用域后，经 `mod tests` 的 `use super::*`
// 与同名 async trait 方法（如 `get`）产生歧义。
use crate::backend::{BackendScore, Scores};
use crate::error::{OxCacheError, OxCacheResult};
use crate::i18n::messages::{MSG_DETAIL_NOT_SUPPORTED_MOKA_SYNC_CURRENT_THREAD, t};
use crate::impl_backend_builder;
use async_trait::async_trait;
use moka::Expiry;
use moka::ops::compute::{CompResult, Op};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Moka 缓存条目：承载 value 与 per-entry 过期时间戳。
///
/// `expires_at=None` 表示该条目无 per-entry TTL（由 [`MokaExpiry`] 以全局 TTL
/// 兜底；未配置全局 TTL 则永不过期）。`Some(Instant)` 表示在该时刻过期。
///
/// 通过 [`MokaExpiry`] 将 `expires_at` 暴露给 moka 淘汰策略，使 moka 在
/// `expire_after_create` / `expire_after_update` 时知道真实过期时间。
#[derive(Clone, Debug)]
pub(crate) struct MokaEntry {
    pub(crate) value: Vec<u8>,
    pub(crate) expires_at: Option<Instant>,
}

/// [`Expiry`] 实现：把 [`MokaEntry`] 的 `expires_at` 转换为 moka 期望的
/// "从创建/更新时刻起的剩余 Duration"。
///
/// `expires_at=None` 时回退到 `global_ttl`（对应 `builder.ttl(...)` 的全局
/// TTL：`set(ttl=None)` 沿用全局，`set(ttl=Some(_))` 覆盖全局）。全局 TTL
/// 必须在这里兜底而不能经 moka 的 `time_to_live` 配置——moka 对自定义
/// `Expiry` 与 `time_to_live` 并存时按 `last_modified + ttl` 无条件封顶
/// 每个条目，会使 per-entry TTL 无法长于全局 TTL，违反 README
/// "全局 TTL 被 per-entry TTL 覆盖" 的跨后端契约。
///
/// `expire_after_read` 使用默认实现（返回 `duration_until_expiry`，不变更过期），
/// 保证读操作不会意外延长或缩短 TTL。
#[derive(Default, Clone)]
pub(crate) struct MokaExpiry {
    global_ttl: Option<Duration>,
}

impl MokaExpiry {
    pub(crate) fn new(global_ttl: Option<Duration>) -> Self {
        Self { global_ttl }
    }
}

impl Expiry<Arc<str>, MokaEntry> for MokaExpiry {
    fn expire_after_create(
        &self,
        _key: &Arc<str>,
        val: &MokaEntry,
        created_at: Instant,
    ) -> Option<Duration> {
        val.expires_at
            .map(|e| e.saturating_duration_since(created_at))
            .or(self.global_ttl)
    }

    fn expire_after_update(
        &self,
        _key: &Arc<str>,
        val: &MokaEntry,
        updated_at: Instant,
        _duration_until_expiry: Option<Duration>,
    ) -> Option<Duration> {
        val.expires_at
            .map(|e| e.saturating_duration_since(updated_at))
            .or(self.global_ttl)
    }
}

/// Moka-based memory backend
///
/// This backend uses Moka's high-performance in-memory cache with
/// LRU/TinyLFU eviction policies and built-in TTL support.
#[derive(Clone)]
pub struct MokaMemoryBackend {
    cache: Arc<moka::future::Cache<Arc<str>, MokaEntry>>,
    capacity: u64,
}

impl_backend_builder!(MokaMemoryBackend, MokaMemoryBackendBuilder);

impl MokaMemoryBackend {
    /// Get the capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get the entry count
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }
}

impl Default for MokaMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MokaMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaMemoryBackend")
            .field("capacity", &self.capacity)
            .field("entry_count", &self.cache.entry_count())
            .finish()
    }
}

#[async_trait]
impl CacheReader for MokaMemoryBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(self.cache.get(key).await.map(|e| e.value))
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(self.cache.contains_key(key))
    }

    // get() refreshes the entry's access time, so with `time_to_idle`
    // configured this query postpones idle eviction — moka exposes no
    // non-refreshing read that returns the remaining TTL.
    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let now = Instant::now();
        Ok(self
            .cache
            .get(key)
            .await
            .and_then(|e| e.expires_at.and_then(|exp| exp.checked_duration_since(now))))
    }

    async fn len(&self) -> OxCacheResult<u64> {
        // moka entry_count 为惰性维护的估计值，写入后直读可能拿到过期计数（甚至 0）；
        // 读前先执行待维护任务保证最终一致（与 sync 侧 ByteWeightCache::entry_count 同口径）
        self.cache.run_pending_tasks().await;
        Ok(self.cache.entry_count())
    }

    async fn is_empty(&self) -> OxCacheResult<bool> {
        self.cache.run_pending_tasks().await;
        Ok(self.cache.entry_count() == 0)
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(self.capacity)
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        self.cache.run_pending_tasks().await;
        stats.insert("type".to_string(), "moka".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert(
            "entry_count".to_string(),
            self.cache.entry_count().to_string(),
        );
        Ok(stats)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        // 多采一条用于判定是否超限：恰好 MAX_KEYS_RETURN 条匹配是允许的，
        // 超过才拒绝（与 redis / disk 后端的判定口径一致）
        crate::security::validate_glob_pattern(pattern)?;
        let keys = self.collect_matching(pattern, MAX_KEYS_RETURN + 1).await;
        // 键空间可达百万级：一次性物化全部匹配键会打满调用方内存
        if keys.len() > MAX_KEYS_RETURN {
            return Err(keys_limit_exceeded(MAX_KEYS_RETURN));
        }
        Ok(keys)
    }
}

#[async_trait]
impl CacheWriter for MokaMemoryBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let entry = MokaEntry {
            value: (*value).clone(),
            expires_at,
        };
        self.cache.insert(key, entry).await;
        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.cache.invalidate(key).await;
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let new_expires_at = Instant::now() + ttl;
        let key_arc: Arc<str> = Arc::from(key);
        let result = self
            .cache
            .entry(key_arc)
            .and_compute_with(
                |maybe_entry: Option<moka::Entry<Arc<str>, MokaEntry>>| async move {
                    match maybe_entry {
                        Some(entry) => {
                            let mut old = entry.into_value();
                            old.expires_at = Some(new_expires_at);
                            Op::Put(old)
                        }
                        None => Op::Nop,
                    }
                },
            )
            .await;
        match result {
            CompResult::ReplacedWith(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

#[async_trait]
impl CacheConnector for MokaMemoryBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        // Moka is always healthy as it's in-memory
        Ok(())
    }

    async fn shutdown(&self) {
        self.cache.invalidate_all();
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Moka
    }

    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        Some(self)
    }

    fn as_pattern_ops(&self) -> Option<&dyn CachePatternOps> {
        Some(self)
    }
}

/// 模式级操作的批量删除粒度：逐条 `invalidate` 会放大 moka 内部锁竞争，
/// 按批删除把锁持有次数降到批数次。
const PATTERN_DELETE_BATCH: usize = 512;

#[async_trait]
impl CachePatternOps for MokaMemoryBackend {
    async fn pattern_count(&self, pattern: &str) -> OxCacheResult<u64> {
        crate::security::validate_glob_pattern(pattern)?;
        let mut count = 0u64;
        for (key, _entry) in self.cache.iter() {
            if crate::backend::interface::glob_match(pattern, key.as_ref()) {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn pattern_delete(&self, pattern: &str) -> OxCacheResult<u64> {
        crate::security::validate_glob_pattern(pattern)?;
        let mut deleted = 0u64;
        let mut batch: Vec<String> = Vec::new();
        for (key, _entry) in self.cache.iter() {
            if !crate::backend::interface::glob_match(pattern, key.as_ref()) {
                continue;
            }
            batch.push(key.as_ref().to_string());
            if batch.len() >= PATTERN_DELETE_BATCH {
                for pending in batch.drain(..) {
                    self.cache.invalidate(pending.as_str()).await;
                }
                deleted += PATTERN_DELETE_BATCH as u64;
            }
        }
        let remaining = batch.len();
        for pending in batch.drain(..) {
            self.cache.invalidate(pending.as_str()).await;
        }
        deleted += remaining as u64;
        Ok(deleted)
    }
}

// Synchronous trait implementations
//
// Moka 0.12 的 `future::Cache` 未暴露 `blocking_*` 方法，但 `get`/`insert`/
// `invalidate` 的前台 future 不依赖 tokio runtime 驱动（无 `tokio::spawn`/
// `tokio::time` 调用），可通过 `block_on` 安全轮询。`sync_block_on` 的三种
// 执行环境：multi-thread runtime 内复用当前 runtime（`block_in_place`）；
// runtime 外创建临时 current-thread runtime 驱动（确保 waker 正确注册）；
// current-thread runtime 的异步上下文内——tokio 禁止任何嵌套阻塞驱动
// （临时 runtime 亦被 context 检查拒绝），唯一出路是显性报错而非 panic。

/// 驱动 future 至完成；失败（仅 current-thread 异步上下文）显性报错。
///
/// - 已有 multi-thread runtime：`block_in_place` + `handle.block_on`（安全）。
/// - 无 runtime：创建临时 current-thread runtime（~1μs 量级开销）。
/// - current-thread runtime 的异步上下文内：返回 `Err(NotSupported)`——
///   此前该分支直接 `handle.block_on` 会触发 tokio 的
///   "Cannot start a runtime from within a runtime" panic；改为显性错误，
///   指引 multi-thread runtime 或 runtime 外调用。
fn sync_block_on<F: std::future::Future>(fut: F) -> OxCacheResult<F::Output> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            // Multi-thread runtime: use block_in_place to safely block
            Ok(tokio::task::block_in_place(|| handle.block_on(fut)))
        }
        Ok(_) => Err(OxCacheError::NotSupported(t(
            MSG_DETAIL_NOT_SUPPORTED_MOKA_SYNC_CURRENT_THREAD,
            &[],
        ))),
        Err(_) => {
            // No runtime: create a temporary current_thread runtime.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create temporary tokio runtime for sync_block_on");
            Ok(rt.block_on(fut))
        }
    }
}

impl crate::backend::interface::SyncCacheReader for MokaMemoryBackend {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        Ok(sync_block_on(self.cache.get(key))?.map(|e| e.value))
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        Ok(self.cache.contains_key(key))
    }

    // Same TTI-refresh side effect as the async ttl() — see comment above.
    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let now = Instant::now();
        Ok(sync_block_on(self.cache.get(key))?
            .and_then(|e| e.expires_at.and_then(|exp| exp.checked_duration_since(now))))
    }

    fn len(&self) -> OxCacheResult<u64> {
        sync_block_on(self.cache.run_pending_tasks())?;
        Ok(self.cache.entry_count())
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(self.capacity)
    }

    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        sync_block_on(self.cache.run_pending_tasks())?;
        stats.insert("type".to_string(), "moka".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert(
            "entry_count".to_string(),
            self.cache.entry_count().to_string(),
        );
        Ok(stats)
    }
}

impl crate::backend::interface::SyncCacheWriter for MokaMemoryBackend {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let entry = MokaEntry {
            value: (*value).clone(),
            expires_at,
        };
        sync_block_on(self.cache.insert(key, entry))?;
        Ok(())
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        sync_block_on(self.cache.invalidate(key))?;
        Ok(())
    }

    fn clear(&self) -> OxCacheResult<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let new_expires_at = Instant::now() + ttl;
        let key_arc: Arc<str> = Arc::from(key);
        let result = sync_block_on(self.cache.entry(key_arc).and_compute_with(
            |maybe_entry: Option<moka::Entry<Arc<str>, MokaEntry>>| async move {
                match maybe_entry {
                    Some(entry) => {
                        let mut old = entry.into_value();
                        old.expires_at = Some(new_expires_at);
                        Op::Put(old)
                    }
                    None => Op::Nop,
                }
            },
        ))?;
        match result {
            CompResult::ReplacedWith(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

impl crate::backend::interface::SyncCacheConnector for MokaMemoryBackend {
    fn health_check(&self) -> OxCacheResult<()> {
        // Moka is always healthy as it's in-memory
        Ok(())
    }

    fn shutdown(&self) {
        self.cache.invalidate_all();
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Moka
    }

    fn as_sync_atomic_writer(
        &self,
    ) -> Option<&dyn crate::backend::interface::SyncAtomicCacheWriter> {
        Some(self)
    }
}

impl crate::backend::interface::SyncAtomicCacheWriter for MokaMemoryBackend {
    fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        sync_block_on(AtomicCacheWriter::incr(self, key, delta, ttl))?
    }

    fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        sync_block_on(AtomicCacheWriter::compare_and_swap(
            self, key, expected, new, ttl,
        ))?
    }

    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        sync_block_on(AtomicCacheWriter::set_if_absent(self, key, value, ttl))?
    }
}

// CacheBackend is automatically implemented via blanket implementation

impl BackendScore for MokaMemoryBackend {
    fn score(&self) -> u8 {
        Scores::MOKA
    }

    fn is_persistent(&self) -> bool {
        false
    }

    fn backend_name(&self) -> &'static str {
        "moka"
    }
}

// AtomicCacheWriter Implementation

#[async_trait]
impl AtomicCacheWriter for MokaMemoryBackend {
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        let key_arc: Arc<str> = Arc::from(key);
        let expires_at = ttl.map(|d| Instant::now() + d);

        let result = self
            .cache
            .entry(key_arc.clone())
            .and_compute_with(
                |maybe_entry: Option<moka::Entry<Arc<str>, MokaEntry>>| async move {
                    let current_val = match maybe_entry {
                        Some(entry) => {
                            let old = entry.into_value();
                            // Parse existing value as i64, return Nop if invalid
                            match String::from_utf8(old.value) {
                                Ok(s) => match s.parse::<i64>() {
                                    Ok(v) => v,
                                    Err(_) => return Op::Nop,
                                },
                                Err(_) => return Op::Nop,
                            }
                        }
                        None => 0,
                    };
                    let new_val = match current_val.checked_add(delta) {
                        Some(v) => v,
                        None => {
                            // Overflow: do not modify the entry, return Nop
                            return Op::Nop;
                        }
                    };
                    Op::Put(MokaEntry {
                        value: new_val.to_string().into_bytes(),
                        expires_at,
                    })
                },
            )
            .await;

        match result {
            CompResult::Inserted(entry) | CompResult::ReplacedWith(entry) => {
                let val_str = String::from_utf8(entry.value().value.clone()).map_err(|e| {
                    crate::error::OxCacheError::Operation(format!(
                        "incr: invalid UTF-8 in stored value: {}",
                        e
                    ))
                })?;
                val_str.parse::<i64>().map_err(|e| {
                    crate::error::OxCacheError::Operation(format!(
                        "incr: invalid integer in stored value: {}",
                        e
                    ))
                })
            }
            // Op::Nop → Unchanged (entry existed, not modified) or StillNone (no entry)
            CompResult::Unchanged(_) | CompResult::StillNone(_) => Err(
                crate::error::OxCacheError::Operation("incr: i64 overflow".to_string()),
            ),
            _ => Err(crate::error::OxCacheError::Operation(
                "incr: unexpected compute result".to_string(),
            )),
        }
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let key_arc: Arc<str> = Arc::from(key);
        let expires_at = ttl.map(|d| Instant::now() + d);
        let expected_owned = expected.map(|b| b.to_vec());
        let new_clone = new.clone();

        let result = self
            .cache
            .entry(key_arc)
            .and_compute_with(
                |maybe_entry: Option<moka::Entry<Arc<str>, MokaEntry>>| async move {
                    match &expected_owned {
                        None => {
                            // SETNX: set only if key doesn't exist
                            if maybe_entry.is_none() {
                                Op::Put(MokaEntry {
                                    value: new_clone,
                                    expires_at,
                                })
                            } else {
                                Op::Nop
                            }
                        }
                        Some(exp_bytes) => {
                            // CAS: set only if current value matches
                            match &maybe_entry {
                                Some(entry) if entry.value().value == *exp_bytes => {
                                    Op::Put(MokaEntry {
                                        value: new_clone,
                                        expires_at,
                                    })
                                }
                                _ => Op::Nop,
                            }
                        }
                    }
                },
            )
            .await;

        match result {
            CompResult::Inserted(_) | CompResult::ReplacedWith(_) => Ok(true),
            _ => Ok(false),
        }
    }

    async fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let key_arc: Arc<str> = Arc::from(key);
        let expires_at = ttl.map(|d| Instant::now() + d);

        let result = self
            .cache
            .entry(key_arc)
            .and_compute_with(
                |maybe_entry: Option<moka::Entry<Arc<str>, MokaEntry>>| async move {
                    if maybe_entry.is_none() {
                        Op::Put(MokaEntry { value, expires_at })
                    } else {
                        Op::Nop
                    }
                },
            )
            .await;

        match result {
            CompResult::Inserted(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

// Override keys() for CacheReader
impl MokaMemoryBackend {
    /// List keys matching a glob pattern.
    ///
    /// 结果条数上限为 [`MAX_KEYS_RETURN`]：超过上限后停止收集，
    /// 避免一次性物化全量键拖垮调用方内存。**截断是静默的**——本方法
    /// 不返回错误，无法告知调用方结果是否被截断；需要判定截断的调用方
    /// 改用 [`try_keys_matching`](Self::try_keys_matching)。
    ///
    /// 模式长度超限时返回空列表（本方法无错误通道，静默语义与截断一致）；
    /// 需要区分「无匹配」与「模式非法」的调用方改用
    /// [`try_keys_matching`](Self::try_keys_matching)。
    pub async fn keys_matching(&self, pattern: &str) -> Vec<String> {
        if crate::security::validate_glob_pattern(pattern).is_err() {
            return Vec::new();
        }
        self.collect_matching(pattern, MAX_KEYS_RETURN).await
    }

    /// List keys matching a glob pattern，超出上限时报错。
    ///
    /// 与 [`keys_matching`](Self::keys_matching) 的差别：本方法在匹配键数
    /// 超过 [`MAX_KEYS_RETURN`] 时返回错误（恰好等于上限时正常返回），
    /// 调用方据此判断结果是否完整。模式长度超限时同样返回错误。
    pub async fn try_keys_matching(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        crate::security::validate_glob_pattern(pattern)?;
        let keys = self.collect_matching(pattern, MAX_KEYS_RETURN + 1).await;
        if keys.len() > MAX_KEYS_RETURN {
            return Err(keys_limit_exceeded(MAX_KEYS_RETURN));
        }
        Ok(keys)
    }

    /// 收集至多 `cap` 条匹配键（`cap` 用于多采一条判定超限）。
    async fn collect_matching(&self, pattern: &str, cap: usize) -> Vec<String> {
        let mut keys = Vec::new();
        for (key_arc, _entry) in self.cache.iter() {
            if keys.len() >= cap {
                break;
            }
            let key_str: &str = key_arc.as_ref();
            if crate::backend::interface::glob_match(pattern, key_str) {
                keys.push(key_str.to_string());
            }
        }
        keys
    }
}

/// Builder for MokaMemoryBackend
#[derive(Default)]
pub struct MokaMemoryBackendBuilder {
    capacity: u64,
    max_capacity_bytes: Option<u64>,
    ttl: Option<Duration>,
    time_to_idle: Option<Duration>,
}

impl MokaMemoryBackendBuilder {
    /// Set the maximum number of entries
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the byte budget for the cache.
    ///
    /// 条目数上限在大值场景下会内存超卖（单值上限 5MB × 10k 条目
    /// 理论可达 50GB）。设置后 moka `max_capacity` 单位切换为字节（weigher 按
    /// value 字节长计权），条目数隐式受"每条至少 1 字节"约束；`capacity`
    /// 被忽略。未设置（默认）时行为与现状一致。
    pub fn max_capacity_bytes(mut self, max_capacity_bytes: u64) -> Self {
        self.max_capacity_bytes = Some(max_capacity_bytes.max(1));
        self
    }

    /// Set the time-to-live for entries
    ///
    /// 全局 TTL：`set(ttl=None)` 的条目沿用该值过期；`set(ttl=Some(d))` 的
    /// 条目覆盖全局（`d` 可长于全局 TTL）。
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the time-to-idle for entries
    pub fn time_to_idle(mut self, ttl: Duration) -> Self {
        self.time_to_idle = Some(ttl);
        self
    }

    /// Build the Moka backend
    pub fn build(self) -> MokaMemoryBackend {
        // 字节预算优先：weigher 按 value 字节长计权，容量单位切换为字节；
        // 未设置时按条目数（默认 10_000）
        let (capacity, weigher) = match self.max_capacity_bytes {
            Some(bytes) => (bytes, true),
            None => (
                if self.capacity > 0 {
                    self.capacity
                } else {
                    10_000
                },
                false,
            ),
        };

        // 全局 TTL 经 MokaExpiry 兜底（见 MokaExpiry 文档），不走 moka 的
        // time_to_live——它与自定义 Expiry 并存时会无条件封顶每个条目，
        // 破坏"per-entry TTL 覆盖全局 TTL"契约。
        let mut builder = moka::future::Cache::builder()
            .max_capacity(capacity)
            .expire_after(MokaExpiry::new(self.ttl));

        if weigher {
            builder = builder.weigher(|_k: &Arc<str>, v: &MokaEntry| {
                v.value.len().min(u32::MAX as usize) as u32
            });
        }

        if let Some(tti) = self.time_to_idle {
            builder = builder.time_to_idle(tti);
        }

        let cache = Arc::new(builder.build());

        MokaMemoryBackend { cache, capacity }
    }
}

/// Convenience function to create a Moka memory backend
pub fn moka_memory() -> MokaMemoryBackend {
    MokaMemoryBackend::new()
}

/// Convenience function to create a Moka memory backend with capacity
pub fn moka_memory_with_capacity(capacity: u64) -> MokaMemoryBackend {
    MokaMemoryBackend::builder().capacity(capacity).build()
}

/// Convenience function to create a Moka memory backend with capacity and TTL
pub fn moka_memory_with_capacity_and_ttl(capacity: u64, ttl: Duration) -> MokaMemoryBackend {
    MokaMemoryBackend::builder()
        .capacity(capacity)
        .ttl(ttl)
        .build()
}

/// Default memory backend (Moka-based)
pub fn default_memory_backend() -> MokaMemoryBackend {
    moka_memory()
}

#[cfg(test)]
mod tests;
