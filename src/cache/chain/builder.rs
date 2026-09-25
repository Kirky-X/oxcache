// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! ChainCache 构建器
//!
//! 提供 `ChainCacheBuilder` 用于分步构建 `ChainCache` 实例。

use super::{ChainCache, ChainLink, ChainReadStrategy};
use crate::backend::BackendScore;
use crate::backend::CacheBackend;
use crate::core::EventPublisher;
#[cfg(feature = "invalidation")]
use crate::features::invalidation::{InvalidatingBackend, InvalidationBus};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

/// 链式缓存构建器
#[derive(Default)]
pub struct ChainCacheBuilder {
    links: Vec<ChainLink>,
    backfill_enabled: bool,
    read_strategy: ChainReadStrategy,
    default_ttl: Option<Duration>,
    event_publisher: Option<Arc<dyn EventPublisher>>,
    /// 写路径失效总线（`invalidation` feature；构建时包装持久层 link）
    #[cfg(feature = "invalidation")]
    invalidation_bus: Option<Arc<InvalidationBus>>,
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

    /// 兼容别名：启用竞速读策略（映射 [`ChainReadStrategy::Race`]，
    /// 即原「并发查询所有后端、取分数最高命中」语义，问题 4.1）。
    /// 新代码建议直接使用 [`Self::read_strategy`]。
    pub fn enable_race_read(mut self) -> Self {
        self.read_strategy = ChainReadStrategy::Race;
        self
    }

    /// 兼容别名：禁用竞速读（映射 [`ChainReadStrategy::Sequential`]）。
    pub fn disable_race_read(mut self) -> Self {
        self.read_strategy = ChainReadStrategy::Sequential;
        self
    }

    /// 设置链路读策略（absorb-hitbox-features T014）。
    ///
    /// - [`ChainReadStrategy::Sequential`]：逐个读取，命中即返回（默认）
    /// - [`ChainReadStrategy::Race`]：并发全读，取分数最高命中
    /// - [`ChainReadStrategy::ParallelFreshest`]：并发全读 + 并发查 TTL，
    ///   取剩余最长（最新鲜）命中
    pub fn read_strategy(mut self, strategy: ChainReadStrategy) -> Self {
        self.read_strategy = strategy;
        self
    }

    /// 设置事件发布器
    ///
    /// 配置后，链式缓存的后端操作失败会通过 `EventPublisher` 抛出事件，
    /// 而非日志输出。用户可自行决定处理方式（日志、metrics、告警或忽略）。
    pub fn event_publisher(mut self, publisher: Arc<dyn EventPublisher>) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    /// 启用写路径失效广播（`invalidation` feature）。
    ///
    /// 构建时将所有**持久层**（`is_persistent == true`）link 的后端以
    /// [`InvalidatingBackend`](crate::features::invalidation::InvalidatingBackend)
    /// 包装：该层 set/delete/clear/set_many/delete_many 成功后经 `bus` 广播
    /// 失效事件，其他实例的监听任务失效各自本地缓存；expire 既有语义不变
    /// （不广播）。非持久层（本地 L1）不包装、不广播。
    ///
    /// 注意：包装层不实现 `SyncCacheBackend`，含持久层 link 的链此后不再
    /// 支持 sync API（返回 `NotSupported`）。
    #[cfg(feature = "invalidation")]
    pub fn with_invalidation(mut self, bus: Arc<InvalidationBus>) -> Self {
        self.invalidation_bus = Some(bus);
        self
    }

    /// 构建链式缓存
    ///
    /// # Panics
    ///
    /// Panics if no links were added — an empty chain silently turns every
    /// `get` into a `None` miss, which is nearly always a misconfiguration.
    /// This is a programmer error (forgot `.link(...)`/`.backend(...)`), so it
    /// fails loudly at construction time; use [`ChainCache::new`] directly if
    /// an intentionally empty chain is ever required.
    pub fn build(self) -> ChainCache {
        // 按分数降序排序
        let mut links = self.links;
        links.sort_by_key(|link| std::cmp::Reverse(link.score()));
        assert!(
            !links.is_empty(),
            "ChainCacheBuilder::build requires at least one link; \
             add one via .link(...) or .backend(...), or use ChainCache::new \
             for an intentionally empty chain"
        );

        // 持久层 link 以失效广播装饰器包装（仅 `invalidation` feature；
        // 语义与 `InvalidatingBackend` 既有行为一致：set/delete 广播，expire 不广播）
        #[cfg(feature = "invalidation")]
        if let Some(bus) = self.invalidation_bus {
            links = links
                .into_iter()
                .map(|link| {
                    if link.is_persistent() {
                        let wrapped = Arc::new(InvalidatingBackend::new(
                            link.backend().clone(),
                            bus.clone(),
                        ));
                        ChainLink::from_arc(
                            wrapped,
                            link.score(),
                            link.is_persistent(),
                            link.name(),
                        )
                    } else {
                        link
                    }
                })
                .collect();
        }

        ChainCache {
            links,
            backfill_enabled: self.backfill_enabled,
            read_strategy: self.read_strategy,
            default_ttl: self.default_ttl,
            sync_backends: OnceLock::new(),
            event_publisher: self.event_publisher,
        }
    }
}
