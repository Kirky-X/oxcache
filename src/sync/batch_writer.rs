//! Copyright (c) 2025, Kirky.X
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
use tokio::sync::Notify;

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
        Self {
            buffer: Arc::new(DashMap::new()),
            l2,
            flush_trigger: Arc::new(Notify::new()),
            config,
            service_name,
        }
    }

    /// 创建带有默认配置的批量写入器
    pub fn new_with_default_config(service_name: String, l2: Arc<L2Backend>) -> Self {
        Self::new(service_name, l2, BatchWriterConfig::default())
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

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_millis(config.flush_interval_ms));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::flush(&buffer, &l2, &config, &service_name).await;
                    }
                    _ = trigger.notified() => {
                        Self::flush(&buffer, &l2, &config, &service_name).await;
                    }
                }
            }
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
        let key = match &operation {
            BatchOperation::Set { key, .. } => key.clone(),
            BatchOperation::Delete { key } => key.clone(),
        };

        self.buffer.insert(key, BufferEntry { operation });

        // 更新指标
        crate::metrics::GLOBAL_METRICS.set_batch_buffer_size(&self.service_name, self.buffer.len());

        if self.buffer.len() >= self.config.max_batch_size {
            self.flush_trigger.notify_one();
        }
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
