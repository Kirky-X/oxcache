//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了批量写入器，用于优化L2缓存的写入性能。

use super::common::*;
use crate::backend::l2::L2Backend;
use crate::error::Result;

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, Semaphore};

/// 缓冲区条目
///
/// 表示批处理缓冲区中的一个条目
struct BufferEntry {
    /// 批量操作
    operation: BatchOperation,
}

/// 批量写入器
///
/// 负责将缓存操作批量写入L2缓存，以提高性能
pub struct BatchWriter {
    /// 缓冲区
    buffer: Arc<DashMap<String, BufferEntry>>,
    /// L2缓存后端
    l2: Arc<L2Backend>,
    /// 刷新触发器
    flush_trigger: Arc<Notify>,
    /// 配置
    config: BatchWriterConfig,

    /// 服务名称
    service_name: String,

    /// 背压信号量（防止 buffer 无限增长）
    backpressure: Arc<Semaphore>,

    /// 取消令牌（用于优雅关闭）
    shutdown_token: Arc<tokio_util::sync::CancellationToken>,
}

impl BatchWriter {
    /// 创建新的批量写入器
    ///
    /// # 参数
    ///
    /// * `service_name` - 服务名称
    /// * `l2` - L2缓存后端
    /// * `config` - 批量写入器配置
    /// * `wal` - WAL管理器
    ///
    /// # 返回值
    ///
    /// 返回新的批量写入器实例
    pub fn new(service_name: String, l2: Arc<L2Backend>, config: BatchWriterConfig) -> Self {
        // 背压信号量的许可数是最大缓冲区大小的 2 倍
        // 这样可以防止 buffer 无限增长
        let backpressure_permits = config.max_buffer_size * 2;

        Self {
            buffer: Arc::new(DashMap::new()),
            l2,
            flush_trigger: Arc::new(Notify::new()),
            config,
            service_name,
            backpressure: Arc::new(Semaphore::new(backpressure_permits)),
            shutdown_token: Arc::new(tokio_util::sync::CancellationToken::new()),
        }
    }

    /// 创建带有默认配置的批量写入器
    pub fn new_with_default_config(service_name: String, l2: Arc<L2Backend>) -> Self {
        Self::new(service_name, l2, BatchWriterConfig::default())
    }

    /// 停止批量写入器
    ///
    /// 取消后台任务，等待所有缓冲区数据刷新完成
    pub async fn shutdown(&self) {
        self.shutdown_token.cancel();
        self.flush_trigger.notify_one(); // 触发最后一次刷新

        // 等待缓冲区清空
        while !self.buffer.is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        tracing::info!("批量写入器已停止: {}", self.service_name);
    }

    /// 启动批量写入器
    ///
    /// 启动后台任务，定期或按需刷新缓冲区
    pub async fn start(&self) {
        let buffer = self.buffer.clone();
        let l2 = self.l2.clone();
        let trigger = self.flush_trigger.clone();
        let config = self.config.clone();
        let service_name = self.service_name.clone();
        let shutdown_token = self.shutdown_token.clone();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_millis(config.flush_interval_ms));

            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        // 收到取消信号，执行最后一次刷新后退出
                        tracing::info!("批量写入器收到关闭信号，执行最后一次刷新");
                        Self::flush(&buffer, &l2, &config, &service_name).await;
                        break;
                    }
                    _ = interval.tick() => {
                        Self::flush(&buffer, &l2, &config, &service_name).await;
                    }
                    _ = trigger.notified() => {
                        Self::flush(&buffer, &l2, &config, &service_name).await;
                    }
                }
            }

            tracing::info!("批量写入器后台任务已退出: {}", service_name);
        });
    }

    /// 将条目加入缓冲区
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值
    /// * `ttl` - 过期时间
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn enqueue(&self, key: String, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let operation = BatchOperation::Set {
            key: key.clone(),
            value,
            ttl,
        };
        self.enqueue_operation(operation).await
    }

    /// 将删除操作加入缓冲区
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn enqueue_delete(&self, key: String) -> Result<()> {
        let operation = BatchOperation::Delete { key: key.clone() };
        self.enqueue_operation(operation).await
    }

    /// 将操作加入缓冲区
    ///
    /// # 参数
    ///
    /// * `operation` - 批量操作
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn enqueue_operation(&self, operation: BatchOperation) -> Result<()> {
        // 检查是否已关闭
        if self.shutdown_token.is_cancelled() {
            return Err(crate::error::CacheError::L2Error(
                "批量写入器已关闭".to_string(),
            ));
        }

        // 背压机制：等待许可，防止 buffer 无限增长
        let permit = tokio::time::timeout(Duration::from_secs(5), self.backpressure.acquire())
            .await
            .map_err(|_| {
                crate::error::CacheError::L2Error("批量写入器背压超时：缓冲区已满".to_string())
            })?
            .map_err(|_| {
                crate::error::CacheError::L2Error("批量写入器背压信号量已关闭".to_string())
            })?;

        let key = match &operation {
            BatchOperation::Set { key, .. } => key.clone(),
            BatchOperation::Delete { key } => key.clone(),
        };

        // 检查 buffer 大小限制
        if self.buffer.len() >= self.config.max_buffer_size {
            tracing::warn!(
                "批量写入器缓冲区已达到最大限制 ({}), 立即触发刷新",
                self.config.max_buffer_size
            );
            self.flush_trigger.notify_one();
        }

        self.buffer.insert(key, BufferEntry { operation });

        // 更新指标
        crate::metrics::GLOBAL_METRICS.set_batch_buffer_size(&self.service_name, self.buffer.len());

        if self.buffer.len() >= self.config.max_batch_size {
            self.flush_trigger.notify_one();
        }

        // 释放许可
        drop(permit);

        Ok(())
    }

    /// 刷新缓冲区
    ///
    /// 将缓冲区中的所有条目批量写入L2缓存
    ///
    /// # 参数
    ///
    /// * `buffer` - 缓冲区
    /// * `l2` - L2缓存后端
    /// * `config` - 批量写入器配置
    /// * `service_name` - 服务名称
    async fn flush(
        buffer: &DashMap<String, BufferEntry>,
        l2: &L2Backend,
        config: &BatchWriterConfig,
        service_name: &str,
    ) {
        if buffer.is_empty() {
            return;
        }

        // 分离set和delete操作
        let mut set_items = Vec::new();
        let mut delete_keys = Vec::new();
        let mut keys_to_remove = Vec::new();

        for entry in buffer.iter() {
            let key = entry.key().clone();
            match &entry.value().operation {
                BatchOperation::Set { value, ttl, .. } => {
                    set_items.push((key.clone(), value.clone(), *ttl));
                    keys_to_remove.push(key);
                }
                BatchOperation::Delete { .. } => {
                    delete_keys.push(key.clone());
                    keys_to_remove.push(key);
                }
            }

            // 达到最大批量大小就停止
            if keys_to_remove.len() >= config.max_batch_size {
                break;
            }
        }

        // 执行批量操作
        let mut all_success = true;

        // 批量设置
        if !set_items.is_empty() {
            let set_len = set_items.len();
            match l2.pipeline_set_batch(set_items).await {
                Ok(_) => {
                    tracing::debug!("成功批量设置 {} 个条目", set_len);
                }
                Err(e) => {
                    tracing::error!("批量设置失败: {}", e);
                    all_success = false;
                }
            }
        }

        // 批量删除
        if !delete_keys.is_empty() {
            let del_len = delete_keys.len();
            match l2.pipeline_del_batch(delete_keys).await {
                Ok(_) => {
                    tracing::debug!("成功批量删除 {} 个条目", del_len);
                }
                Err(e) => {
                    tracing::error!("批量删除失败: {}", e);
                    all_success = false;
                }
            }
        }

        // 如果所有操作都成功，从缓冲区中删除条目
        if all_success {
            for key in keys_to_remove {
                buffer.remove(&key);
            }
        }

        // 更新指标
        crate::metrics::GLOBAL_METRICS.set_batch_buffer_size(service_name, buffer.len());
        crate::metrics::GLOBAL_METRICS
            .set_wal_size("batch_buffer", if all_success { 0 } else { buffer.len() });
    }
}
