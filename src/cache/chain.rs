// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 链式缓存核心实现
//
// ChainCache 提供多后端链式访问，按分数从高到低遍历后端。
// 读取时从高分后端开始，写入时写入所有后端。

use crate::backend::BackendScore;
use crate::backend::{BackendKind, CacheBackend, CacheConnector, CacheReader, CacheWriter, SyncCacheBackend};
use crate::error::{OxCacheError, OxCacheResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::Arc;
use std::time::Duration;
// tracing::instrument 仅在 tracing/full feature 下可用
#[cfg(any(feature = "tracing", feature = "full"))]
use tracing::instrument;

/// 链式缓存中的一个后端链接
///
/// ChainLink 封装了一个后端实例及其分数信息。
/// 分数用于确定链式访问的顺序。
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
    /// 是否启用竞速读（并发查询所有后端，返回最先命中者）
    race_read_enabled: bool,
    /// 默认 TTL
    default_ttl: Option<Duration>,
    /// 懒缓存的 sync backend 收集结果（问题 4.4）：
    /// 链构建后 links 不可变，首次收集后复用，避免每次 sync 调用重复 clone 所有 Arc。
    sync_backends: OnceLock<Option<Vec<Arc<dyn SyncCacheBackend>>>>,
}

impl ChainCache {
    /// 创建新的链式缓存
    pub fn new(links: Vec<ChainLink>) -> Self {
        Self::builder().links(links).build()
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

    /// 获取最低分后端
    pub fn lowest_score_backend(&self) -> Option<&ChainLink> {
        self.links.last()
    }

    /// 异步写入：写入所有链接，透传 TTL（公开 API，内部转为 `Arc` 零拷贝分发）。
    ///
    /// key/value 在 `CacheWriter::set` trait 层以 `Arc` 共享所有权（问题 2.2 / 2.3），
    /// 本方法作为用户入口保持 `&str` / `Vec<u8>` 签名，仅做一次 Arc 装箱。
    #[cfg_attr(any(feature = "tracing", feature = "full"), instrument(skip(self, value), fields(key = %key)))]
    pub async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let key = Arc::from(key);
        let value = Arc::new(value);
        CacheWriter::set(self, key, value, ttl).await
    }

    /// 获取所有持久化后端
    pub fn persistent_backends(&self) -> Vec<&ChainLink> {
        self.links.iter().filter(|link| link.is_persistent()).collect()
    }

    /// 获取所有非持久化后端
    pub fn non_persistent_backends(&self) -> Vec<&ChainLink> {
        self.links.iter().filter(|link| !link.is_persistent()).collect()
    }

    /// 从链中读取数据
    ///
    /// 单个后端失败时记录 warn 日志（问题 5.1），并继续尝试下一个后端（L1 失败降级到 L2）。
    /// 若启用了竞速读（race_read），则并发查询所有后端并返回最先命中者。
    /// 所有后端都失败时返回 `Err`（与竞速读语义一致）。
    #[cfg_attr(any(feature = "tracing", feature = "full"), instrument(skip(self), fields(key = %key)))]
    async fn read_from_chain(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        if self.race_read_enabled {
            return self.race_read_from_chain(key).await;
        }

        let mut all_failed = true;
        let mut last_err: Option<OxCacheError> = None;

        for (index, link) in self.links.iter().enumerate() {
            match link.backend().get(key).await {
                Ok(Some(value)) => {
                    // 回填到更高分后端
                    if self.backfill_enabled && index > 0 {
                        self.backfill_to_higher_backends(
                            Arc::from(key),
                            Arc::new(value.clone()),
                            index,
                        )
                        .await;
                    }
                    return Ok(Some(value));
                }
                Ok(None) => {
                    all_failed = false; // 至少有一个后端明确返回 miss
                    continue;
                }
                Err(e) => {
                    #[cfg(any(feature = "tracing", feature = "full"))]
                    tracing::warn!(
                        key = %key,
                        backend = %link.name(),
                        error = %e,
                        "cache read backend failed; degrading to next backend"
                    );
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
    #[cfg_attr(any(feature = "tracing", feature = "full"), instrument(skip(self), fields(key = %key)))]
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
                    #[cfg(any(feature = "tracing", feature = "full"))]
                    tracing::warn!(
                        key = %key,
                        backend = %self.links[index].name(),
                        error = %e,
                        "cache race-read backend failed"
                    );
                    errs.push((self.links[index].name(), e));
                }
                Err(e) => errs.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        // 取最先命中（分数最高，即 index 最小）的结果
        if let Some((index, value)) = hits.into_iter().min_by_key(|(i, _)| *i) {
            if self.backfill_enabled && index > 0 {
                self.backfill_to_higher_backends(Arc::from(key), Arc::new(value.clone()), index).await;
            }
            return Ok(Some(value));
        }

        if errs.len() == self.links.len() {
            return Err(OxCacheError::Operation(
                "All backends failed during race read".to_string(),
            ));
        }

        Ok(None)
    }

    /// 回填数据到更高分后端（使用各 backend 自己的默认 TTL）
    ///
    /// 顺序写入更高分后端（问题 4.2）。key/value 以 `Arc` 共享所有权（问题 2.2 / 2.3），
    /// 各后端 `Arc::clone` 仅增加引用计数，无堆拷贝。失败时记录 warn 日志，不回滚读取结果。
    async fn backfill_to_higher_backends(&self, key: Arc<str>, value: Arc<Vec<u8>>, from_index: usize) {
        for link in &self.links[..from_index] {
            let backend = link.backend().clone();
            let name = link.name();
            if let Err(e) = backend.set(key.clone(), value.clone(), None).await {
                #[cfg(any(feature = "tracing", feature = "full"))]
                tracing::warn!(
                    key = %key,
                    backend = %name,
                    error = %e,
                    "backfill to higher backend failed"
                );
            }
        }
    }

    /// 写入数据到所有后端
    /// ttl=None 时各 backend 用自己的默认 TTL
    /// ttl=Some 时所有 backend 用同一个 TTL
    ///
    /// 并发写入所有后端（JoinSet），写入延迟从 O(Σbackend) 降至 O(max(backend))（问题 4.3）。
    /// key/value 以 `Arc` 共享所有权传入，各后端 `Arc::clone` 零拷贝（问题 2.2 / 2.3）。
    #[cfg_attr(any(feature = "tracing", feature = "full"), instrument(skip(self, value), fields(key = %key)))]
    async fn write_to_all_backends(&self, key: &Arc<str>, value: &Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
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
            set.spawn(async move {
                (name, backend.set(key, value, effective_ttl).await)
            });
        }

        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((_name, Ok(()))) => {}
                Ok((name, Err(e))) => errors.push((name, e)),
                Err(e) => errors.push(("unknown", OxCacheError::Operation(e.to_string()))),
            }
        }

        // 记录单个后端失败（问题 5.1）
        #[cfg(any(feature = "tracing", feature = "full"))]
        for (name, e) in &errors {
            tracing::warn!(
                key = %key,
                backend = %name,
                error = %e,
                "cache write backend failed"
            );
        }

        if errors.len() == self.links.len() {
            return Err(OxCacheError::Operation("All backends failed to write".to_string()));
        }

        Ok(())
    }

    /// 从所有后端删除数据
    #[cfg_attr(any(feature = "tracing", feature = "full"), instrument(skip(self), fields(key = %key)))]
    async fn delete_from_all_backends(&self, key: &str) -> OxCacheResult<()> {
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend().delete(key).await {
                #[cfg(any(feature = "tracing", feature = "full"))]
                tracing::warn!(
                    key = %key,
                    backend = %link.name(),
                    error = %e,
                    "cache delete backend failed"
                );
                errors.push((link.name(), e));
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
        cached
            .as_deref()
            .ok_or_else(|| {
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
            return Err(OxCacheError::Operation("All backends failed to write".to_string()));
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
}

#[async_trait]
impl CacheWriter for ChainCache {
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
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
                _ => continue,
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

/// 链式缓存构建器
#[derive(Default)]
pub struct ChainCacheBuilder {
    links: Vec<ChainLink>,
    backfill_enabled: bool,
    race_read_enabled: bool,
    default_ttl: Option<Duration>,
}

impl ChainCacheBuilder {
    /// 添加后端链接
    pub fn link(mut self, link: ChainLink) -> Self {
        self.links.push(link);
        self
    }

    /// 添加多个后端链接
    pub fn links(mut self, mut links: Vec<ChainLink>) -> Self {
        self.links.append(&mut links);
        self
    }

    /// 添加后端（自动创建链接）
    pub fn backend<B>(self, backend: B) -> Self
    where
        B: CacheBackend + BackendScore + 'static,
    {
        self.link(ChainLink::from_backend(backend))
    }

    /// 设置默认 TTL
    pub fn default_time_to_live(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 启用回填
    pub fn enable_backfill(mut self) -> Self {
        self.backfill_enabled = true;
        self
    }

    /// 禁用回填
    pub fn disable_backfill(mut self) -> Self {
        self.backfill_enabled = false;
        self
    }

    /// 启用竞速读（默认关闭）
    ///
    /// 开启后 `get` 会并发查询所有后端，返回最先命中者（问题 4.1）。
    /// 适用于 L1/L2 延迟差异小但可用性要求高的场景。
    pub fn enable_race_read(mut self) -> Self {
        self.race_read_enabled = true;
        self
    }

    /// 禁用竞速读
    pub fn disable_race_read(mut self) -> Self {
        self.race_read_enabled = false;
        self
    }

    /// 构建链式缓存
    pub fn build(self) -> ChainCache {
        // 按分数降序排序
        let mut links = self.links;
        links.sort_by_key(|link| std::cmp::Reverse(link.score()));

        ChainCache {
            links,
            backfill_enabled: self.backfill_enabled,
            race_read_enabled: self.race_read_enabled,
            default_ttl: self.default_ttl,
            sync_backends: OnceLock::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{DashMapMemoryBackend, MokaMemoryBackend};
    use crate::testing::MockBackend;

    #[test]
    fn test_chain_link_creation() {
        let backend = MockBackend::new("test", 50, false);
        let link = ChainLink::from_backend(backend);

        assert_eq!(link.score(), 50);
        assert!(!link.is_persistent());
        assert_eq!(link.name(), "test");
    }

    #[test]
    fn test_chain_cache_builder() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let chain = ChainCache::builder()
            .backend(low)
            .backend(high)
            .enable_backfill()
            .build();

        assert_eq!(chain.links().len(), 2);
        assert_eq!(chain.links()[0].score(), 100);
        assert_eq!(chain.links()[1].score(), 50);
    }

    #[tokio::test]
    async fn test_chain_cache_get_set() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let chain = ChainCache::builder().backend(high).backend(low).build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();

        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_chain_cache_delete() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let chain = ChainCache::builder().backend(high).backend(low).build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();
        chain.delete("key").await.unwrap();

        let exists = chain.exists("key").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_chain_cache_backfill() {
        // Build chain with backfill enabled
        let chain = ChainCache::builder()
            .link(ChainLink::new(MockBackend::new("high", 100, false), 100, false, "high"))
            .link(ChainLink::new(MockBackend::new("low", 50, true), 50, true, "low"))
            .enable_backfill()
            .build();

        // Set value in chain (writes to all backends)
        chain.set("key", b"value".to_vec(), None).await.unwrap();

        // Read should succeed
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_empty_chain() {
        let chain = ChainCache::new(vec![]);

        let value = chain.get("key").await.unwrap();
        assert!(value.is_none());

        let exists = chain.exists("key").await.unwrap();
        assert!(!exists);
    }

    // ========================================================================
    // ChainLink tests
    // ========================================================================

    #[test]
    fn test_chain_link_new_constructor() {
        let backend = MokaMemoryBackend::new();
        let link = ChainLink::new(backend, 75, true, "custom");

        assert_eq!(link.score(), 75);
        assert!(link.is_persistent());
        assert_eq!(link.name(), "custom");
        // backend() getter should return a usable reference
        let _backend_ref = link.backend();
    }

    #[test]
    fn test_chain_link_from_backend_moka() {
        let backend = MokaMemoryBackend::new();
        let link = ChainLink::from_backend(backend);

        // Moka scores 100 (Scores::MOKA), non-persistent, name "moka"
        assert_eq!(link.score(), 100);
        assert!(!link.is_persistent());
        assert_eq!(link.name(), "moka");
    }

    #[test]
    fn test_chain_link_debug() {
        let backend = MokaMemoryBackend::new();
        let link = ChainLink::new(backend, 80, true, "dbg");

        let debug_str = format!("{:?}", link);
        assert!(debug_str.contains("ChainLink"));
        assert!(debug_str.contains("80"));
        assert!(debug_str.contains("dbg"));
    }

    // ========================================================================
    // ChainCache accessor tests
    // ========================================================================

    #[test]
    fn test_chain_cache_new_constructor() {
        let link = ChainLink::from_backend(MokaMemoryBackend::new());
        let chain = ChainCache::new(vec![link]);

        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_chain_cache_len_is_empty() {
        let empty = ChainCache::new(vec![]);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();
        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_chain_cache_get_by_score() {
        let chain = ChainCache::builder()
            .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
            .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
            .build();

        assert!(chain.get_by_score(100).is_some());
        assert!(chain.get_by_score(50).is_some());
        assert!(chain.get_by_score(75).is_none());
    }

    #[test]
    fn test_chain_cache_highest_lowest_backend() {
        // Add low first to verify sorting works
        let chain = ChainCache::builder()
            .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
            .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
            .build();

        let highest = chain.highest_score_backend().unwrap();
        assert_eq!(highest.score(), 100);
        assert_eq!(highest.name(), "high");

        let lowest = chain.lowest_score_backend().unwrap();
        assert_eq!(lowest.score(), 50);
        assert_eq!(lowest.name(), "low");
    }

    #[test]
    fn test_chain_cache_highest_lowest_empty() {
        let chain = ChainCache::new(vec![]);
        assert!(chain.highest_score_backend().is_none());
        assert!(chain.lowest_score_backend().is_none());
    }

    #[test]
    fn test_chain_cache_persistent_filters() {
        let chain = ChainCache::builder()
            .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
            .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
            .build();

        let persistent = chain.persistent_backends();
        assert_eq!(persistent.len(), 1);
        assert_eq!(persistent[0].name(), "low");

        let non_persistent = chain.non_persistent_backends();
        assert_eq!(non_persistent.len(), 1);
        assert_eq!(non_persistent[0].name(), "high");
    }

    #[test]
    fn test_chain_cache_links_accessor() {
        let chain = ChainCache::builder()
            .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
            .build();

        let links = chain.links();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].name(), "high");
    }

    // ========================================================================
    // Builder tests
    // ========================================================================

    #[test]
    fn test_builder_link_method() {
        let link = ChainLink::new(MokaMemoryBackend::new(), 100, false, "moka");
        let chain = ChainCache::builder().link(link).build();
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_builder_links_method() {
        let links = vec![
            ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"),
            ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"),
        ];
        let chain = ChainCache::builder().links(links).build();
        assert_eq!(chain.len(), 2);
        // Verify sorting by score descending
        assert_eq!(chain.links()[0].score(), 100);
        assert_eq!(chain.links()[1].score(), 50);
    }

    #[tokio::test]
    async fn test_builder_default_time_to_live() {
        let chain = ChainCache::builder()
            .backend(MokaMemoryBackend::new())
            .default_time_to_live(Duration::from_secs(60))
            .build();

        // set with None should use default_ttl
        chain.set("key", b"value".to_vec(), None).await.unwrap();
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[test]
    fn test_builder_disable_backfill() {
        let chain = ChainCache::builder()
            .backend(MokaMemoryBackend::new())
            .enable_backfill()
            .disable_backfill()
            .build();

        assert_eq!(chain.len(), 1);
    }

    // ========================================================================
    // UnifiedCache trait tests (get_bytes / set_bytes)
    // ========================================================================

    #[tokio::test]
    async fn test_chain_cache_get_bytes_set_bytes() {
        use crate::UnifiedCache;
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        chain.set_bytes("key", b"value".to_vec(), None).await.unwrap();
        let value = chain.get_bytes("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_chain_cache_get_bytes_missing() {
        use crate::UnifiedCache;
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        let value = chain.get_bytes("missing").await.unwrap();
        assert!(value.is_none());
    }

    // ========================================================================
    // CacheWriter tests
    // ========================================================================

    #[tokio::test]
    async fn test_chain_cache_clear() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();
        assert!(chain.exists("key").await.unwrap());

        chain.clear().await.unwrap();
        assert!(!chain.exists("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_chain_cache_clear_empty() {
        let chain = ChainCache::new(vec![]);
        assert!(chain.clear().await.is_ok());
    }

    #[tokio::test]
    async fn test_chain_cache_expire() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();
        // Moka now supports per-entry TTL via Expiry trait; expire on existing key returns true
        let result = chain.expire("key", Duration::from_secs(60)).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_chain_cache_expire_missing_key() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        let result = chain.expire("missing", Duration::from_secs(60)).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_chain_cache_set_empty_chain_error() {
        let chain = ChainCache::new(vec![]);
        let result = chain.set("key", b"value".to_vec(), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chain_cache_delete_empty_chain() {
        let chain = ChainCache::new(vec![]);
        assert!(chain.delete("key").await.is_ok());
    }

    #[tokio::test]
    async fn test_chain_cache_set_with_explicit_ttl() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        chain
            .set("key", b"value".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_chain_cache_multi_backend_set_writes_all() {
        let high = MokaMemoryBackend::new();
        let low = MokaMemoryBackend::new();

        let high_ref = high.clone();
        let low_ref = low.clone();

        let chain = ChainCache::builder()
            .link(ChainLink::new(high, 100, false, "high"))
            .link(ChainLink::new(low, 50, true, "low"))
            .build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();

        // Both backends should have the value
        assert_eq!(high_ref.get("key").await.unwrap(), Some(b"value".to_vec()));
        assert_eq!(low_ref.get("key").await.unwrap(), Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_chain_cache_delete_removes_from_all() {
        let high = MokaMemoryBackend::new();
        let low = MokaMemoryBackend::new();

        let high_ref = high.clone();
        let low_ref = low.clone();

        let chain = ChainCache::builder()
            .link(ChainLink::new(high, 100, false, "high"))
            .link(ChainLink::new(low, 50, true, "low"))
            .build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();
        chain.delete("key").await.unwrap();

        assert!(high_ref.get("key").await.unwrap().is_none());
        assert!(low_ref.get("key").await.unwrap().is_none());
    }

    // ========================================================================
    // Backfill behavior tests
    // ========================================================================

    #[tokio::test]
    async fn test_chain_cache_backfill_populates_higher() {
        let high = MokaMemoryBackend::new();
        let low = MokaMemoryBackend::new();

        let high_ref = high.clone();
        let low_ref = low.clone();

        let chain = ChainCache::builder()
            .link(ChainLink::new(high, 100, false, "high"))
            .link(ChainLink::new(low, 50, true, "low"))
            .enable_backfill()
            .build();

        // Set value only in low backend (bypass chain)
        low_ref.set(Arc::from("key"), Arc::new(b"low_value".to_vec()), None).await.unwrap();

        // Verify high doesn't have it yet
        assert!(high_ref.get("key").await.unwrap().is_none());

        // Get from chain - should find in low and backfill to high
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"low_value".to_vec()));

        // Verify high now has the value (backfilled asynchronously — poll)
        let mut backfilled = false;
        for _ in 0..10 {
            if high_ref.get("key").await.unwrap().is_some() {
                backfilled = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let high_value = high_ref.get("key").await.unwrap();
        assert_eq!(high_value, Some(b"low_value".to_vec()), "backfill should populate high backend");
        assert!(backfilled, "backfill should complete asynchronously");
    }

    #[tokio::test]
    async fn test_chain_cache_no_backfill_when_disabled() {
        let high = MokaMemoryBackend::new();
        let low = MokaMemoryBackend::new();

        let high_ref = high.clone();
        let low_ref = low.clone();

        let chain = ChainCache::builder()
            .link(ChainLink::new(high, 100, false, "high"))
            .link(ChainLink::new(low, 50, true, "low"))
            .build(); // backfill disabled by default

        // Set value only in low backend (bypass chain)
        low_ref.set(Arc::from("key"), Arc::new(b"low_value".to_vec()), None).await.unwrap();

        // Get from chain - should find in low but NOT backfill to high
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"low_value".to_vec()));

        // Verify high still doesn't have the value
        assert!(high_ref.get("key").await.unwrap().is_none());
    }

    // ========================================================================
    // CacheReader trait tests
    // ========================================================================

    #[tokio::test]
    async fn test_chain_cache_ttl_len_capacity() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        chain.set("key", b"value".to_vec(), None).await.unwrap();

        // ttl - Moka returns None for per-entry TTL
        let ttl = chain.ttl("key").await.unwrap();
        assert!(ttl.is_none());

        // len (CacheReader trait) - Moka's entry_count is approximate
        let len = CacheReader::len(&chain).await.unwrap();
        assert!(len <= 100, "len should be reasonable after single insert");

        // capacity
        let capacity = chain.capacity().await.unwrap();
        assert!(capacity > 0);
    }

    #[tokio::test]
    async fn test_chain_cache_reader_empty() {
        let chain = ChainCache::new(vec![]);

        assert_eq!(CacheReader::len(&chain).await.unwrap(), 0);
        assert!(CacheReader::is_empty(&chain).await.unwrap());
        assert_eq!(chain.capacity().await.unwrap(), 0);

        let ttl = chain.ttl("key").await.unwrap();
        assert!(ttl.is_none());
    }

    #[tokio::test]
    async fn test_chain_cache_stats() {
        let chain = ChainCache::builder()
            .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "high"))
            .link(ChainLink::new(MokaMemoryBackend::new(), 50, true, "low"))
            .build();

        let stats = chain.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"chain".to_string()));
        assert_eq!(stats.get("backend_count"), Some(&"2".to_string()));
        assert_eq!(stats.get("backend_0_name"), Some(&"high".to_string()));
        assert_eq!(stats.get("backend_0_score"), Some(&"100".to_string()));
        assert_eq!(stats.get("backend_1_name"), Some(&"low".to_string()));
        assert_eq!(stats.get("backend_1_score"), Some(&"50".to_string()));
    }

    #[tokio::test]
    async fn test_chain_cache_stats_empty() {
        let chain = ChainCache::new(vec![]);
        let stats = chain.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"chain".to_string()));
        assert_eq!(stats.get("backend_count"), Some(&"0".to_string()));
    }

    // ========================================================================
    // CacheConnector trait tests
    // ========================================================================

    #[tokio::test]
    async fn test_chain_cache_health_check() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        assert!(chain.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_chain_cache_health_check_empty() {
        let chain = ChainCache::new(vec![]);
        assert!(chain.health_check().await.is_ok());
    }

    // ========================================================================
    // 故障降级与错误可见性测试 (问题 5.1 / 5.2 / 7.3)
    // ========================================================================

    #[tokio::test]
    async fn test_chain_read_degrades_when_high_backend_fails() {
        // L1 (score=100) get 注入故障，L2 (score=50) 有数据
        // 链式 get 应降级到 L2 返回数据（L1 失败降级）
        let high = MockBackend::new("high", 100, false).with_fail_get();
        let low = MockBackend::new("low", 50, true);
        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(high))
            .link(ChainLink::from_backend(low))
            .build();

        // 仅在 L2 写入数据（通过链引用访问）
        chain.links()[1].backend().set(Arc::from("key"), Arc::new(b"low_value".to_vec()), None).await.unwrap();

        let value = chain.get("key").await.unwrap();
        assert_eq!(
            value,
            Some(b"low_value".to_vec()),
            "L1 get 失败时应降级到 L2 读取"
        );
    }

    #[tokio::test]
    async fn test_chain_read_returns_none_when_all_backends_fail() {
        // L7 修复验证：所有后端 get 都失败时，链式 get 应返回 Err（与竞速读语义一致）
        let high = MockBackend::new("high", 100, false).with_fail_get();
        let low = MockBackend::new("low", 50, true).with_fail_get();

        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(high))
            .link(ChainLink::from_backend(low))
            .build();

        let result = chain.get("key").await;
        assert!(result.is_err(), "所有后端 get 失败时应返回 Err");
    }

    #[tokio::test]
    async fn test_chain_health_check_fails_when_backend_unhealthy() {
        // 任一后端 health_check 失败，链式 health_check 应返回错误
        let healthy = MockBackend::new("ok", 100, false);
        let unhealthy = MockBackend::new("down", 50, true).with_fail_health();

        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(healthy))
            .link(ChainLink::from_backend(unhealthy))
            .build();

        let result = chain.health_check().await;
        assert!(result.is_err(), "含不健康后端时 health_check 应失败");
    }

    #[tokio::test]
    async fn test_chain_write_succeeds_when_partial_backend_fails() {
        // 一个后端 set 注入故障，另一个正常：写操作不应整体失败
        let failing = MockBackend::new("failing", 100, false).with_fail_set();
        let ok = MokaMemoryBackend::new();

        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(failing))
            .link(ChainLink::from_backend(ok))
            .build();

        // 不应报错（只要至少一个后端成功）
        chain.set("key", b"value".to_vec(), None).await.unwrap();
    }

    #[tokio::test]
    async fn test_chain_cache_shutdown() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        // Should not panic
        chain.shutdown().await;
    }

    #[test]
    fn test_chain_cache_backend_kind() {
        let chain = ChainCache::builder().backend(MokaMemoryBackend::new()).build();

        assert_eq!(chain.backend_kind(), BackendKind::Chain);
    }

    // ========================================================================
    // TTL 透传契约测试 (spec: universal-per-entry-ttl / Decision 4c)
    // ========================================================================

    #[tokio::test]
    async fn test_chain_set_with_ttl_propagates_to_all_links() {
        // 链中 Moka (score=100) + DashMap (score=50) + Mock (score=30)
        // set 50ms TTL，等 100ms，三者皆过期
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();
        let mock = MockBackend::new("mock", 30, false);

        let moka_ref = moka.clone();
        let dashmap_ref = dashmap.clone();
        // MockBackend 是 #[cfg(test)] 且不 Clone（Arc<RwLock<...>> 内部，但 struct 未 derive Clone）
        // 用 ChainLink::new + 独立实例验证：这里我们用 mock 的 backend() 引用直接查询
        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(moka))
            .link(ChainLink::from_backend(dashmap))
            .link(ChainLink::new(mock, 30, false, "mock"))
            .build();

        chain
            .set("k", b"v".to_vec(), Some(Duration::from_millis(50)))
            .await
            .unwrap();

        // 立即链式 get 应返回 Some
        assert_eq!(chain.get("k").await.unwrap(), Some(b"v".to_vec()));

        // 等 100ms 让 TTL 过期
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Moka 后端：moka 异步清理可能略有延迟，循环等待最多 500ms
        let mut moka_expired = false;
        for _ in 0..10 {
            if moka_ref.get("k").await.unwrap().is_none() {
                moka_expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(moka_expired, "moka link should expire after TTL");

        // DashMap 后端：lazy 过期，get 应返回 None
        assert_eq!(
            dashmap_ref.get("k").await.unwrap(),
            None,
            "dashmap link should expire after TTL"
        );

        // 链式 get：所有链接都过期，应返回 None
        assert_eq!(
            chain.get("k").await.unwrap(),
            None,
            "chain get should return None after all links expired"
        );
    }

    #[tokio::test]
    async fn test_chain_ttl_returns_highest_score_link_ttl() {
        // Moka (score=100) + DashMap (score=50) 都 set 60s TTL
        // chain.ttl 应返回 Moka 的 ttl（最高分优先）
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(moka))
            .link(ChainLink::from_backend(dashmap))
            .build();

        chain
            .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();

        let ttl = chain.ttl("k").await.unwrap();
        assert!(ttl.is_some(), "chain ttl should return Some for highest-score link");
        let ttl = ttl.unwrap();
        // 58s < ttl <= 60s（最高分链接 Moka 的剩余 TTL）
        assert!(
            ttl > Duration::from_secs(58) && ttl <= Duration::from_secs(60),
            "chain ttl={} should be in (58s, 60s]",
            ttl.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_chain_expire_any_link_success_returns_true() {
        // Moka + DashMap 都已 set，expire 任一成功返回 true
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(moka))
            .link(ChainLink::from_backend(dashmap))
            .build();

        chain
            .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();

        let result = chain.expire("k", Duration::from_secs(120)).await.unwrap();
        assert!(result, "chain expire should return true when any link succeeds");
    }

    #[tokio::test]
    async fn test_chain_expire_all_missing_returns_false() {
        // 所有链接都没有 "missing" 键
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let chain = ChainCache::builder()
            .link(ChainLink::from_backend(moka))
            .link(ChainLink::from_backend(dashmap))
            .build();

        let result = chain.expire("missing", Duration::from_secs(60)).await.unwrap();
        assert!(!result, "chain expire should return false when all links miss");
    }

    // ========================================================================
    // Sync API tests (任务组 15)
    //
    // 验证 ChainCache 的 sync API：get_sync / set_sync / delete_sync
    // 契约：链中任一链接未实现 SyncCacheBackend 时返回 Err(NotSupported)
    // ========================================================================

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chain_sync_get_set() {
        // 链中所有链接实现 SyncCacheBackend（Moka + DashMap）
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let chain = ChainCache::builder()
            .link(ChainLink::from_sync_backend(moka))
            .link(ChainLink::from_sync_backend(dashmap))
            .build();

        // sync set + get roundtrip
        chain.set_sync("k", b"v".to_vec(), None).unwrap();
        let value = chain.get_sync("k").unwrap();
        assert_eq!(value, Some(b"v".to_vec()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chain_sync_get_returns_highest_score_hit() {
        // Moka (score=100) + DashMap (score=90)
        // 两个后端都存有 "k"，但值不同
        // get_sync 应返回最高分链接 Moka 的值
        use crate::backend::SyncCacheWriter;

        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        // 直接通过 sync API 在各后端写入不同值
        SyncCacheWriter::set(&moka, Arc::from("k"), Arc::new(b"high".to_vec()), None).unwrap();
        SyncCacheWriter::set(&dashmap, Arc::from("k"), Arc::new(b"low".to_vec()), None).unwrap();

        let chain = ChainCache::builder()
            .link(ChainLink::from_sync_backend(moka))
            .link(ChainLink::from_sync_backend(dashmap))
            .build();

        let value = chain.get_sync("k").unwrap();
        assert_eq!(
            value,
            Some(b"high".to_vec()),
            "get_sync should return highest-score link's value (Moka=100 > DashMap=90)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chain_sync_with_unsupported_link_falls_back_to_err() {
        // Moka (sync) + MockBackend (async-only，不实现 SyncCacheBackend)
        // 链中含非 sync backend，sync API 应返回 Err(NotSupported)
        use crate::error::OxCacheError;

        let moka = MokaMemoryBackend::new();
        let mock = MockBackend::new("mock", 30, false);

        let chain = ChainCache::builder()
            .link(ChainLink::from_sync_backend(moka))
            .link(ChainLink::from_backend(mock)) // async-only
            .build();

        let result = chain.get_sync("k");
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "get_sync should return NotSupported when chain has non-sync link, got {:?}",
            result
        );

        let result = chain.set_sync("k", b"v".to_vec(), None);
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "set_sync should return NotSupported when chain has non-sync link, got {:?}",
            result
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chain_sync_set_propagates_ttl() {
        // Moka (score=100) + DashMap (score=90)
        // set_sync with 50ms TTL，等 100ms，get_sync 应返回 None（两者皆过期）
        let moka = MokaMemoryBackend::new();
        let dashmap = DashMapMemoryBackend::new();

        let chain = ChainCache::builder()
            .link(ChainLink::from_sync_backend(moka))
            .link(ChainLink::from_sync_backend(dashmap))
            .build();

        // set with 50ms TTL
        chain
            .set_sync("k", b"v".to_vec(), Some(Duration::from_millis(50)))
            .unwrap();

        // 立即 get_sync 应返回 Some
        let value = chain.get_sync("k").unwrap();
        assert_eq!(value, Some(b"v".to_vec()));

        // 等 100ms 让 TTL 过期
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Moka 后端：moka 异步清理可能略有延迟，循环等待
        let mut expired = false;
        for _ in 0..10 {
            if chain.get_sync("k").unwrap().is_none() {
                expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            expired,
            "chain get_sync should return None after TTL expires on all links"
        );
    }

    // ========================================================================
    // 竞速读测试 (问题 4.1)
    // ========================================================================

    #[tokio::test]
    async fn test_chain_race_read_returns_earliest_hit() {
        // race_read 默认关闭时走串行路径，返回值仍正确
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);
        high.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await.unwrap();

        let chain = ChainCache::builder().backend(high).backend(low).build();
        let value = chain.get("k").await.unwrap();
        assert_eq!(value, Some(b"v".to_vec()));

        // race_read 开启后同样返回命中值
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);
        low.set(Arc::from("k"), Arc::new(b"l2".to_vec()), None).await.unwrap();

        let chain = ChainCache::builder()
            .backend(high)
            .backend(low)
            .enable_race_read()
            .build();
        let value = chain.get("k").await.unwrap();
        assert_eq!(
            value,
            Some(b"l2".to_vec()),
            "race read should return value from whichever backend has it"
        );
    }

    #[tokio::test]
    async fn test_chain_race_read_backs_off_on_backend_error() {
        // L1 失败、L2 命中：race read 应返回 L2 值而非 Err（5.1 降级语义）
        let failing = MockBackend::new("high", 100, false).with_fail_get();
        let ok = MockBackend::new("low", 50, true);
        ok.set(Arc::from("k"), Arc::new(b"l2".to_vec()), None).await.unwrap();

        let chain = ChainCache::builder()
            .backend(failing)
            .backend(ok)
            .enable_race_read()
            .build();

        let value = chain.get("k").await.unwrap();
        assert_eq!(
            value,
            Some(b"l2".to_vec()),
            "race read should degrade past failing backend"
        );
    }

    #[tokio::test]
    async fn test_chain_race_read_all_backends_fail() {
        use crate::error::OxCacheError;

        let failing1 = MockBackend::new("high", 100, false).with_fail_get();
        let failing2 = MockBackend::new("low", 50, true).with_fail_get();

        let chain = ChainCache::builder()
            .backend(failing1)
            .backend(failing2)
            .enable_race_read()
            .build();

        let result = chain.get("k").await;
        assert!(
            matches!(result, Err(OxCacheError::Operation(ref msg)) if msg.contains("All backends failed")),
            "race read should error when all backends fail, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_chain_race_read_miss_returns_none() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let chain = ChainCache::builder()
            .backend(high)
            .backend(low)
            .enable_race_read()
            .build();

        let value = chain.get("missing").await.unwrap();
        assert_eq!(value, None, "race read with no hits should return None");
    }
}
