//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存客户端的接口和实现。

pub mod db_loader;

#[cfg(feature = "l1-moka")]
pub mod l1;

#[cfg(feature = "l2-redis")]
pub mod l2;

#[cfg(all(feature = "l1-moka", feature = "l2-redis"))]
pub mod two_level;

#[cfg(feature = "redis-native")]
pub mod redis_native;

#[cfg(feature = "ttl-control")]
pub mod ttl_control;

#[cfg(feature = "tiered-cache")]
pub mod tiered_cache;

use crate::error::Result;
use async_trait::async_trait;
use std::any::Any;

use crate::serialization::Serializer;
use serde::{de::DeserializeOwned, Serialize};
use tracing::instrument;

/// 缓存扩展特征
///
/// 提供类型安全的缓存操作接口
#[async_trait]
pub trait CacheExt: CacheOps {
    /// 获取缓存值（反序列化）
    #[instrument(skip(self), level = "debug")]
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let bytes = self.get_bytes(key).await?;
        match bytes {
            Some(data) => {
                let val = self.serializer().deserialize(&data)?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// 设置缓存值（序列化）
    #[instrument(skip(self, value), level = "debug")]
    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<u64>,
    ) -> Result<()> {
        let bytes = self.serializer().serialize(value)?;
        self.set_bytes(key, bytes, ttl).await
    }

    /// 仅设置 L1 缓存（如果支持）
    #[instrument(skip(self, value), level = "debug")]
    async fn set_l1_only<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<u64>,
    ) -> Result<()> {
        let bytes = self.serializer().serialize(value)?;
        self.set_l1_bytes(key, bytes, ttl).await
    }

    #[instrument(skip(self, value), level = "debug")]
    async fn set_l2_only<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<u64>,
    ) -> Result<()> {
        let bytes = self.serializer().serialize(value)?;
        self.set_l2_bytes(key, bytes, ttl).await
    }

    /// 获取缓存值，如果不存在则调用回调函数获取并缓存
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `ttl` - 缓存时间（秒）
    /// * `fetch` - 当缓存未命中时调用的回调函数
    ///
    /// # 返回值
    ///
    /// 返回缓存的值或从回调函数获取的值
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let user = cache
    ///     .get_or_fetch("user:123", Some(3600), || async {
    ///         database::get_user("123").await
    ///     })
    ///     .await?;
    /// ```
    #[instrument(skip(self, fetch), level = "debug")]
    async fn get_or_fetch<T, F, Fut>(&self, key: &str, ttl: Option<u64>, fetch: F) -> Result<T>
    where
        T: DeserializeOwned + Serialize + Send + Sync + Clone,
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        // 尝试从缓存获取
        if let Some(cached) = self.get::<T>(key).await? {
            return Ok(cached);
        }

        // 缓存未命中，从数据源获取
        let value = fetch().await?;

        // 缓存结果
        self.set(key, &value, ttl).await?;

        Ok(value)
    }

    /// 尝试获取，如果不存在返回 None（不触发 fetch）
    ///
    /// 这是一个便捷方法，避免使用 Option 处理
    #[instrument(skip(self), level = "debug")]
    async fn try_get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        self.get(key).await
    }

    /// 删除并返回旧值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回被删除的值（如果存在）
    #[instrument(skip(self), level = "debug")]
    async fn remove<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let old_value = self.get::<T>(key).await?;
        self.delete(key).await?;
        Ok(old_value)
    }

    /// 检查键是否存在
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 如果键存在返回 true，否则返回 false
    #[instrument(skip(self), level = "debug")]
    async fn contains(&self, key: &str) -> Result<bool> {
        Ok(self.get_bytes(key).await?.is_some())
    }
}

impl<T: CacheOps + ?Sized> CacheExt for T {}

/// 缓存操作特征
///
/// 定义缓存系统的基本操作接口
#[async_trait]
pub trait CacheOps: Send + Sync + Any {
    /// 获取缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回缓存值，如果不存在则返回None
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 获取 L1 缓存值（字节）
    async fn get_l1_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Err(crate::error::CacheError::NotSupported(
            "get_l1_bytes".to_string(),
        ))
    }

    /// 获取 L2 缓存值（字节）
    async fn get_l2_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Err(crate::error::CacheError::NotSupported(
            "get_l2_bytes".to_string(),
        ))
    }

    /// 设置缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值
    /// * `ttl` - 过期时间（秒），None表示使用默认值
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()>;

    /// 设置 L1 缓存值（字节）
    async fn set_l1_bytes(&self, _key: &str, _value: Vec<u8>, _ttl: Option<u64>) -> Result<()> {
        Err(crate::error::CacheError::NotSupported(
            "set_l1_bytes".to_string(),
        ))
    }

    /// 设置 L2 缓存值（字节）
    async fn set_l2_bytes(&self, _key: &str, _value: Vec<u8>, _ttl: Option<u64>) -> Result<()> {
        Err(crate::error::CacheError::NotSupported(
            "set_l2_bytes".to_string(),
        ))
    }

    /// 尝试获取分布式锁
    ///
    /// 使用 SET NX PX 实现，自动生成安全的随机锁值
    ///
    /// # 参数
    ///
    /// * `key` - 锁的键
    /// * `ttl` - 锁的过期时间（秒）
    ///
    /// # 返回值
    ///
    /// * `Ok(Some(value))` - 成功获取锁，value 为生成的锁值
    /// * `Ok(None)` - 锁已被其他进程持有
    /// * `Err(...)` - 发生错误
    async fn lock(&self, _key: &str, _ttl: u64) -> Result<Option<String>> {
        Ok(None)
    }

    /// 释放分布式锁
    ///
    /// # 参数
    ///
    /// * `key` - 锁的键
    /// * `value` - 锁的值（必须匹配才能释放）
    ///
    /// # 返回值
    ///
    /// 成功释放返回 true，否则返回 false（例如锁不存在或值不匹配）
    async fn unlock(&self, _key: &str, _value: &str) -> Result<bool> {
        Ok(false)
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
    async fn delete(&self, key: &str) -> Result<()>;

    /// 获取序列化器
    ///
    /// 返回当前客户端使用的序列化器
    fn serializer(&self) -> &crate::serialization::SerializerEnum;

    /// 将 trait object 转换为 Any，用于向下转型
    fn as_any(&self) -> &dyn Any;

    /// 将 `Arc<Trait>` 转换为 `Arc<dyn Any>`，支持 Arc 下的向下转型
    fn into_any_arc(self: std::sync::Arc<Self>) -> std::sync::Arc<dyn Any + Send + Sync>;

    /// 清空 L1 缓存
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    async fn clear_l1(&self) -> Result<()> {
        Err(crate::error::CacheError::NotSupported(
            "clear_l1".to_string(),
        ))
    }

    /// 清空 L2 缓存
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    async fn clear_l2(&self) -> Result<()> {
        Err(crate::error::CacheError::NotSupported(
            "clear_l2".to_string(),
        ))
    }

    /// 清空 WAL 日志
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    async fn clear_wal(&self) -> Result<()> {
        Err(crate::error::CacheError::NotSupported(
            "clear_wal".to_string(),
        ))
    }

    /// 优雅关闭客户端
    ///
    /// 关闭所有后台任务，释放资源
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
