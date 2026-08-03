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
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
// tracing::instrument 仅在 tracing/full feature 下可用
#[cfg(any(feature = "tracing", feature = "full"))]
use tracing::instrument;

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
                        // 查询原始 TTL 并在回填时保留
                        let original_ttl = self.links[index].backend().ttl(key).await.ok().flatten();
                        // 将 value 移入 Arc 后直接用于回填和返回，避免额外 clone（OCR #36）
                        let value = Arc::new(value);
                        self.backfill_to_higher_backends(Arc::from(key), value.clone(), index, original_ttl)
                            .await;
                        return Ok(Some(Arc::try_unwrap(value).unwrap_or_else(|arc| (*arc).clone())));
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
            return Err(last_err
                .unwrap_or_else(|| OxCacheError::Operation("All backends failed during sequential read".to_string())));
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
                // 查询原始 TTL 并在回填时保留
                let original_ttl = self.links[index].backend().ttl(key).await.ok().flatten();
                self.backfill_to_higher_backends(Arc::from(key), Arc::new(value.clone()), index, original_ttl)
                    .await;
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
            let name = link.name();
            if let Err(e) = backend.set(key.clone(), value.clone(), ttl).await {
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
        cached.as_deref().ok_or_else(|| {
            OxCacheError::NotSupported("chain sync API requires all links to support SyncCacheBackend".to_string())
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
                let result = tokio::time::timeout(HEALTH_CHECK_TIMEOUT, backend.health_check()).await;
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
