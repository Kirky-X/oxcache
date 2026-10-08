// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DashMap backend implementation for high-performance concurrent in-memory caching

use crate::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::backend::{BackendScore, Scores};
use crate::error::OxCacheResult;
use crate::impl_backend_builder;
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Entry with metadata for TTL tracking
#[derive(Clone, Debug)]
pub(crate) struct CacheEntry {
    value: Arc<Vec<u8>>,
    expires_at: Option<Instant>,
    /// 插入序列号，用于识别 FIFO 队列中的陈旧条目（key 被重新 set 后旧条目作废）
    seq: u64,
}

/// 一次淘汰的条目数 = capacity / 该比率（至少 1），减少触发频率
const EVICT_BATCH_RATIO: usize = 10;

/// FIFO 队列长度超过该阈值（相对实际条目数）时触发重建，防止
/// 频繁 re-set 导致 FIFO 无限增长
const FIFO_COMPACT_RATIO: usize = 4;

/// FIFO 队列条目：key + 插入序列号
type FifoItem = (Arc<str>, u64);

/// DashMap cache backend
///
/// This backend uses DashMap for high-performance concurrent in-memory caching.
/// Unlike Moka, DashMap provides lock-free concurrent access but requires
/// manual TTL management.
///
/// # Features
///
/// - **High Concurrency**: Lock-free design for minimal contention
/// - **FIFO Eviction**: Over-capacity writes evict the oldest entries in batch
/// - **Manual TTL**: TTL must be checked on access
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::memory::DashMapMemoryBackend;
/// use std::time::Duration;
///
/// // Create with default settings
/// let backend = DashMapMemoryBackend::new();
///
/// // Create with custom capacity and TTL
/// let backend = DashMapMemoryBackend::builder()
///     .capacity(10000)
///     .default_ttl(Duration::from_secs(3600))
///     .build();
/// ```
#[derive(Clone)]
pub struct DashMapMemoryBackend {
    /// The main cache storage
    cache: Arc<DashMap<Arc<str>, CacheEntry>>,
    /// FIFO 插入顺序队列 `(key, seq)`，淘汰时从队头 O(1) 弹出
    fifo: Arc<Mutex<VecDeque<FifoItem>>>,
    /// 全局单调递增的序列号（每次 set 分配一个）
    next_seq: Arc<AtomicU64>,
    /// Statistics counters
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
    /// Maximum capacity
    capacity: usize,
    /// Default TTL for new entries
    default_ttl: Option<Duration>,
    /// 字节预算上限（审计 F10：None = 不启用，条目数口径不变）
    max_capacity_bytes: Option<u64>,
    /// 当前字节占用近似值（Σ value.len()，最终一致）
    bytes_used: Arc<AtomicU64>,
}

impl_backend_builder!(DashMapMemoryBackend, DashMapBackendBuilder);

impl DashMapMemoryBackend {
    /// 从 FIFO 队头批量淘汰条目，淘汰直到容量达标或达到单次上限。
    ///
    /// 相比旧的 O(n) 全表扫描，这里每次只从队头弹出，摊销 O(1)。
    /// FIFO 中的条目带 `seq`：若 key 已被重新 set（seq 不匹配）或已删除，
    /// 该队列条目视为陈旧直接跳过。过期条目同样可以被淘汰。
    ///
    /// 触发条件（审计 F10）：条目数超 `capacity` **或**字节占用超
    /// `max_capacity_bytes` 任一超标即持续淘汰，直到达标或达单次上限。
    fn evict_if_full(&self) {
        let batch = (self.capacity / EVICT_BATCH_RATIO).max(1);
        let now = Instant::now();
        let byte_over = || {
            self.max_capacity_bytes
                .is_some_and(|max| self.bytes_used.load(Ordering::Relaxed) > max)
        };

        let mut evicted = 0usize;
        loop {
            if evicted >= batch {
                break;
            }
            if self.cache.len() <= self.capacity && !byte_over() {
                break;
            }
            let (key, seq) = match self.fifo.lock().unwrap().pop_front() {
                Some(item) => item,
                None => break,
            };
            // 淘汰条件：seq 匹配（未被 re-set）OR 条目已过期
            // 即使 seq 不匹配（re-set 过），如果已过期也应淘汰以释放内存
            let mut removed_bytes = 0u64;
            let should_remove = self
                .cache
                .remove_if(&key, |_, entry| {
                    let evictable =
                        entry.seq == seq || entry.expires_at.is_some_and(|exp| exp <= now);
                    if evictable {
                        removed_bytes = entry.value.len() as u64;
                    }
                    evictable
                })
                .is_some();
            if should_remove {
                // 近似饱和扣减：并发窗口内短暂偏差与条目数口径一致
                let cur = self.bytes_used.load(Ordering::Relaxed);
                self.bytes_used
                    .store(cur.saturating_sub(removed_bytes), Ordering::Relaxed);
                evicted += 1;
            }
        }

        // 淘汰事件计入全局统一指标（与 chain/redis 埋点同一模式）
        #[cfg(feature = "metrics")]
        if evicted > 0 {
            crate::infra::GLOBAL_UNIFIED_METRICS.record_eviction(evicted as u64);
        }

        self.compact_fifo();
    }

    /// FIFO 中陈旧条目过多时重建队列，防止频繁 re-set 导致无限增长
    fn compact_fifo(&self) {
        let mut fifo = self.fifo.lock().unwrap();
        let cache_len = self.cache.len();
        if fifo.len() > cache_len.saturating_mul(FIFO_COMPACT_RATIO).max(1024) {
            let mut live = Vec::with_capacity(cache_len);
            for r in self.cache.iter() {
                live.push((r.key().clone(), r.value().seq));
            }
            *fifo = live.into();
        }
    }

    /// Get the current capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the current entry count
    pub fn entry_count(&self) -> usize {
        self.cache.len()
    }

    /// Get the hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl Default for DashMapMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DashMapMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashMapMemoryBackend")
            .field("capacity", &self.capacity)
            .field("entry_count", &self.cache.len())
            .field("hit_rate", &self.hit_rate())
            .finish()
    }
}

#[async_trait]
impl CacheReader for DashMapMemoryBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let now = Instant::now();

        let found = match self.cache.get(key) {
            Some(entry_ref) => {
                let entry = entry_ref.value();
                if let Some(expires_at) = entry.expires_at
                    && expires_at <= now
                {
                    // 过期即物理删除（与 exists/ttl 同一口径），防止条目滞留内存
                    drop(entry_ref);
                    self.cache.remove_if(key, |_, entry| {
                        entry.expires_at.is_some_and(|exp| exp <= now)
                    });
                    None
                } else {
                    Some((*entry.value).clone())
                }
            }
            None => None,
        };

        // 统一计数：命中/未命中仅在此处计数一次
        match found {
            Some(value) => {
                self.hits.fetch_add(1, Ordering::SeqCst);
                Ok(Some(value))
            }
            None => {
                self.misses.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }
        }
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at
                && expires_at <= now
            {
                drop(entry_ref); // 释放 Ref 后再原子删除
                self.cache.remove_if(key, |_, entry| {
                    entry.expires_at.is_some_and(|exp| exp <= now)
                });
                return Ok(false);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at {
                if expires_at > now {
                    return Ok(Some(expires_at.duration_since(now)));
                } else {
                    drop(entry_ref); // 释放 Ref 后再原子删除过期条目
                    self.cache.remove_if(key, |_, entry| {
                        entry.expires_at.is_some_and(|exp| exp <= now)
                    });
                    return Ok(None);
                }
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    async fn len(&self) -> OxCacheResult<u64> {
        Ok(self.cache.len() as u64)
    }

    async fn is_empty(&self) -> OxCacheResult<bool> {
        Ok(self.cache.is_empty())
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(self.capacity as u64)
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "dashmap".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert("entry_count".to_string(), self.cache.len().to_string());
        stats.insert(
            "hits".to_string(),
            self.hits.load(Ordering::Relaxed).to_string(),
        );
        stats.insert(
            "misses".to_string(),
            self.misses.load(Ordering::Relaxed).to_string(),
        );
        stats.insert("hit_rate".to_string(), format!("{:.4}", self.hit_rate()));
        stats.insert(
            "bytes_used".to_string(),
            self.bytes_used.load(Ordering::Relaxed).to_string(),
        );
        if let Some(max) = self.max_capacity_bytes {
            stats.insert("max_capacity_bytes".to_string(), max.to_string());
        }
        Ok(stats)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        // glob 匹配口径与 MokaMemoryBackend::keys_matching 一致
        Ok(self
            .cache
            .iter()
            .filter(|entry| crate::backend::interface::glob_match(pattern, entry.key()))
            .map(|entry| entry.key().to_string())
            .collect())
    }
}

#[async_trait]
impl CacheWriter for DashMapMemoryBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let now = Instant::now();
        let expires_at = ttl.or(self.default_ttl).map(|duration| now + duration);
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let value_len = value.len() as u64;

        let entry = CacheEntry {
            value,
            expires_at,
            seq,
        };

        // key 已是 Arc<str>，直接插入 + 记入 FIFO，零拷贝共享
        self.cache.insert(key.clone(), entry);
        self.fifo.lock().unwrap().push_back((key, seq));
        self.bytes_used.fetch_add(value_len, Ordering::Relaxed);

        // Evict if at entry capacity or over byte budget (审计 F10)
        if self.cache.len() > self.capacity
            || self
                .max_capacity_bytes
                .is_some_and(|max| self.bytes_used.load(Ordering::Relaxed) > max)
        {
            self.evict_if_full();
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        if let Some((_, entry)) = self.cache.remove(key) {
            let cur = self.bytes_used.load(Ordering::Relaxed);
            self.bytes_used.store(
                cur.saturating_sub(entry.value.len() as u64),
                Ordering::Relaxed,
            );
        }
        // FIFO 中的陈旧条目（含被删 key 的字符串）依赖紧缩回收，
        // 删除路径主动检查一次，避免低于容量的删除密集负载下队列无限增长
        self.compact_fifo();
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.cache.clear();
        self.fifo.lock().unwrap().clear();
        self.bytes_used.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let now = Instant::now();
        let new_expires_at = now + ttl;

        if let Some(mut entry_ref) = self.cache.get_mut(key) {
            entry_ref.expires_at = Some(new_expires_at);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[async_trait]
impl CacheConnector for DashMapMemoryBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        // DashMap is always healthy as in-memory
        Ok(())
    }

    async fn shutdown(&self) {
        self.cache.clear();
        self.fifo.lock().unwrap().clear();
        self.bytes_used.store(0, Ordering::Relaxed);
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::DashMap
    }
}

// ============================================================================
// Synchronous trait implementations
// ============================================================================
//
// DashMap 本身是同步的，sync impl 直接复用 async 方法逻辑（去掉 async/.await），
// 无需像 moka 那样通过 `block_on` 桥接。实现使用全限定路径
// (`impl crate::backend::interface::SyncCacheReader for DashMapMemoryBackend`)，
// 避免将 sync trait 名导入本模块作用域后，经 `mod tests` 的 `use super::*`
// 与同名 async trait 方法（如 `get`）产生歧义。

impl crate::backend::interface::SyncCacheReader for DashMapMemoryBackend {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let now = Instant::now();

        let found = match self.cache.get(key) {
            Some(entry_ref) => {
                let entry = entry_ref.value();
                if let Some(expires_at) = entry.expires_at
                    && expires_at <= now
                {
                    // 过期即物理删除（与 exists/ttl 同一口径），防止条目滞留内存
                    drop(entry_ref);
                    self.cache.remove_if(key, |_, entry| {
                        entry.expires_at.is_some_and(|exp| exp <= now)
                    });
                    None
                } else {
                    Some((*entry.value).clone())
                }
            }
            None => None,
        };

        // 统一计数：命中/未命中仅在此处计数一次
        match found {
            Some(value) => {
                self.hits.fetch_add(1, Ordering::SeqCst);
                Ok(Some(value))
            }
            None => {
                self.misses.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }
        }
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at
                && expires_at <= now
            {
                drop(entry_ref); // 释放 Ref 后再原子删除
                self.cache.remove_if(key, |_, entry| {
                    entry.expires_at.is_some_and(|exp| exp <= now)
                });
                return Ok(false);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let now = Instant::now();

        if let Some(entry_ref) = self.cache.get(key) {
            let entry = entry_ref.value();
            if let Some(expires_at) = entry.expires_at {
                if expires_at > now {
                    return Ok(Some(expires_at.duration_since(now)));
                } else {
                    drop(entry_ref); // 释放 Ref 后再原子删除过期条目
                    self.cache.remove_if(key, |_, entry| {
                        entry.expires_at.is_some_and(|exp| exp <= now)
                    });
                    return Ok(None);
                }
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    fn len(&self) -> OxCacheResult<u64> {
        Ok(self.cache.len() as u64)
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(self.capacity as u64)
    }

    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "dashmap".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert("entry_count".to_string(), self.cache.len().to_string());
        stats.insert(
            "hits".to_string(),
            self.hits.load(Ordering::Relaxed).to_string(),
        );
        stats.insert(
            "misses".to_string(),
            self.misses.load(Ordering::Relaxed).to_string(),
        );
        stats.insert("hit_rate".to_string(), format!("{:.4}", self.hit_rate()));
        Ok(stats)
    }
}

impl crate::backend::interface::SyncCacheWriter for DashMapMemoryBackend {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let now = Instant::now();
        let expires_at = ttl.or(self.default_ttl).map(|duration| now + duration);
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let value_len = value.len() as u64;

        let entry = CacheEntry {
            value,
            expires_at,
            seq,
        };

        // key 已是 Arc<str>，直接插入 + 记入 FIFO，零拷贝共享
        self.cache.insert(key.clone(), entry);
        self.fifo.lock().unwrap().push_back((key, seq));
        self.bytes_used.fetch_add(value_len, Ordering::Relaxed);

        // Evict if at entry capacity or over byte budget (审计 F10)
        if self.cache.len() > self.capacity
            || self
                .max_capacity_bytes
                .is_some_and(|max| self.bytes_used.load(Ordering::Relaxed) > max)
        {
            self.evict_if_full();
        }

        Ok(())
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        if let Some((_, entry)) = self.cache.remove(key) {
            let cur = self.bytes_used.load(Ordering::Relaxed);
            self.bytes_used.store(
                cur.saturating_sub(entry.value.len() as u64),
                Ordering::Relaxed,
            );
        }
        // FIFO 中的陈旧条目（含被删 key 的字符串）依赖紧缩回收，
        // 删除路径主动检查一次，避免低于容量的删除密集负载下队列无限增长
        self.compact_fifo();
        Ok(())
    }

    fn clear(&self) -> OxCacheResult<()> {
        self.cache.clear();
        self.fifo.lock().unwrap().clear();
        self.bytes_used.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let now = Instant::now();
        let new_expires_at = now + ttl;

        if let Some(mut entry_ref) = self.cache.get_mut(key) {
            entry_ref.expires_at = Some(new_expires_at);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl crate::backend::interface::SyncCacheConnector for DashMapMemoryBackend {
    fn health_check(&self) -> OxCacheResult<()> {
        // DashMap is always healthy as in-memory
        Ok(())
    }

    fn shutdown(&self) {
        self.cache.clear();
        self.fifo.lock().unwrap().clear();
        self.bytes_used.store(0, Ordering::Relaxed);
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::DashMap
    }
}

// CacheBackend is automatically implemented via blanket implementation

impl BackendScore for DashMapMemoryBackend {
    fn score(&self) -> u8 {
        Scores::DASHMAP
    }

    fn is_persistent(&self) -> bool {
        false
    }

    fn backend_name(&self) -> &'static str {
        "dashmap"
    }
}

/// Builder for DashMapMemoryBackend
#[derive(Debug, Clone, Default)]
pub struct DashMapBackendBuilder {
    capacity: usize,
    default_ttl: Option<Duration>,
    max_capacity_bytes: Option<u64>,
}

impl DashMapBackendBuilder {
    /// Set the maximum number of entries
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the default TTL for new entries
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Set the byte budget for the cache.
    ///
    /// 审计 F10：条目数上限按条计不按字节，大值场景会内存超卖。设置后写入
    /// 路径记账 Σvalue 字节数，条目数**或**字节任一超标即触发 FIFO 淘汰。
    /// 未设置（默认）时行为与现状一致。
    pub fn max_capacity_bytes(mut self, max_capacity_bytes: u64) -> Self {
        self.max_capacity_bytes = Some(max_capacity_bytes.max(1));
        self
    }

    /// Build the DashMap backend
    pub fn build(self) -> DashMapMemoryBackend {
        // Use a reasonable default capacity if not set
        let capacity = if self.capacity > 0 {
            self.capacity
        } else {
            10_000 // Default capacity of 10,000 entries
        };

        DashMapMemoryBackend {
            cache: Arc::new(DashMap::new()),
            fifo: Arc::new(Mutex::new(VecDeque::new())),
            next_seq: Arc::new(AtomicU64::new(0)),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
            capacity,
            default_ttl: self.default_ttl,
            max_capacity_bytes: self.max_capacity_bytes,
            bytes_used: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Convenience function to create a DashMap memory backend
pub fn dashmap_memory() -> DashMapMemoryBackend {
    DashMapMemoryBackend::new()
}

/// Convenience function to create a DashMap memory backend with capacity
pub fn dashmap_memory_with_capacity(capacity: usize) -> DashMapMemoryBackend {
    DashMapMemoryBackend::builder().capacity(capacity).build()
}

/// Convenience function to create a DashMap memory backend with capacity and TTL
pub fn dashmap_memory_with_capacity_and_ttl(
    capacity: usize,
    ttl: Duration,
) -> DashMapMemoryBackend {
    DashMapMemoryBackend::builder()
        .capacity(capacity)
        .default_ttl(ttl)
        .build()
}

#[cfg(test)]
mod tests;
