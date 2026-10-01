// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 链式缓存核心实现
//
// ChainCache 提供多后端链式访问，按分数从高到低遍历后端。
// 读取时从高分后端开始，写入时写入所有后端。

use crate::backend::BackendScore;
use crate::backend::{
    AtomicCacheWriter, BackendKind, CacheBackend, CacheConnector, CacheReader, CacheWriter,
    SyncCacheBackend,
};
use crate::core::EventPublisher;
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "metrics")]
use crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

// ---- telemetry helpers (zero overhead when `telemetry` feature off) ----

#[cfg(feature = "telemetry")]
#[inline]
fn oxcache_telemetry_backfill_ok(key: &str, backend: &str) {
    tracing::debug!(
        target = "oxcache::chain",
        key,
        backend,
        "backfill succeeded"
    );
}

#[cfg(feature = "telemetry")]
#[inline]
fn oxcache_telemetry_backfill_failed(key: &str, backend: &str, err: &OxCacheError) {
    tracing::warn!(target = "oxcache::chain", key, backend, %err, "backfill failed");
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn oxcache_telemetry_backfill_ok(_key: &str, _backend: &str) {}

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_read_strategy(key: &str, strategy: &str, backend: &str, hit: bool) {
    tracing::debug!(
        target = "oxcache::chain",
        key,
        strategy,
        backend,
        hit,
        "chain read completed"
    );
}

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_expire_backend_failed(key: &str, backend: &str, err: &OxCacheError) {
    tracing::warn!(
        target = "oxcache::chain",
        key,
        backend,
        %err,
        "backend expire failed"
    );
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_expire_backend_failed(_key: &str, _backend: &str, _err: &OxCacheError) {}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_read_strategy(_key: &str, _strategy: &str, _backend: &str, _hit: bool) {}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn oxcache_telemetry_backfill_failed(_key: &str, _backend: &str, _err: &OxCacheError) {}

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_iter_entries_key_failed(key: &str, backend: &str, err: &OxCacheError) {
    tracing::warn!(
        target = "oxcache::chain",
        key,
        backend,
        %err,
        "iter_entries batch layer failed for key"
    );
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_iter_entries_key_failed(_key: &str, _backend: &str, _err: &OxCacheError) {}

// Submodules
mod builder;
#[cfg(test)]
mod tests;

// Re-exports from submodules
pub use self::builder::ChainCacheBuilder;

/// 链式缓存中的一个后端链接
///
/// ChainLink 封装了一个后端实例及其分数信息。
/// 分数用于确定链式访问的顺序。
/// 链路读策略。
///
/// - `Sequential`：按分数降序逐个读取，命中即返回（默认）
/// - `Race`：并发查询全部链接，全部完成后取 index 最小（分数最高）的命中
///   （原 `enable_race_read()` 行为）
/// - `ParallelFreshest`：并发查询全部链接，命中者并发查询剩余 TTL，
///   返回剩余最长（最新鲜）者；并列或全 None 时取 index 最小
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChainReadStrategy {
    #[default]
    Sequential,
    Race,
    ParallelFreshest,
}

#[derive(Clone)]
pub struct ChainLink {
    /// 后端实例（async trait object）
    backend: Arc<dyn CacheBackend>,
    /// 同步后端实例（可选，当后端实现 SyncCacheBackend 时填充）
    backend_sync: Option<Arc<dyn SyncCacheBackend>>,
    /// 后端分数（越高越快）
    score: u8,
    /// 是否为持久化后端
    is_persistent: bool,
    /// 后端名称
    name: &'static str,
}

impl ChainLink {
    /// 创建新的链式链接
    pub fn new<B>(backend: B, score: u8, is_persistent: bool, name: &'static str) -> Self
    where
        B: CacheBackend + BackendScore + 'static,
    {
        Self {
            backend: Arc::new(backend),
            backend_sync: None,
            score,
            is_persistent,
            name,
        }
    }

    /// 从实现了 BackendScore 的后端创建链接
    pub fn from_backend<B>(backend: B) -> Self
    where
        B: CacheBackend + BackendScore + 'static,
    {
        let score = backend.score();
        let is_persistent = backend.is_persistent();
        let name = backend.backend_name();
        Self {
            backend: Arc::new(backend),
            backend_sync: None,
            score,
            is_persistent,
            name,
        }
    }

    /// 从实现了 SyncCacheBackend 的后端创建链接（同时支持 async 与 sync API）
    ///
    /// 与 `from_backend` 不同，此构造函数要求后端同时实现 `SyncCacheBackend`，
    /// 会同时填充 `backend`（async trait object）和 `backend_sync`（sync trait object），
    /// 使该链接可参与 `ChainCache` 的 sync API。
    pub fn from_sync_backend<B>(backend: B) -> Self
    where
        B: CacheBackend + BackendScore + SyncCacheBackend + 'static,
    {
        let score = backend.score();
        let is_persistent = backend.is_persistent();
        let name = backend.backend_name();
        let arc = Arc::new(backend);
        // 显式标注 sync_arc 类型以触发 unsized coercion（Option 不传播 coercion）
        let sync_arc: Arc<dyn SyncCacheBackend> = arc.clone();
        Self {
            backend: arc,
            backend_sync: Some(sync_arc),
            score,
            is_persistent,
            name,
        }
    }

    /// 从已擦除的后端 trait 对象创建链接（分层构建器使用）
    ///
    /// 分数/持久化标志/名称由调用方提供（装饰器包装后的 `Arc<dyn CacheBackend>`
    /// 无法再查询 [`BackendScore`]）。
    pub fn from_arc(
        backend: Arc<dyn CacheBackend>,
        score: u8,
        is_persistent: bool,
        name: &'static str,
    ) -> Self {
        Self {
            backend,
            backend_sync: None,
            score,
            is_persistent,
            name,
        }
    }

    /// 获取后端实例引用
    pub fn backend(&self) -> &Arc<dyn CacheBackend> {
        &self.backend
    }

    /// 尝试获取同步后端实例
    ///
    /// 返回 `Some` 当且仅当该链接通过 `from_sync_backend` 创建（或后端同时实现
    /// `SyncCacheBackend`）。返回 `None` 表示该链接不支持 sync API。
    pub fn try_as_sync_backend(&self) -> Option<Arc<dyn SyncCacheBackend>> {
        self.backend_sync.clone()
    }

    /// 获取后端分数
    pub fn score(&self) -> u8 {
        self.score
    }

    /// 是否为持久化后端
    pub fn is_persistent(&self) -> bool {
        self.is_persistent
    }

    /// 获取后端名称
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl std::fmt::Debug for ChainLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainLink")
            .field("score", &self.score)
            .field("is_persistent", &self.is_persistent)
            .field("name", &self.name)
            .finish()
    }
}

/// 链式缓存
///
/// ChainCache 管理多个后端，按分数从高到低排序。
/// 读取时从高分后端开始，找到即返回；写入时写入所有后端。
///
/// # TTL 行为契约
///
/// ChainCache 不存储 TTL，所有 TTL 操作透传给链中后端。契约如下：
///
/// - **`set(key, value, ttl=Some(d))`**：所有链接用同一 TTL `d`（透传）
/// - **`set(key, value, ttl=None)`**：所有链接用 `default_ttl.or(None)`；
///   `default_ttl=None` 时各链接使用自己的全局 TTL（如 Moka 的 `time_to_live`）
/// - **`ttl(key)`**：遍历链接（按分数从高到低），返回首个 `Some(ttl)`；
///   即"最高分链接的剩余 TTL"。所有链接都返回 `None` 时（key 不存在或无
///   per-entry TTL）返回 `None`
/// - **`expire(key, ttl)`**：透传给所有链接，任一返回 `Ok(true)` 则返回
///   `Ok(true)`；所有链接都返回 `Ok(false)`（key 不存在）才返回 `Ok(false)`
///
/// `default_ttl` 优先级：`set(ttl=Some) > default_ttl > 各后端自己的全局 TTL`
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::cache::{ChainCache, ChainLink};
/// use oxcache::backend::MokaMemoryBackend;
///
/// let l1 = MokaMemoryBackend::builder().capacity(10000).ttl(Duration::from_secs(300)).build();
/// let l2 = oxcache::backend::RedisBackend::builder().ttl(Duration::from_secs(3600)).build().await?;
///
/// let chain = ChainCache::builder()
///     .link(ChainLink::from_backend(l1))  // L1: 5分钟 TTL
///     .link(ChainLink::from_backend(l2))  // L2: 1小时 TTL
///     .enable_backfill()
///     .build();
///
/// chain.set("key", value, None).await?;  // L1 用 5分钟，L2 用 1小时
/// ```
pub struct ChainCache {
    /// 后端链接列表（按分数降序排列）
    links: Vec<ChainLink>,
    /// 是否启用回填
    backfill_enabled: bool,
    /// 链路读策略（默认 Sequential；原 race_read_enabled 布尔的枚举化）
    read_strategy: ChainReadStrategy,
    /// 默认 TTL
    default_ttl: Option<Duration>,
    /// 懒缓存的 sync backend 收集结果（问题 4.4）：
    /// 链构建后 links 不可变，首次收集后复用，避免每次 sync 调用重复 clone 所有 Arc。
    sync_backends: OnceLock<Option<Vec<Arc<dyn SyncCacheBackend>>>>,
    /// 事件发布器（可选），用于抛出后端错误事件而非日志输出
    event_publisher: Option<Arc<dyn EventPublisher>>,
}

impl ChainCache {
    /// 创建新的链式缓存
    ///
    /// 与 [`ChainCacheBuilder::build`] 不同，此显式构造器允许空链
    /// （运行时语义：`get` 返回 `None`、`set` 返回错误）。
    pub fn new(links: Vec<ChainLink>) -> Self {
        // 按分数降序排序（与 builder::build 保持一致）
        let mut links = links;
        links.sort_by_key(|link| std::cmp::Reverse(link.score()));

        ChainCache {
            links,
            backfill_enabled: false,
            read_strategy: ChainReadStrategy::Sequential,
            default_ttl: None,
            sync_backends: OnceLock::new(),
            event_publisher: None,
        }
    }

    /// 创建链式缓存构建器
    pub fn builder() -> ChainCacheBuilder {
        ChainCacheBuilder::default()
    }

    /// 获取后端链接列表
    pub fn links(&self) -> &[ChainLink] {
        &self.links
    }

    /// 获取后端数量
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// 获取指定分数的后端
    pub fn get_by_score(&self, score: u8) -> Option<&ChainLink> {
        self.links.iter().find(|link| link.score() == score)
    }

    /// 获取最高分后端
    pub fn highest_score_backend(&self) -> Option<&ChainLink> {
        self.links.first()
    }

    /// 抛出后端错误事件（不中断流程）
    ///
    /// 通过 `EventPublisher` 发射后端操作失败事件，调用方自行决定处理方式
    /// （日志、metrics、告警或忽略）。未配置 publisher 时为零开销 no-op。
    fn emit_backend_error(&self, key: &str, backend: &str, error: &OxCacheError) {
        if let Some(publisher) = &self.event_publisher {
            let _ = publisher.publish_error(
                Some(key.to_string()),
                format!("backend {}: {}", backend, error),
            );
        }
    }

    /// 获取最低分后端
    pub fn lowest_score_backend(&self) -> Option<&ChainLink> {
        self.links.last()
    }

    /// 异步写入：写入所有链接，透传 TTL（公开 API，内部转为 `Arc` 零拷贝分发）。
    ///
    /// key/value 在 `CacheWriter::set` trait 层以 `Arc` 共享所有权（问题 2.2 / 2.3），
    /// 本方法作为用户入口保持 `&str` / `Vec<u8>` 签名，仅做一次 Arc 装箱。
    pub async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let key = Arc::from(key);
        let value = Arc::new(value);
        CacheWriter::set(self, key, value, ttl).await
    }

    /// 批量读取：命中层单层批量读（公开 API）。
    ///
    /// 按分数从高到低逐层处理尚未命中的键，每层一次
    /// [`CacheReader::get_many`] 调用（`RedisBackend` 的 `get_many` 已是
    /// pipeline 批量读，见 `backend/memory/redis/pipeline.rs`）；命中键由
    /// 最高分命中层应答。与逐键 [`ChainCache::get`] 的差异：每层网络
    /// 往返从 N 次降为 1 次。
    ///
    /// 失败语义：
    /// - 某层批量读**整批失败**时，错误**按键拆分映射**——每个未解析键
    ///   独立记录错误（事件发布 + warn），键继续降级到下一层；不向上
    ///   传播 `Err`；
    /// - 全部层处理完后仍未命中的键以 `None` 结束（miss 与失败同形，
    ///   失败路径额外有逐键 warn）。
    ///
    /// 输出顺序与输入 `keys` 一致；不触发回填（批量读为观测/预取语义）；
    /// 逐键 `get`/`keys` API 与各后端 trait 实现不受影响。
    pub async fn iter_entries(&self, keys: &[&str]) -> Vec<(String, Option<Vec<u8>>)> {
        if keys.is_empty() {
            return Vec::new();
        }

        let mut values: Vec<Option<Vec<u8>>> = vec![None; keys.len()];
        // 尚未命中的键下标；某层整批失败时原样保留（降级到下一层）
        let mut pending: Vec<usize> = (0..keys.len()).collect();

        for link in &self.links {
            if pending.is_empty() {
                break;
            }

            let batch_keys: Vec<String> = pending.iter().map(|&i| keys[i].to_string()).collect();

            match link.backend().get_many(&batch_keys).await {
                // 长度必须与请求键数一致（trait 契约）；错位即整批失败口径
                Ok(results) if results.len() == batch_keys.len() => {
                    let mut still_pending = Vec::with_capacity(pending.len());
                    for (i, result) in std::mem::take(&mut pending).into_iter().zip(results) {
                        match result {
                            Some(value) => values[i] = Some(value),
                            None => still_pending.push(i),
                        }
                    }
                    pending = still_pending;
                }
                Ok(results) => {
                    // 返回长度与请求数不符：无法按键对位，若按 zip 截断处理
                    // 被截断键会静默按 miss 收尾（失败被吞）——按整批失败
                    // 显性化，未解析键继续降级到下一层
                    let e = OxCacheError::Operation(format!(
                        "get_many returned {} results for {} keys",
                        results.len(),
                        batch_keys.len()
                    ));
                    for &i in &pending {
                        self.emit_backend_error(keys[i], link.name(), &e);
                        telemetry_iter_entries_key_failed(keys[i], link.name(), &e);
                    }
                }
                Err(e) => {
                    // 整批错误按键拆分映射：每键独立上报
                    for &i in &pending {
                        self.emit_backend_error(keys[i], link.name(), &e);
                        telemetry_iter_entries_key_failed(keys[i], link.name(), &e);
                    }
                }
            }
        }

        keys.iter().map(|k| k.to_string()).zip(values).collect()
    }

    /// 获取所有持久化后端
    pub fn persistent_backends(&self) -> Vec<&ChainLink> {
        self.links
            .iter()
            .filter(|link| link.is_persistent())
            .collect()
    }

    /// 获取所有非持久化后端
    pub fn non_persistent_backends(&self) -> Vec<&ChainLink> {
        self.links
            .iter()
            .filter(|link| !link.is_persistent())
            .collect()
    }

    /// 从链中读取数据
    ///
    /// 单个后端失败时记录 warn 日志（问题 5.1），并继续尝试下一个后端（L1 失败降级到 L2）。
    /// 若启用了竞速读（race_read），则并发查询所有后端并返回最先命中者。
    /// 所有后端都失败时返回 `Err`（与竞速读语义一致）。
    async fn read_from_chain(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        match self.read_strategy {
            ChainReadStrategy::Race => return self.race_read_from_chain(key).await,
            ChainReadStrategy::ParallelFreshest => {
                return self.parallel_freshest_read(key).await;
            }
            ChainReadStrategy::Sequential => {}
        }

        let mut all_failed = true;
        let mut last_err: Option<OxCacheError> = None;

        for (index, link) in self.links.iter().enumerate() {
            match link.backend().get(key).await {
                Ok(Some(value)) => {
                    // 回填到更高分后端
                    if self.backfill_enabled && index > 0 {
                        // 查询原始 TTL 并在回填时保留
                        let original_ttl =
                            self.links[index].backend().ttl(key).await.ok().flatten();
                        // 将 value 移入 Arc 后直接用于回填和返回，避免额外 clone
                        let value = Arc::new(value);
                        self.backfill_to_higher_backends(
                            Arc::from(key),
                            value.clone(),
                            index,
                            original_ttl,
                        )
                        .await;
                        telemetry_read_strategy(key, "sequential", self.links[index].name(), true);
                        return Ok(Some(
                            Arc::try_unwrap(value).unwrap_or_else(|arc| (*arc).clone()),
                        ));
                    }
                    telemetry_read_strategy(key, "sequential", self.links[index].name(), true);
                    return Ok(Some(value));
                }
                Ok(None) => {
                    all_failed = false; // 至少有一个后端明确返回 miss
                    continue;
                }
                Err(e) => {
                    self.emit_backend_error(key, link.name(), &e);
                    last_err = Some(e);
                    continue;
                }
            }
        }

        // 所有后端都返回 Err 时传播错误，与竞速读语义一致
        if all_failed && self.links.is_empty() {
            return Ok(None);
        }
        if all_failed {
            return Err(last_err.unwrap_or_else(|| {
                OxCacheError::Operation("All backends failed during sequential read".to_string())
            }));
        }
        Ok(None)
    }

    /// 竞速读：并发向所有后端发起 `get`，返回最先命中者（问题 4.1）。
    ///
    /// 适用于 L1/L2 延迟差异小但可用性要求高的场景。最先返回命中值的后端
    /// 获胜；若全部未命中则返回 `None`；若全部失败则返回 `Err`。命中时若有
    /// 回填开启，异步回填到更高分后端。
    async fn race_read_from_chain(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        if self.links.is_empty() {
            return Ok(None);
        }

        let mut set = tokio::task::JoinSet::new();
        for (index, link) in self.links.iter().enumerate() {
            let backend = link.backend().clone();
            let key = key.to_string();
            set.spawn(async move { (index, backend.get(&key).await) });
        }

        let mut errs: Vec<(&'static str, OxCacheError)> = Vec::new();
        let mut hits: Vec<(usize, Vec<u8>)> = Vec::new();

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((index, Ok(Some(value)))) => hits.push((index, value)),
                Ok((_index, Ok(None))) => {}
                Ok((index, Err(e))) => {
                    self.emit_backend_error(key, self.links[index].name(), &e);
                    errs.push((self.links[index].name(), e));
                }
                Err(e) => errs.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        // 取最先命中（分数最高，即 index 最小）的结果
        if let Some((index, value)) = hits.into_iter().min_by_key(|(i, _)| *i) {
            if self.backfill_enabled && index > 0 {
                // 查询原始 TTL 并在回填时保留
                let original_ttl = self.links[index].backend().ttl(key).await.ok().flatten();
                self.backfill_to_higher_backends(
                    Arc::from(key),
                    Arc::new(value.clone()),
                    index,
                    original_ttl,
                )
                .await;
            }
            telemetry_read_strategy(key, "race", self.links[index].name(), true);
            return Ok(Some(value));
        }

        if errs.len() == self.links.len() {
            return Err(OxCacheError::Operation(
                "All backends failed during race read".to_string(),
            ));
        }

        Ok(None)
    }

    /// 并行择新读：并发查询全部链接，
    /// 收集命中后并发查询各自剩余 TTL，返回剩余最长者（None 视为最低
    /// 优先级）；并列或全 None 时取 index 最小（分数最高）。单链接错误
    /// 容忍口径与 race read 一致；全部失败时传播错误。
    async fn parallel_freshest_read(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        if self.links.is_empty() {
            return Ok(None);
        }

        let mut set = tokio::task::JoinSet::new();
        for (index, link) in self.links.iter().enumerate() {
            let backend = link.backend().clone();
            let key = key.to_string();
            set.spawn(async move { (index, backend.get(&key).await) });
        }

        let mut errs: Vec<(&'static str, OxCacheError)> = Vec::new();
        let mut hits: Vec<(usize, Vec<u8>)> = Vec::new();

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((index, Ok(Some(value)))) => hits.push((index, value)),
                Ok((_index, Ok(None))) => {}
                Ok((index, Err(e))) => {
                    self.emit_backend_error(key, self.links[index].name(), &e);
                    errs.push((self.links[index].name(), e));
                }
                Err(e) => errs.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        if hits.is_empty() {
            if errs.len() == self.links.len() {
                return Err(OxCacheError::Operation(
                    "All backends failed during parallel freshest read".to_string(),
                ));
            }
            return Ok(None);
        }

        // 并发查询命中者的剩余 TTL，剩余最长者胜出（None 最低优先）
        let freshness_queries: Vec<(usize, Arc<dyn crate::backend::CacheBackend>)> = hits
            .iter()
            .map(|(index, _)| (*index, self.links[*index].backend().clone()))
            .collect();
        let values: std::collections::HashMap<usize, Vec<u8>> = hits.into_iter().collect();
        let mut ttl_set = tokio::task::JoinSet::new();
        for (index, backend) in freshness_queries {
            let key = key.to_string();
            ttl_set.spawn(async move { (index, backend.ttl(&key).await.ok().flatten()) });
        }
        let mut freshness: Vec<(usize, Option<Duration>)> = Vec::with_capacity(values.len());
        while let Some(joined) = ttl_set.join_next().await {
            if let Ok((index, remaining)) = joined {
                freshness.push((index, remaining));
            }
        }
        let pick = freshness
            .into_iter()
            .max_by(|a, b| {
                // 剩余 TTL 比较：None 视为最低优先；并列取 index 最小
                let key = |(i, t): &(usize, Option<Duration>)| (*t, std::cmp::Reverse(*i));
                key(a).cmp(&key(b))
            })
            .map(|(index, _)| index)
            .expect("hits non-empty implies freshness non-empty");
        let value = values[&pick].clone();

        telemetry_read_strategy(key, "parallel_freshest", self.links[pick].name(), true);
        if self.backfill_enabled && pick > 0 {
            let original_ttl = self.links[pick].backend().ttl(key).await.ok().flatten();
            self.backfill_to_higher_backends(
                Arc::from(key),
                Arc::new(value.clone()),
                pick,
                original_ttl,
            )
            .await;
        }
        Ok(Some(value))
    }

    /// 回填数据到更高分后端，保留原始 TTL
    ///
    /// 顺序写入更高分后端（问题 4.2）。key/value 以 `Arc` 共享所有权（问题 2.2 / 2.3），
    /// 各后端 `Arc::clone` 仅增加引用计数，无堆拷贝。失败时记录 warn 日志，不回滚读取结果。
    /// `ttl` 为从源后端查询到的原始 TTL，传递给目标后端以保持过期语义一致。
    async fn backfill_to_higher_backends(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        from_index: usize,
        ttl: Option<Duration>,
    ) {
        for link in &self.links[..from_index] {
            let backend = link.backend().clone();
            match backend.set(key.clone(), value.clone(), ttl).await {
                Ok(()) => {
                    #[cfg(feature = "metrics")]
                    GLOBAL_UNIFIED_METRICS.record_backfill_success();
                    oxcache_telemetry_backfill_ok(&key, link.name());
                }
                Err(e) => {
                    #[cfg(feature = "metrics")]
                    GLOBAL_UNIFIED_METRICS.record_backfill_failed();
                    oxcache_telemetry_backfill_failed(&key, link.name(), &e);
                    self.emit_backend_error(&key, link.name(), &e);
                }
            }
        }
    }

    /// 写入数据到所有后端
    /// ttl=None 时各 backend 用自己的默认 TTL
    /// ttl=Some 时所有 backend 用同一个 TTL
    ///
    /// # 一致性窗口（已知权衡）
    ///
    /// 并发写所有后端**不保证原子性**：部分后端失败时仅记录事件不回滚——
    /// 例如 L1 写成功、L2 写失败时，本实例读到新值而其他实例回源读到旧值，
    /// 跨实例读取存在分叉窗口。缓解手段：写入后通过 `invalidation` 失效总线
    /// 广播失效事件（`crate::features::invalidation`），或依赖各后端 TTL 最终
    /// 收敛。需要跨后端强一致的场景应在调用方引入版本号
    /// （`crate::features::versioning`）或放弃多后端双写。
    ///
    /// 并发写入所有后端（JoinSet），写入延迟从 O(Σbackend) 降至 O(max(backend))（问题 4.3）。
    /// key/value 以 `Arc` 共享所有权传入，各后端 `Arc::clone` 零拷贝（问题 2.2 / 2.3）。
    async fn write_to_all_backends(
        &self,
        key: &Arc<str>,
        value: &Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let count = self.links.len();

        if count == 0 {
            return Ok(());
        }

        let effective_ttl = ttl.or(self.default_ttl);

        // 并发写入所有后端（问题 4.3）
        // 所有后端共享同一份 Arc 分配，`Arc::clone` 仅增加引用计数，无堆拷贝（问题 2.3）
        let mut errors: Vec<(&'static str, OxCacheError)> = Vec::new();
        let mut set = tokio::task::JoinSet::new();
        for link in &self.links {
            let backend = link.backend().clone();
            let name = link.name();
            let key = key.clone();
            let value = value.clone();
            set.spawn(async move { (name, backend.set(key, value, effective_ttl).await) });
        }

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((_name, Ok(()))) => {}
                Ok((name, Err(e))) => errors.push((name, e)),
                Err(e) => errors.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        // 抛出后端写入失败事件
        for (name, e) in &errors {
            self.emit_backend_error(key, name, e);
        }

        if errors.len() == self.links.len() {
            return Err(OxCacheError::Operation(
                "All backends failed to write".to_string(),
            ));
        }

        Ok(())
    }

    /// 从所有后端并发删除数据
    ///
    /// 使用 JoinSet 并发执行，延迟从 O(Σbackend) 降至 O(max(backend))，
    /// 与 `write_to_all_backends()` 语义对称。部分后端失败时记录 warn 日志，
    /// 仅当所有后端都失败才返回 Err。
    async fn delete_from_all_backends(&self, key: &str) -> OxCacheResult<()> {
        let count = self.links.len();

        if count == 0 {
            return Ok(());
        }

        let mut set = tokio::task::JoinSet::new();
        for link in &self.links {
            let backend = link.backend().clone();
            let name = link.name();
            let key = key.to_string();
            set.spawn(async move { (name, backend.delete(&key).await) });
        }

        let mut errors: Vec<(&'static str, OxCacheError)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((_name, Ok(()))) => {}
                Ok((name, Err(e))) => {
                    self.emit_backend_error(key, name, &e);
                    errors.push((name, e));
                }
                Err(e) => errors.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        if errors.len() == self.links.len() {
            return Err(OxCacheError::Operation(format!(
                "All backends failed to delete: {:?}",
                errors
            )));
        }

        Ok(())
    }

    // ========================================================================
    // Sync API (任务组 15)
    //
    // 同步版链式访问。语义与 async 版一致：get_sync 按分数从高到低遍历，
    // set_sync 写入所有链接（透传 TTL），delete_sync 从所有链接删除。
    //
    // 契约：链中任一链接未实现 SyncCacheBackend（`try_as_sync_backend` 返回
    // None）时，所有 sync API 返回 `Err(NotSupported)`。这避免部分链接静默
    // 跳过导致的写丢失风险。
    // ========================================================================

    /// 收集链中所有链接的 sync backend。
    ///
    /// 链中任一链接未实现 `SyncCacheBackend` 时返回 `Err(NotSupported)`。
    /// links 已按分数降序排列，故返回的 Vec 也是降序。结果懒缓存（问题 4.4）：
    /// 首次调用后复用，避免每次 sync 操作重复 clone 所有 Arc。
    fn collect_sync_backends(&self) -> OxCacheResult<&[Arc<dyn SyncCacheBackend>]> {
        let cached = self.sync_backends.get_or_init(|| {
            self.links
                .iter()
                .map(|link| link.try_as_sync_backend())
                .collect::<Option<Vec<_>>>()
        });
        cached.as_deref().ok_or_else(|| {
            OxCacheError::NotSupported(
                "chain sync API requires all links to support SyncCacheBackend".to_string(),
            )
        })
    }

    /// 同步读取：按分数从高到低遍历 sync backends，返回首个命中
    pub fn get_sync(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let sync_backends = self.collect_sync_backends()?;
        for backend in sync_backends {
            match backend.get(key) {
                Ok(Some(value)) => return Ok(Some(value)),
                Ok(None) => continue,
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    /// 同步写入：写入所有 sync backends，透传 TTL
    pub fn set_sync(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let sync_backends = self.collect_sync_backends()?;

        if sync_backends.is_empty() {
            return Err(OxCacheError::Operation("Chain has no backends".to_string()));
        }

        let effective_ttl = ttl.or(self.default_ttl);
        let key_arc: Arc<str> = Arc::from(key);
        let value_arc: Arc<Vec<u8>> = Arc::new(value);
        let mut errors = Vec::new();

        // 所有后端共享同一份 Arc 分配，`Arc::clone` 仅增加引用计数，无堆拷贝（问题 2.2 / 2.3）
        for backend in sync_backends.iter() {
            if let Err(e) = backend.set(key_arc.clone(), value_arc.clone(), effective_ttl) {
                errors.push(e);
            }
        }

        if errors.len() == sync_backends.len() {
            return Err(OxCacheError::Operation(
                "All backends failed to write".to_string(),
            ));
        }

        Ok(())
    }

    /// 同步删除：从所有 sync backends 删除
    pub fn delete_sync(&self, key: &str) -> OxCacheResult<()> {
        let sync_backends = self.collect_sync_backends()?;

        let mut errors = Vec::new();

        for backend in sync_backends {
            if let Err(e) = backend.delete(key) {
                errors.push(e);
            }
        }

        if errors.len() == sync_backends.len() && !sync_backends.is_empty() {
            return Err(OxCacheError::Operation(format!(
                "All backends failed to delete: {:?}",
                errors
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl CacheReader for ChainCache {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        if self.links.is_empty() {
            return Ok(None);
        }
        self.read_from_chain(key).await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        for link in &self.links {
            match link.backend().exists(key).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(_) => continue,
            }
        }
        Ok(false)
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        for link in &self.links {
            match link.backend().ttl(key).await {
                Ok(Some(ttl)) => return Ok(Some(ttl)),
                Ok(None) => continue,
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    async fn len(&self) -> OxCacheResult<u64> {
        if let Some(link) = self.links.first() {
            link.backend().len().await
        } else {
            Ok(0)
        }
    }

    async fn is_empty(&self) -> OxCacheResult<bool> {
        if let Some(link) = self.links.first() {
            link.backend().is_empty().await
        } else {
            Ok(true)
        }
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        if let Some(link) = self.links.first() {
            link.backend().capacity().await
        } else {
            Ok(0)
        }
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "chain".to_string());
        stats.insert("backend_count".to_string(), self.links.len().to_string());

        for (index, link) in self.links.iter().enumerate() {
            stats.insert(format!("backend_{}_name", index), link.name().to_string());
            stats.insert(format!("backend_{}_score", index), link.score().to_string());
        }

        Ok(stats)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for link in &self.links {
            if let Ok(keys) = link.backend().keys(pattern).await {
                for k in keys {
                    if seen.insert(k.clone()) {
                        result.push(k);
                    }
                }
            }
        }
        Ok(result)
    }
}

#[async_trait]
impl CacheWriter for ChainCache {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        if self.links.is_empty() {
            return Err(OxCacheError::Operation("Chain has no backends".to_string()));
        }
        self.write_to_all_backends(&key, &value, ttl).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        if self.links.is_empty() {
            return Ok(());
        }
        self.delete_from_all_backends(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend().clear().await {
                errors.push((link.name(), e));
            }
        }

        if errors.len() == self.links.len() && !self.links.is_empty() {
            return Err(OxCacheError::Operation(format!(
                "All backends failed to clear: {:?}",
                errors
            )));
        }

        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let mut any_success = false;

        for link in &self.links {
            match link.backend().expire(key, ttl).await {
                Ok(true) => any_success = true,
                // 键不存在属正常业务结果，非后端故障
                Ok(false) => continue,
                Err(e) => {
                    telemetry_expire_backend_failed(key, link.name(), &e);
                    self.emit_backend_error(key, link.name(), &e);
                }
            }
        }

        Ok(any_success)
    }
}

#[async_trait]
impl CacheConnector for ChainCache {
    /// 并发检查所有后端健康状态，每个后端有独立 5s 超时（问题 5.2）。
    ///
    /// 任一后端失败（含超时）即返回错误；所有后端健康才返回 Ok。
    async fn health_check(&self) -> OxCacheResult<()> {
        if self.links.is_empty() {
            return Ok(());
        }

        const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

        let mut set = tokio::task::JoinSet::new();
        for link in &self.links {
            let backend = link.backend().clone();
            let name = link.name();
            set.spawn(async move {
                let result =
                    tokio::time::timeout(HEALTH_CHECK_TIMEOUT, backend.health_check()).await;
                (name, result)
            });
        }

        let mut failures = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((_name, Ok(Ok(())))) => {}
                Ok((name, Ok(Err(e)))) => failures.push((name, e)),
                Ok((name, Err(_))) => failures.push((
                    name,
                    OxCacheError::Timeout("health_check timed out after 5s".to_string()),
                )),
                Err(e) => failures.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(OxCacheError::Operation(format!(
                "health_check failed for {} backend(s): {:?}",
                failures.len(),
                failures
            )))
        }
    }

    async fn shutdown(&self) {
        for link in &self.links {
            link.backend().shutdown().await;
        }
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Chain
    }
}

// ============================================================================
// AtomicCacheWriter for ChainCache
// ============================================================================

#[async_trait]
impl AtomicCacheWriter for ChainCache {
    /// 委托最高分后端的 `AtomicCacheWriter`。无原子后端时返回 `Err(NotSupported)`。
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        let writer = self
            .links
            .first()
            .and_then(|link| link.backend().as_atomic_writer())
            .ok_or_else(|| {
                OxCacheError::NotSupported(
                    "incr: no link in chain implements AtomicCacheWriter".to_string(),
                )
            })?;
        writer.incr(key, delta, ttl).await
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let writer = self
            .links
            .first()
            .and_then(|link| link.backend().as_atomic_writer())
            .ok_or_else(|| {
                OxCacheError::NotSupported(
                    "compare_and_swap: no link in chain implements AtomicCacheWriter".to_string(),
                )
            })?;
        writer.compare_and_swap(key, expected, new, ttl).await
    }

    async fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let writer = self
            .links
            .first()
            .and_then(|link| link.backend().as_atomic_writer())
            .ok_or_else(|| {
                OxCacheError::NotSupported(
                    "set_if_absent: no link in chain implements AtomicCacheWriter".to_string(),
                )
            })?;
        writer.set_if_absent(key, value, ttl).await
    }
}

// ============================================================================
// keys() override for ChainCache
// ============================================================================
