// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 链式缓存核心实现
//
// ChainCache 提供多后端链式访问，按分数从高到低遍历后端。
// 读取时从高分后端开始，写入时写入所有后端。

use crate::backend::interface::{BackendKind, CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::backend::score::BackendScore;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::instrument;

/// 链式缓存中的一个后端链接
///
/// ChainLink 封装了一个后端实例及其分数信息。
/// 分数用于确定链式访问的顺序。
#[derive(Clone)]
pub struct ChainLink {
    /// 后端实例
    backend: Arc<dyn CacheBackend>,
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
            score,
            is_persistent,
            name,
        }
    }

    /// 获取后端实例引用
    pub fn backend(&self) -> &Arc<dyn CacheBackend> {
        &self.backend
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
/// # TTL 行为
///
/// - `chain.set(key, value, None)` → 各 backend 使用自己的默认 TTL
/// - `chain.set(key, value, Some(ttl))` → 所有 backend 使用传入的 TTL
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

    /// 获取所有持久化后端
    pub fn persistent_backends(&self) -> Vec<&ChainLink> {
        self.links.iter().filter(|link| link.is_persistent()).collect()
    }

    /// 获取所有非持久化后端
    pub fn non_persistent_backends(&self) -> Vec<&ChainLink> {
        self.links.iter().filter(|link| !link.is_persistent()).collect()
    }

    /// 从链中读取数据
    #[instrument(skip(self), fields(key = %key))]
    async fn read_from_chain(&self, key: &str) -> Result<Option<Vec<u8>>> {
        for (index, link) in self.links.iter().enumerate() {
            match link.backend().get(key).await {
                Ok(Some(value)) => {
                    // 回填到更高分后端
                    if self.backfill_enabled && index > 0 {
                        self.backfill_to_higher_backends(key, &value, index).await;
                    }
                    return Ok(Some(value));
                }
                Ok(None) => continue,
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    /// 回填数据到更高分后端（使用各 backend 自己的默认 TTL）
    async fn backfill_to_higher_backends(&self, key: &str, value: &[u8], from_index: usize) {
        for link in &self.links[..from_index] {
            let _ = link.backend().set(key, value.to_vec(), None).await;
        }
    }

    /// 写入数据到所有后端
    /// ttl=None 时各 backend 用自己的默认 TTL
    /// ttl=Some 时所有 backend 用同一个 TTL
    #[instrument(skip(self, value), fields(key = %key))]
    async fn write_to_all_backends(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let mut errors = Vec::new();
        let count = self.links.len();

        if count == 0 {
            return Ok(());
        }

        // Clone for all but the last backend
        for link in self.links.iter().take(count - 1) {
            if let Err(e) = link.backend().set(key, value.clone(), ttl).await {
                errors.push((link.name(), e));
            }
        }

        // Last backend: use the owned value directly (no clone)
        if let Some(link) = self.links.last() {
            if let Err(e) = link.backend().set(key, value, ttl).await {
                errors.push((link.name(), e));
            }
        }

        if errors.len() == self.links.len() {
            return Err(CacheError::Operation("All backends failed to write".to_string()));
        }

        Ok(())
    }

    /// 从所有后端删除数据
    #[instrument(skip(self), fields(key = %key))]
    async fn delete_from_all_backends(&self, key: &str) -> Result<()> {
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend().delete(key).await {
                errors.push((link.name(), e));
            }
        }

        if errors.len() == self.links.len() {
            return Err(CacheError::Operation(format!(
                "All backends failed to delete: {:?}",
                errors
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl CacheReader for ChainCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if self.links.is_empty() {
            return Ok(None);
        }
        self.read_from_chain(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        for link in &self.links {
            match link.backend().exists(key).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(_) => continue,
            }
        }
        Ok(false)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        for link in &self.links {
            match link.backend().ttl(key).await {
                Ok(Some(ttl)) => return Ok(Some(ttl)),
                Ok(None) => continue,
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    async fn len(&self) -> Result<u64> {
        if let Some(link) = self.links.first() {
            link.backend().len().await
        } else {
            Ok(0)
        }
    }

    async fn is_empty(&self) -> Result<bool> {
        if let Some(link) = self.links.first() {
            link.backend().is_empty().await
        } else {
            Ok(true)
        }
    }

    async fn capacity(&self) -> Result<u64> {
        if let Some(link) = self.links.first() {
            link.backend().capacity().await
        } else {
            Ok(0)
        }
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
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
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        if self.links.is_empty() {
            return Err(CacheError::Operation("Chain has no backends".to_string()));
        }
        self.write_to_all_backends(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        if self.links.is_empty() {
            return Ok(());
        }
        self.delete_from_all_backends(key).await
    }

    async fn clear(&self) -> Result<()> {
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend().clear().await {
                errors.push((link.name(), e));
            }
        }

        if errors.len() == self.links.len() && !self.links.is_empty() {
            return Err(CacheError::Operation(format!(
                "All backends failed to clear: {:?}",
                errors
            )));
        }

        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
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
    async fn health_check(&self) -> Result<()> {
        if self.links.is_empty() {
            return Ok(());
        }

        for link in &self.links {
            link.backend().health_check().await?;
        }

        Ok(())
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

    /// 构建链式缓存
    pub fn build(self) -> ChainCache {
        // 按分数降序排序
        let mut links = self.links;
        links.sort_by_key(|link| std::cmp::Reverse(link.score()));

        ChainCache {
            links,
            backfill_enabled: self.backfill_enabled,
        }
    }
}

// ============================================================================
// User-Friendly API (OxCacheBuilder)
// ============================================================================

/// 用户友好的缓存构建器别名
///
/// 与 ChainCacheBuilder 功能相同，但名称更简洁。
pub type OxCacheBuilder = ChainCacheBuilder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock::MockBackend;

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
}
