//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 分片锁模块
//!
//! 提供高性能的分布式锁实现，使用分片技术减少锁竞争。

use crate::backend::strategy::L2BackendStrategy;
use crate::error::Result;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OwnedRwLockWriteGuard;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// 锁信息
#[derive(Clone, Debug)]
struct LockInfo {
    /// 锁值（用于安全释放）
    lock_value: String,
    /// 过期时间
    expire_at: std::time::Instant,
    /// TTL（秒）
    ttl: u64,
}

impl LockInfo {
    fn new(lock_value: String, ttl: u64) -> Self {
        Self {
            lock_value,
            expire_at: std::time::Instant::now() + Duration::from_secs(ttl),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        std::time::Instant::now() >= self.expire_at
    }
}

/// 分片锁管理器
///
/// 使用多个分片来分散锁操作，减少单点竞争。
/// 每个分片包含本地锁缓存和分布式锁状态。
#[derive(Clone)]
pub struct ShardedLockManager {
    /// 分片数量
    num_shards: usize,
    /// 分片锁
    shards: Vec<Arc<DashMap<String, LockInfo>>>,
    /// 本地锁缓存（用于减少 L2 访问）
    local_cache: Vec<Arc<DashMap<String, (String, std::time::Instant)>>>,
    /// L2 后端
    l2_backend: Arc<dyn L2BackendStrategy>,
    /// 获取锁的计数器
    lock_counter: Arc<AtomicU64>,
    /// 释放锁的计数器
    unlock_counter: Arc<AtomicU64>,
    /// 当前分片索引
    current_shard: Arc<AtomicUsize>,
}

impl ShardedLockManager {
    /// 创建新的分片锁管理器
    ///
    /// # 参数
    /// * `l2_backend` - L2 后端（用于分布式锁）
    /// * `num_shards` - 分片数量（建议为 CPU 核心数的 2-4 倍）
    pub fn new(l2_backend: Arc<dyn L2BackendStrategy>, num_shards: usize) -> Self {
        let shards: Vec<_> = (0..num_shards)
            .map(|_| Arc::new(DashMap::new()))
            .collect();
        let local_cache: Vec<_> = (0..num_shards)
            .map(|_| Arc::new(DashMap::new()))
            .collect();

        Self {
            num_shards,
            shards,
            local_cache,
            l2_backend,
            lock_counter: Arc::new(AtomicU64::new(0)),
            unlock_counter: Arc::new(AtomicU64::new(0)),
            current_shard: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 获取默认分片数量（CPU 核心数 * 2）
    pub fn default_num_shards() -> usize {
        std::thread::available_parallelism()
            .map(|p| p.get() * 2)
            .unwrap_or(16)
    }

    /// 计算键应该属于哪个分片
    fn shard_for_key(&self, key: &str) -> usize {
        // 使用一致性哈希计算分片
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(key, &mut hasher);
        let hash = std::hash::Hasher::finish(&hasher);
        (hash as usize) % self.num_shards
    }

    /// 尝试获取锁
    ///
    /// # 参数
    /// * `key` - 锁键
    /// * `ttl` - 锁过期时间（秒）
    /// * `retry_interval` - 重试间隔（毫秒）
    /// * `max_retries` - 最大重试次数
    ///
    /// # 返回值
    /// * `Ok(Some(lock_value))` - 成功获取锁
    /// * `Ok(None)` - 锁已被占用
    /// * `Err(...)` - 发生错误
    pub async fn try_lock(
        &self,
        key: &str,
        ttl: u64,
        retry_interval: u64,
        max_retries: u32,
    ) -> Result<Option<String>> {
        let shard_idx = self.shard_for_key(key);
        let shard = &self.shards[shard_idx];
        let cache = &self.local_cache[shard_idx];

        // 生成唯一的锁值
        let lock_value = format!("{}-{}", uuid::Uuid::new_v4(), std::process::id());

        // 先检查本地缓存
        if let Some((cached_value, expire_at)) = cache.get(key) {
            if std::time::Instant::now() < *expire_at {
                // 本地缓存有效，检查是否是当前进程持有的锁
                // 实际上这里应该检查 L2，但我们先假设本地缓存是有效的
                return Ok(Some(cached_value.clone()));
            }
        }

        // 尝试获取分布式锁
        for attempt in 0..=max_retries {
            match self.l2_backend.lock(key, ttl).await {
                Ok(Some(value)) => {
                    // 获取锁成功
                    self.lock_counter.fetch_add(1, Ordering::Relaxed);

                    // 更新本地缓存
                    cache.insert(
                        key.to_string(),
                        (value.clone(), std::time::Instant::now() + Duration::from_secs(ttl / 2)),
                    );

                    // 更新分片锁状态
                    shard.insert(key.to_string(), LockInfo::new(value.clone(), ttl));

                    debug!("Acquired lock for key {} on attempt {}", key, attempt + 1);
                    return Ok(Some(value));
                }
                Ok(None) => {
                    // 锁已被占用
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_millis(retry_interval)).await;
                    }
                }
                Err(e) => {
                    error!("Failed to acquire lock for key {}: {}", key, e);
                    return Err(e);
                }
            }
        }

        // 所有重试次数都用完，仍然无法获取锁
        debug!("Failed to acquire lock for key {} after {} attempts", key, max_retries + 1);
        Ok(None)
    }

    /// 带超时获取锁
    ///
    /// # 参数
    /// * `key` - 锁键
    /// * `ttl` - 锁过期时间（秒）
    /// * `timeout_ms` - 总超时时间（毫秒）
    ///
    /// # 返回值
    /// * `Ok(Some(lock_value))` - 成功获取锁
    /// * `Ok(None)` - 超时或锁已被占用
    /// * `Err(...)` - 发生错误
    pub async fn lock_with_timeout(
        &self,
        key: &str,
        ttl: u64,
        timeout_ms: u64,
    ) -> Result<Option<String>> {
        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_millis(timeout_ms);
        let retry_interval = 100; // 100ms 重试间隔

        // 计算最大重试次数
        let max_retries = (timeout_ms / retry_interval) as u32;

        let shard_idx = self.shard_for_key(key);
        let shard = &self.shards[shard_idx];
        let cache = &self.local_cache[shard_idx];

        // 生成唯一的锁值
        let lock_value = format!("{}-{}", uuid::Uuid::new_v4(), std::process::id());

        // 先检查本地缓存
        if let Some((cached_value, expire_at)) = cache.get(key) {
            if std::time::Instant::now() < *expire_at {
                return Ok(Some(cached_value.clone()));
            }
        }

        // 带超时尝试获取分布式锁
        loop {
            // 检查是否超时
            if start.elapsed() >= timeout_duration {
                debug!("Lock acquisition timed out for key {} after {}ms", key, timeout_ms);
                return Ok(None);
            }

            match self.l2_backend.lock(key, ttl).await {
                Ok(Some(value)) => {
                    // 获取锁成功
                    self.lock_counter.fetch_add(1, Ordering::Relaxed);

                    // 更新本地缓存
                    cache.insert(
                        key.to_string(),
                        (value.clone(), std::time::Instant::now() + Duration::from_secs(ttl / 2)),
                    );

                    // 更新分片锁状态
                    shard.insert(key.to_string(), LockInfo::new(value.clone(), ttl));

                    debug!("Acquired lock for key {}", key);
                    return Ok(Some(value));
                }
                Ok(None) => {
                    // 锁已被占用，等待后重试
                    let remaining = timeout_duration.saturating_sub(start.elapsed());
                    let sleep_duration = Duration::from_millis(retry_interval).min(remaining);

                    if sleep_duration.is_zero() {
                        debug!("Failed to acquire lock for key {} within timeout", key);
                        return Ok(None);
                    }

                    tokio::time::sleep(sleep_duration).await;
                }
                Err(e) => {
                    error!("Failed to acquire lock for key {}: {}", key, e);
                    return Err(e);
                }
            }
        }
    }

    /// 释放锁
    ///
    /// # 参数
    /// * `key` - 锁键
    /// * `lock_value` - 锁值（创建锁时生成的值）
    ///
    /// # 返回值
    /// * `Ok(true)` - 锁已释放
    /// * `Ok(false)` - 锁不存在或值不匹配
    /// * `Err(...)` - 发生错误
    pub async fn unlock(&self, key: &str, lock_value: &str) -> Result<bool> {
        let shard_idx = self.shard_for_key(key);
        let shard = &self.shards[shard_idx];
        let cache = &self.local_cache[shard_idx];

        // 先检查本地锁状态
        if let Some(lock_info) = shard.get(key) {
            if lock_info.lock_value != lock_value {
                // 锁值不匹配，可能是过期的锁
                debug!("Lock value mismatch for key {}", key);
                return Ok(false);
            }
        }

        // 尝试释放分布式锁
        match self.l2_backend.unlock(key, lock_value).await {
            Ok(released) => {
                if released {
                    self.unlock_counter.fetch_add(1, Ordering::Relaxed);

                    // 清除本地状态
                    shard.remove(key);
                    cache.remove(key);

                    debug!("Released lock for key {}", key);
                } else {
                    debug!("Failed to release lock for key {}: lock not found or value mismatch", key);
                }
                Ok(released)
            }
            Err(e) => {
                error!("Error releasing lock for key {}: {}", key, e);
                Err(e)
            }
        }
    }

    /// 检查锁是否存在
    pub async fn is_locked(&self, key: &str) -> bool {
        let shard_idx = self.shard_for_key(key);
        let shard = &self.shards[shard_idx];

        if let Some(lock_info) = shard.get(key) {
            if lock_info.is_expired() {
                // 锁已过期，清理
                shard.remove(key);
                return false;
            }
            return true;
        }

        false
    }

    /// 延长锁的过期时间
    ///
    /// # 参数
    /// * `key` - 锁键
    /// * `lock_value` - 锁值
    /// * `ttl` - 新的 TTL（秒）
    ///
    /// # 返回值
    /// * `Ok(true)` - 延长成功
    /// * `Ok(false)` - 锁不存在或值不匹配
    /// * `Err(...)` - 发生错误
    pub async fn extend_lock(&self, key: &str, lock_value: &str, ttl: u64) -> Result<bool> {
        let shard_idx = self.shard_for_key(key);
        let shard = &self.shards[shard_idx];
        let cache = &self.local_cache[shard_idx];

        // 检查锁是否存在且值匹配
        if let Some(lock_info) = shard.get(key) {
            if lock_info.lock_value != lock_value {
                return Ok(false);
            }

            // 检查锁是否已过期
            if lock_info.is_expired() {
                shard.remove(key);
                cache.remove(key);
                return Ok(false);
            }

            // 重新获取锁（这会更新过期时间）
            match self.l2_backend.lock(key, ttl).await {
                Ok(Some(new_value)) => {
                    if new_value == lock_value {
                        // 成功延长
                        let new_info = LockInfo::new(lock_value.to_string(), ttl);
                        shard.insert(key.to_string(), new_info);

                        // 更新本地缓存
                        cache.insert(
                            key.to_string(),
                            (lock_value.to_string(), std::time::Instant::now() + Duration::from_secs(ttl / 2)),
                        );

                        debug!("Extended lock for key {} with TTL {}s", key, ttl);
                        return Ok(true);
                    } else {
                        // 锁已被其他进程获取
                        shard.remove(key);
                        cache.remove(key);
                        return Ok(false);
                    }
                }
                Ok(None) => {
                    // 锁已被释放
                    shard.remove(key);
                    cache.remove(key);
                    return Ok(false);
                }
                Err(e) => return Err(e),
            }
        }

        Ok(false)
    }

    /// 获取锁统计信息
    pub fn stats(&self) -> LockStats {
        let mut total_locks = 0;
        let mut expired_locks = 0;
        let mut local_cache_size = 0;

        for shard in &self.shards {
            total_locks += shard.len();
            for entry in shard.iter() {
                if entry.value().is_expired() {
                    expired_locks += 1;
                }
            }
        }

        for cache in &self.local_cache {
            local_cache_size += cache.len();
        }

        LockStats {
            total_locks,
            expired_locks,
            local_cache_size,
            num_shards: self.num_shards,
            locks_acquired: self.lock_counter.load(Ordering::Relaxed),
            locks_released: self.unlock_counter.load(Ordering::Relaxed),
        }
    }

    /// 清理过期的锁
    pub async fn cleanup(&self) {
        let now = std::time::Instant::now();

        for shard in &self.shards {
            let expired_keys: Vec<_> = shard
                .iter()
                .filter(|e| e.value().expire_at <= now)
                .map(|e| e.key().clone())
                .collect();

            for key in expired_keys {
                shard.remove(&key);
            }
        }

        // 清理本地缓存
        for cache in &self.local_cache {
            let expired_keys: Vec<_> = cache
                .iter()
                .filter(|e| e.value().1 <= now)
                .map(|e| e.key().clone())
                .collect();

            for key in expired_keys {
                cache.remove(&key);
            }
        }

        debug!(
            "Cleaned up expired locks: {} total locks across {} shards",
            self.shards.iter().map(|s| s.len()).sum::<usize>(),
            self.num_shards
        );
    }
}

/// 锁统计信息
#[derive(Debug, Clone)]
pub struct LockStats {
    /// 总锁数量
    pub total_locks: usize,
    /// 过期锁数量
    pub expired_locks: usize,
    /// 本地缓存大小
    pub local_cache_size: usize,
    /// 分片数量
    pub num_shards: usize,
    /// 获取锁的次数
    pub locks_acquired: u64,
    /// 释放锁的次数
    pub locks_released: u64,
}

/// 锁守卫
///
/// 用于自动释放锁的 RAII 结构。
#[derive(Debug)]
pub struct LockGuard {
    /// 分片锁管理器
    manager: ShardedLockManager,
    /// 锁键
    key: String,
    /// 锁值
    lock_value: String,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // 在析构时自动释放锁
        tokio::spawn(async move {
            if let Err(e) = self.manager.unlock(&self.key, &self.lock_value).await {
                warn!("Failed to auto-release lock for key {}: {}", self.key, e);
            }
        });
    }
}

impl LockGuard {
    /// 创建新的锁守卫
    pub fn new(manager: ShardedLockManager, key: String, lock_value: String) -> Self {
        Self {
            manager,
            key,
            lock_value,
        }
    }

    /// 获取锁值
    pub fn lock_value(&self) -> &str {
        &self.lock_value
    }
}
