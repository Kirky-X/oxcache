// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// OxCacheBuilder - 新的缓存构建器
//
// 提供类型安全的链式缓存构建 API，自动排序后端。

use crate::backend::interface::CacheBackend;
use crate::backend::score::BackendScore;
use crate::builder::sorter::BackendSorter;
use crate::cache::chain::{ChainCache, ChainLink};
use crate::error::{CacheError, Result};
use std::time::Duration;

/// OxCacheBuilder - 新的缓存构建器
///
/// 提供类型安全的链式缓存构建 API，自动排序后端。
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::builder::OxCacheBuilder;
/// use oxcache::backend::{MokaMemoryBackend, RedisBackend};
///
/// // 创建后端
/// let moka = MokaMemoryBackend::new();
/// let redis = RedisBackend::new("redis://localhost:6379").await?;
///
/// // 构建链式缓存
/// let cache = OxCacheBuilder::new()
///     .backend(moka)
///     .backend(redis)
///     .default_ttl(Duration::from_secs(3600))
///     .enable_backfill()
///     .build()?;
/// ```
pub struct OxCacheBuilder {
    /// 后端列表
    backends: Vec<ChainLink>,
    /// 默认 TTL
    default_ttl: Option<Duration>,
    /// 是否启用回填
    backfill_enabled: bool,
    /// 最大容量
    max_capacity: Option<u64>,
}

impl Default for OxCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OxCacheBuilder {
    /// 创建新的 OxCacheBuilder
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            default_ttl: None,
            backfill_enabled: true,
            max_capacity: None,
        }
    }

    /// 添加后端
    ///
    /// 后端会被自动排序，高分后端在前。
    ///
    /// # Arguments
    ///
    /// * `backend` - 实现了 CacheBackend 和 BackendScore 的后端
    pub fn backend<B>(mut self, backend: B) -> Self
    where
        B: CacheBackend + BackendScore + 'static,
    {
        let link = ChainLink::from_backend(backend);
        self.backends.push(link);
        self
    }

    /// 添加 ChainLink
    ///
    /// # Arguments
    ///
    /// * `link` - ChainLink 实例
    pub fn link(mut self, link: ChainLink) -> Self {
        self.backends.push(link);
        self
    }

    /// 添加多个后端
    ///
    /// # Arguments
    ///
    /// * `backends` - 后端列表
    pub fn backends<B>(mut self, backends: Vec<B>) -> Self
    where
        B: CacheBackend + BackendScore + Clone + 'static,
    {
        let links = BackendSorter::from_backends(backends);
        self.backends.extend(links);
        self
    }

    /// 设置默认 TTL
    ///
    /// # Arguments
    ///
    /// * `ttl` - 默认 TTL
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 启用回填
    ///
    /// 当从低分后端读取数据时，自动回填到高分后端。
    pub fn enable_backfill(mut self) -> Self {
        self.backfill_enabled = true;
        self
    }

    /// 禁用回填
    pub fn disable_backfill(mut self) -> Self {
        self.backfill_enabled = false;
        self
    }

    /// 设置最大容量
    ///
    /// # Arguments
    ///
    /// * `capacity` - 最大容量
    pub fn max_capacity(mut self, capacity: u64) -> Self {
        self.max_capacity = Some(capacity);
        self
    }

    /// 构建链式缓存
    ///
    /// # Returns
    ///
    /// 构建好的 ChainCache 实例
    ///
    /// # Errors
    ///
    /// 如果没有配置后端，返回错误。
    pub fn build(self) -> Result<ChainCache> {
        if self.backends.is_empty() {
            return Err(CacheError::InvalidInput(
                "No backends configured. Use .backend() to add at least one backend.".to_string(),
            ));
        }

        // 排序后端
        let sorted_links = BackendSorter::sort_links(self.backends);

        // 验证配置
        let validation = BackendSorter::validate(&sorted_links);
        if !validation.is_valid() {
            return Err(CacheError::InvalidInput(format!(
                "Invalid backend configuration: {:?}",
                validation.errors
            )));
        }

        // 打印警告（静默处理）
        let _ = &validation.warnings;

        Ok(ChainCache::builder()
            .links(sorted_links)
            .default_ttl(self.default_ttl.unwrap_or_else(|| Duration::from_secs(3600)))
            .enable_backfill()
            .build())
    }

    /// 构建链式缓存（异步版本）
    ///
    /// 用于需要异步初始化的场景。
    pub async fn build_async(self) -> Result<ChainCache> {
        self.build()
    }

    /// 获取后端数量
    pub fn backend_count(&self) -> usize {
        self.backends.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

/// 快捷构建函数
impl OxCacheBuilder {
    /// 创建仅内存缓存
    ///
    /// # Arguments
    ///
    /// * `capacity` - 缓存容量
    pub fn memory(capacity: u64) -> Self {
        Self::new().backend(crate::backend::MokaMemoryBackend::builder().capacity(capacity).build())
    }

    /// 创建仅 Redis 缓存
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis 连接字符串
    #[cfg(feature = "redis")]
    pub async fn redis(connection_string: &str) -> Result<Self> {
        let redis = crate::backend::client::RedisBackend::new(connection_string).await?;
        Ok(Self::new().backend(redis))
    }

    /// 创建两级缓存（内存 + Redis）
    ///
    /// # Arguments
    ///
    /// * `memory_capacity` - 内存缓存容量
    /// * `redis_connection` - Redis 连接字符串
    #[cfg(feature = "redis")]
    pub async fn tiered(memory_capacity: u64, redis_connection: &str) -> Result<Self> {
        let moka = crate::backend::MokaMemoryBackend::builder()
            .capacity(memory_capacity)
            .build();
        let redis = crate::backend::client::RedisBackend::new(redis_connection).await?;

        Ok(Self::new().backend(moka).backend(redis).enable_backfill())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock::MockBackend;

    // (测试代码继续...)

    #[test]
    fn test_builder_empty() {
        let result = OxCacheBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_single_backend() {
        let backend = MockBackend::new("test", 50, false);
        let result = OxCacheBuilder::new().backend(backend).build();
        assert!(result.is_ok());

        let cache = result.unwrap();
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_builder_multiple_backends() {
        let high = MockBackend::new("high", 100, false);
        let low = MockBackend::new("low", 50, true);

        let result = OxCacheBuilder::new().backend(low).backend(high).build();

        assert!(result.is_ok());

        let cache = result.unwrap();
        assert_eq!(cache.len(), 2);

        // 验证排序
        assert_eq!(cache.links()[0].score, 100);
        assert_eq!(cache.links()[1].score, 50);
    }

    #[test]
    fn test_builder_with_ttl() {
        let backend = MockBackend::new("test", 50, false);

        let result = OxCacheBuilder::new()
            .backend(backend)
            .default_ttl(Duration::from_secs(60))
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_memory() {
        let result = OxCacheBuilder::memory(1000).build();
        assert!(result.is_ok());

        let cache = result.unwrap();
        assert_eq!(cache.len(), 1);
    }
}
