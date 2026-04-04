//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 布隆过滤器实现 - 用于缓存穿透防护
//! 通过 `bloom-filter` feature 控制启用/禁用

#![allow(dead_code)]

#[cfg(feature = "bloom-filter")]
use crate::error::CacheError;
#[cfg(feature = "bloom-filter")]
use lru::LruCache;
#[cfg(feature = "bloom-filter")]
use murmur3::murmur3_32;
#[cfg(feature = "bloom-filter")]
use std::collections::HashMap;
#[cfg(feature = "bloom-filter")]
use std::sync::Arc;
#[cfg(feature = "bloom-filter")]
use std::sync::{
    atomic::{AtomicU64, Ordering},
    RwLock, RwLockReadGuard, RwLockWriteGuard,
};

/// 布隆过滤器配置
#[cfg(feature = "bloom-filter")]
#[derive(Clone, Debug)]
pub struct BloomFilterOptions {
    pub expected_elements: usize,
    pub false_positive_rate: f64,
    pub name: String,
}

#[cfg(feature = "bloom-filter")]
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
#[cfg(feature = "bloom-filter")]
pub struct BloomFilter {
    options: BloomFilterOptions,
    bit_array: Vec<u8>,
    seeds: Vec<u32>,
    added_count: Arc<AtomicU64>,
    checked_count: Arc<AtomicU64>,
    false_positive_count: Arc<AtomicU64>,
    /// 哈希缓存 - 使用 LRU 缓存实现真正的 LRU 淘汰策略
    /// 避免内存无限增长，同时保持最近使用的哈希结果
    #[allow(clippy::type_complexity)]
    hash_cache: Arc<RwLock<LruCache<Arc<Vec<u8>>, Vec<usize>>>>,
}

#[cfg(feature = "bloom-filter")]
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

        // 创建 LRU 哈希缓存，限制最大容量为 10000 条
        let hash_cache = Arc::new(RwLock::new(LruCache::new(
            std::num::NonZeroUsize::new(10000).expect("10000 is a valid non-zero usize"),
        )));

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

    pub fn contains(&self, item: &[u8]) -> Result<bool, CacheError> {
        self.checked_count.fetch_add(1, Ordering::SeqCst);

        // 尝试从 LRU 缓存获取哈希位置
        let item_key = Arc::new(item.to_vec());
        let cached_positions = {
            let mut cache = self
                .hash_cache
                .write()
                .map_err(|_| CacheError::L1Error("Hash cache lock poisoned".to_string()))?;
            // LruCache::get 需要 &mut self，所以我们使用写锁
            cache.get(&item_key).cloned()
        };

        if let Some(cached_positions) = cached_positions {
            // 使用缓存的位置进行检查
            return Ok(self.check_positions(&cached_positions));
        }

        // 缓存未命中，计算新的位置
        let positions = self.calculate_positions(item);

        // 将结果存入 LRU 缓存（容量自动管理）
        {
            let mut cache = self
                .hash_cache
                .write()
                .map_err(|_| CacheError::L1Error("Hash cache lock poisoned".to_string()))?;
            cache.put(item_key, positions.clone());
        }

        Ok(self.check_positions(&positions))
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

    pub fn add(&mut self, item: &[u8]) -> Result<(), CacheError> {
        // 尝试从 LRU 缓存获取哈希位置
        let item_key = Arc::new(item.to_vec());
        let positions = {
            let mut cache = self
                .hash_cache
                .write()
                .map_err(|_| CacheError::L1Error("Hash cache lock poisoned".to_string()))?;
            // LruCache::get 需要 &mut self，所以我们使用写锁
            if let Some(cached_positions) = cache.get(&item_key).cloned() {
                cached_positions
            } else {
                let positions = self.calculate_positions(item);
                // 将结果存入 LRU 缓存
                cache.put(item_key, positions.clone());
                positions
            }
        };

        for pos in &positions {
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;

            if byte_idx < self.bit_array.len() {
                self.bit_array[byte_idx] |= 1 << bit_idx;
            }
        }

        self.added_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn add_checked(&mut self, item: &[u8]) -> Result<bool, CacheError> {
        let existed = self.contains(item)?;
        if !existed {
            self.add(item)?;
        }
        Ok(!existed)
    }

    pub fn contains_and_add(&mut self, item: &[u8]) -> Result<bool, CacheError> {
        let result = self.contains(item)?;
        if !result {
            self.add(item)?;
        }
        Ok(result)
    }

    pub fn remove(&self, _item: &[u8]) -> bool {
        false
    }

    pub fn get_stats(&self) -> BloomFilterStats {
        let total_bits = self.bit_array.len() as u64 * 8;
        let used_bits: u64 = self.bit_array.iter().map(|byte| byte.count_ones() as u64).sum();
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
        let used_bits: f64 = self.bit_array.iter().map(|byte| byte.count_ones() as f64).sum();

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
#[cfg(feature = "bloom-filter")]
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

#[cfg(feature = "bloom-filter")]
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
#[cfg(feature = "bloom-filter")]
#[derive(Clone)]
pub struct BloomFilterShared {
    filter: Arc<RwLock<BloomFilter>>,
    name: String,
}

#[cfg(feature = "bloom-filter")]
impl BloomFilterShared {
    pub fn new(filter: BloomFilter) -> Self {
        let name = filter.options.name.clone();
        Self {
            filter: Arc::new(RwLock::new(filter)),
            name,
        }
    }

    pub fn contains(&self, item: &[u8]) -> Result<bool, CacheError> {
        self.filter
            .read()
            .map_err(|_| CacheError::L1Error("Filter lock poisoned".to_string()))?
            .contains(item)
    }

    pub async fn add(&self, item: &[u8]) -> Result<(), CacheError> {
        self.filter
            .write()
            .map_err(|_| CacheError::L1Error("Filter lock poisoned".to_string()))?
            .add(item)
    }

    pub async fn contains_and_add(&self, item: &[u8]) -> Result<bool, CacheError> {
        self.filter
            .write()
            .map_err(|_| CacheError::L1Error("Filter lock poisoned".to_string()))?
            .contains_and_add(item)
    }

    pub fn get_stats(&self) -> Result<BloomFilterStats, CacheError> {
        Ok(self
            .filter
            .read()
            .map_err(|_| CacheError::L1Error("Filter lock poisoned".to_string()))?
            .get_stats())
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// 布隆过滤器管理器
///
/// 管理和复用多个布隆过滤器实例
#[cfg(feature = "bloom-filter")]
#[derive(Clone, Default)]
pub struct BloomFilterManager {
    filters: Arc<RwLock<HashMap<String, BloomFilterShared>>>,
}

#[cfg(feature = "bloom-filter")]
impl BloomFilterManager {
    pub fn new() -> Self {
        Self {
            filters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_or_create(&self, options: BloomFilterOptions) -> Result<BloomFilterShared, CacheError> {
        let mut guard: RwLockWriteGuard<'_, HashMap<String, BloomFilterShared>> = self
            .filters
            .write()
            .map_err(|_| CacheError::L1Error("Filters lock poisoned".to_string()))?;

        if let Some(existing) = guard.get(&options.name) {
            let existing: &BloomFilterShared = existing;
            return Ok(existing.clone());
        }

        let filter = BloomFilter::new(options.clone());
        let shared = BloomFilterShared::new(filter);
        guard.insert(options.name.clone(), shared.clone());
        Ok(shared)
    }

    pub fn get(&self, name: &str) -> Result<Option<BloomFilterShared>, CacheError> {
        Ok(self
            .filters
            .read()
            .map_err(|_| CacheError::L1Error("Filters lock poisoned".to_string()))?
            .get(name)
            .cloned())
    }

    pub fn remove(&self, name: &str) -> Result<bool, CacheError> {
        Ok(self
            .filters
            .write()
            .map_err(|_| CacheError::L1Error("Filters lock poisoned".to_string()))?
            .remove(name)
            .is_some())
    }

    pub fn list_names(&self) -> Result<Vec<String>, CacheError> {
        Ok(self
            .filters
            .read()
            .map_err(|_| CacheError::L1Error("Filters lock poisoned".to_string()))?
            .keys()
            .cloned()
            .collect())
    }

    pub async fn get_all_stats(&self) -> Result<Vec<BloomFilterStats>, CacheError> {
        let guard: RwLockReadGuard<'_, HashMap<String, BloomFilterShared>> = self
            .filters
            .read()
            .map_err(|_| CacheError::L1Error("Filters lock poisoned".to_string()))?;
        let mut stats = Vec::with_capacity(guard.len());

        for filter in guard.values() {
            let filter: &BloomFilterShared = filter;
            if let Ok(stat) = filter.get_stats() {
                stats.push(stat);
            }
        }

        Ok(stats)
    }
}

// ============================================================================
// 当 bloom-filter 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "bloom-filter"))]
/// 布隆过滤器配置（空实现）
#[derive(Clone, Debug, Default)]
pub struct BloomFilterOptions;

#[cfg(not(feature = "bloom-filter"))]
impl BloomFilterOptions {
    pub fn new(_name: String, _expected_elements: usize, _false_positive_rate: f64) -> Self {
        Self
    }

    pub fn default_with_name(_name: String) -> Self {
        Self
    }

    pub fn optimal_size(&self) -> usize {
        0
    }

    pub fn optimal_num_hashes(&self) -> usize {
        0
    }
}

/// 布隆过滤器（空实现）
#[cfg(not(feature = "bloom-filter"))]
#[derive(Clone, Debug)]
pub struct BloomFilter;

#[cfg(not(feature = "bloom-filter"))]
use crate::error::CacheError;

#[cfg(not(feature = "bloom-filter"))]
impl BloomFilter {
    pub fn new(_options: BloomFilterOptions) -> Self {
        Self
    }

    pub fn contains(&self, _item: &[u8]) -> Result<bool, CacheError> {
        Ok(false)
    }

    pub fn add(&mut self, _item: &[u8]) -> Result<(), CacheError> {
        Ok(())
    }

    pub fn add_checked(&mut self, _item: &[u8]) -> Result<bool, CacheError> {
        Ok(false)
    }

    pub fn contains_and_add(&mut self, _item: &[u8]) -> Result<bool, CacheError> {
        Ok(false)
    }

    pub fn remove(&self, _item: &[u8]) -> bool {
        false
    }

    pub fn get_stats(&self) -> BloomFilterStats {
        BloomFilterStats::default()
    }

    pub fn get_estimated_count(&self) -> usize {
        0
    }

    pub fn clear(&mut self) {}
}

/// 布隆过滤器统计信息（空实现）
#[cfg(not(feature = "bloom-filter"))]
#[derive(Clone, Debug, Default)]
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

#[cfg(not(feature = "bloom-filter"))]
impl std::fmt::Display for BloomFilterStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BloomFilter (Disabled)")
    }
}

/// 布隆过滤器共享包装器（空实现）
#[cfg(not(feature = "bloom-filter"))]
#[derive(Clone, Default)]
pub struct BloomFilterShared;

#[cfg(not(feature = "bloom-filter"))]
impl BloomFilterShared {
    pub fn new(_filter: BloomFilter) -> Self {
        Self
    }

    pub fn contains(&self, _item: &[u8]) -> Result<bool, CacheError> {
        Ok(false)
    }

    pub async fn add(&self, _item: &[u8]) -> Result<(), CacheError> {
        Ok(())
    }

    pub async fn contains_and_add(&self, _item: &[u8]) -> Result<bool, CacheError> {
        Ok(false)
    }

    pub fn get_stats(&self) -> Result<BloomFilterStats, CacheError> {
        Ok(BloomFilterStats::default())
    }

    pub fn name(&self) -> &str {
        ""
    }
}

/// 布隆过滤器管理器（空实现）
#[cfg(not(feature = "bloom-filter"))]
#[derive(Clone, Default)]
pub struct BloomFilterManager;

#[cfg(not(feature = "bloom-filter"))]
impl BloomFilterManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_or_create(&self, _options: BloomFilterOptions) -> Result<BloomFilterShared, CacheError> {
        Ok(BloomFilterShared::new(BloomFilter::new(BloomFilterOptions::default())))
    }

    pub fn get(&self, _name: &str) -> Result<Option<BloomFilterShared>, CacheError> {
        Ok(None)
    }

    pub fn remove(&self, _name: &str) -> Result<bool, CacheError> {
        Ok(false)
    }

    pub fn list_names(&self) -> Result<Vec<String>, CacheError> {
        Ok(Vec::new())
    }

    pub async fn get_all_stats(&self) -> Result<Vec<BloomFilterStats>, CacheError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
#[cfg(feature = "bloom-filter")]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() -> Result<(), CacheError> {
        let options = BloomFilterOptions::default_with_name("test".to_string());
        let mut filter = BloomFilter::new(options);

        assert!(!filter.contains(b"hello")?);
        assert!(!filter.contains(b"world")?);

        filter.add(b"hello")?;

        assert!(filter.contains(b"hello")?);
        assert!(!filter.contains(b"world")?);

        filter.add(b"world")?;

        assert!(filter.contains(b"hello")?);
        assert!(filter.contains(b"world")?);
        Ok(())
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() -> Result<(), CacheError> {
        let options = BloomFilterOptions::new("test_fp".to_string(), 10000, 0.01);
        let mut filter = BloomFilter::new(options);

        for i in 0..1000 {
            filter.add(format!("item_{}", i).as_bytes())?;
        }

        let mut false_positives = 0;
        for i in 1000..2000 {
            if filter.contains(format!("fake_{}", i).as_bytes())? {
                false_positives += 1;
            }
        }

        let fp_rate = false_positives as f64 / 1000.0;
        assert!(fp_rate < 0.05, "False positive rate too high: {}", fp_rate);
        Ok(())
    }

    #[test]
    fn test_bloom_filter_contains_and_add() -> Result<(), CacheError> {
        let options = BloomFilterOptions::default_with_name("test_caa".to_string());
        let mut filter = BloomFilter::new(options);

        assert!(!filter.contains_and_add(b"new_item")?);
        assert!(filter.contains_and_add(b"new_item")?);
        Ok(())
    }

    #[test]
    fn test_optimal_size_calculation() {
        let options = BloomFilterOptions::new("test".to_string(), 100000, 0.01);
        assert!(options.optimal_size() > 0);
        assert!(options.optimal_num_hashes() > 0);
    }
}
