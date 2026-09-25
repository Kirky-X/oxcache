// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Bloom Filter module for negative query filtering.
//!
//! Provides [`BloomFilter`] — a capacity/fpr-configurable Bloom filter backed
//! by the `bloomfilter` crate. State is shared via `Arc<RwLock<>>` so that
//! [`BloomFilter`] is cheaply `Clone` and mutations are visible across clones.
//!
//! 泛型键类型 [`BloomFilter<K>`](BloomFilter)（`K: ?Sized`，默认 `str`，既有
//! 消费者源码零改动）：K=str 走 crate 原生逐位哈希路径不变；K≠str 的哈希
//! 质量依赖 `std Hash` 实现（仅影响假阳性率，无假阴性）。

#[cfg(any(feature = "memory", feature = "redis"))]
mod backend;

#[cfg(any(feature = "memory", feature = "redis"))]
pub use backend::{BloomFilterBackend, BloomFilterBackendBuilder};

mod bloom_filter_impl;

use std::sync::{Arc, RwLock};

use bloomfilter::Bloom;

/// Internal mutable state guarded by an `RwLock`.
struct BloomState<K: ?Sized> {
    bloom: Bloom<K>,
    capacity: usize,
    false_positive_rate: f64,
    inserted_count: u64,
}

/// A Bloom filter for negative query filtering.
///
/// Sized for an estimated `capacity` (maximum number of items) at a target
/// `false_positive_rate`. Backed by [`bloomfilter::Bloom`] and shared through
/// `Arc<RwLock<>>`, so cloning a `BloomFilter` shares the underlying state —
/// inserts on one clone are visible to all others.
///
/// 键类型 `K` 为 `?Sized`，默认 `str`——裸名 `BloomFilter` 即 `BloomFilter<str>`，
/// 既有消费者源码零改动；`insert`/`contains` 对任意 `K: Hash` 生效（如
/// `BloomFilter<u64>`）。K≠str 的哈希质量依赖其 `std Hash` 实现：只影响
/// 假阳性率，无假阴性。
///
/// # Panics
///
/// `new` and `rebuild` panic if `capacity` is `0` or `false_positive_rate` is
/// not in the open interval `(0.0, 1.0)`, or if the underlying seed generation
/// fails.
pub struct BloomFilter<K: ?Sized = str> {
    state: Arc<RwLock<BloomState<K>>>,
}

// 手写 Clone（仅 clone Arc）：derive(Clone) 会生成 `K: Clone` bound，
// 与 K: ?Sized（str 非 Sized 亦非 Clone）冲突。
impl<K: ?Sized> Clone for BloomFilter<K> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_insert_contains() {
        let bf = BloomFilter::new(1000, 0.01);
        bf.insert("key1");
        // Inserted key must return true (no false negatives).
        assert!(bf.contains("key1"));
    }

    #[test]
    fn test_bloom_filter_no_false_negatives() {
        let bf = BloomFilter::new(10_000, 0.01);
        for i in 0..1000 {
            bf.insert(&format!("key:{}", i));
        }
        // Every inserted key must return true (BF has no false negatives).
        for i in 0..1000 {
            assert!(
                bf.contains(&format!("key:{}", i)),
                "false negative for key:{}",
                i
            );
        }
        // A non-inserted key should return false (true negative).
        assert!(!bf.contains("non-inserted-key"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate_within_bounds() {
        let capacity = 100_000;
        let fpr = 0.01;
        let bf = BloomFilter::new(capacity, fpr);
        // Insert 100000 distinct keys.
        for i in 0..capacity {
            bf.insert(&format!("inserted:{}", i));
        }
        // Query 100000 non-inserted keys, count false positives.
        let mut false_positives = 0u64;
        for i in 0..capacity {
            if bf.contains(&format!("query:{}", i)) {
                false_positives += 1;
            }
        }
        // Theoretical 1% = 1000; allow up to 1500 (50% margin per spec).
        assert!(
            false_positives < 1500,
            "false positives {} exceeded threshold 1500",
            false_positives
        );
    }

    #[test]
    fn test_bloom_filter_clear_resets_state() {
        let bf = BloomFilter::new(1000, 0.01);
        bf.insert("key1");
        bf.insert("key2");
        assert_eq!(bf.len(), 2);
        bf.clear();
        assert_eq!(bf.len(), 0);
        assert!(!bf.contains("key1"));
        assert!(!bf.contains("key2"));
    }

    #[test]
    fn test_bloom_filter_rebuild_changes_capacity() {
        let bf = BloomFilter::new(1000, 0.01);
        assert_eq!(bf.capacity(), 1000);
        bf.insert("key1");
        bf.rebuild(5000);
        assert_eq!(bf.capacity(), 5000);
        // rebuild clears all recorded keys.
        assert_eq!(bf.len(), 0);
        assert!(!bf.contains("key1"));
    }

    // ========================================================================
    // 泛型化：hash_count / set_bits / new_with_hash_count / K≠str
    // ========================================================================

    /// K=str 逐位一致不变量：用过滤器自身序列化出的位图+种子重建裸
    /// `Bloom<str>`，独立 set 同一 key，位图必须逐位一致——证明泛型化后
    /// K=str 仍走 crate 原生 `set` 逐位路径（哈希算法与种子机制未变）。
    #[test]
    fn k_str_bits_identical_to_crate_native_set_path() {
        let bf = BloomFilter::new(64, 0.05);
        let raw = Bloom::<str>::from_slice(bf.state.read().unwrap().bloom.as_slice())
            .expect("self-serialized bitmap must round-trip");
        bf.insert("k1");

        let mut expected = raw;
        expected.set("k1");
        let after = bf.state.read().unwrap().bloom.as_slice().to_vec();
        assert_eq!(
            expected.as_slice(),
            after.as_slice(),
            "K=str 应走 crate 原生 set 逐位路径"
        );
    }

    #[test]
    fn set_bits_reflects_inserts() {
        let bf = BloomFilter::new(64, 0.05);
        assert_eq!(bf.set_bits(), 0, "空过滤器置位数为 0");
        bf.insert("k1");
        let after_one = bf.set_bits();
        let k = u64::from(bf.hash_count());
        assert!(
            (1..=k).contains(&after_one),
            "单键置位数应在 [1, k], got {after_one} (k={k})"
        );
    }

    #[test]
    fn new_with_hash_count_back_solves_hash_count() {
        for k in [1u32, 2, 3, 4, 7, 16] {
            let bf = BloomFilter::<str>::new_with_hash_count(10_000, 0.01, k);
            assert_eq!(bf.hash_count(), k, "二分反解应精确命中 k={k}");
            assert_eq!(bf.capacity(), 10_000);
            assert_eq!(bf.false_positive_rate(), 0.01);
        }
    }

    #[test]
    fn u64_keys_supported_by_generic_filter() {
        let bf = BloomFilter::<u64>::new_with_hash_count(1000, 0.01, 4);
        assert_eq!(bf.hash_count(), 4);
        for i in 0..500u64 {
            bf.insert(&i);
        }
        assert_eq!(bf.len(), 500);
        for i in 0..500u64 {
            assert!(bf.contains(&i), "无假阴性 {i}");
        }
        // 非插入键通常返回 false（真阴性；fpr=1% 允许个别误判，不在此断言多键）
        assert!(!bf.contains(&u64::MAX));
    }

    #[test]
    fn clone_shares_state_without_k_clone_bound() {
        // 手写 Clone 仅 clone Arc：K: ?Sized（如 str）无需 Clone 即可克隆
        let bf = BloomFilter::<u64>::new_with_hash_count(100, 0.01, 3);
        let snapshot = bf.clone();
        bf.insert(&7u64);
        assert!(snapshot.contains(&7u64), "clone 应共享底层状态");
        assert_eq!(snapshot.set_bits(), bf.set_bits());
    }

    #[test]
    #[should_panic(expected = "hash_count")]
    fn new_with_hash_count_unreachable_target_panics() {
        // capacity=1 时最小位图(1 字节)反解出 k=max(round(8·ln2),1)=6，
        // 更小的 hash_count 在该容量下不可达
        let _ = BloomFilter::<str>::new_with_hash_count(1, 0.01, 2);
    }

    #[test]
    #[should_panic(expected = "hash_count")]
    fn new_with_hash_count_zero_panics() {
        let _ = BloomFilter::<str>::new_with_hash_count(1000, 0.01, 0);
    }
}
