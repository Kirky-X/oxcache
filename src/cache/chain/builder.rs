// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! ChainCache 构建器
//!
//! 提供 `ChainCacheBuilder` 用于分步构建 `ChainCache` 实例。

use super::{ChainCache, ChainLink};
use crate::backend::BackendScore;
use crate::backend::CacheBackend;
use std::sync::OnceLock;
use std::time::Duration;

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
