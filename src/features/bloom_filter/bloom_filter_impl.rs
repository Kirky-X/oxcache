// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Bloom Filter impl blocks extracted from mod.rs

use super::*;
use bloomfilter::Bloom;
use std::sync::{Arc, RwLock};

impl BloomFilter {
    /// Create a new Bloom filter sized for `capacity` items at the target
    /// `false_positive_rate`.
    ///
    /// `false_positive_rate` must be in `(0.0, 1.0)` and `capacity` must be
    /// greater than `0`.
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "false_positive_rate must be in (0.0, 1.0)"
        );
        let bloom = Bloom::<str>::new_for_fp_rate(capacity, false_positive_rate)
            .expect("failed to create bloom filter: random seed generation failed");
        Self {
            state: Arc::new(RwLock::new(BloomState {
                bloom,
                capacity,
                false_positive_rate,
                inserted_count: 0,
            })),
        }
    }

    /// Record the presence of `key`.
    pub fn insert(&self, key: &str) {
        let mut state = self.state.write().unwrap();
        state.bloom.set(key);
        state.inserted_count += 1;
    }

    /// Check if `key` may be present.
    ///
    /// 对**本过滤器的 insert 集合**无假阴性：每个 insert 过的 key 恒返回
    /// `true`。未插入的 key 通常返回 `false`，也可能返回 `true`（误判率
    /// 为配置的 false positive rate）。
    ///
    /// # 进程边界（重要）
    ///
    /// 过滤器状态为进程内存：进程重启即清零、多实例各自独立、不随后端
    /// 持久化。对共享持久后端（如 Redis）中"已存在但不在本进程插入集合
    /// 内"的 key，`contains` 返回 `false` 属于假阴性——装饰器会据此短路
    /// 返回 miss。重启 / 多实例部署后必须预热对齐（prefill）。
    pub fn contains(&self, key: &str) -> bool {
        let state = self.state.read().unwrap();
        state.bloom.check(key)
    }

    /// Clear all recorded keys, resetting the filter to its initial empty
    /// state without changing capacity or false positive rate.
    pub fn clear(&self) {
        let mut state = self.state.write().unwrap();
        state.bloom.clear();
        state.inserted_count = 0;
    }

    /// Estimated number of inserted items.
    pub fn len(&self) -> u64 {
        let state = self.state.read().unwrap();
        state.inserted_count
    }

    /// Returns `true` if no keys have been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Configured capacity (the item count the filter is sized for).
    pub fn capacity(&self) -> usize {
        let state = self.state.read().unwrap();
        state.capacity
    }

    /// Configured target false positive rate.
    pub fn false_positive_rate(&self) -> f64 {
        let state = self.state.read().unwrap();
        state.false_positive_rate
    }

    /// Current load factor: `inserted_count / capacity`.
    ///
    /// Returns `0.0` when the filter is empty.
    pub fn load_factor(&self) -> f64 {
        let state = self.state.read().unwrap();
        // `new()` asserts `capacity > 0`, so division by zero is impossible.
        state.inserted_count as f64 / state.capacity as f64
    }

    /// Rebuild the filter with `new_capacity`, preserving the configured
    /// false positive rate. All recorded keys are cleared.
    pub fn rebuild(&self, new_capacity: usize) {
        assert!(new_capacity > 0, "new_capacity must be greater than 0");
        let fpr = self.false_positive_rate();
        let new_bloom = Bloom::<str>::new_for_fp_rate(new_capacity, fpr)
            .expect("failed to rebuild bloom filter: random seed generation failed");
        let mut state = self.state.write().unwrap();
        state.bloom = new_bloom;
        state.capacity = new_capacity;
        state.inserted_count = 0;
    }
}
