// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Bloom Filter impl blocks extracted from mod.rs

use super::*;
use bloomfilter::Bloom;
use std::hash::Hash;
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

impl<K: ?Sized> BloomFilter<K> {
    /// Create a new Bloom filter with an exact hash-function count.
    ///
    /// `hash_count` 为哈希函数个数（≥1）。crate 无直接指定 k 的构造入口——
    /// k 由 `optimal_k_num(bitmap_bits, items_count)` 从位图位数与容量推导，
    /// 且随位数单调不减；此处对位图字节数做二分，反解出使 k 恰为目标值的
    /// 位图大小，再以随机种子构造。容量极小（如 capacity=1）时相邻字节的
    /// k 步进大于 1，部分 `hash_count` 不可达（panic，见断言）。
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is `0`, `hash_count` is `0`, `false_positive_rate`
    /// is not in `(0.0, 1.0)`, or the target `hash_count` is unreachable for
    /// the given `capacity`.
    pub fn new_with_hash_count(capacity: usize, false_positive_rate: f64, hash_count: u32) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(hash_count >= 1, "hash_count must be greater than 0");
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "false_positive_rate must be in (0.0, 1.0)"
        );

        // k_num 只依赖 (bitmap_bits, items_count)，与种子无关：用全零种子
        // 探针反解，最终实例以随机种子构造
        let k_num_at = |bitmap_size: usize| {
            Bloom::<K>::new_with_seed(bitmap_size, capacity, &[0u8; 32])
                .expect("failed to probe bloom filter size")
                .number_of_hash_functions()
        };

        // 上界按 bits ≈ capacity·k/ln2 估算并放大，保证 k_num(hi) ≥ 目标
        let mut hi = ((capacity as f64) * f64::from(hash_count) / std::f64::consts::LN_2).ceil()
            as usize
            / 8
            + 16;
        if k_num_at(hi) < hash_count {
            panic!("hash_count {hash_count} unreachable for capacity {capacity}");
        }

        let mut lo = 1usize;
        let mut size = hi;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if k_num_at(mid) >= hash_count {
                size = mid;
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        assert_eq!(
            k_num_at(size),
            hash_count,
            "hash_count {hash_count} unreachable for capacity {capacity}: \
             k steps by more than 1 per bitmap byte at this capacity"
        );

        let bloom = Bloom::<K>::new(size, capacity)
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

    /// 哈希函数个数（crate 由容量与目标误判率推导或由 `new_with_hash_count`
    /// 指定）。
    pub fn hash_count(&self) -> u32 {
        let state = self.state.read().unwrap();
        state.bloom.number_of_hash_functions()
    }

    /// 位图中已置 1 的位数（饱和度观测：越接近总位数，假阳性率越退化）。
    ///
    /// 序列化布局为「头部位图元数据 + 数据区」，数据区字节数恰为总位数/8；
    /// 从切片尾部切出数据区统计置位数，不依赖私有头部长度常量。
    pub fn set_bits(&self) -> u64 {
        let state = self.state.read().unwrap();
        let bloom = &state.bloom;
        let bits = bloom.len() as usize;
        let data_bytes = bits.div_ceil(8);
        let slice = bloom.as_slice();
        let data = &slice[slice.len() - data_bytes..];
        data.iter().map(|b| u64::from(b.count_ones())).sum()
    }
}

impl<K: ?Sized + Hash> BloomFilter<K> {
    /// Record the presence of `key`.
    pub fn insert(&self, key: &K) {
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
    pub fn contains(&self, key: &K) -> bool {
        let state = self.state.read().unwrap();
        state.bloom.check(key)
    }
}
