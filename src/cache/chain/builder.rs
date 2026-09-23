// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! ChainCache 构建器
//!
//! 提供 `ChainCacheBuilder` 用于分步构建 `ChainCache` 实例。

use super::{ChainCache, ChainLink, ChainReadStrategy};
use crate::backend::BackendScore;
use crate::backend::CacheBackend;
use crate::core::EventPublisher;
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
