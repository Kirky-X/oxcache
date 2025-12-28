//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 优化的L2批量写入器 - 完成test.md和uat.md中L2批量写入优化

use super::common::*;
use crate::backend::l2::L2Backend;
use crate::error::{CacheError, Result};
use crate::recovery::wal::WalManager;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Notify, RwLock};
use tokio::time::interval;

/// 优先级队列类型别名 - 简化复杂类型定义
type PriorityQueue = Arc<RwLock<Vec<(String, u8, Instant)>>>;

/// 缓冲区条目 - 优化版本
#[derive(Debug, Clone)]
struct OptimizedBufferEntry {
    operation: BatchOperation,
    retry_count: Arc<AtomicUsize>,
}

impl OptimizedBufferEntry {
    fn new(operation: BatchOperation, _priority: u8) -> Self {
        Self {
            operation,
            retry_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn increment_retry(&self) -> usize {
        self.retry_count.fetch_add(1, Ordering::Relaxed)
    }

    fn get_retry_count(&self) -> usize {
        self.retry_count.load(Ordering::Relaxed)
    }
}

/// 优化的批量写入器配置
#[derive(Debug, Clone)]
pub struct OptimizedBatchWriterConfig {
    /// 基本配置
    pub base: BatchWriterConfig,
    /// 最大重试次数
    pub max_retry_count: usize,
    /// 重试延迟（毫秒）
    pub retry_delay_ms: u64,
    /// 缓冲区最大容量
    pub max_buffer_size: usize,
    /// 高水位标记（触发背压）
    pub high_water_mark: usize,
    /// 低水位标记（解除背压）
    pub low_water_mark: usize,
    /// 是否启用WAL
    pub enable_wal: bool,
    /// 是否启用压缩
    pub enable_compression: bool,
    /// 压缩阈值（字节）
    pub compression_threshold: usize,
}

impl Default for OptimizedBatchWriterConfig {
    fn default() -> Self {
        Self {
            base: BatchWriterConfig::default(),
            max_retry_count: 3,
            retry_delay_ms: 1000,
            max_buffer_size: 10000,
            high_water_mark: 8000,
            low_water_mark: 2000,
            enable_wal: true,
            enable_compression: true,
            compression_threshold: 1024, // 1KB
        }
    }
}

/// 批量写入统计
#[derive(Debug, Default)]
pub struct BatchWriterStats {
    pub total_operations: AtomicU64,
    pub successful_operations: AtomicU64,
    pub failed_operations: AtomicU64,
    pub retried_operations: AtomicU64,
    pub dropped_operations: AtomicU64,
    pub batch_count: AtomicU64,
    pub average_batch_size: AtomicU64,
    pub total_bytes_written: AtomicU64,
    pub compression_ratio: AtomicU64, // 百分比 * 100
}

/// 优化的批量写入器
pub struct OptimizedBatchWriter {
    /// 主缓冲区（按优先级排序）
    buffer: Arc<DashMap<String, OptimizedBufferEntry>>,
    /// 优先级队列（用于快速获取高优先级条目）
    priority_queue: PriorityQueue,
    /// L2缓存后端
    l2: Arc<L2Backend>,
    /// WAL管理器
    wal: Arc<WalManager>,
    /// 刷新触发器
    flush_trigger: Arc<Notify>,
    /// 背压触发器
    backpressure_trigger: Arc<Notify>,
    /// 配置
    config: OptimizedBatchWriterConfig,
    /// 服务名称
    service_name: String,
    /// 统计信息
    stats: Arc<BatchWriterStats>,
    /// 关闭信号
    shutdown: Arc<Notify>,
    /// 背压状态
    backpressure_active: Arc<RwLock<bool>>,
}

impl OptimizedBatchWriter {
    /// 创建新的优化批量写入器
    pub fn new(
        service_name: String,
        l2: Arc<L2Backend>,
        config: OptimizedBatchWriterConfig,
        wal: Arc<WalManager>,
    ) -> Self {
        Self {
            buffer: Arc::new(DashMap::new()),
            priority_queue: Arc::new(RwLock::new(Vec::new())),
            l2,
            wal,
            flush_trigger: Arc::new(Notify::new()),
            backpressure_trigger: Arc::new(Notify::new()),
            config,
            service_name,
            stats: Arc::new(BatchWriterStats::default()),
            shutdown: Arc::new(Notify::new()),
            backpressure_active: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动优化的批量写入器
    pub async fn start(&self) {
        // 启动主刷新任务
        self.start_flush_task().await;

        // 启动重试任务
        self.start_retry_task().await;

        // 启动背压监控任务
        self.start_backpressure_task().await;

        // 启动统计报告任务
        self.start_stats_task().await;
    }

    /// 停止批量写入器
    pub async fn stop(&self) {
        self.shutdown.notify_one();

        // 等待缓冲区清空
        let mut attempts = 0;
        while !self.buffer.is_empty() && attempts < 10 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            attempts += 1;
        }

        if !self.buffer.is_empty() {
            tracing::warn!("缓冲区未完全清空，剩余 {} 个条目", self.buffer.len());
        }
    }

    /// 将操作加入缓冲区（带背压控制）
    pub async fn enqueue_operation(&self, operation: BatchOperation, priority: u8) -> Result<()> {
        // 检查背压状态
        if self.is_backpressure_active().await {
            return Err(CacheError::L2Error("缓冲区已满，请稍后重试".to_string()));
        }

        // 检查缓冲区大小
        if self.buffer.len() >= self.config.max_buffer_size {
            return Err(CacheError::L2Error("缓冲区已达到最大容量".to_string()));
        }

        let key = match &operation {
            BatchOperation::Set { key, .. } => key.clone(),
            BatchOperation::Delete { key } => key.clone(),
        };

        let entry = OptimizedBufferEntry::new(operation.clone(), priority);

        // 写入WAL（如果启用）
        if self.config.enable_wal {
            let entry = crate::recovery::wal::WalEntry {
                timestamp: std::time::SystemTime::now(),
                operation: match &operation {
                    BatchOperation::Set { .. } => crate::recovery::wal::Operation::Set,
                    BatchOperation::Delete { .. } => crate::recovery::wal::Operation::Delete,
                },
                key: key.clone(),
                value: Some(self.serialize_operation(&operation)),
                ttl: match &operation {
                    BatchOperation::Set { ttl, .. } => ttl.map(|t| t as i64),
                    BatchOperation::Delete { .. } => None,
                },
            };
            self.wal.append(entry).await?;
        }

        // 添加到缓冲区
        self.buffer.insert(key.clone(), entry);

        // 添加到优先级队列
        {
            let mut queue = self.priority_queue.write().await;
            queue.push((key, priority, Instant::now()));
            queue.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2))); // 按优先级降序，时间升序
        }

        // 更新统计
        self.stats.total_operations.fetch_add(1, Ordering::Relaxed);

        // 触发刷新（如果达到批处理大小）
        if self.buffer.len() >= self.config.base.max_batch_size {
            self.flush_trigger.notify_one();
        }

        Ok(())
    }

    /// 批量设置（便捷方法）
    pub async fn batch_set(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Option<u64>,
        priority: u8,
    ) -> Result<()> {
        self.enqueue_operation(BatchOperation::Set { key, value, ttl }, priority)
            .await
    }

    /// 批量删除（便捷方法）
    pub async fn batch_delete(&self, key: String, priority: u8) -> Result<()> {
        self.enqueue_operation(BatchOperation::Delete { key }, priority)
            .await
    }

    /// 启动刷新任务
    async fn start_flush_task(&self) {
        let buffer = self.buffer.clone();
        let priority_queue = self.priority_queue.clone();
        let l2 = self.l2.clone();
        let flush_trigger = self.flush_trigger.clone();
        let shutdown = self.shutdown.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        let service_name = self.service_name.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.base.flush_interval_ms));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::flush_batch(&buffer, &priority_queue, &l2, &config, &stats, &service_name).await;
                    }
                    _ = flush_trigger.notified() => {
                        Self::flush_batch(&buffer, &priority_queue, &l2, &config, &stats, &service_name).await;
                    }
                    _ = shutdown.notified() => {
                        tracing::info!("刷新任务收到关闭信号");
                        break;
                    }
                }
            }

            // 关闭前清空缓冲区
            Self::flush_batch(
                &buffer,
                &priority_queue,
                &l2,
                &config,
                &stats,
                &service_name,
            )
            .await;
        });
    }

    /// 启动重试任务
    async fn start_retry_task(&self) {
        // 重试逻辑在flush_batch中处理
    }

    /// 启动背压监控任务
    async fn start_backpressure_task(&self) {
        let buffer = self.buffer.clone();
        let backpressure_trigger = self.backpressure_trigger.clone();
        let backpressure_active = self.backpressure_active.clone();
        let config = self.config.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let buffer_size = buffer.len();
                        let mut is_active = backpressure_active.write().await;

                        if buffer_size >= config.high_water_mark && !*is_active {
                            *is_active = true;
                            tracing::warn!("背压激活：缓冲区大小 {} >= 高水位标记 {}", buffer_size, config.high_water_mark);
                        } else if buffer_size <= config.low_water_mark && *is_active {
                            *is_active = false;
                            tracing::info!("背压解除：缓冲区大小 {} <= 低水位标记 {}", buffer_size, config.low_water_mark);
                            backpressure_trigger.notify_one();
                        }
                    }
                    _ = shutdown.notified() => {
                        break;
                    }
                }
            }
        });
    }

    /// 启动统计报告任务
    async fn start_stats_task(&self) {
        let stats = self.stats.clone();
        let shutdown = self.shutdown.clone();
        let service_name = self.service_name.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // 每分钟报告一次

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::report_stats(&service_name, &stats).await;
                    }
                    _ = shutdown.notified() => {
                        break;
                    }
                }
            }
        });
    }

    /// 批量刷新
    async fn flush_batch(
        buffer: &DashMap<String, OptimizedBufferEntry>,
        priority_queue: &PriorityQueue,
        l2: &L2Backend,
        config: &OptimizedBatchWriterConfig,
        stats: &BatchWriterStats,
        _service_name: &str,
    ) {
        if buffer.is_empty() {
            return;
        }

        // 获取优先级最高的条目
        let mut batch = Vec::new();
        let mut batch_size = 0;
        let mut keys_to_remove = Vec::new();

        {
            let mut queue = priority_queue.write().await;

            while !queue.is_empty() && batch.len() < config.base.max_batch_size {
                if let Some((key, priority, _)) = queue.pop() {
                    if let Some(entry) = buffer.get(&key) {
                        // 检查重试次数
                        if entry.get_retry_count() >= config.max_retry_count {
                            stats.dropped_operations.fetch_add(1, Ordering::Relaxed);
                            keys_to_remove.push(key.clone());
                            continue;
                        }

                        let operation_size = Self::estimate_operation_size(&entry.operation);
                        if batch_size + operation_size > 1024 * 1024 {
                            // 1MB限制
                            queue.push((key, priority, Instant::now()));
                            break;
                        }

                        batch.push((key.clone(), entry.operation.clone()));
                        batch_size += operation_size;
                        keys_to_remove.push(key);
                    }
                }
            }
        }

        if batch.is_empty() {
            return;
        }

        // 执行批量操作
        let start_time = Instant::now();
        let mut success_count = 0;
        let mut total_bytes = 0;

        // 分离SET和DELETE操作
        let mut set_operations = Vec::new();
        let mut delete_operations = Vec::new();

        for (key, operation) in &batch {
            match &operation {
                BatchOperation::Set { value, ttl, .. } => {
                    set_operations.push((key.clone(), value.clone(), *ttl));
                    total_bytes += value.len();
                }
                BatchOperation::Delete { .. } => {
                    delete_operations.push(key.clone());
                }
            }
        }

        // 执行SET操作
        if !set_operations.is_empty() {
            match l2.pipeline_set_batch(set_operations.clone()).await {
                Ok(_) => {
                    success_count += set_operations.len();
                    stats
                        .successful_operations
                        .fetch_add(set_operations.len() as u64, Ordering::Relaxed);
                }
                Err(e) => {
                    tracing::error!("批量SET操作失败: {}", e);

                    // 重试逻辑：将失败的条目重新加入缓冲区
                    for (key, operation) in &batch {
                        if let BatchOperation::Set { .. } = operation {
                            if let Some(entry) = buffer.get(key) {
                                let retry_count = entry.increment_retry();
                                if retry_count < config.max_retry_count {
                                    // 延迟重试
                                    tokio::spawn({
                                        let key = key.clone();
                                        let _buffer = buffer.clone();
                                        let priority_queue = priority_queue.clone();
                                        let retry_delay_ms = config.retry_delay_ms;
                                        async move {
                                            tokio::time::sleep(Duration::from_millis(
                                                retry_delay_ms,
                                            ))
                                            .await;
                                            priority_queue.write().await.push((
                                                key.clone(),
                                                255,
                                                Instant::now(),
                                            ));
                                        }
                                    });
                                } else {
                                    stats.dropped_operations.fetch_add(1, Ordering::Relaxed);
                                    keys_to_remove.push(key.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // 执行DELETE操作
        if !delete_operations.is_empty() {
            for key in &delete_operations {
                match l2.delete(key).await {
                    Ok(_) => {
                        success_count += 1;
                        stats.successful_operations.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        tracing::error!("DELETE操作失败 {}: {}", key, e);
                        stats.failed_operations.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        // 清理已处理的条目
        for key in keys_to_remove {
            buffer.remove(&key);
        }

        // 更新统计
        stats.batch_count.fetch_add(1, Ordering::Relaxed);
        stats
            .total_bytes_written
            .fetch_add(total_bytes as u64, Ordering::Relaxed);

        let avg_batch_size = (stats.total_operations.load(Ordering::Relaxed) as f64
            / stats.batch_count.load(Ordering::Relaxed) as f64
            * 100.0) as u64;
        stats
            .average_batch_size
            .store(avg_batch_size, Ordering::Relaxed);

        let duration = start_time.elapsed();
        tracing::info!(
            "批量刷新完成：{} 操作成功，{} 操作失败，耗时 {:?}",
            success_count,
            batch.len() - success_count,
            duration
        );
    }

    /// 估算操作大小
    fn estimate_operation_size(operation: &BatchOperation) -> usize {
        match operation {
            BatchOperation::Set { key, value, .. } => key.len() + value.len(),
            BatchOperation::Delete { key } => key.len(),
        }
    }

    /// 序列化操作（用于WAL）
    fn serialize_operation(&self, operation: &BatchOperation) -> Vec<u8> {
        // 简单的序列化实现
        match operation {
            BatchOperation::Set { key, value, ttl } => {
                let mut result = Vec::new();
                result.push(0u8); // SET操作标记
                result.extend_from_slice(key.as_bytes());
                result.push(0u8); // 分隔符
                result.extend_from_slice(value);
                if let Some(ttl) = ttl {
                    result.push(1u8); // 有TTL标记
                    result.extend_from_slice(&ttl.to_le_bytes());
                } else {
                    result.push(0u8); // 无TTL标记
                }
                result
            }
            BatchOperation::Delete { key } => {
                let mut result = Vec::new();
                result.push(1u8); // DELETE操作标记
                result.extend_from_slice(key.as_bytes());
                result
            }
        }
    }

    /// 检查背压状态
    async fn is_backpressure_active(&self) -> bool {
        *self.backpressure_active.read().await
    }

    /// 报告统计信息
    async fn report_stats(service_name: &str, stats: &BatchWriterStats) {
        let total = stats.total_operations.load(Ordering::Relaxed);
        let success = stats.successful_operations.load(Ordering::Relaxed);
        let failed = stats.failed_operations.load(Ordering::Relaxed);
        let dropped = stats.dropped_operations.load(Ordering::Relaxed);
        let batches = stats.batch_count.load(Ordering::Relaxed);
        let avg_batch_size = stats.average_batch_size.load(Ordering::Relaxed);
        let total_bytes = stats.total_bytes_written.load(Ordering::Relaxed);

        let success_rate = if total > 0 {
            (success as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let avg_bytes_per_op = if success > 0 {
            total_bytes as f64 / success as f64
        } else {
            0.0
        };

        tracing::info!(
            "批量写入器统计 - 总操作: {}, 成功: {}, 失败: {}, 丢弃: {}, 成功率: {:.2}%, 批次数: {}, 平均批大小: {}, 总字节: {}, 平均每操作字节: {:.2}",
            total, success, failed, dropped, success_rate, batches, avg_batch_size, total_bytes, avg_bytes_per_op
        );

        // 更新全局指标
        crate::metrics::GLOBAL_METRICS.set_batch_success_rate(service_name, success_rate);
        crate::metrics::GLOBAL_METRICS.set_batch_throughput(service_name, success as f64 / 60.0);
        // ops/sec
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> BatchWriterStats {
        BatchWriterStats {
            total_operations: AtomicU64::new(self.stats.total_operations.load(Ordering::Relaxed)),
            successful_operations: AtomicU64::new(
                self.stats.successful_operations.load(Ordering::Relaxed),
            ),
            failed_operations: AtomicU64::new(self.stats.failed_operations.load(Ordering::Relaxed)),
            retried_operations: AtomicU64::new(
                self.stats.retried_operations.load(Ordering::Relaxed),
            ),
            dropped_operations: AtomicU64::new(
                self.stats.dropped_operations.load(Ordering::Relaxed),
            ),
            batch_count: AtomicU64::new(self.stats.batch_count.load(Ordering::Relaxed)),
            average_batch_size: AtomicU64::new(
                self.stats.average_batch_size.load(Ordering::Relaxed),
            ),
            total_bytes_written: AtomicU64::new(
                self.stats.total_bytes_written.load(Ordering::Relaxed),
            ),
            compression_ratio: AtomicU64::new(self.stats.compression_ratio.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_buffer_entry() {
        let operation = BatchOperation::Set {
            key: "test_key".to_string(),
            value: b"test_value".to_vec(),
            ttl: Some(60),
        };

        let entry = OptimizedBufferEntry::new(operation.clone(), 128);
        assert_eq!(entry.get_retry_count(), 0);
        assert_eq!(entry.increment_retry(), 0);
        assert_eq!(entry.get_retry_count(), 1);
    }

    #[test]
    fn test_batch_operation_size() {
        let set_op = BatchOperation::Set {
            key: "key".to_string(),
            value: b"value".to_vec(),
            ttl: None,
        };
        assert_eq!(OptimizedBatchWriter::estimate_operation_size(&set_op), 8); // "key" (3) + "value" (5)

        let delete_op = BatchOperation::Delete {
            key: "key".to_string(),
        };
        assert_eq!(OptimizedBatchWriter::estimate_operation_size(&delete_op), 3);
        // "key"
    }

    #[test]
    fn test_config_default() {
        let config = OptimizedBatchWriterConfig::default();
        assert_eq!(config.base.max_batch_size, 1000);
        assert_eq!(config.base.flush_interval_ms, 100);
        assert_eq!(config.max_retry_count, 3);
        assert_eq!(config.max_buffer_size, 10000);
        assert!(config.enable_wal);
        assert!(config.enable_compression);
    }
}
