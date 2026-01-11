//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了批量写入器的公共功能
//! 通过 `batch-write` feature 控制启用/禁用

#[cfg(feature = "batch-write")]
use crate::backend::l2::L2Backend;
#[cfg(feature = "batch-write")]
use crate::error::Result;
#[cfg(feature = "batch-write")]
use crate::recovery::wal::WalManager;
#[cfg(feature = "batch-write")]
use std::sync::Arc;
#[cfg(feature = "batch-write")]
use std::time::Duration;
#[cfg(feature = "batch-write")]
use tokio::sync::Notify;

/// 批量写入操作类型
#[cfg(feature = "batch-write")]
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
#[cfg(feature = "batch-write")]
#[derive(Debug, Clone)]
pub struct BatchWriterConfig {
    /// 最大批量大小
    pub max_batch_size: usize,
    /// 刷新间隔（毫秒）
    pub flush_interval_ms: u64,
    /// 最大缓冲区大小（防止内存泄漏）
    pub max_buffer_size: usize,
}

#[cfg(feature = "batch-write")]
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
#[cfg(feature = "batch-write")]
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
#[cfg(feature = "batch-write")]
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
#[cfg(feature = "batch-write")]
pub fn estimate_operation_size(operation: &BatchOperation) -> usize {
    match operation {
        BatchOperation::Set { key, value, .. } => key.len() + value.len(),
        BatchOperation::Delete { key } => key.len(),
    }
}

/// 计算重试延迟的工具函数 (指数退避)
#[cfg(feature = "batch-write")]
pub fn calculate_retry_delay(attempt: usize, base_delay_ms: u64) -> Duration {
    let delay = base_delay_ms * (2_u64.pow(attempt as u32));
    Duration::from_millis(delay)
}

// ============================================================================
// 当 batch-write 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "batch-write"))]
/// 批量写入操作类型（空实现）
#[derive(Debug, Clone, Default)]
pub enum BatchOperation {
    #[default]
    Set,
    Delete,
}

/// 批量写入器的基本配置（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct BatchWriterConfig;

/// 批量写入器的共享接口（空实现）
#[cfg(not(feature = "batch-write"))]
pub trait BatchWriterCommon {}

/// 公共的批量刷新函数（空实现）
#[cfg(not(feature = "batch-write"))]
pub async fn common_flush_batch<F, G>(
    _buffer_len: usize,
    _max_batch_size: usize,
    _flush_operation: F,
    _metrics_update: G,
) where
    F: Fn(usize) -> Result<()>,
    G: Fn(usize),
{
}

/// 估算操作大小的工具函数（空实现）
#[cfg(not(feature = "batch-write"))]
pub fn estimate_operation_size(_operation: &BatchOperation) -> usize {
    0
}

/// 计算重试延迟的工具函数（空实现）
#[cfg(not(feature = "batch-write"))]
pub fn calculate_retry_delay(_attempt: usize, _base_delay_ms: u64) -> Duration {
    Duration::ZERO
}
