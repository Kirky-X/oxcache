// Copyright (c) 2025-2026 Kirky.X
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
}

impl_backend_builder!(DashMapMemoryBackend, DashMapBackendBuilder);

impl DashMapMemoryBackend {
    /// 从 FIFO 队头批量淘汰条目，淘汰直到容量达标或达到单次上限。
    ///
    /// 相比旧的 O(n) 全表扫描，这里每次只从队头弹出，摊销 O(1)。
    /// FIFO 中的条目带 `seq`：若 key 已被重新 set（seq 不匹配）或已删除，
    /// 该队列条目视为陈旧直接跳过。过期条目同样可以被淘汰。
    fn evict_if_full(&self) {
        let batch = (self.capacity / EVICT_BATCH_RATIO).max(1);
        let now = Instant::now();

        let mut evicted = 0usize;
        loop {
            if self.cache.len() <= self.capacity || evicted >= batch {
                break;
            }
            let (key, seq) = match self.fifo.lock().unwrap().pop_front() {
                Some(item) => item,
                None => break,
            };
            // 淘汰条件：seq 匹配（未被 re-set）OR 条目已过期
            // 即使 seq 不匹配（re-set 过），如果已过期也应淘汰以释放内存
            let should_remove = self
                .cache
                .remove_if(&key, |_, entry| {
                    if entry.seq == seq {
                        return true;
                    }
                    // 即使 seq 不匹配，过期条目也应淘汰
                    if let Some(exp) = entry.expires_at {
                        if exp <= now {
                            return true;
                        }
                    }
                    false
                })
                .is_some();
            if should_remove {
                evicted += 1;
            }
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

        if total == 0 { 0.0 } else { hits as f64 / total as f64 }
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

        // 查找：DashMap 返回 Option<Ref>，仅检查 key 是否存在
        let found = self.cache.get(key).map(|entry_ref| {
            let entry = entry_ref.value();
            // 过期检查（持有 Ref 期间不能 remove，留给下次访问或淘汰清理）
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    return None; // expired
                }
            }
            Some((*entry.value).clone())
        });

        // 统一计数：flatten 后判断最终命中/未命中，仅在此处计数一次
        match found.flatten() {
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
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    drop(entry_ref); // 释放 Ref 后再原子删除
                    self.cache
                        .remove_if(key, |_, entry| entry.expires_at.is_some_and(|exp| exp <= now));
                    return Ok(false);
                }
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
                    self.cache
                        .remove_if(key, |_, entry| entry.expires_at.is_some_and(|exp| exp <= now));
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
        stats.insert("hits".to_string(), self.hits.load(Ordering::Relaxed).to_string());
        stats.insert("misses".to_string(), self.misses.load(Ordering::Relaxed).to_string());
        stats.insert("hit_rate".to_string(), format!("{:.4}", self.hit_rate()));
        Ok(stats)
    }
}

#[async_trait]
impl CacheWriter for DashMapMemoryBackend {
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let now = Instant::now();
        let expires_at = ttl.or(self.default_ttl).map(|duration| now + duration);
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);

        let entry = CacheEntry { value, expires_at, seq };

        // key 已是 Arc<str>，直接插入 + 记入 FIFO，零拷贝共享
        self.cache.insert(key.clone(), entry);
        self.fifo.lock().unwrap().push_back((key, seq));

        // Evict if at capacity
        if self.cache.len() > self.capacity {
            self.evict_if_full();
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.cache.remove(key);
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.cache.clear();
        self.fifo.lock().unwrap().clear();
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
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::DashMap
    }
}

// ============================================================================
// Synchronous trait implementations (任务组 7)
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

        // 查找：DashMap 返回 Option<Ref>，仅检查 key 是否存在
        let found = self.cache.get(key).map(|entry_ref| {
            let entry = entry_ref.value();
            // 过期检查（持有 Ref 期间不能 remove，留给下次访问或淘汰清理）
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    return None; // expired
                }
            }
            Some((*entry.value).clone())
        });

        // 统一计数：flatten 后判断最终命中/未命中，仅在此处计数一次
        match found.flatten() {
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
            if let Some(expires_at) = entry.expires_at {
                if expires_at <= now {
                    drop(entry_ref); // 释放 Ref 后再原子删除
                    self.cache
                        .remove_if(key, |_, entry| entry.expires_at.is_some_and(|exp| exp <= now));
                    return Ok(false);
                }
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
                    self.cache
                        .remove_if(key, |_, entry| entry.expires_at.is_some_and(|exp| exp <= now));
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
        stats.insert("hits".to_string(), self.hits.load(Ordering::Relaxed).to_string());
        stats.insert("misses".to_string(), self.misses.load(Ordering::Relaxed).to_string());
        stats.insert("hit_rate".to_string(), format!("{:.4}", self.hit_rate()));
        Ok(stats)
    }
}

impl crate::backend::interface::SyncCacheWriter for DashMapMemoryBackend {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let now = Instant::now();
        let expires_at = ttl.or(self.default_ttl).map(|duration| now + duration);
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);

        let entry = CacheEntry { value, expires_at, seq };

        // key 已是 Arc<str>，直接插入 + 记入 FIFO，零拷贝共享
        self.cache.insert(key.clone(), entry);
        self.fifo.lock().unwrap().push_back((key, seq));

        // Evict if at capacity
        if self.cache.len() > self.capacity {
            self.evict_if_full();
        }

        Ok(())
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.cache.remove(key);
        Ok(())
    }

    fn clear(&self) -> OxCacheResult<()> {
        self.cache.clear();
        self.fifo.lock().unwrap().clear();
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
pub fn dashmap_memory_with_capacity_and_ttl(capacity: usize, ttl: Duration) -> DashMapMemoryBackend {
    DashMapMemoryBackend::builder()
        .capacity(capacity)
        .default_ttl(ttl)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashmap_backend_builder() {
        let backend = DashMapMemoryBackend::builder()
            .capacity(1000)
            .default_ttl(Duration::from_secs(3600))
            .build();

        assert_eq!(backend.capacity(), 1000);
    }

    #[test]
    fn test_dashmap_backend_default() {
        let backend = DashMapMemoryBackend::default();
        // Default capacity should be reasonable
        assert!(backend.capacity() > 0);
    }

    #[tokio::test]
    async fn test_dashmap_basic_operations() {
        let backend = DashMapMemoryBackend::new();

        // Test set and get
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());

        // Test health check
        backend.health_check().await.unwrap();

        // Test stats
        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"dashmap".to_string()));
        assert_eq!(stats.get("capacity"), Some(&backend.capacity().to_string()));
    }

    #[tokio::test]
    async fn test_dashmap_ttl() {
        let backend = DashMapMemoryBackend::new();

        // Set with TTL
        backend
            .set(
                Arc::from("key1"),
                Arc::new(b"value1".to_vec()),
                Some(Duration::from_millis(100)),
            )
            .await
            .unwrap();

        // Should exist immediately
        assert!(backend.exists("key1").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[test]
    fn test_convenience_functions() {
        let backend1 = dashmap_memory();
        let backend2 = dashmap_memory_with_capacity(1000);
        let backend3 = dashmap_memory_with_capacity_and_ttl(1000, Duration::from_secs(3600));

        assert!(backend1.capacity() > 0);
        assert_eq!(backend2.capacity(), 1000);
        assert_eq!(backend3.capacity(), 1000);
    }

    // ========================================================================
    // Eviction tests (问题 2.1 / 7.2)
    // ========================================================================

    #[tokio::test]
    async fn test_eviction_fifo_oldest_evicted() {
        // capacity=3，插入 4 个无 TTL 条目，最早插入的 key1 应被淘汰
        let backend = dashmap_memory_with_capacity(3);

        backend
            .set(Arc::from("key1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key2"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key3"), Arc::new(b"v3".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key4"), Arc::new(b"v4".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.entry_count(), 3);
        assert_eq!(backend.get("key1").await.unwrap(), None, "最旧的 key1 应被淘汰");
        assert_eq!(backend.get("key4").await.unwrap(), Some(b"v4".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_no_ttl_entries_are_evictable() {
        // 无 TTL 条目现在也必须能被淘汰（旧实现中被 filter_map 过滤掉）
        let backend = dashmap_memory_with_capacity(2);

        backend
            .set(Arc::from("a"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("b"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("c"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.entry_count(), 2);
        assert_eq!(backend.get("a").await.unwrap(), None);
        assert_eq!(backend.get("b").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_reset_key_stays_fresh() {
        // 满容量后 re-set 已存在的 key 不应立即被淘汰（seq 更新）
        let backend = dashmap_memory_with_capacity(3);

        backend
            .set(Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("k2"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("k3"), Arc::new(b"v3".to_vec()), None)
            .await
            .unwrap();

        // re-set k1：其 FIFO 旧条目作废，新条目在队尾
        backend
            .set(Arc::from("k1"), Arc::new(b"v1b".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.get("k1").await.unwrap(), Some(b"v1b".to_vec()));
        assert_eq!(backend.entry_count(), 3);

        // 再插入新 key，队头的 k2 应被淘汰而非刚 re-set 的 k1
        backend
            .set(Arc::from("k4"), Arc::new(b"v4".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.get("k2").await.unwrap(), None, "应淘汰最早的 k2");
        assert_eq!(backend.get("k1").await.unwrap(), Some(b"v1b".to_vec()));
        assert_eq!(backend.entry_count(), 3);
    }

    #[tokio::test]
    async fn test_eviction_batch_evicts_multiple() {
        // capacity=10，一次性插入 25 条，应批量淘汰到容量内
        let backend = dashmap_memory_with_capacity(10);

        for i in 0..25 {
            backend
                .set(
                    Arc::from(format!("key{i}")),
                    Arc::new(format!("v{i}").into_bytes()),
                    None,
                )
                .await
                .unwrap();
        }

        assert_eq!(backend.entry_count(), 10);
        // 最早插入的 15 条应被淘汰
        assert_eq!(backend.get("key0").await.unwrap(), None);
        assert_eq!(backend.get("key14").await.unwrap(), None);
        assert_eq!(backend.get("key15").await.unwrap(), Some(b"v15".to_vec()));
        assert_eq!(backend.get("key24").await.unwrap(), Some(b"v24".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_expired_entries_evicted() {
        // 有 TTL 的条目过期后，容量检查应将其淘汰
        let backend = dashmap_memory_with_capacity(3);

        backend
            .set(
                Arc::from("short"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();
        backend
            .set(Arc::from("a"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("b"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("c"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        assert_eq!(backend.get("short").await.unwrap(), None, "过期条目不可读");
        assert_eq!(backend.entry_count(), 3);
        assert_eq!(backend.get("a").await.unwrap(), Some(b"v".to_vec()));
    }

    // ========================================================================
    // 过期条目清理测试 (L2 修复验证)
    // ========================================================================

    #[tokio::test]
    async fn test_exists_removes_expired_entry() {
        // exists 检测到过期条目时应原子删除，释放内存
        let backend = dashmap_memory_with_capacity(100);

        backend
            .set(
                Arc::from("expire_me"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        // exists 应返回 false 并从 cache 中移除条目
        assert!(!backend.exists("expire_me").await.unwrap());
        // 验证条目已从底层 DashMap 中删除
        assert_eq!(backend.cache.len(), 0, "过期条目应从 cache 中物理删除");
    }

    #[tokio::test]
    async fn test_ttl_removes_expired_entry() {
        // ttl 检测到过期条目时应原子删除
        let backend = dashmap_memory_with_capacity(100);

        backend
            .set(
                Arc::from("expire_me"),
                Arc::new(b"v".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        // ttl 应返回 None 并从 cache 中移除条目
        assert_eq!(backend.ttl("expire_me").await.unwrap(), None);
        assert_eq!(backend.cache.len(), 0, "过期条目应从 cache 中物理删除");
    }

    #[tokio::test]
    async fn test_eviction_expired_entry_with_stale_seq() {
        // 过期条目即使 seq 不匹配（被 re-set 过）也应被淘汰
        let backend = dashmap_memory_with_capacity(2);

        // 插入短 TTL 条目
        backend
            .set(
                Arc::from("ttl_key"),
                Arc::new(b"v1".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();
        backend
            .set(Arc::from("other"), Arc::new(b"v2".to_vec()), None)
            .await
            .unwrap();

        // re-set ttl_key，使旧 FIFO 条目 seq 失效，但新条目仍然有短 TTL
        backend
            .set(
                Arc::from("ttl_key"),
                Arc::new(b"v1b".to_vec()),
                Some(Duration::from_millis(30)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;

        // 插入新条目触发淘汰，过期的 ttl_key 应被淘汰（即使 seq 不匹配旧 FIFO 条目）
        backend
            .set(Arc::from("new_key"), Arc::new(b"v3".to_vec()), None)
            .await
            .unwrap();

        assert_eq!(backend.get("ttl_key").await.unwrap(), None, "过期条目应被淘汰");
        assert_eq!(backend.get("other").await.unwrap(), Some(b"v2".to_vec()));
    }

    #[tokio::test]
    async fn test_eviction_stress_100_writers() {
        // 100 并发 writer + capacity=1000，验证高并发下容量不超限
        let backend = dashmap_memory_with_capacity(1000);

        let mut handles = Vec::new();
        for w in 0..100u64 {
            let backend = backend.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..200u64 {
                    let key = format!("w{w}_k{i}");
                    backend
                        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
                        .await
                        .unwrap();
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert!(
            backend.entry_count() <= 1000,
            "capacity exceeded: {}",
            backend.entry_count()
        );
        // 总数 20000 条 > capacity 1000，必然发生过淘汰
        assert_eq!(backend.entry_count(), 1000);
    }

    // ========================================================================
    // Synchronous trait hierarchy tests (任务组 7)
    //
    // 隔离在嵌套 `mod sync_tests` 内：sync trait 的 import 仅在此模块可见，
    // 避免与父模块 `mod tests` 中 async `CacheReader::get` 等同名方法产生
    // 歧义。方法调用通过 trait object (`&dyn SyncCacheReader` 等) 消歧。
    // ========================================================================
    mod sync_tests {
        use super::DashMapMemoryBackend;
        use crate::backend::{BackendKind, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
        use std::sync::Arc;
        use std::time::Duration;

        #[test]
        fn test_dashmap_sync_get_set_basic() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert_eq!(reader.get("key1").unwrap(), Some(b"value1".to_vec()));
            assert!(reader.exists("key1").unwrap());
            assert!(!reader.exists("key2").unwrap());
            assert!(reader.capacity().unwrap() > 0);
            assert_eq!(reader.len().unwrap(), 1);
            assert!(!reader.is_empty().unwrap());

            let stats = reader.stats().unwrap();
            assert_eq!(stats.get("type"), Some(&"dashmap".to_string()));
        }

        #[test]
        fn test_dashmap_sync_set_with_ttl() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(50)))
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            // 立即可读
            assert_eq!(reader.get("k").unwrap(), Some(b"v".to_vec()));

            // DashMap 在访问时按 expires_at 校验，无后台驱逐；等待过期后读应返回 None
            std::thread::sleep(Duration::from_millis(120));
            assert_eq!(reader.get("k").unwrap(), None);
            assert!(!reader.exists("k").unwrap());
        }

        #[test]
        fn test_dashmap_sync_expire() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
                .unwrap();

            // expire 已存在 key → true，TTL 延长至 120s
            let ok = writer.expire("k", Duration::from_secs(120)).unwrap();
            assert!(ok, "expire on existing key should return true");

            let reader: &dyn SyncCacheReader = &backend;
            let new_ttl = reader.ttl("k").unwrap().expect("ttl should be Some after expire");
            assert!(
                new_ttl > Duration::from_secs(118),
                "new_ttl={} should be > 118s",
                new_ttl.as_secs_f64()
            );

            // expire 不存在 key → false
            let ok = writer.expire("missing", Duration::from_secs(10)).unwrap();
            assert!(!ok, "expire on missing key should return false");

            // shrink TTL 后应过期
            let ok = writer.expire("k", Duration::from_millis(50)).unwrap();
            assert!(ok, "expire shrink on existing key should return true");
            std::thread::sleep(Duration::from_millis(120));
            assert_eq!(reader.get("k").unwrap(), None);
        }

        #[test]
        fn test_dashmap_sync_ttl_query() {
            let backend = DashMapMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer
                .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
                .unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            let ttl = reader.ttl("k").unwrap().expect("ttl should be Some for TTL'd key");
            assert!(
                ttl > Duration::from_secs(58),
                "ttl={} should be > 58s",
                ttl.as_secs_f64()
            );
            assert!(
                ttl <= Duration::from_secs(60),
                "ttl={} should be <= 60s",
                ttl.as_secs_f64()
            );

            // 无 TTL 的 key 返回 None
            writer.set(Arc::from("no_ttl"), Arc::new(b"v".to_vec()), None).unwrap();
            assert_eq!(reader.ttl("no_ttl").unwrap(), None);
            // 不存在的 key 返回 None
            assert_eq!(reader.ttl("missing").unwrap(), None);

            // connector: health_check / shutdown / backend_kind
            let connector: &dyn SyncCacheConnector = &backend;
            connector.health_check().unwrap();
            assert_eq!(connector.backend_kind(), BackendKind::DashMap);
            connector.shutdown();
        }
    }
}
