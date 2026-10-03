// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 热 key 采样观测（审计 F11：库内此前无任何热 key 检测能力）。
//!
//! [`HotKeyTracker`] 为**独立观测组件**，不接入读路径、不影响任何既有 API
//! 的行为与性能；调用方可在 audit 事件流 / metrics 钩子 / 业务层调用
//! [`record`](HotKeyTracker::record)。
//!
//! # Example
//!
//! ```
//! use oxcache::features::hotkey::HotKeyTracker;
//!
//! let tracker = HotKeyTracker::new(16);
//! for _ in 0..100 {
//!     tracker.record("product:42");
//! }
//! tracker.record("product:1");
//!
//! let top = tracker.snapshot_top(10);
//! assert_eq!(top[0].0, "product:42");
//!
//! // 快照时计数半衰：历史热点逐渐让位给新增热点
//! tracker.snapshot_top(10);
//! assert!(tracker.peek("product:42") < 100);
//! ```

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const MIN_SHARDS: usize = 1;
const MAX_SHARDS: usize = 1024;

/// 单个分片：key → 计数（DashMap 分片锁，record 无采样丢失）
type ShardMap = DashMap<String, AtomicU64>;

/// 轻量热 key 追踪器：分片原子计数 + 快照半衰。
///
/// - `record`：一次 hash + 一次原子加（Relaxed，最终一致的近似计数）；
/// - `snapshot_top`：遍历快照返回按计数降序的 top-k，同时全量计数右移
///   1 位半衰，防止历史热点永久霸榜；
/// - 内存量与 shards × 不同 key 数线性：高频大键空间下应定期
///   `snapshot_top` 或 `reset`。
pub struct HotKeyTracker {
    shards: Vec<Arc<ShardMap>>,
}

impl HotKeyTracker {
    /// 创建指定分片数的追踪器（1..=1024，越界收敛到边界）。
    pub fn new(shards: usize) -> Self {
        let shards = shards.clamp(MIN_SHARDS, MAX_SHARDS);
        Self {
            shards: (0..shards).map(|_| Arc::new(DashMap::new())).collect(),
        }
    }

    fn shard_index(&self, key: &str) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }

    /// 记录一次 key 访问（DashMap entry 锁内完成存在性判定与累加，无采样丢失；
    /// `get` + `or_insert` 的两段式在并发首次插入时会丢弃后到者的 +1）。
    pub fn record(&self, key: &str) {
        let idx = self.shard_index(key);
        self.shards[idx]
            .entry(key.to_string())
            .and_modify(|c| {
                c.fetch_add(1, Ordering::Relaxed);
            })
            .or_insert(AtomicU64::new(1));
    }

    /// 读取当前计数（不半衰）。
    pub fn peek(&self, key: &str) -> u64 {
        let idx = self.shard_index(key);
        self.shards[idx]
            .get(key)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// 返回按计数降序的 top-k，并将全量计数右移 1 位半衰。
    pub fn snapshot_top(&self, k: usize) -> Vec<(String, u64)> {
        let mut all: Vec<(String, u64)> = Vec::new();
        for shard in &self.shards {
            for entry in shard.iter() {
                let count = entry.value().load(Ordering::Relaxed);
                if count > 0 {
                    all.push((entry.key().clone(), count));
                }
            }
        }
        all.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        all.truncate(k);

        // 半衰：右移 1 位，新热点可以顶替历史热点；衰减为 0 的条目移除，
        // 防止大键空间下内存线性膨胀
        for shard in &self.shards {
            for entry in shard.iter() {
                let count = entry.value().load(Ordering::Relaxed);
                entry.value().store(count >> 1, Ordering::Relaxed);
            }
            shard.retain(|_, c| c.load(Ordering::Relaxed) > 0);
        }
        all
    }

    /// 清空全部计数。
    pub fn reset(&self) {
        for shard in &self.shards {
            shard.clear();
        }
    }
}

/// 共享句柄别名（多任务共用同一追踪器）。
pub type SharedHotKeyTracker = Arc<HotKeyTracker>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    /// 验收：并发 record 后 top-k 按计数降序
    #[test]
    fn concurrent_record_then_top_k_sorted() {
        let tracker = Arc::new(HotKeyTracker::new(16));
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for t in 0..8u64 {
            let tracker = tracker.clone();
            let barrier = barrier.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                for _ in 0..100 {
                    tracker.record(&format!("key-{}", t % 2));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let top = tracker.snapshot_top(10);
        assert_eq!(top.len(), 2);
        // 两个 key 各 ~400 次（半衰前快照），排序稳定即可
        assert!(top[0].1 >= top[1].1);
        let total: u64 = top.iter().map(|(_, c)| c).sum();
        assert!(total >= 800, "并发采样不应大量丢失，实际 {total}");
    }

    /// 验收：半衰后旧计数下降，新热点可顶替
    #[test]
    fn snapshot_halves_counts() {
        let tracker = HotKeyTracker::new(4);
        for _ in 0..100 {
            tracker.record("old-hot");
        }
        let top = tracker.snapshot_top(10);
        assert_eq!(top[0].0, "old-hot");
        assert_eq!(top[0].1, 100);
        // 快照后半衰：100 → 50
        assert_eq!(tracker.peek("old-hot"), 50);
        // 两次快照后旧热点 < 新热点
        for _ in 0..60 {
            tracker.record("new-hot");
        }
        let top = tracker.snapshot_top(10);
        assert_eq!(top[0].0, "new-hot", "半衰应允许新热点顶替旧热点");
    }

    /// 验收：reset 后快照为空
    #[test]
    fn reset_clears_all() {
        let tracker = HotKeyTracker::new(8);
        tracker.record("k");
        tracker.reset();
        assert!(tracker.snapshot_top(10).is_empty());
    }
}
