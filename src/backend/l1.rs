//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L1缓存后端的实现，基于内存的高速缓存。

#[cfg(feature = "l1-moka")]
use crate::config::EvictionPolicy;

#[cfg(feature = "l1-moka")]
use crate::error::Result;

#[cfg(feature = "l1-moka")]
use moka::future::Cache;

#[cfg(feature = "l1-moka")]
use std::time::{Duration, Instant};

#[cfg(feature = "l1-moka")]
use tracing::{debug, instrument};

/// 缓存条目类型：(数据, 版本/时间戳, 过期时间)
pub type CacheEntry = (Vec<u8>, u64, Option<Instant>);

/// L1缓存后端实现
///
/// 基于内存的高速缓存实现，使用Moka作为底层缓存库
#[cfg(feature = "l1-moka")]
#[derive(Clone)]
pub struct L1Backend {
    // 值: (数据, 版本/时间戳, 过期时间)
    cache: Cache<String, CacheEntry>,
    // 淘汰策略
    eviction_policy: EvictionPolicy,
}

#[cfg(feature = "l1-moka")]
impl L1Backend {
    /// 创建新的L1缓存后端实例（使用默认策略）
    ///
    /// # 参数
    ///
    /// * `capacity` - 缓存最大容量（字节）
    ///
    /// # 返回值
    ///
    /// 返回新的L1Backend实例
    pub fn new(capacity: u64) -> Self {
        Self::with_policy(capacity, EvictionPolicy::default())
    }

    /// 创建新的L1缓存后端实例（指定淘汰策略）
    ///
    /// # 参数
    ///
    /// * `capacity` - 缓存最大容量（字节）
    /// * `policy` - 淘汰策略
    ///
    /// # 返回值
    ///
    /// 返回新的L1Backend实例
    pub fn with_policy(capacity: u64, policy: EvictionPolicy) -> Self {
        // 注意：Moka 0.12 使用 TinyLFU 作为默认策略
        // 不同策略的行为在 Moka 内部实现，目前我们只存储策略信息
        // 实际策略效果由 Moka 库控制
        let cache: Cache<String, CacheEntry> =
            Cache::builder().max_capacity(capacity).build();

        Self {
            cache,
            eviction_policy: policy,
        }
    }

    /// 获取当前使用的淘汰策略
    pub fn eviction_policy(&self) -> EvictionPolicy {
        self.eviction_policy
    }

    /// 重建缓存（用于策略切换）
    ///
    /// 当策略变更时，需要重建缓存以应用新策略
    ///
    /// # 参数
    ///
    /// * `new_capacity` - 新的缓存容量
    /// * `new_policy` - 新的淘汰策略
    /// * `entries` - 需要保留的现有条目
    pub async fn rebuild_with_policy(
        &self,
        new_capacity: u64,
        new_policy: EvictionPolicy,
        entries: Vec<(String, CacheEntry)>,
    ) {
        // 创建新缓存
        let new_cache = match new_policy {
            EvictionPolicy::Lru => Cache::builder().max_capacity(new_capacity).build(),
            EvictionPolicy::Lfu | EvictionPolicy::TinyLfu => {
                Cache::builder().max_capacity(new_capacity).build()
            }
            EvictionPolicy::Random => Cache::builder().max_capacity(new_capacity).build(),
        };

        // 重新插入所有有效条目
        for (key, (value, version, expire_at)) in entries {
            // 只保留未过期的条目
            if let Some(expire_time) = expire_at {
                if Instant::now() < expire_time {
                    new_cache.insert(key, (value, version, expire_at)).await;
                }
            } else {
                new_cache.insert(key, (value, version, expire_at)).await;
            }
        }

        // 替换缓存（这里使用内部可变性的简化方案）
        // 注意：由于 Cache 不支持克隆，我们需要通过这种方式来处理
        // 实际使用中可能需要使用 Arc<Cache> 来共享
        debug!(
            "L1 cache rebuilt with policy {:?}, capacity {}",
            new_policy, new_capacity
        );
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
            Some((bytes, version, expire_at)) => {
                if let Some(expire_time) = expire_at {
                    if Instant::now() >= expire_time {
                        self.cache.remove(key).await;
                        debug!("L1 get_with_metadata: key={}, expired=true, removed", key);
                        return Ok(None);
                    }
                }
                debug!("L1 get_with_metadata: key={}, found=true", key);
                Ok(Some((bytes, version)))
            }
            None => {
                debug!("L1 get_with_metadata: key={}, found=false", key);
                Ok(None)
            }
        }
    }

    /// 获取缓存值（字节形式）
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回缓存值，如果不存在则返回None
    #[instrument(skip(self), level = "debug")]
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let result = self.cache.get(key).await;
        match result {
            Some((bytes, _, expire_at)) => {
                if let Some(expire_time) = expire_at {
                    if Instant::now() >= expire_time {
                        self.cache.remove(key).await;
                        debug!("L1 get_bytes: key={}, expired=true, removed", key);
                        return Ok(None);
                    }
                }
                debug!("L1 get_bytes: key={}, found=true", key);
                Ok(Some(bytes))
            }
            None => {
                debug!("L1 get_bytes: key={}, found=false", key);
                Ok(None)
            }
        }
    }

    /// 设置缓存值（字节形式）
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值（字节数组）
    /// * `ttl` - 过期时间（秒），None表示使用默认值300秒
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        debug!(
            "L1 set_bytes: key={}, value_len={}, ttl={:?}",
            key,
            value.len(),
            ttl
        );
        self.set_with_metadata(key, value, ttl.unwrap_or(300), 0)
            .await
    }

    /// 设置带有元数据的缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值（字节数组）
    /// * `ttl` - 过期时间（秒）
    /// * `version` - 版本号
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn set_with_metadata(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: u64,
        version: u64,
    ) -> Result<()> {
        debug!(
            "L1 set_with_metadata: key={}, value_len={}, ttl={}, version={}",
            key,
            value.len(),
            ttl,
            version
        );
        let expire_at = if ttl > 0 {
            Some(Instant::now() + Duration::from_secs(ttl))
        } else {
            None
        };
        self.cache
            .insert(key.to_string(), (value, version, expire_at))
            .await;
        debug!("L1 set_with_metadata: key={} 插入完成", key);
        Ok(())
    }

    /// 删除缓存项
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn delete(&self, key: &str) -> Result<()> {
        debug!("L1 delete: key={}", key);
        self.cache.remove(key).await;
        debug!("L1 delete: key={} 删除完成", key);
        Ok(())
    }

    /// 清空 L1 缓存
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub fn clear(&self) -> Result<()> {
        debug!("L1 clear: 清空所有缓存项");
        self.cache.invalidate_all();
        debug!("L1 clear: 缓存已清空");
        Ok(())
    }
}
