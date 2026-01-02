//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L1缓存后端的实现，基于内存的高速缓存。

use crate::error::Result;
use moka::future::Cache;
use std::time::{Duration, Instant};
use tracing::{debug, instrument};

/// L1缓存后端实现
///
/// 基于内存的高速缓存实现，使用Moka作为底层缓存库
#[derive(Clone)]
pub struct L1Backend {
    // 值: (数据, 版本/时间戳, 过期时间)
    cache: Cache<String, (Vec<u8>, u64, Option<Instant>)>,
}

impl L1Backend {
    /// 创建新的L1缓存后端实例
    ///
    /// # 参数
    ///
    /// * `capacity` - 缓存最大容量（字节）
    ///
    /// # 返回值
    ///
    /// 返回新的L1Backend实例
    pub fn new(capacity: u64) -> Self {
        Self {
            cache: Cache::builder().max_capacity(capacity).build(),
        }
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
