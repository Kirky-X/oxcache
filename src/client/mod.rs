//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存客户端的接口和实现。

pub mod db_loader;
pub mod l1;
pub mod l2;
pub mod two_level;

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
    /// 注意：此实现默认行为是 set_bytes，因为 CacheOps 没有区分 L1/L2。
    /// 如果需要真正的 L1-only，需要底层支持或使用 L1OnlyClient。
    /// 这里我们为了兼容示例代码，提供默认实现，但实际可能需要扩展 CacheOps。
    /// 然而，CacheOps 是基础 trait，L1/L2 控制应该由具体 Client 类型或配置决定。
    /// 如果用户想要手动控制，应该使用 set_bytes 并自己处理逻辑，或者我们在 CacheOps 中增加方法。
    /// 鉴于 PRD F3 "独立缓存层支持"，可以通过 Client 类型实现。
    /// 但示例代码中使用了 set_l1_only。
    /// 我们这里先添加 set_l1_only 到 CacheExt，但行为可能只是 set。
    /// 为了真正支持，我们需要在 CacheOps 中添加 set_l1_bytes 等，或者扩展 CacheExt 在特定实现中覆盖。
    /// 但 CacheExt 是 trait default impl，无法访问私有字段。
    ///
    /// 既然 F3 已经实现（通过 L1OnlyClient），我们可以让用户获取特定类型的 client。
    /// 但示例代码是在同一个 client 上调用 set_l1_only。这意味着 TwoLevelClient 应该支持这个。
    ///
    /// 让我们修改 CacheOps 增加 set_l1_bytes / set_l2_bytes 接口，默认实现可以是 no-op 或 fallback。
    /// 或者，我们只在 CacheExt 中提供，但需要 CacheOps 支持。
    ///
    /// 考虑到接口稳定性，我们在 CacheOps 中添加 set_l1_bytes 和 set_l2_bytes，并在 TwoLevelClient 中实现。
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
    /// # 参数
    ///
    /// * `key` - 锁的键
    /// * `value` - 锁的值（通常是唯一标识符，用于释放锁）
    /// * `ttl` - 锁的过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 成功获取锁返回 true，否则返回 false
    async fn lock(&self, _key: &str, _value: &str, _ttl: u64) -> Result<bool> {
        Ok(false)
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
