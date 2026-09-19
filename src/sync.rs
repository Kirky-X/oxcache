// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 同步字节权重缓存（tokio-free，直连 `moka::sync`）
//!
//! 为进程内 L1 场景提供按**字节预算**（而非条目数）控容量的同步缓存封装：
//! 容量单位是值的估算字节数（weigher 闭包注入），配合单条准入阈值与大结果
//! single-flight，防止单条巨型值击穿内存预算。与 [`crate::cache`] 的
//! future/多层封装互补：本模块不依赖 tokio 运行时，适合嵌入同步调用方。
//!
//! # 能力
//!
//! - **字节权重预算**：`max_capacity_bytes` 经 moka weigher 生效，逐出以
//!   估算字节为准；
//! - **单条准入阈值**：`with_max_entry_bytes` 后，[`insert`](ByteWeightCache::insert)
//!   显式跳过超阈值的大值（大结果不入缓存，但 [`get_or_compute`](ByteWeightCache::get_or_compute)
//!   路径由 weigher 预算兜底逐出、返回值不受影响）；
//! - **single-flight**：基于 moka `try_get_with`，并发相同 key 仅 leader 执行
//!   计算，错误原样传播给所有等待者且不写缓存（无 "backend error" 包装）；
//! - **命中统计**：moka 0.12 不内建计数器，由本模块维护（leader 计 miss、
//!   follower 计 hit）。
//!
//! # Example
//!
//! ```rust
//! use oxcache::sync::ByteWeightCache;
//!
//! // 1MB 预算，按字符串字节长度计权重，单条上限 4KB
//! let cache: ByteWeightCache<String, String, _> =
//!     ByteWeightCache::new(1024 * 1024, |v: &String| v.len() as u64).with_max_entry_bytes(4096);
//!
//! cache.insert("k1".to_string(), "v1".to_string());
//! assert_eq!(cache.get(&"k1".to_string()), Some("v1".to_string()));
//!
//! // 超过单条准入阈值的结果不入缓存
//! let big = "x".repeat(8192);
//! cache.insert("k2".to_string(), big);
//! assert_eq!(cache.get(&"k2".to_string()), None);
//! ```

use std::hash::Hash;
use std::hash::RandomState;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 缓存统计快照（指标端点消费）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteWeightStats {
    /// 命中次数（single-flight follower 共享结果计 hit）。
    pub hits: u64,
    /// 未命中次数（single-flight leader 实际执行计算计 miss）。
    pub misses: u64,
    /// 当前条目数（moka 最终一致值）。
    pub entry_count: u64,
}

/// 同步字节权重缓存。
///
/// 线程安全（`Send + Sync`），容量为字节权重预算，无时间 TTL（仅权重逐出）。
/// `W` 为值体积估算闭包：返回值仅需下界近似（准入决策与逐出排序用途）。
pub struct ByteWeightCache<K, V, W>
where
    K: Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    W: Fn(&V) -> u64 + Send + Sync + 'static,
{
    inner: moka::sync::Cache<K, V, RandomState>,
    max_entry_bytes: Option<u64>,
    // Arc 共享：moka weigher 闭包与准入检查各持一份
    weigher: Arc<W>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl<K, V, W> ByteWeightCache<K, V, W>
where
    K: Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    W: Fn(&V) -> u64 + Send + Sync + 'static,
{
    /// 创建指定字节预算的缓存（无单条准入阈值）。
    ///
    /// moka weigher 权重类型为 `u32`，估算值超过 `u32::MAX` 时截断。
    pub fn new(max_capacity_bytes: u64, weigher: W) -> Self {
        let weigher = Arc::new(weigher);
        let moka_weigher = weigher.clone();
        let inner = moka::sync::Cache::builder()
            .max_capacity(max_capacity_bytes)
            .weigher(move |_k, v: &V| (moka_weigher)(v).min(u32::MAX as u64) as u32)
            .build();
        Self {
            inner,
            max_entry_bytes: None,
            weigher,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// 设置单条准入阈值：估算超过该值的条目在 [`insert`](Self::insert) 时跳过写入。
    pub fn with_max_entry_bytes(mut self, max_entry_bytes: u64) -> Self {
        self.max_entry_bytes = Some(max_entry_bytes);
        self
    }

    /// 查询缓存。命中返回值的克隆，未命中返回 `None`。
    pub fn get(&self, key: &K) -> Option<V> {
        match self.inner.get(key) {
            Some(v) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(v)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// 写入缓存（设置了单条准入阈值时，超阈值的大值跳过写入）。
    pub fn insert(&self, key: K, value: V) {
        if let Some(max) = self.max_entry_bytes
            && (self.weigher)(&value) > max
        {
            return;
        }
        self.inner.insert(key, value);
    }

    /// 查询或计算（single-flight）。
    ///
    /// 并发相同 key 仅 leader 执行 `compute`，等待者共享 leader 结果；
    /// `compute` 返回 `Err` 时错误原样传播给所有等待者且不写缓存。
    /// （moka `try_get_with` 的错误以 `Arc` 共享，此处解包/克隆还原。）
    pub fn get_or_compute<F, E>(&self, key: K, compute: F) -> Result<V, E>
    where
        F: FnOnce() -> Result<V, E>,
        E: Clone + Send + Sync + 'static,
    {
        let ran = AtomicBool::new(false);
        let outcome = self.inner.try_get_with(key, || {
            ran.store(true, Ordering::Relaxed);
            compute()
        });
        if ran.load(Ordering::Relaxed) {
            // leader 实际执行 compute：计一次 miss（follower 共享结果计 hit）
            self.misses.fetch_add(1, Ordering::Relaxed);
        } else {
            self.hits.fetch_add(1, Ordering::Relaxed);
        }
        match outcome {
            Ok(v) => Ok(v),
            Err(e) => Err(Arc::unwrap_or_clone(e)),
        }
    }

    /// 当前缓存条目数（先同步执行 moka 待维护任务，保证读取时点准确）。
    pub fn entry_count(&self) -> u64 {
        self.inner.run_pending_tasks();
        self.inner.entry_count()
    }

    /// 缓存统计。
    pub fn stats(&self) -> ByteWeightStats {
        self.inner.run_pending_tasks();
        ByteWeightStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            entry_count: self.inner.entry_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;
    use std::time::Duration;

    /// 字节长度权重 + 1KB 预算的小缓存。
    fn small_cache() -> ByteWeightCache<String, String, impl Fn(&String) -> u64> {
        ByteWeightCache::new(1024, |v: &String| v.len() as u64)
    }

    #[test]
    fn test_get_hit_miss_and_clone_semantics() {
        let cache = small_cache();
        assert_eq!(cache.get(&"k".to_string()), None);
        cache.insert("k".to_string(), "v".to_string());
        assert_eq!(cache.get(&"k".to_string()), Some("v".to_string()));
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_oversized_entry_skipped_by_admission() {
        let cache =
            ByteWeightCache::new(64 * 1024, |v: &String| v.len() as u64).with_max_entry_bytes(256);
        // 512 字节 > 256 阈值：跳过写入，但 get_or_compute 返回值不受影响
        let big = "x".repeat(512);
        cache.insert("big".to_string(), big.clone());
        assert_eq!(cache.get(&"big".to_string()), None);
        let got = cache
            .get_or_compute("big".to_string(), || Ok::<String, String>(big.clone()))
            .unwrap();
        assert_eq!(got.len(), 512);
    }

    #[test]
    fn test_byte_budget_bounds_entries() {
        // 预算 100 字节，每条 ~64 字节：至多容纳 1 条
        let cache = ByteWeightCache::new(100, |v: &String| v.len() as u64);
        for i in 0..6 {
            cache.insert(format!("k{i}"), "x".repeat(64));
        }
        assert!(
            cache.entry_count() <= 2,
            "字节预算应限制条目数，实际 {}",
            cache.entry_count()
        );
    }

    #[test]
    fn test_error_not_cached_and_propagates() {
        let cache = small_cache();
        let err = cache
            .get_or_compute("k".to_string(), || Err("boom".to_string()))
            .unwrap_err();
        assert_eq!(err, "boom");
        assert_eq!(cache.get(&"k".to_string()), None, "错误结果不写入缓存");
    }

    #[test]
    fn test_single_flight_compute_exactly_once() {
        let cache = Arc::new(small_cache());
        let count = Arc::new(AtomicU64::new(0));
        let barrier = Arc::new(Barrier::new(8));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let cache = cache.clone();
                let count = count.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    cache
                        .get_or_compute("hot".to_string(), || {
                            count.fetch_add(1, Ordering::SeqCst);
                            thread::sleep(Duration::from_millis(20));
                            Ok::<_, String>("v".to_string())
                        })
                        .unwrap()
                })
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), "v");
        }
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "single-flight：compute 应恰执行 1 次"
        );
        // leader 计 miss、7 个 follower 计 hit
        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 7);
    }

    #[test]
    fn test_stats_snapshot_fields() {
        let cache = small_cache();
        let _ = cache.get(&"miss".to_string());
        cache.insert("hit".to_string(), "v".to_string());
        let _ = cache.get(&"hit".to_string());
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entry_count, 1);
    }
}
