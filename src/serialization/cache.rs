//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 序列化缓存模块
//!
//! 提供带容量管理的序列化缓存，支持混合限制（条目数 + 内存）。

use crate::config::validation::DEFAULT_MAX_MEMORY_BYTES;
use crate::error::Result;
use crate::serialization::Serializer;
use dashmap::DashMap;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// 序列化缓存条目
#[derive(Clone, Debug)]
pub struct SerializationCacheEntry<S: Serializer + Clone> {
    /// 键
    key: String,
    /// 序列化后的数据
    serialized: Vec<u8>,
    /// 序列化大小（字节）
    serialized_size: usize,
    /// 创建时间
    created_at: Instant,
    /// 最后访问时间
    last_accessed: Instant,
    /// 访问次数
    access_count: u64,
    /// TTL（秒）
    ttl: Option<u64>,
    /// 序列化器（用于反序列化）
    _serializer: S,
}

impl<S: Serializer + Clone> SerializationCacheEntry<S> {
    /// 创建新的缓存条目
    pub fn new(key: String, serialized: Vec<u8>, ttl: Option<u64>, serializer: S) -> Self {
        let now = Instant::now();
        let serialized_size = serialized.len();
        Self {
            key,
            serialized,
            serialized_size,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            ttl,
            _serializer: serializer,
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            if Instant::now().duration_since(self.created_at) > Duration::from_secs(ttl) {
                return true;
            }
        }
        false
    }

    /// 更新访问
    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }

    /// 获取序列化数据引用
    pub fn serialized_data(&self) -> &[u8] {
        &self.serialized
    }

    /// 获取序列化数据（克隆）
    pub fn into_serialized(self) -> Vec<u8> {
        self.serialized
    }

    /// 获取序列化大小
    pub fn size(&self) -> usize {
        self.serialized_size + self.key.len() + 64
    }

    /// 获取访问次数
    pub fn access_count(&self) -> u64 {
        self.access_count
    }

    /// 获取键
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// 序列化缓存配置
#[derive(Debug, Clone)]
pub struct SerializationCacheConfig {
    /// 最大条目数
    pub max_entries: u64,
    /// 最大内存使用（字节）
    pub max_memory_bytes: u64,
    /// 默认 TTL（秒）
    pub default_ttl: Option<u64>,
    /// 淘汰阈值（当达到此百分比时触发淘汰）
    pub eviction_threshold: f64,
    /// 是否启用 TTL
    pub enable_ttl: bool,
}

impl Default for SerializationCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES as u64,
            default_ttl: None,
            eviction_threshold: 0.9,
            enable_ttl: true,
        }
    }
}

/// 序列化缓存统计
#[derive(Debug, Clone, Default)]
pub struct SerializationCacheStats {
    /// 当前条目数
    pub entry_count: u64,
    /// 当前内存使用（字节）
    pub memory_bytes: u64,
    /// 最大条目数
    pub max_entries: u64,
    /// 最大内存（字节）
    pub max_memory_bytes: u64,
    /// 总命中次数
    pub hit_count: u64,
    /// 总未命中次数
    pub miss_count: u64,
    /// 总获取次数
    pub total_accesses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 总序列化次数
    pub serialize_count: u64,
    /// 总反序列化次数
    pub deserialize_count: u64,
    /// 淘汰次数
    pub eviction_count: u64,
    /// 平均序列化时间（微秒）
    pub avg_serialize_us: f64,
    /// 平均反序列化时间（微秒）
    pub avg_deserialize_us: f64,
}

/// 序列化缓存
///
/// 提供带容量管理的序列化缓存，支持混合限制（条目数 + 内存）。
#[derive(Clone)]
pub struct SerializationCache<S: Serializer + Clone> {
    /// 缓存存储
    cache: Arc<DashMap<String, SerializationCacheEntry<S>>>,
    /// 序列化器
    serializer: S,
    /// 配置
    config: Arc<SerializationCacheConfig>,
    /// 统计
    entry_count: Arc<AtomicU64>,
    memory_bytes: Arc<AtomicU64>,
    hit_count: Arc<AtomicU64>,
    miss_count: Arc<AtomicU64>,
    serialize_count: Arc<AtomicU64>,
    deserialize_count: Arc<AtomicU64>,
    eviction_count: Arc<AtomicU64>,
    total_serialize_us: Arc<AtomicU64>,
    total_deserialize_us: Arc<AtomicU64>,
}

impl<S: Serializer + Clone> SerializationCache<S> {
    /// 创建新的序列化缓存
    ///
    /// # 参数
    /// * `serializer` - 序列化器
    /// * `config` - 缓存配置
    ///
    /// # 返回值
    /// * 新的 SerializationCache 实例
    pub fn new(serializer: S, config: SerializationCacheConfig) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            serializer,
            config: Arc::new(config),
            entry_count: Arc::new(AtomicU64::new(0)),
            memory_bytes: Arc::new(AtomicU64::new(0)),
            hit_count: Arc::new(AtomicU64::new(0)),
            miss_count: Arc::new(AtomicU64::new(0)),
            serialize_count: Arc::new(AtomicU64::new(0)),
            deserialize_count: Arc::new(AtomicU64::new(0)),
            eviction_count: Arc::new(AtomicU64::new(0)),
            total_serialize_us: Arc::new(AtomicU64::new(0)),
            total_deserialize_us: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 设置值
    ///
    /// # 参数
    /// * `key` - 缓存键
    /// * `value` - 值
    /// * `ttl` - TTL（秒）
    ///
    /// # 返回值
    /// * 成功返回 Ok(())
    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl: Option<u64>) -> Result<()> {
        let ttl = ttl.or(self.config.default_ttl);

        // 序列化
        let start = Instant::now();
        let serialized = self.serializer.serialize(value)?;
        let elapsed = start.elapsed().as_micros() as u64;
        self.serialize_count.fetch_add(1, Ordering::Relaxed);
        self.total_serialize_us
            .fetch_add(elapsed, Ordering::Relaxed);

        let serialized_len = serialized.len();
        let entry_size = key.len() + serialized_len + 64;

        // 检查是否超出容量
        self.maybe_evict(entry_size).await;

        // 插入新条目
        let entry =
            SerializationCacheEntry::new(key.to_string(), serialized, ttl, self.serializer.clone());

        self.cache.insert(key.to_string(), entry);
        self.entry_count.fetch_add(1, Ordering::Relaxed);
        self.memory_bytes
            .fetch_add(entry_size as u64, Ordering::Relaxed);

        debug!(
            "Cached key {} with {} bytes (entries: {}, memory: {} KB)",
            key,
            serialized_len,
            self.entry_count.load(Ordering::Relaxed),
            self.memory_bytes.load(Ordering::Relaxed) / 1024
        );

        Ok(())
    }

    /// 获取序列化数据
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * Some(序列化数据) 如果存在
    /// * None 如果不存在
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // 使用 mutate 更新访问时间
        if let Some(mut entry) = self.cache.get_mut(key) {
            if self.config.enable_ttl && entry.is_expired() {
                let entry_size = entry.size() as u64;
                drop(entry); // 释放锁
                self.cache.remove(key);
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_bytes.fetch_sub(entry_size, Ordering::Relaxed);
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }

            // 更新访问时间和计数
            entry.touch();
            let data = entry.serialized.clone();
            drop(entry); // 显式释放锁

            self.hit_count.fetch_add(1, Ordering::Relaxed);
            Ok(Some(data))
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            Ok(None)
        }
    }

    /// 获取并反序列化值
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * Some(值) 如果存在
    /// * None 如果不存在
    pub async fn get_deserialized<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let start = Instant::now();
        match self.get(key).await {
            Ok(Some(data)) => {
                let result = self.serializer.deserialize::<T>(&data);
                let elapsed = start.elapsed().as_micros() as u64;
                self.deserialize_count.fetch_add(1, Ordering::Relaxed);
                self.total_deserialize_us
                    .fetch_add(elapsed, Ordering::Relaxed);

                match result {
                    Ok(value) => Ok(Some(value)),
                    Err(e) => {
                        warn!("Failed to deserialize cache entry: {}", e);
                        self.delete(key).await.ok();
                        Ok(None)
                    }
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 删除条目
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * true 如果删除了条目
    /// * false 如果条目不存在
    pub async fn delete(&self, key: &str) -> Result<bool> {
        if let Some((_, entry)) = self.cache.remove(key) {
            self.entry_count.fetch_sub(1, Ordering::Relaxed);
            self.memory_bytes
                .fetch_sub(entry.size() as u64, Ordering::Relaxed);
            debug!("Deleted key {}", key);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        if let Some(mut entry) = self.cache.get_mut(key) {
            if self.config.enable_ttl && entry.is_expired() {
                drop(entry);
                self.cache.remove(key);
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                return Ok(false);
            }
            // 更新访问时间
            entry.touch();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 清空缓存
    pub async fn clear(&self) {
        self.cache.clear();
        self.entry_count.store(0, Ordering::Relaxed);
        self.memory_bytes.store(0, Ordering::Relaxed);
        debug!("Serialization cache cleared");
    }

    /// 触发淘汰
    async fn maybe_evict(&self, additional_size: usize) {
        let max_entries = self.config.max_entries;
        let max_memory = self.config.max_memory_bytes;
        let threshold = self.config.eviction_threshold;

        let current_entries = self.entry_count.load(Ordering::Relaxed);
        let current_memory = self.memory_bytes.load(Ordering::Relaxed);

        // 检查是否需要淘汰
        let need_evict_entries = current_entries >= max_entries;
        let need_evict_memory = (current_memory + additional_size as u64) > max_memory;
        let memory_threshold = current_memory as f64 / max_memory as f64 >= threshold;

        if need_evict_entries || need_evict_memory || memory_threshold {
            self.evict().await;
        }
    }

    /// 淘汰低优先级条目
    async fn evict(&self) {
        let max_entries = self.config.max_entries;
        let max_memory = self.config.max_memory_bytes;

        let current_entries = self.entry_count.load(Ordering::Relaxed);
        let current_memory = self.memory_bytes.load(Ordering::Relaxed);

        // 目标：减少到 70%
        let target_entries = (max_entries as f64 * 0.7) as u64;
        let target_memory = (max_memory as f64 * 0.7) as u64;

        let to_remove: Vec<String> = self
            .cache
            .iter()
            .filter(|_entry| current_entries > target_entries || current_memory > target_memory)
            .filter(|entry| {
                // 过期条目优先淘汰
                if self.config.enable_ttl && entry.is_expired() {
                    true
                } else {
                    // 基于访问频率和时间局部性计算分数
                    self.calculate_eviction_score(entry) > 0.5
                }
            })
            .map(|entry| entry.key().clone())
            .collect();

        let remove_count = to_remove.len();

        // 淘汰条目
        for key in to_remove {
            if let Some((_, entry)) = self.cache.remove(&key) {
                self.eviction_count.fetch_add(1, Ordering::Relaxed);
                self.entry_count.fetch_sub(1, Ordering::Relaxed);
                self.memory_bytes
                    .fetch_sub(entry.size() as u64, Ordering::Relaxed);
            }
        }

        if remove_count > 0 {
            debug!("Evicted {} entries", remove_count);
        }
    }

    /// 计算淘汰分数（0-1，越高越应该淘汰）
    fn calculate_eviction_score(&self, entry: &SerializationCacheEntry<S>) -> f64 {
        // 访问频率越低，越应该淘汰
        let access_freq_score = 1.0 / (entry.access_count as f64 + 1.0);

        // 时间局部性：越久没访问，越应该淘汰
        let recency = entry.last_accessed.elapsed().as_secs_f64();
        let recency_score = recency / (recency + 60.0); // 60秒归一化

        // 综合分数
        access_freq_score * 0.6 + recency_score * 0.4
    }

    /// 获取统计信息
    pub fn stats(&self) -> SerializationCacheStats {
        let total =
            self.hit_count.load(Ordering::Relaxed) + self.miss_count.load(Ordering::Relaxed);
        let hit_rate = if total > 0 {
            self.hit_count.load(Ordering::Relaxed) as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        let avg_serialize = if self.serialize_count.load(Ordering::Relaxed) > 0 {
            self.total_serialize_us.load(Ordering::Relaxed) as f64
                / self.serialize_count.load(Ordering::Relaxed) as f64
        } else {
            0.0
        };

        let avg_deserialize = if self.deserialize_count.load(Ordering::Relaxed) > 0 {
            self.total_deserialize_us.load(Ordering::Relaxed) as f64
                / self.deserialize_count.load(Ordering::Relaxed) as f64
        } else {
            0.0
        };

        SerializationCacheStats {
            entry_count: self.entry_count.load(Ordering::Relaxed),
            memory_bytes: self.memory_bytes.load(Ordering::Relaxed),
            max_entries: self.config.max_entries,
            max_memory_bytes: self.config.max_memory_bytes,
            hit_count: self.hit_count.load(Ordering::Relaxed),
            miss_count: self.miss_count.load(Ordering::Relaxed),
            total_accesses: total,
            hit_rate,
            serialize_count: self.serialize_count.load(Ordering::Relaxed),
            deserialize_count: self.deserialize_count.load(Ordering::Relaxed),
            eviction_count: self.eviction_count.load(Ordering::Relaxed),
            avg_serialize_us: avg_serialize,
            avg_deserialize_us: avg_deserialize,
        }
    }

    /// 获取当前条目数
    pub fn len(&self) -> u64 {
        self.entry_count.load(Ordering::Relaxed)
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.entry_count.load(Ordering::Relaxed) == 0
    }

    /// 获取当前内存使用
    pub fn memory_usage(&self) -> u64 {
        self.memory_bytes.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialization::JsonSerializer;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestStruct {
        name: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let serializer = JsonSerializer::default();
        let config = SerializationCacheConfig {
            max_entries: 100,
            max_memory_bytes: 1024 * 1024,
            ..Default::default()
        };

        let cache = SerializationCache::new(serializer, config);

        // Test set and get
        cache
            .set("key1", &"value1".to_string(), None)
            .await
            .unwrap();
        let result = cache.get("key1").await.unwrap();
        assert_eq!(result, Some(b"\"value1\"".to_vec()));

        // Test get non-existent key
        let result = cache.get("nonexistent").await.unwrap();
        assert!(result.is_none());

        // Test delete
        let deleted = cache.delete("key1").await.unwrap();
        assert!(deleted);

        // Test exists
        let exists = cache.exists("key1").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_stats() {
        let serializer = JsonSerializer::default();
        let config = SerializationCacheConfig {
            max_entries: 100,
            max_memory_bytes: 1024 * 1024,
            ..Default::default()
        };

        let cache = SerializationCache::new(serializer, config);

        // Initial stats
        let stats = cache.stats();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);

        // Add some entries
        for i in 0..10 {
            cache
                .set(&format!("key{}", i), &format!("value{}", i), None)
                .await
                .unwrap();
        }

        // Get some entries
        for i in 0..5 {
            cache.get(&format!("key{}", i)).await.unwrap();
        }

        // Get non-existent
        cache.get("nonexistent").await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 10);
        assert_eq!(stats.hit_count, 5);
        assert_eq!(stats.miss_count, 1);
    }

    #[tokio::test]
    async fn test_clear() {
        let serializer = JsonSerializer::default();
        let config = SerializationCacheConfig::default();

        let cache = SerializationCache::new(serializer, config);

        cache
            .set("key1", &"value1".to_string(), None)
            .await
            .unwrap();
        cache
            .set("key2", &"value2".to_string(), None)
            .await
            .unwrap();

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 2);

        cache.clear().await;

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[tokio::test]
    async fn test_deserialize() {
        let serializer = JsonSerializer::default();
        let config = SerializationCacheConfig::default();

        let cache = SerializationCache::new(serializer, config);

        let test_value = TestStruct {
            name: "test".to_string(),
            value: 42,
        };

        cache.set("key1", &test_value, None).await.unwrap();
        let result = cache.get_deserialized::<TestStruct>("key1").await.unwrap();

        assert!(result.is_some());
        let inner = result.unwrap();
        assert_eq!(inner.name, "test");
        assert_eq!(inner.value, 42);
    }
}
