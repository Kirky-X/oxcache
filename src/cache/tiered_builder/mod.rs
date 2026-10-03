// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 分层构建器 API
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

use super::chain::{ChainCache, ChainLink, ChainReadStrategy};
use crate::backend::CacheBackend;
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "invalidation")]
use crate::features::invalidation::InvalidationBus;
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
                        ));
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
    read_strategy: ChainReadStrategy,
    default_ttl: Option<Duration>,
    /// 写路径失效总线（`invalidation` feature；透传给 `ChainCacheBuilder`）
    #[cfg(feature = "invalidation")]
    invalidation_bus: Option<Arc<InvalidationBus>>,
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

    pub fn enable_race_read(mut self) -> Self {
        self.read_strategy = ChainReadStrategy::Race;
        self
    }

    /// 设置链路读策略。
    pub fn read_strategy(mut self, strategy: ChainReadStrategy) -> Self {
        self.read_strategy = strategy;
        self
    }

    /// 链默认 TTL
    pub fn default_time_to_live(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// 启用写路径失效广播（`invalidation` feature）。
    ///
    /// 语义与 [`ChainCacheBuilder::with_invalidation`](crate::cache::ChainCacheBuilder::with_invalidation) 一致：构建时将
    /// 持久层（`.persistent(true)`，通常为 L2）link 以 `InvalidatingBackend`
    /// 包装，写成功后经 `bus` 广播失效；expire 不广播。
    #[cfg(feature = "invalidation")]
    pub fn with_invalidation(mut self, bus: Arc<InvalidationBus>) -> Self {
        self.invalidation_bus = Some(bus);
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
            links.push(ChainLink::from_arc(
                l2.build().await?,
                score,
                persistent,
                "l2",
            ));
        }
        for (backend, score, persistent, name) in self.extra {
            links.push(ChainLink::from_arc(backend, score, persistent, name));
        }

        if links.is_empty() {
            return Err(OxCacheError::InvalidInput(
                "ChainBuilder requires at least one layer: add .l1(...) and/or .l2(...)"
                    .to_string(),
            ));
        }

        let mut builder = ChainCache::builder().links(links);
        if self.backfill_enabled {
            builder = builder.enable_backfill();
        }
        if self.read_strategy != ChainReadStrategy::Sequential {
            builder = builder.read_strategy(self.read_strategy);
        }
        if let Some(ttl) = self.default_ttl {
            builder = builder.default_time_to_live(ttl);
        }
        #[cfg(feature = "invalidation")]
        if let Some(bus) = self.invalidation_bus {
            builder = builder.with_invalidation(bus);
        }
        Ok(builder.build())
    }
}

/// 一站式装配带写路径失效广播的分层链（`memory` + `invalidation` feature）。
///
/// 等价于 `ChainBuilder::new().l1(l1).l2(l2).with_invalidation(bus)`：
/// 持久层（`.persistent(true)`，通常为 L2）写成功后经 `bus` 广播失效事件，
/// 其他实例的监听任务失效各自本地 L1；expire 不广播。
///
/// ```rust,ignore
/// use oxcache::cache::{L1Builder, L2Builder, tiered_with_invalidation};
///
/// let chain = tiered_with_invalidation(
///     L1Builder::new().capacity(10_000),
///     L2Builder::new().redis("redis://127.0.0.1:6379").persistent(true),
///     bus,
/// ).await?;
/// ```
#[cfg(feature = "invalidation")]
pub async fn tiered_with_invalidation(
    l1: L1Builder,
    l2: L2Builder,
    bus: Arc<InvalidationBus>,
) -> OxCacheResult<ChainCache> {
    ChainBuilder::new()
        .l1(l1)
        .l2(l2)
        .with_invalidation(bus)
        .build()
        .await
}

#[cfg(test)]
mod tests;
