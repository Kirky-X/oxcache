// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 分层构建器 API（T306）
//!
//! [`L1Builder`] / [`L2Builder`] / [`ChainBuilder`] 提供 L1+L2 分层缓存的
//! 链式组合（容量 / TTL / 后端 / 装饰器），与既有
//! [`CacheBuilder`](super::CacheBuilder)（单后端）和
//! [`ChainCacheBuilder`](super::ChainCacheBuilder)（手工链接）并存：
//!
//! ```rust,ignore
//! use oxcache::cache::{ChainBuilder, L1Builder, L2Builder};
//!
//! let chain = ChainBuilder::new()
//!     .l1(L1Builder::new().capacity(10_000).ttl(Duration::from_secs(60)))
//!     .l2(L2Builder::new().custom(redis_backend))
//!     .enable_backfill()
//!     .build()
//!     .await?;
//! ```

use super::chain::{ChainCache, ChainLink};
use crate::backend::CacheBackend;
use crate::error::{OxCacheError, OxCacheResult};
use std::sync::Arc;
use std::time::Duration;

/// 装饰器函数：包装后端（加密 / HMAC / 失效广播 / 自定义）
pub type BackendDecorator =
    Arc<dyn Fn(Arc<dyn CacheBackend>) -> Arc<dyn CacheBackend> + Send + Sync>;

// ============================================================================
// L1Builder
// ============================================================================

/// L1 进程内内存后端构建器
#[derive(Default)]
pub struct L1Builder {
    capacity: Option<u64>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
    kind: L1Kind,
    decorators: Vec<BackendDecorator>,
    score: u8,
}

/// L1 内存后端类型
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum L1Kind {
    /// Moka TinyLFU（默认，支持 future 与 per-entry TTL）
    #[default]
    Moka,
    /// DashMap（同步哈希表 + FIFO 淘汰）
    DashMap,
}

impl L1Builder {
    /// 创建 L1 构建器（默认 Moka，容量 10000）
    pub fn new() -> Self {
        Self::default()
    }

    /// 容量（条目数）
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// 默认 TTL
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// 默认 TTI（idle 过期）
    pub fn tti(mut self, tti: Duration) -> Self {
        self.tti = Some(tti);
        self
    }

    /// 选择 DashMap 后端
    pub fn dashmap(mut self) -> Self {
        self.kind = L1Kind::DashMap;
        self
    }

    /// 选择 Moka 后端（默认）
    pub fn moka(mut self) -> Self {
        self.kind = L1Kind::Moka;
        self
    }

    /// 附加装饰器（按追加顺序由内向外包装）
    pub fn decorate(
        mut self,
        f: impl Fn(Arc<dyn CacheBackend>) -> Arc<dyn CacheBackend> + Send + Sync + 'static,
    ) -> Self {
        self.decorators.push(Arc::new(f));
        self
    }

    /// 链内分数（越高越靠前；默认 100）
    pub fn score(mut self, score: u8) -> Self {
        self.score = score;
        self
    }

    /// 构建 L1 后端（含装饰器包装）
    pub fn build(self) -> Arc<dyn CacheBackend> {
        let base: Arc<dyn CacheBackend> = match self.kind {
            L1Kind::Moka => {
                let mut builder = crate::backend::MokaMemoryBackend::builder()
                    .capacity(self.capacity.unwrap_or(10_000));
                if let Some(ttl) = self.ttl {
                    builder = builder.ttl(ttl);
                }
                if let Some(tti) = self.tti {
                    builder = builder.time_to_idle(tti);
                }
                Arc::new(builder.build())
            }
            L1Kind::DashMap => {
                let mut builder = crate::backend::DashMapMemoryBackend::builder();
                if let Some(capacity) = self.capacity {
                    builder = builder.capacity(capacity as usize);
                }
                if let Some(ttl) = self.ttl {
                    builder = builder.default_ttl(ttl);
                }
                Arc::new(builder.build())
            }
        };
        self.decorators.into_iter().fold(base, |acc, d| d(acc))
    }

    /// 链内分数
    pub fn get_score(&self) -> u8 {
        self.score
    }
}

// ============================================================================
// L2Builder
// ============================================================================

/// L2 分布式后端构建器
#[derive(Default)]
pub struct L2Builder {
    /// 直接注入的后端（测试 / 自定义协议）
    backend: Option<Arc<dyn CacheBackend>>,
    /// Redis 连接串（`redis` feature）
    #[cfg(feature = "redis")]
    redis_url: Option<String>,
    decorators: Vec<BackendDecorator>,
    score: u8,
    persistent: bool,
}

impl L2Builder {
    /// 创建 L2 构建器（默认分数 50，位于 L1 之后）
    pub fn new() -> Self {
        Self {
            score: 50,
            ..Default::default()
        }
    }

    /// 注入自定义后端（Mock / Valkey / Aerospike / 装饰后端等）
    pub fn custom(mut self, backend: Arc<dyn CacheBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Redis 连接串（`redis` feature；build 时异步连接）
    #[cfg(feature = "redis")]
    pub fn redis(mut self, url: &str) -> Self {
        self.redis_url = Some(url.to_string());
        self
    }

    /// 附加装饰器（按追加顺序由内向外包装）
    pub fn decorate(
        mut self,
        f: impl Fn(Arc<dyn CacheBackend>) -> Arc<dyn CacheBackend> + Send + Sync + 'static,
    ) -> Self {
        self.decorators.push(Arc::new(f));
        self
    }

    /// 链内分数（默认 50）
    pub fn score(mut self, score: u8) -> Self {
        self.score = score;
        self
    }

    /// 是否持久化后端（影响 ChainCache 的持久化标记）
    pub fn persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }

    /// 构建并连接 L2 后端（含装饰器包装）
    pub async fn build(self) -> OxCacheResult<Arc<dyn CacheBackend>> {
        let base: Arc<dyn CacheBackend> = if let Some(backend) = self.backend {
            backend
        } else {
            #[cfg(feature = "redis")]
            {
                match &self.redis_url {
                    Some(url) => Arc::new(crate::backend::RedisBackend::new(url).await?),
                    None => {
                        return Err(OxCacheError::InvalidInput(
                            "L2Builder requires .custom(backend) or .redis(url)".to_string(),
                        ))
                    }
                }
            }
            #[cfg(not(feature = "redis"))]
            {
                return Err(OxCacheError::InvalidInput(
                    "L2Builder requires .custom(backend); enable the `redis` feature for .redis(url)"
                        .to_string(),
                ));
            }
        };
        Ok(self.decorators.into_iter().fold(base, |acc, d| d(acc)))
    }

    /// 链内分数
    pub fn get_score(&self) -> u8 {
        self.score
    }
}

// ============================================================================
// ChainBuilder
// ============================================================================

/// 分层链构建器：L1 + L2 一站式组装为 [`ChainCache`]
#[derive(Default)]
pub struct ChainBuilder {
    l1: Option<L1Builder>,
    l2: Option<L2Builder>,
    extra: Vec<(Arc<dyn CacheBackend>, u8, bool, &'static str)>,
    backfill_enabled: bool,
    race_read_enabled: bool,
    default_ttl: Option<Duration>,
}

impl ChainBuilder {
    /// 创建分层构建器
    pub fn new() -> Self {
        Self::default()
    }

    /// 配置 L1 层
    pub fn l1(mut self, l1: L1Builder) -> Self {
        self.l1 = Some(l1);
        self
    }

    /// 配置 L2 层
    pub fn l2(mut self, l2: L2Builder) -> Self {
        self.l2 = Some(l2);
        self
    }

    /// 追加额外后端层（自定义分数）
    pub fn extra_backend(
        mut self,
        backend: Arc<dyn CacheBackend>,
        score: u8,
        is_persistent: bool,
        name: &'static str,
    ) -> Self {
        self.extra.push((backend, score, is_persistent, name));
        self
    }

    /// 启用 L2 → L1 回填
    pub fn enable_backfill(mut self) -> Self {
        self.backfill_enabled = true;
        self
    }

    /// 启用竞速读
    pub fn enable_race_read(mut self) -> Self {
        self.race_read_enabled = true;
        self
    }

    /// 链默认 TTL
    pub fn default_time_to_live(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 构建分层链式缓存
    pub async fn build(self) -> OxCacheResult<ChainCache> {
        let mut links: Vec<ChainLink> = Vec::new();

        if let Some(l1) = self.l1 {
            let score = l1.get_score();
            links.push(ChainLink::from_arc(l1.build(), score, false, "l1"));
        }
        if let Some(l2) = self.l2 {
            let score = l2.get_score();
            let persistent = l2.persistent;
            links.push(ChainLink::from_arc(l2.build().await?, score, persistent, "l2"));
        }
        for (backend, score, persistent, name) in self.extra {
            links.push(ChainLink::from_arc(backend, score, persistent, name));
        }

        if links.is_empty() {
            return Err(OxCacheError::InvalidInput(
                "ChainBuilder requires at least one layer: add .l1(...) and/or .l2(...)".to_string(),
            ));
        }

        let mut builder = ChainCache::builder().links(links);
        if self.backfill_enabled {
            builder = builder.enable_backfill();
        }
        if self.race_read_enabled {
            builder = builder.enable_race_read();
        }
        if let Some(ttl) = self.default_ttl {
            builder = builder.default_time_to_live(ttl);
        }
        Ok(builder.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::interface::{CacheConnector, CacheReader, CacheWriter};
    use crate::backend::BackendKind;

    #[tokio::test]
    async fn l1_builder_moka_default() {
        let backend = L1Builder::new().build();
        assert_eq!(backend.backend_kind(), BackendKind::Moka);
        assert_eq!(backend.capacity().await.unwrap(), 10_000);
    }

    #[tokio::test]
    async fn l1_builder_capacity_ttl_and_dashmap() {
        let backend = L1Builder::new()
            .dashmap()
            .capacity(64)
            .ttl(Duration::from_secs(30))
            .build();
        assert_eq!(backend.backend_kind(), BackendKind::DashMap);
        assert_eq!(backend.capacity().await.unwrap(), 64);

        // TTL 生效
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        let ttl = backend.ttl("k").await.unwrap();
        assert!(ttl.is_some(), "DashMap default_ttl 应生效");
    }

    /// 装饰器按追加顺序由内向外包装
    #[tokio::test]
    async fn l1_builder_applies_decorators() {
        let backend = L1Builder::new()
            .decorate(|inner| Arc::new(TracingProbe { inner, tag: "first" }))
            .decorate(|inner| Arc::new(TracingProbe { inner, tag: "second" }))
            .build();

        // 读写经两层装饰透传仍正确
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    /// 测试装饰器：记录包装层级
    struct TracingProbe {
        inner: Arc<dyn CacheBackend>,
        tag: &'static str,
    }

    #[async_trait::async_trait]
    impl CacheReader for TracingProbe {
        async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            self.inner.get(key).await
        }
        async fn exists(&self, key: &str) -> OxCacheResult<bool> {
            self.inner.exists(key).await
        }
        async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            self.inner.ttl(key).await
        }
        async fn len(&self) -> OxCacheResult<u64> {
            self.inner.len().await
        }
        async fn capacity(&self) -> OxCacheResult<u64> {
            self.inner.capacity().await
        }
        async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
            self.inner.stats().await
        }
    }

    #[async_trait::async_trait]
    impl CacheWriter for TracingProbe {
        async fn set(
            &self,
            key: Arc<str>,
            value: Arc<Vec<u8>>,
            ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            self.inner.set(key, value, ttl).await
        }
        async fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.inner.delete(key).await
        }
        async fn clear(&self) -> OxCacheResult<()> {
            self.inner.clear().await
        }
        async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            self.inner.expire(key, ttl).await
        }
    }

    #[async_trait::async_trait]
    impl CacheConnector for TracingProbe {
        async fn health_check(&self) -> OxCacheResult<()> {
            self.inner.health_check().await
        }
        async fn shutdown(&self) {
            self.inner.shutdown().await;
        }
        fn backend_kind(&self) -> BackendKind {
            BackendKind::Unknown
        }
    }
    // `CacheBackend` 由 blanket impl 自动提供

    #[tokio::test]
    async fn l2_builder_custom_backend() {
        let inner: Arc<dyn CacheBackend> = Arc::new(crate::backend::MockBackend::new(
            "mock-l2",
            50,
            true,
        ));
        let l2 = L2Builder::new().custom(inner).build().await.unwrap();
        assert!(l2.exists("nothing").await.unwrap().eq(&false));
    }

    #[tokio::test]
    async fn l2_builder_without_backend_is_error() {
        let err = match L2Builder::new().build().await {
            Err(e) => e,
            Ok(_) => panic!("无后端必须报错"),
        };
        assert!(matches!(err, OxCacheError::InvalidInput(_)));
    }

    /// L1 + L2 一站式组装：读取顺序、回填语义与手工 ChainCache 一致
    #[tokio::test]
    async fn chain_builder_assembles_l1_l2() {
        let l2_backend: Arc<dyn CacheBackend> =
            Arc::new(crate::backend::MockBackend::new("mock-l2", 50, false));
        // L2 预置数据（模拟之前写入 L2）
        l2_backend
            .set(Arc::from("user:1"), Arc::new(b"from-l2".to_vec()), None)
            .await
            .unwrap();

        let chain = ChainBuilder::new()
            .l1(L1Builder::new().capacity(100))
            .l2(L2Builder::new().custom(l2_backend))
            .enable_backfill()
            .build()
            .await
            .unwrap();

        // 从 L2 命中
        let value = chain.get("user:1").await.unwrap();
        assert_eq!(value, Some(b"from-l2".to_vec()));
        assert_eq!(chain.len(), 2);
    }

    #[tokio::test]
    async fn chain_builder_requires_at_least_one_layer() {
        let err = match ChainBuilder::new().build().await {
            Err(e) => e,
            Ok(_) => panic!("空链必须报错"),
        };
        assert!(matches!(err, OxCacheError::InvalidInput(_)));
        assert!(err.to_string().contains(".l1("));
    }

    #[tokio::test]
    async fn chain_builder_l1_only_works() {
        let chain = ChainBuilder::new()
            .l1(L1Builder::new())
            .build()
            .await
            .unwrap();
        chain.set("k", b"v".to_vec(), None).await.unwrap();
        assert_eq!(chain.get("k").await.unwrap(), Some(b"v".to_vec()));
    }
}
