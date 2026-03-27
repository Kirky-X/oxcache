// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 链式缓存核心实现
//
// ChainCache 提供多后端链式访问，按分数从高到低遍历后端。
// 读取时从高分后端开始，写入时写入所有后端。

use crate::backend::interface::CacheBackend;
use crate::backend::score::BackendScore;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use std::any::Any;
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
    pub backend: Arc<dyn CacheBackend>,
    /// 后端分数（越高越快）
    pub score: u8,
    /// 是否为持久化后端
    pub is_persistent: bool,
    /// 后端名称
    pub name: &'static str,
}

impl ChainLink {
    /// 创建新的链式链接
    ///
    /// # Arguments
    ///
    /// * `backend` - 后端实例
    /// * `score` - 后端分数
    /// * `is_persistent` - 是否持久化
    /// * `name` - 后端名称
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
    ///
    /// # Arguments
    ///
    /// * `backend` - 实现了 BackendScore 的后端
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

    /// 从 Arc 创建链接
    ///
    /// # Arguments
    ///
    /// * `backend` - Arc 包装的后端
    /// * `score` - 后端分数
    /// * `is_persistent` - 是否持久化
    /// * `name` - 后端名称
    pub fn from_arc(backend: Arc<dyn CacheBackend>, score: u8, is_persistent: bool, name: &'static str) -> Self {
        Self {
            backend,
            score,
            is_persistent,
            name,
        }
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
/// # 工作原理
///
/// 1. **读取**: 从高分后端开始，找到数据即返回（并回填到更高分后端）
/// 2. **写入**: 写入所有后端，确保数据一致性
/// 3. **删除**: 从所有后端删除
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::chain::{ChainCache, ChainLink};
/// use oxcache::backend::{MokaMemoryBackend, RedisBackend, BackendScore};
///
/// // 创建后端
/// let moka = MokaMemoryBackend::new();
/// let redis = RedisBackend::new("redis://localhost:6379").await?;
///
/// // 创建链式链接
/// let links = vec![
///     ChainLink::from_backend(moka),
///     ChainLink::from_backend(redis),
/// ];
///
/// // 创建链式缓存
/// let chain = ChainCache::new(links);
///
/// // 使用链式缓存
/// chain.set("key", b"value".to_vec(), None).await?;
/// let value = chain.get("key").await?;
/// ```
pub struct ChainCache {
    /// 后端链接列表（按分数降序排列）
    links: Vec<ChainLink>,
    /// 默认 TTL
    default_ttl: Option<Duration>,
    /// 是否启用回填
    backfill_enabled: bool,
}

impl ChainCache {
    /// 创建新的链式缓存
    ///
    /// # Arguments
    ///
    /// * `links` - 后端链接列表（将被自动排序）
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
        self.links.iter().find(|link| link.score == score)
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
        self.links.iter().filter(|link| link.is_persistent).collect()
    }

    /// 获取所有非持久化后端
    pub fn non_persistent_backends(&self) -> Vec<&ChainLink> {
        self.links.iter().filter(|link| !link.is_persistent).collect()
    }

    /// 从链中读取数据
    ///
    /// 从高分后端开始读取，找到即返回。
    /// 如果启用回填，会将数据写入更高分后端。
    #[instrument(skip(self), fields(key = %key))]
    async fn read_from_chain(&self, key: &str) -> Result<Option<Vec<u8>>> {
        for (index, link) in self.links.iter().enumerate() {
            match link.backend.get(key).await {
                Ok(Some(value)) => {
                    // 回填到更高分后端
                    if self.backfill_enabled && index > 0 {
                        self.backfill_to_higher_backends(key, &value, index).await;
                    }

                    return Ok(Some(value));
                }
                Ok(None) => {
                    continue;
                }
                Err(_) => {
                    continue;
                }
            }
        }

        Ok(None)
    }

    /// 回填数据到更高分后端
    async fn backfill_to_higher_backends(&self, key: &str, value: &[u8], from_index: usize) {
        for link in &self.links[..from_index] {
            let _ = link.backend.set(key, value.to_vec(), self.default_ttl).await;
        }
    }

    /// 写入数据到所有后端
    #[instrument(skip(self, value), fields(key = %key))]
    async fn write_to_all_backends(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let effective_ttl = ttl.or(self.default_ttl);
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend.set(key, value.clone(), effective_ttl).await {
                errors.push((link.name, e));
            }
        }

        // 如果所有后端都失败，返回错误
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
            if let Err(e) = link.backend.delete(key).await {
                errors.push((link.name, e));
            }
        }

        // 如果所有后端都失败，返回错误
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
impl CacheBackend for ChainCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if self.links.is_empty() {
            return Ok(None);
        }
        self.read_from_chain(key).await
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        if self.links.is_empty() {
            return Err(CacheError::ConfigError("Chain has no backends".to_string()));
        }
        self.write_to_all_backends(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        if self.links.is_empty() {
            return Ok(());
        }
        self.delete_from_all_backends(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        for link in &self.links {
            match link.backend.exists(key).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(_) => continue,
            }
        }
        Ok(false)
    }

    async fn clear(&self) -> Result<()> {
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend.clear().await {
                errors.push((link.name, e));
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

    async fn close(&self) -> Result<()> {
        let mut errors = Vec::new();

        for link in &self.links {
            if let Err(e) = link.backend.close().await {
                errors.push((link.name, e));
            }
        }

        if errors.len() == self.links.len() && !self.links.is_empty() {
            return Err(CacheError::Operation(format!(
                "All backends failed to close: {:?}",
                errors
            )));
        }

        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        // 返回第一个找到的 TTL
        for link in &self.links {
            match link.backend.ttl(key).await {
                Ok(Some(ttl)) => return Ok(Some(ttl)),
                Ok(None) => continue,
                Err(_) => continue,
            }
        }
        Ok(None)
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut any_success = false;

        for link in &self.links {
            match link.backend.expire(key, ttl).await {
                Ok(true) => any_success = true,
                _ => continue,
            }
        }

        Ok(any_success)
    }

    async fn health_check(&self) -> Result<bool> {
        if self.links.is_empty() {
            return Ok(true);
        }

        // 至少有一个后端健康即可
        for link in &self.links {
            if link.backend.health_check().await.unwrap_or(false) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "chain".to_string());
        stats.insert("backend_count".to_string(), self.links.len().to_string());

        for (index, link) in self.links.iter().enumerate() {
            stats.insert(format!("backend_{}_name", index), link.name.to_string());
            stats.insert(format!("backend_{}_score", index), link.score.to_string());
        }

        Ok(stats)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn len(&self) -> Result<u64> {
        // 返回最高分后端的长度
        if let Some(link) = self.links.first() {
            link.backend.len().await
        } else {
            Ok(0)
        }
    }

    async fn is_empty(&self) -> Result<bool> {
        if let Some(link) = self.links.first() {
            link.backend.is_empty().await
        } else {
            Ok(true)
        }
    }

    async fn capacity(&self) -> Result<u64> {
        // 返回最高分后端的容量
        if let Some(link) = self.links.first() {
            link.backend.capacity().await
        } else {
            Ok(0)
        }
    }
}

/// 链式缓存构建器
#[derive(Default)]
pub struct ChainCacheBuilder {
    links: Vec<ChainLink>,
    default_ttl: Option<Duration>,
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

    /// 设置默认 TTL
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
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

    /// 构建链式缓存
    pub fn build(self) -> ChainCache {
        // 按分数降序排序
        let mut links = self.links;
        links.sort_by(|a, b| b.score.cmp(&a.score));

        ChainCache {
            links,
            default_ttl: self.default_ttl,
            backfill_enabled: self.backfill_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock::MockBackend;

    #[test]
    fn test_chain_link_creation() {
        let backend = MockBackend::new("test", 50, false);
        let link = ChainLink::from_backend(backend);

        assert_eq!(link.score, 50);
        assert!(!link.is_persistent);
        assert_eq!(link.name, "test");
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

        // 应该按分数降序排列
        assert_eq!(chain.links().len(), 2);
        assert_eq!(chain.links()[0].score, 100);
        assert_eq!(chain.links()[1].score, 50);
    }

    #[tokio::test]
    async fn test_chain_cache_get_set() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let chain = ChainCache::builder().backend(high).backend(low).build();

        // 设置值
        chain.set("key", b"value".to_vec(), None).await.unwrap();

        // 获取值
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_chain_cache_delete() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let chain = ChainCache::builder().backend(high).backend(low).build();

        // 设置并删除
        chain.set("key", b"value".to_vec(), None).await.unwrap();
        chain.delete("key").await.unwrap();

        // 应该不存在
        let exists = chain.exists("key").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_chain_cache_backfill() {
        let high = Arc::new(MockBackend::new("high", 100, false));
        let low = Arc::new(MockBackend::new("low", 50, true));

        // 只在低分后端设置值
        low.set("key", b"value".to_vec(), None).await.unwrap();

        let chain = ChainCache::builder()
            .link(ChainLink::from_arc(
                high.clone() as Arc<dyn CacheBackend>,
                100,
                false,
                "high",
            ))
            .link(ChainLink::from_arc(
                low.clone() as Arc<dyn CacheBackend>,
                50,
                true,
                "low",
            ))
            .enable_backfill()
            .build();

        // 读取应该触发回填
        let value = chain.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));

        // 高分后端现在应该有值了
        let high_value = high.get("key").await.unwrap();
        assert_eq!(high_value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_empty_chain() {
        let chain = ChainCache::new(vec![]);

        // 空链应该返回 None
        let value = chain.get("key").await.unwrap();
        assert!(value.is_none());

        // 空链的 exists 应该返回 false
        let exists = chain.exists("key").await.unwrap();
        assert!(!exists);
    }
}
