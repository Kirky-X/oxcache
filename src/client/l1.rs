//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L1-only缓存客户端的实现。

#[cfg(feature = "l1-moka")]
use super::CacheOps;
#[cfg(feature = "l1-moka")]
use crate::backend::l1::L1Backend;
#[cfg(feature = "l1-moka")]
use crate::error::Result;
#[cfg(feature = "l1-moka")]
use async_trait::async_trait;
#[cfg(feature = "l1-moka")]
use std::sync::Arc;
#[cfg(feature = "l1-moka")]
use tracing::instrument;

#[cfg(feature = "l1-moka")]
use crate::serialization::SerializerEnum;

#[cfg(feature = "l1-moka")]
use crate::metrics::GLOBAL_METRICS;

/// L1-only 缓存客户端实现
///
/// 仅使用内存缓存，提供极高性能但无数据持久化
#[cfg(feature = "l1-moka")]
pub struct L1Client {
    /// 服务名称
    service_name: String,
    /// L1缓存后端
    l1: Arc<L1Backend>,
    /// 序列化器
    serializer: SerializerEnum,
}

#[cfg(feature = "l1-moka")]
impl L1Client {
    /// 创建新的L1-only缓存客户端
    pub fn new(service_name: String, l1: Arc<L1Backend>, serializer: SerializerEnum) -> Self {
        Self {
            service_name,
            l1,
            serializer,
        }
    }
}

#[cfg(feature = "l1-moka")]
#[async_trait]
impl CacheOps for L1Client {
    /// 获取序列化器
    fn serializer(&self) -> &SerializerEnum {
        &self.serializer
    }

    /// 将 trait object 转换为 Any
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// 将 `Arc<Trait>` 转换为 `Arc<dyn Any>`
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }

    /// 获取缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        GLOBAL_METRICS.record_request(&self.service_name, "L1", "get", "attempt");
        if let Some((bytes, _)) = self.l1.get_with_metadata(key).await? {
            GLOBAL_METRICS.record_request(&self.service_name, "L1", "get", "hit");
            return Ok(Some(bytes));
        }
        GLOBAL_METRICS.record_request(&self.service_name, "L1", "get", "miss");
        Ok(None)
    }

    /// 设置缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let start = std::time::Instant::now();
        self.l1.set_bytes(key, value, ttl).await?;
        let duration = start.elapsed().as_secs_f64();
        GLOBAL_METRICS.record_duration(&self.service_name, "L1", "set", duration);
        Ok(())
    }

    /// 设置 L1 缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_l1_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        self.set_bytes(key, value, ttl).await
    }

    /// 获取 L1 缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l1_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_bytes(key).await
    }

    /// 获取 L2 缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l2_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// 删除缓存项
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn delete(&self, key: &str) -> Result<()> {
        self.l1.delete(key).await
    }

    /// 清空 L1 缓存
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn clear_l1(&self) -> Result<()> {
        self.l1.clear()?;
        GLOBAL_METRICS.record_request(&self.service_name, "L1", "clear", "success");
        Ok(())
    }
}

#[cfg(feature = "l1-moka")]
#[cfg(feature = "ttl-control")]
#[async_trait::async_trait]
impl crate::client::ttl_control::TtlControl for L1Client {
    /// 获取 L1 缓存剩余 TTL
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l1_ttl(&self, key: &str) -> Result<Option<u64>> {
        self.l1.ttl(key).await
    }

    /// 获取 L2 缓存剩余 TTL（L1-only 客户端不支持）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l2_ttl(&self, _key: &str) -> Result<Option<u64>> {
        Ok(None)
    }

    /// 获取缓存剩余 TTL（优先 L1）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_ttl(&self, key: &str) -> Result<Option<u64>> {
        self.get_l1_ttl(key).await
    }

    /// 刷新 L1 缓存 TTL
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn refresh_l1_ttl(&self, key: &str, ttl: u64) -> Result<bool> {
        self.l1.refresh_ttl(key, ttl).await
    }

    /// 刷新 L2 缓存 TTL（L1-only 客户端不支持）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn refresh_l2_ttl(&self, _key: &str, _ttl: u64) -> Result<bool> {
        Ok(false)
    }

    /// 刷新缓存 TTL（同时刷新 L1 和 L2）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn refresh_ttl(&self, key: &str, ttl: u64) -> Result<bool> {
        self.refresh_l1_ttl(key, ttl).await
    }

    /// Touch 操作（刷新访问时间但不返回值）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn touch(&self, key: &str) -> Result<bool> {
        // 对于 L1 缓存，touch 操作等同于刷新 TTL
        // 但是由于 L1 缓存不保留原始 TTL，这里返回 false
        // 如果需要真正的 touch，需要在 L1Backend 中存储原始 TTL
        Ok(false)
    }
}
