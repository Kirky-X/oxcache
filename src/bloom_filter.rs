//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 布隆过滤器实现 - 用于缓存穿透防护

use murmur3::murmur3_32;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    RwLock, RwLockReadGuard, RwLockWriteGuard,
};

/// 布隆过滤器配置
#[derive(Clone, Debug)]
pub struct BloomFilterOptions {
    pub expected_elements: usize,
    pub false_positive_rate: f64,
    pub name: String,
}

impl BloomFilterOptions {
    pub fn new(name: String, expected_elements: usize, false_positive_rate: f64) -> Self {
        Self {
            name,
            expected_elements,
            false_positive_rate,
        }
    }

    pub fn default_with_name(name: String) -> Self {
        Self {
            name,
            expected_elements: 100000,
            false_positive_rate: 0.01,
        }
    }

    pub fn optimal_size(&self) -> usize {
        let num_items = self.expected_elements as f64;
        let false_positive_prob = self.false_positive_rate;
        let size = -num_items * false_positive_prob.ln() / (std::f64::consts::LN_2).powi(2);
        (size as usize / 8) * 8
    }

    pub fn optimal_num_hashes(&self) -> usize {
        let size = self.optimal_size() as f64 * 8.0;
        let num_items = self.expected_elements as f64;
        (size / num_items * std::f64::consts::LN_2).round() as usize
    }
}

/// 布隆过滤器
///
/// 使用位数组和多个哈希函数实现的空间效率型概率数据结构
/// 用于快速判断元素是否可能存在于集合中
#[allow(clippy::type_complexity)]
pub struct BloomFilter {
    options: BloomFilterOptions,
    bit_array: Vec<u8>,
    seeds: Vec<u32>,
    added_count: Arc<AtomicU64>,
    checked_count: Arc<AtomicU64>,
    false_positive_count: Arc<AtomicU64>,
    /// 哈希缓存 - 使用 Arc<Vec<u8>> 作为键，避免重复内存分配
    hash_cache: Arc<RwLock<HashMap<Arc<Vec<u8>>, Vec<usize>>>>,
}

impl BloomFilter {
    /// 创建新的布隆过滤器
    pub fn new(options: BloomFilterOptions) -> Self {
        let size = options.optimal_size();
        let num_hashes = options.optimal_num_hashes();

        let mut seeds = Vec::with_capacity(num_hashes);
        let mut seed = 0xc3f3e5f3u32;
        for _ in 0..num_hashes {
            seeds.push(seed);
            seed = seed.wrapping_mul(0xc13fa9a9u32);
        }

        // 创建哈希缓存
        let hash_cache = Arc::new(RwLock::new(HashMap::new()));

        Self {
            options,
            bit_array: vec![0; size],
            seeds,
            added_count: Arc::new(AtomicU64::new(0)),
            checked_count: Arc::new(AtomicU64::new(0)),
            false_positive_count: Arc::new(AtomicU64::new(0)),
            hash_cache,
        }
    }

    #[allow(dead_code)]
    fn bit_array_len(&self) -> usize {
        self.bit_array.len()
    }

    fn calculate_positions(&self, mut item: &[u8]) -> Vec<usize> {
        let bit_array_len = self.bit_array.len();
        let mut positions = Vec::with_capacity(self.seeds.len());
        for &seed in &self.seeds {
            let hash = murmur3_32(&mut item, seed).unwrap_or(0);
            let pos = (hash as usize) % (bit_array_len * 8);
            positions.push(pos);
        }
        positions
    }

    pub fn contains(&self, item: &[u8]) -> bool {
        self.checked_count.fetch_add(1, Ordering::SeqCst);

        // 尝试从缓存获取哈希位置
        let item_key = Arc::new(item.to_vec());
        if let Some(cached_positions) = {
            let cache = self.hash_cache.read().expect("Hash cache lock poisoned");
            cache.get(&item_key).cloned()
        } {
            // 使用缓存的位置进行检查
            return self.check_positions(&cached_positions);
        }

        // 缓存未命中，计算新的位置
        let positions = self.calculate_positions(item);

        // 将结果存入缓存（限制缓存大小以避免内存无限增长）
        {
            let mut cache = self.hash_cache.write().expect("Hash cache lock poisoned");

            // 如果缓存过大，使用 LRU 策略移除部分条目
            if cache.len() > 10000 {
                let mut to_remove = Vec::new();
                for entry in cache.iter() {
                    to_remove.push(entry.0.clone());
                    if to_remove.len() >= 1000 {
                        break;
                    }
                }
                for key in to_remove {
                    cache.remove(&key);
                }
            }

            cache.insert(item_key, positions.clone());
        }

        self.check_positions(&positions)
    }

    /// 检查位置是否都设置为1
    fn check_positions(&self, positions: &[usize]) -> bool {
        let bit_array = &self.bit_array;

        for pos in positions {
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;

            if byte_idx >= bit_array.len() {
                continue;
            }

            if (bit_array[byte_idx] & (1 << bit_idx)) == 0 {
                return false;
            }
        }

        self.false_positive_count.fetch_add(1, Ordering::SeqCst);
        true
    }

    pub fn add(&mut self, item: &[u8]) {
        // 尝试从缓存获取哈希位置
        let item_key = Arc::new(item.to_vec());
        let positions = if let Some(cached_positions) = {
            let cache = self.hash_cache.read().expect("Hash cache lock poisoned");
            cache.get(&item_key).cloned()
        } {
            cached_positions
        } else {
            let positions = self.calculate_positions(item);

            // 将结果存入缓存
            {
                let mut cache = self.hash_cache.write().expect("Hash cache lock poisoned");

                // 如果缓存过大，使用 LRU 策略移除部分条目
                if cache.len() > 10000 {
                    let mut to_remove = Vec::new();
                    for entry in cache.iter() {
                        to_remove.push(entry.0.clone());
                        if to_remove.len() >= 1000 {
                            break;
                        }
                    }
                    for key in to_remove {
                        cache.remove(&key);
                    }
                }

                cache.insert(item_key, positions.clone());
            }

            positions
        };

        for pos in &positions {
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;

            if byte_idx < self.bit_array.len() {
                self.bit_array[byte_idx] |= 1 << bit_idx;
            }
        }

        self.added_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn add_checked(&mut self, item: &[u8]) -> bool {
        let existed = self.contains(item);
        if !existed {
            self.add(item);
        }
        !existed
    }

    pub fn contains_and_add(&mut self, item: &[u8]) -> bool {
        let result = self.contains(item);
        if !result {
            self.add(item);
        }
        result
    }

    pub fn remove(&self, _item: &[u8]) -> bool {
        false
    }

    pub fn get_stats(&self) -> BloomFilterStats {
        let total_bits = self.bit_array.len() as u64 * 8;
        let used_bits: u64 = self
            .bit_array
            .iter()
            .map(|byte| byte.count_ones() as u64)
            .sum();
        let added = self.added_count.load(Ordering::SeqCst);
        let checked = self.checked_count.load(Ordering::SeqCst);
        let false_positives = self.false_positive_count.load(Ordering::SeqCst);

        let utilization = if total_bits > 0 {
            used_bits as f64 / total_bits as f64
        } else {
            0.0
        };

        let estimated_count = if self.options.false_positive_rate > 0.0 {
            let ln_2_sq = std::f64::consts::LN_2.powi(2);
            (total_bits as f64 * ln_2_sq / used_bits.max(1) as f64 * 2f64.ln()) as u64
        } else {
            added
        };

        BloomFilterStats {
            name: self.options.name.clone(),
            total_bits,
            used_bits,
            utilization,
            estimated_count,
            added_count: added,
            checked_count: checked,
            false_positive_count: false_positives,
            false_positive_rate: if checked > 0 {
                false_positives as f64 / checked as f64
            } else {
                0.0
            },
            configured_fp_rate: self.options.false_positive_rate,
        }
    }

    pub fn get_estimated_count(&self) -> usize {
        let total_bits = self.bit_array.len() as f64 * 8.0;
        let used_bits: f64 = self
            .bit_array
            .iter()
            .map(|byte| byte.count_ones() as f64)
            .sum();

        if used_bits == 0.0 {
            return 0;
        }

        let num_hashes = self.seeds.len() as f64;
        let ln_2_sq = std::f64::consts::LN_2.powi(2);

        ((-total_bits * ln_2_sq / used_bits).exp() * num_hashes) as usize
    }

    pub fn clear(&mut self) {
        for byte in &mut self.bit_array {
            *byte = 0;
        }
        self.added_count.store(0, Ordering::SeqCst);
    }
}

/// 布隆过滤器统计信息
#[derive(Clone, Debug)]
pub struct BloomFilterStats {
    pub name: String,
    pub total_bits: u64,
    pub used_bits: u64,
    pub utilization: f64,
    pub estimated_count: u64,
    pub added_count: u64,
    pub checked_count: u64,
    pub false_positive_count: u64,
    pub false_positive_rate: f64,
    pub configured_fp_rate: f64,
}

impl std::fmt::Display for BloomFilterStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BloomFilter {}: {}/{} bits ({:.2}%), est_count={}, added={}, checked={}, fp_rate={:.4}% (config={:.2}%)",
            self.name,
            self.used_bits,
            self.total_bits,
            self.utilization * 100.0,
            self.estimated_count,
            self.added_count,
            self.checked_count,
            self.false_positive_rate * 100.0,
            self.configured_fp_rate * 100.0,
        )
    }
}

/// 布隆过滤器共享包装器
///
/// 使用Arc包装布隆过滤器，支持多线程共享
#[derive(Clone)]
pub struct BloomFilterShared {
    filter: Arc<RwLock<BloomFilter>>,
    name: String,
}

impl BloomFilterShared {
    pub fn new(filter: BloomFilter) -> Self {
        let name = filter.options.name.clone();
        Self {
            filter: Arc::new(RwLock::new(filter)),
            name,
        }
    }

    pub fn contains(&self, item: &[u8]) -> bool {
        self.filter
            .read()
            .expect("Filter lock poisoned")
            .contains(item)
    }

    pub async fn add(&self, item: &[u8]) {
        self.filter.write().expect("Filter lock poisoned").add(item)
    }

    pub async fn contains_and_add(&self, item: &[u8]) -> bool {
        self.filter
            .write()
            .expect("Filter lock poisoned")
            .contains_and_add(item)
    }

    pub fn get_stats(&self) -> BloomFilterStats {
        self.filter
            .read()
            .expect("Filter lock poisoned")
            .get_stats()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// 布隆过滤器管理器
///
/// 管理和复用多个布隆过滤器实例
#[derive(Clone, Default)]
pub struct BloomFilterManager {
    filters: Arc<RwLock<HashMap<String, BloomFilterShared>>>,
}

impl BloomFilterManager {
    pub fn new() -> Self {
        Self {
            filters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_or_create(&self, options: BloomFilterOptions) -> BloomFilterShared {
        let mut guard: RwLockWriteGuard<'_, HashMap<String, BloomFilterShared>> =
            self.filters.write().expect("Filters lock poisoned");

        if let Some(existing) = guard.get(&options.name) {
            let existing: &BloomFilterShared = existing;
            return existing.clone();
        }

        let filter = BloomFilter::new(options.clone());
        let shared = BloomFilterShared::new(filter);
        guard.insert(options.name.clone(), shared.clone());
        shared
    }

    pub fn get(&self, name: &str) -> Option<BloomFilterShared> {
        self.filters
            .read()
            .expect("Filters lock poisoned")
            .get(name)
            .cloned()
    }

    pub fn remove(&self, name: &str) -> bool {
        self.filters
            .write()
            .expect("Filters lock poisoned")
            .remove(name)
            .is_some()
    }

    pub fn list_names(&self) -> Vec<String> {
        self.filters
            .read()
            .expect("Filters lock poisoned")
            .keys()
            .cloned()
            .collect()
    }

    pub async fn get_all_stats(&self) -> Vec<BloomFilterStats> {
        let guard: RwLockReadGuard<'_, HashMap<String, BloomFilterShared>> =
            self.filters.read().expect("Filters lock poisoned");
        let mut stats = Vec::with_capacity(guard.len());

        for filter in guard.values() {
            let filter: &BloomFilterShared = filter;
            stats.push(filter.get_stats());
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let options = BloomFilterOptions::default_with_name("test".to_string());
        let mut filter = BloomFilter::new(options);

        assert!(!filter.contains(b"hello"));
        assert!(!filter.contains(b"world"));

        filter.add(b"hello");

        assert!(filter.contains(b"hello"));
        assert!(!filter.contains(b"world"));

        filter.add(b"world");

        assert!(filter.contains(b"hello"));
        assert!(filter.contains(b"world"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let options = BloomFilterOptions::new("test_fp".to_string(), 10000, 0.01);
        let mut filter = BloomFilter::new(options);

        for i in 0..1000 {
            filter.add(format!("item_{}", i).as_bytes());
        }

        let mut false_positives = 0;
        for i in 1000..2000 {
            if filter.contains(format!("fake_{}", i).as_bytes()) {
                false_positives += 1;
            }
        }

        let fp_rate = false_positives as f64 / 1000.0;
        assert!(fp_rate < 0.05, "False positive rate too high: {}", fp_rate);
    }

    #[test]
    fn test_bloom_filter_contains_and_add() {
        let options = BloomFilterOptions::default_with_name("test_caa".to_string());
        let mut filter = BloomFilter::new(options);

        assert!(!filter.contains_and_add(b"new_item"));
        assert!(filter.contains_and_add(b"new_item"));
    }

    #[test]
    fn test_optimal_size_calculation() {
        let options = BloomFilterOptions::new("test".to_string(), 100000, 0.01);
        assert!(options.optimal_size() > 0);
        assert!(options.optimal_num_hashes() > 0);
    }
}
