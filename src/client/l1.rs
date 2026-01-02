//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L1-only缓存客户端的实现。

use super::CacheOps;
use crate::backend::l1::L1Backend;
use crate::error::Result;
use crate::metrics::GLOBAL_METRICS;
use crate::serialization::SerializerEnum;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::instrument;

/// L1-only 缓存客户端实现
///
/// 仅使用内存缓存，提供极高性能但无数据持久化
pub struct L1Client {
    /// 服务名称
    service_name: String,
    /// L1缓存后端
    l1: Arc<L1Backend>,
    /// 序列化器
    serializer: SerializerEnum,
}

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
