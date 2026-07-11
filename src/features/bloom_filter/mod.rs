// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Bloom Filter module for negative query filtering.
//!
//! Provides [`BloomFilter`] — a capacity/fpr-configurable Bloom filter backed
//! by the `bloomfilter` crate. State is shared via `Arc<RwLock<>>` so that
//! [`BloomFilter`] is cheaply `Clone` and mutations are visible across clones.

#[cfg(any(
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
mod backend;

#[cfg(any(
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "full"
))]
pub use backend::{BloomFilterBackend, BloomFilterBackendBuilder};

use std::sync::{Arc, RwLock};

use bloomfilter::Bloom;

/// Internal mutable state guarded by an `RwLock`.
struct BloomState {
    bloom: Bloom<str>,
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
/// # Panics
///
/// `new` and `rebuild` panic if `capacity` is `0` or `false_positive_rate` is
/// not in the open interval `(0.0, 1.0)`, or if the underlying seed generation
/// fails.
#[derive(Clone)]
pub struct BloomFilter {
    state: Arc<RwLock<BloomState>>,
}

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
    /// Bloom filters have no false negatives: every inserted key returns
    /// `true`. Non-inserted keys usually return `false` but may return `true`
    /// (false positive) at the configured rate.
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
        if state.capacity == 0 {
            return 0.0;
        }
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
            assert!(bf.contains(&format!("key:{}", i)), "false negative for key:{}", i);
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
}
