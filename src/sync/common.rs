//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了批量写入器的公共功能

use crate::backend::l2::L2Backend;
use crate::error::Result;
use crate::recovery::wal::WalManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// 批量写入操作类型
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Set {
        key: String,
        value: Vec<u8>,
        ttl: Option<u64>,
    },
    Delete {
        key: String,
    },
}

/// 批量写入器的基本配置
#[derive(Debug, Clone)]
pub struct BatchWriterConfig {
    /// 最大批量大小
    pub max_batch_size: usize,
    /// 刷新间隔（毫秒）
    pub flush_interval_ms: u64,
    /// 最大缓冲区大小（防止内存泄漏）
    pub max_buffer_size: usize,
}

impl Default for BatchWriterConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            flush_interval_ms: 100,
            max_buffer_size: 10000, // 默认最大缓冲区大小为 10000
        }
    }
}

/// 批量写入器的共享接口
pub trait BatchWriterCommon {
    /// 获取服务名称
    fn get_service_name(&self) -> &str;

    /// 获取L2缓存后端
    fn get_l2_backend(&self) -> &Arc<L2Backend>;

    /// 获取WAL管理器
    fn get_wal_manager(&self) -> &Arc<WalManager>;

    /// 获取刷新触发器
    fn get_flush_trigger(&self) -> &Arc<Notify>;

    /// 获取配置
    fn get_config(&self) -> &BatchWriterConfig;

    /// 将操作加入缓冲区
    fn enqueue_operation(
        &self,
        operation: BatchOperation,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 启动批量写入器
    fn start(&self) -> impl std::future::Future<Output = ()> + Send;

    /// 停止批量写入器
    fn stop(&self) -> impl std::future::Future<Output = ()> + Send;
}

/// 公共的批量刷新函数
pub async fn common_flush_batch<F, G>(
    buffer_len: usize,
    max_batch_size: usize,
    flush_operation: F,
    metrics_update: G,
) where
    F: Fn(usize) -> Result<()>,
    G: Fn(usize),
{
    if buffer_len == 0 {
        return;
    }

    let batch_size = buffer_len.min(max_batch_size);

    match flush_operation(batch_size) {
        Ok(_) => {
            metrics_update(buffer_len - batch_size);
        }
        Err(e) => {
            tracing::error!("批量写入失败: {}", e);
            metrics_update(buffer_len);
        }
    }
}

/// 估算操作大小的工具函数
pub fn estimate_operation_size(operation: &BatchOperation) -> usize {
    match operation {
        BatchOperation::Set { key, value, .. } => key.len() + value.len(),
        BatchOperation::Delete { key } => key.len(),
    }
}

/// 计算重试延迟的工具函数 (指数退避)
pub fn calculate_retry_delay(attempt: usize, base_delay_ms: u64) -> Duration {
    let delay = base_delay_ms * (2_u64.pow(attempt as u32));
    Duration::from_millis(delay)
}
