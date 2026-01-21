//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 基于字节的 L1 容量管理模块
//!
//! 提供基于内存大小的缓存容量管理，支持条目计数和字节限制双模式。

#[cfg(feature = "l1-moka")]
use crate::config::EvictionPolicy;

#[cfg(feature = "l1-moka")]
use crate::error::Result;

#[cfg(feature = "l1-moka")]
use moka::future::Cache;

#[cfg(feature = "l1-moka")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "l1-moka")]
use std::sync::Arc;
#[cfg(feature = "l1-moka")]
use std::time::{Duration, Instant};

#[cfg(feature = "l1-moka")]
use tracing::{debug, instrument};

/// 缓存条目类型：(数据, 版本/时间戳, 过期时间, 字节大小)
#[cfg(feature = "l1-moka")]
pub type CacheEntry = (Vec<u8>, u64, Option<Instant>, usize);

/// 容量统计
#[cfg(feature = "l1-moka")]
#[derive(Debug, Clone, Default)]
pub struct CapacityStats {
    /// 当前条目数
    pub entry_count: u64,
    /// 当前字节数
    pub byte_count: u64,
    /// 最大条目数
    pub max_entries: u64,
    /// 最大字节数
    pub max_bytes: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 总访问次数
    pub total_accesses: u64,
    /// 命中次数
    pub hit_count: u64,
}

/// L1 缓存后端实现（基于字节容量）
///
/// 基于内存的高速缓存实现，使用 Moka 作为底层缓存库，
/// 支持基于条目数量和字节大小的双模式容量管理。
#[cfg(feature = "l1-moka")]
#[derive(Clone)]
pub struct ByteCapacityL1Backend {
    // 值: (数据, 版本/时间戳, 过期时间, 字节大小)
    cache: Cache<String, CacheEntry>,
    // 淘汰策略
    eviction_policy: EvictionPolicy,
    // 最大条目数
    max_entries: u64,
    // 最大字节数
    max_bytes: u64,
    // 当前条目数
    entry_count: Arc<AtomicU64>,
    // 当前字节数
    byte_count: Arc<AtomicU64>,
    // 统计信息
    total_accesses: Arc<AtomicU64>,
    hit_count: Arc<AtomicU64>,
}

#[cfg(feature = "l1-moka")]
impl ByteCapacityL1Backend {
    /// 创建新的 L1 缓存后端实例（基于字节容量）
    ///
    /// # 参数
    /// * `max_entries` - 最大条目数
    /// * `max_bytes` - 最大字节数
    ///
    /// # 返回值
    /// * 新的 L1Backend 实例
    pub fn new(max_entries: u64, max_bytes: u64) -> Self {
        Self::with_policy(max_entries, max_bytes, EvictionPolicy::default())
    }

    /// 创建新的 L1 缓存后端实例（指定淘汰策略）
    ///
    /// # 参数
    /// * `max_entries` - 最大条目数
    /// * `max_bytes` - 最大字节数
    /// * `policy` - 淘汰策略
    ///
    /// # 返回值
    /// * 新的 L1Backend 实例
    pub fn with_policy(max_entries: u64, max_bytes: u64, policy: EvictionPolicy) -> Self {
        // Moka 使用条目数量作为容量限制
        // 字节限制需要我们自己管理
        let cache: Cache<String, CacheEntry> = Cache::builder()
            .max_capacity(max_entries)
            .build();

        Self {
            cache,
            eviction_policy: policy,
            max_entries,
            max_bytes,
            entry_count: Arc::new(AtomicU64::new(0)),
            byte_count: Arc::new(AtomicU64::new(0)),
            total_accesses: Arc::new(AtomicU64::new(0)),
            hit_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 获取当前使用的淘汰策略
    pub fn eviction_policy(&self) -> EvictionPolicy {
        self.eviction_policy
    }

    /// 获取容量统计
    pub fn capacity_stats(&self) -> CapacityStats {
        let total = self.total_accesses.load(Ordering::Relaxed);
        let hits = self.hit_count.load(Ordering::Relaxed);
        let hit_rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };

        CapacityStats {
            entry_count: self.entry_count.load(Ordering::Relaxed),
            byte_count: self.byte_count.load(Ordering::Relaxed),
            max_entries: self.max_entries,
            max_bytes: self.max_bytes,
            hit_rate,
            total_accesses: total,
            hit_count: hits,
        }
    }

    /// 记录访问
    fn record_access(&self, hit: bool) {
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        if hit {
            self.hit_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 计算条目的字节大小
    fn entry_size(key: &str, value: &[u8], version: u64, expire_at: Option<Instant>) -> usize {
        // 键大小 + 值大小 + 版本大小 + 过期时间大小 + 开销
        key.len() + value.len() + 8 + 8 + 32
    }

    /// 获取带有元数据的缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回缓存值和版本号的元组，如果不存在则返回None
    #[instrument(skip(self), level = "debug")]
    pub async fn get_with_metadata(&self, key: &str) -> Result<Option<(Vec<u8>, u64)>> {
        let result = self.cache.get(key).await;
        match result {
            Some((bytes, version, expire_at, _size)) => {
                if let Some(expire_time) = expire_at {
                    if Instant::now() > expire_time {
                        // 键已过期
                        self.cache.remove(key).await;
                        self.record_access(false);
                        return Ok(None);
                    }
                }
                self.record_access(true);
                Ok(Some((bytes, version)))
            }
            None => {
                self.record_access(false);
                Ok(None)
            }
        }
    }

    /// 设置带有元数据的缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值（字节数组）
    /// * `version` - 版本号
    /// * `ttl` - 过期时间（秒），None 表示永不过期
    ///
    /// # 返回值
    ///
    /// 如果设置成功则返回 Ok(())
    #[instrument(skip(self, value), level = "debug")]
    pub async fn set_with_metadata(
        &self,
        key: &str,
        value: &[u8],
        version: u64,
        ttl: Option<u64>,
    ) -> Result<()> {
        let expire_at = ttl.map(|secs| Instant::now() + Duration::from_secs(secs));
        let size = Self::entry_size(key, value, version, expire_at);

        // 检查是否超出字节限制
        let current_bytes = self.byte_count.load(Ordering::Relaxed);
        if size as u64 > self.max_bytes {
            // 单个条目超出限制，跳过缓存
            debug!(
                "Skipping cache for key {}: entry size {} exceeds max bytes {}",
                key, size, self.max_bytes
            );
            return Ok(());
        }

        // 移除旧条目直到有足够空间
        while current_bytes + size as u64 > self.max_bytes {
            if let Some((old_key, old_entry)) = self.cache.iter().next() {
                let old_size = old_entry.3;
                self.cache.remove(old_key).await;
                self.byte_count.fetch_sub(old_size as u64, Ordering::Relaxed);
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
            } else {
                break;
            }
        }

        // 插入新条目
        let entry: CacheEntry = (value.to_vec(), version, expire_at, size);
        self.cache.insert(key.to_string(), entry).await;

        // 更新统计
        self.byte_count.fetch_add(size as u64, Ordering::Relaxed);
        self.entry_count.fetch_add(1, Ordering::Relaxed);

        debug!(
            "Cached key {} with {} bytes (version: {})",
            key,
            value.len(),
            version
        );

        Ok(())
    }

    /// 删除缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 如果键存在且删除成功则返回 Ok(true)，键不存在返回 Ok(false)，错误返回 Err(...)
    #[instrument(skip(self), level = "debug")]
    pub async fn delete(&self, key: &str) -> Result<bool> {
        if let Some((_, _, _, size)) = self.cache.get(key).await {
            self.cache.remove(key).await;
            self.byte_count.fetch_sub(size as u64, Ordering::Relaxed);
            self.entry_count.fetch_sub(1, Ordering::Relaxed);
            debug!("Deleted key {}", key);
            Ok(true)
        } else {
            debug!("Key {} not found for deletion", key);
            Ok(false)
        }
    }

    /// 检查键是否存在
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 如果键存在且未过期则返回 Ok(true)，否则返回 Ok(false)
    #[instrument(skip(self), level = "debug")]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let result = self.cache.get(key).await;
        match result {
            Some((_, _, expire_at, _)) => {
                if let Some(expire_time) = expire_at {
                    if Instant::now() > expire_time {
                        // 键已过期
                        self.cache.remove(key).await;
                        self.record_access(false);
                        return Ok(false);
                    }
                }
                self.record_access(true);
                Ok(true)
            }
            None => {
                self.record_access(false);
                Ok(false)
            }
        }
    }

    /// 清空缓存
    ///
    /// # 返回值
    ///
    /// 清空成功返回 Ok(())
    #[instrument(skip(self), level = "info")]
    pub async fn clear(&self) -> Result<()> {
        self.cache.clear().await;
        self.entry_count.store(0, Ordering::Relaxed);
        self.byte_count.store(0, Ordering::Relaxed);
        debug!("Cache cleared");
        Ok(())
    }

    /// 获取缓存条目数量
    pub fn len(&self) -> u64 {
        self.entry_count.load(Ordering::Relaxed)
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.entry_count.load(Ordering::Relaxed) == 0
    }

    /// 获取当前使用的总字节数
    pub fn used_bytes(&self) -> u64 {
        self.byte_count.load(Ordering::Relaxed)
    }

    /// 获取最大字节数
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// 获取最大条目数
    pub fn max_entries(&self) -> u64 {
        self.max_entries
    }
}
