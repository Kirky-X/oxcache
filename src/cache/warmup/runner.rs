// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 预热执行器：拉取热 key → 去重 → 限额过滤 → 直供回填 / 批量晋升 → 显性报告

use super::{WarmupEntry, WarmupLoader, WarmupReport};
use crate::backend::CacheReader;
use crate::cache::ChainCache;
use crate::error::OxCacheResult;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

/// 单值大小默认上限：与序列化写入面（`MAX_JSON_SIZE`）同口径——超过该值
/// 的条目即使回填也会被序列化准入拒绝，入口处提前丢弃并显性计数
const DEFAULT_MAX_VALUE_BYTES: usize = crate::core::constants::MAX_JSON_SIZE;

/// 单轮预热条目数默认上限：loader 端口返回无界集合时兜底，防止一次预热
/// 无界放大回填任务与失败明细（可经 builder 调整，显式置 0 解除）
const DEFAULT_MAX_ENTRIES: usize = 100_000;

/// 预热器（经 [`Warmup::builder`] 构造，对目标 [`ChainCache`] 执行预热）
#[derive(Clone)]
pub struct Warmup {
    chain: Arc<ChainCache>,
    /// 回填写入的并发上界（滑窗在飞任务数，下界 1）
    concurrency: usize,
    /// 单值大小上限（字节），0 = 不限
    max_value_bytes: usize,
    /// 单轮条目数上限（去重后按 loader 顺序保留前 N 条），0 = 不限
    max_entries: usize,
}

/// 预热构建器
pub struct WarmupBuilder {
    chain: Arc<ChainCache>,
    concurrency: usize,
    max_value_bytes: usize,
    max_entries: usize,
}

impl WarmupBuilder {
    /// 设置回填写入并发上界（默认 8；0 视为 1 串行）
    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// 设置单值大小上限（字节，默认与序列化写入面 `MAX_JSON_SIZE` 对齐；
    /// 0 = 不限）。超限直供条目在入口处显性丢弃并计入报告
    pub fn max_value_bytes(mut self, max_value_bytes: usize) -> Self {
        self.max_value_bytes = max_value_bytes;
        self
    }

    /// 设置单轮预热条目数上限（默认 100_000；0 = 不限）。作用于去重后
    /// 集合，按 loader 顺序保留前 N 条，超出部分显性丢弃并计入报告
    pub fn max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// 构建预热器
    pub fn build(self) -> Warmup {
        Warmup {
            chain: self.chain,
            concurrency: self.concurrency,
            max_value_bytes: self.max_value_bytes,
            max_entries: self.max_entries,
        }
    }
}

/// 单条回填任务载荷：(key, value, ttl)
type BackfillItem = (String, Vec<u8>, Option<std::time::Duration>);

impl Warmup {
    /// 创建预热构建器（目标链必填）
    pub fn builder(chain: Arc<ChainCache>) -> WarmupBuilder {
        WarmupBuilder {
            chain,
            concurrency: 8,
            max_value_bytes: DEFAULT_MAX_VALUE_BYTES,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// 执行一轮预热：从 loader 拉取热 key 集合并异步回填。
    ///
    /// 失败语义：
    /// - loader 端口失败 → 显性返回 `Err`（本轮预热中止，缓存本体不受影响）；
    /// - 单条回填写入失败 → 不中断整批，计入 [`WarmupReport`] 的
    ///   `failed`/`failures`，最终以 `Ok(report)` 呈现（观测型操作的逐条
    ///   失败不掩盖其余条目的成功，也不向上吞掉明细）；
    /// - 超过单值大小上限 / 条目数上限的条目 → 入口处显性丢弃，分别计入
    ///   `dropped_value_too_large` / `dropped_over_entry_cap`。
    pub async fn run(&self, loader: Arc<dyn WarmupLoader>) -> OxCacheResult<WarmupReport> {
        let entries = loader.load_hot_keys().await?;
        let loader_entries = entries.len();

        // 去重：key 首次出现生效（loader 顺序即优先级）
        let mut seen: HashSet<String> = HashSet::with_capacity(entries.len());
        let mut deduped_entries: Vec<WarmupEntry> = entries
            .into_iter()
            .filter(|e| seen.insert(e.key.clone()))
            .collect();
        let deduped = deduped_entries.len();

        let mut report = WarmupReport {
            loader_entries,
            deduped,
            ..WarmupReport::default()
        };

        // 条目数上限：去重后按 loader 顺序保留前 N 条（顺序即优先级），
        // 超出部分显性丢弃
        if self.max_entries > 0 && deduped_entries.len() > self.max_entries {
            report.dropped_over_entry_cap = deduped_entries.len() - self.max_entries;
            deduped_entries.truncate(self.max_entries);
        }

        // 分区：直供值条目直接回填；仅 key 条目走批量读晋升。直供值来自
        // loader 端口、不经链式写入的序列化准入，单值大小上限在此拦截
        let mut direct: Vec<BackfillItem> = Vec::with_capacity(deduped_entries.len());
        let mut key_only: Vec<String> = Vec::new();
        for entry in deduped_entries {
            match entry.value {
                Some(value) => {
                    if self.max_value_bytes > 0
                        && crate::infra::serialization::utils::check_data_size(
                            &value,
                            self.max_value_bytes,
                            "warmup entry value",
                        )
                        .is_err()
                    {
                        report.dropped_value_too_large += 1;
                        continue;
                    }
                    direct.push((entry.key, value, entry.ttl));
                }
                None => key_only.push(entry.key),
            }
        }

        let (warmed, mut failures, peak) = self.backfill_batch(direct).await;
        report.warmed = warmed;
        report.peak_concurrency = peak;
        report.failed += failures.len();
        report.failures.append(&mut failures);

        if !key_only.is_empty() {
            // 批量读复用 ChainCache::iter_entries（逐层批量读、不触发链内
            // 回填），命中值再统一回填到全链
            let key_refs: Vec<&str> = key_only.iter().map(String::as_str).collect();
            let fetched = self.chain.iter_entries(&key_refs).await;
            let mut promote_items: Vec<BackfillItem> = Vec::new();
            for (key, value) in fetched {
                match value {
                    // 晋升透传源 TTL：查链上剩余 TTL 随回填写入，避免把带
                    // 过期语义的源条目重置为链默认 TTL（stale-forever 风险）；
                    // ttl 查询失败时跳过回填并显性计失败（宁可漏晋升，
                    // 不写无界 TTL）
                    Some(value) => {
                        let ttl = match self.chain.ttl(&key).await {
                            Ok(ttl) => ttl,
                            Err(e) => {
                                report.failed += 1;
                                report.failures.push((key, format!("ttl 查询失败: {e}")));
                                continue;
                            }
                        };
                        promote_items.push((key, value, ttl));
                    }
                    None => report.missing += 1,
                }
            }
            let (promoted, mut promote_failures, promote_peak) =
                self.backfill_batch(promote_items).await;
            report.promoted = promoted;
            report.peak_concurrency = report.peak_concurrency.max(promote_peak);
            report.failed += promote_failures.len();
            report.failures.append(&mut promote_failures);
        }

        Ok(report)
    }

    /// 有界并发回填：Semaphore 滑窗（先取许可再 spawn，任一时刻在飞任务数
    /// 恒 ≤ `concurrency`，批间无屏障空转），值落到全部后端。
    /// 返回 `(成功数, 失败明细 (key, 错误), 并发峰值)`。
    async fn backfill_batch(
        &self,
        items: Vec<BackfillItem>,
    ) -> (usize, Vec<(String, String)>, usize) {
        let mut warmed = 0usize;
        let mut failures: Vec<(String, String)> = Vec::new();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let permits = Arc::new(Semaphore::new(self.concurrency.max(1)));
        let mut set = tokio::task::JoinSet::new();

        for (key, value, ttl) in items {
            let permit = permits
                .clone()
                .acquire_owned()
                .await
                .expect("预热信号量不会被 close");
            let chain = self.chain.clone();
            let in_flight = in_flight.clone();
            let peak = peak.clone();
            set.spawn(async move {
                let now = in_flight.fetch_add(1, Ordering::Relaxed) + 1;
                // fetch_max 竞争下取较大值：任何时刻在飞数的最大值
                // 必被某次加计数后的快照捕获
                peak.fetch_max(now, Ordering::Relaxed);
                let result = chain
                    .set(&key, value, ttl)
                    .await
                    .map_err(|e| (key, e.to_string()));
                in_flight.fetch_sub(1, Ordering::Relaxed);
                drop(permit);
                result
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(())) => warmed += 1,
                Ok(Err((key, err))) => failures.push((key, err)),
                Err(join_err) => failures.push(("<join>".to_string(), join_err.to_string())),
            }
        }

        (warmed, failures, peak.load(Ordering::Relaxed))
    }
}
