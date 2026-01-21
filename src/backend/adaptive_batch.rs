//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 自适应批量写入模块
//!
//! 实现动态调整的批量写入策略，根据系统负载和性能指标自动优化批处理参数。

use crate::error::Result;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// 批量写入配置
#[derive(Debug, Clone)]
pub struct BatchWriteConfig {
    /// 初始批量大小
    pub initial_batch_size: usize,
    /// 最大批量大小
    pub max_batch_size: usize,
    /// 最小批量大小
    pub min_batch_size: usize,
    /// 最大等待时间（微秒）
    pub max_wait_us: u64,
    /// 最小等待时间（微秒）
    pub min_wait_us: u64,
    /// 延迟阈值（毫秒），超过此值减少批量
    pub latency_threshold_ms: u64,
    /// 错误率阈值（百分比），超过此值减少批量
    pub error_rate_threshold: f64,
    /// 批量增长因子
    pub growth_factor: f64,
    /// 批量缩减因子
    pub shrink_factor: f64,
    /// 调整间隔（毫秒）
    pub adjustment_interval_ms: u64,
}

impl Default for BatchWriteConfig {
    fn default() -> Self {
        Self {
            initial_batch_size: 100,
            max_batch_size: 1000,
            min_batch_size: 10,
            max_wait_us: 5000,      // 5ms
            min_wait_us: 100,       // 100us
            latency_threshold_ms: 10,
            error_rate_threshold: 5.0,
            growth_factor: 1.2,
            shrink_factor: 0.8,
            adjustment_interval_ms: 1000,
        }
    }
}

/// 批量写入项
#[derive(Debug)]
pub struct BatchItem<K, V> {
    /// 键
    pub key: K,
    /// 值
    pub value: V,
    /// TTL（秒）
    pub ttl: Option<u64>,
    /// 创建时间
    pub created_at: Instant,
    /// 重试次数
    pub retry_count: u32,
}

impl<K, V> BatchItem<K, V> {
    pub fn new(key: K, value: V, ttl: Option<u64>) -> Self {
        Self {
            key,
            value,
            ttl,
            created_at: Instant::now(),
            retry_count: 0,
        }
    }
}

/// 批量统计信息
#[derive(Debug, Clone, Default)]
pub struct BatchMetrics {
    /// 总提交批次
    pub total_batches: u64,
    /// 总写入项
    pub total_items: u64,
    /// 平均批量大小
    pub avg_batch_size: f64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// 批量写入延迟（毫秒）
    pub batch_write_latency_ms: f64,
    /// 错误数
    pub error_count: u64,
    /// 错误率
    pub error_rate: f64,
    /// 当前批量大小
    pub current_batch_size: usize,
    /// 当前等待时间（微秒）
    pub current_wait_us: u64,
    /// 队列深度
    pub queue_depth: usize,
}

/// 自适应批量写入器
///
/// 动态调整批量大小和等待时间以优化写入性能。
pub struct AdaptiveBatchWriter<K, V> {
    /// 配置
    config: Arc<BatchWriteConfig>,
    /// 待写入项队列
    queue: Arc<Mutex<VecDeque<BatchItem<K, V>>>>,
    /// 批量大小
    batch_size: Arc<AtomicUsize>,
    /// 等待时间（微秒）
    wait_us: Arc<AtomicU64>,
    /// 统计信息
    total_batches: Arc<AtomicU64>,
    total_items: Arc<AtomicU64>,
    total_latency_ms: Arc<AtomicU64>,
    batch_write_latency_ms: Arc<AtomicU64>,
    error_count: Arc<AtomicU64>,
    /// 历史延迟窗口
    latency_window: Arc<Mutex<VecDeque<(Instant, Duration)>>>,
    /// 写入通道
    write_tx: Arc<mpsc::Sender<Vec<BatchItem<K, V>>>>,
    /// 写入任务句柄
    write_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// 调整任务句柄
    adjustment_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// 关闭标志
    shutdown_flag: Arc<AtomicBool>,
}

impl<K, V> AdaptiveBatchWriter<K, V>
where
    K: Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    /// 创建新的自适应批量写入器
    ///
    /// # 参数
    /// * `config` - 批量写入配置
    /// * `write_func` - 实际执行批量写入的函数
    ///
    /// # 返回值
    /// * 新的 AdaptiveBatchWriter 实例
    pub fn new<F>(config: BatchWriteConfig, mut write_func: F) -> Self
    where
        F: FnMut(Vec<BatchItem<K, V>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + 'static,
    {
        let config = Arc::new(config);
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let batch_size = Arc::new(AtomicUsize::new(config.initial_batch_size));
        let wait_us = Arc::new(AtomicU64::new(config.max_wait_us));
        let total_batches = Arc::new(AtomicU64::new(0));
        let total_items = Arc::new(AtomicU64::new(0));
        let total_latency_ms = Arc::new(AtomicU64::new(0));
        let batch_write_latency_ms = Arc::new(AtomicU64::new(0));
        let error_count = Arc::new(AtomicU64::new(0));
        let latency_window = Arc::new(Mutex::new(VecDeque::new()));
        let (tx, mut rx) = mpsc::channel::<Vec<BatchItem<K, V>>>(100);

        // 创建写入任务
        let write_task = tokio::spawn({
            let total_batches = total_batches.clone();
            let total_items = total_items.clone();
            let total_latency_ms = total_latency_ms.clone();
            let batch_write_latency_ms = batch_write_latency_ms.clone();
            let error_count = error_count.clone();

            async move {
                while let Some(items) = rx.recv().await {
                    let start = Instant::now();
                    match write_func(items).await {
                        Ok(()) => {
                            let elapsed = start.elapsed();
                            batch_write_latency_ms.fetch_add(
                                elapsed.as_millis() as u64,
                                Ordering::Relaxed,
                            );
                            total_batches.fetch_add(1, Ordering::Relaxed);
                            total_items.fetch_add(items.len() as u64, Ordering::Relaxed);
                        }
                        Err(_) => {
                            error_count.fetch_add(1, Ordering::Relaxed);
                            warn!("Batch write failed for {} items", items.len());
                        }
                    }
                }
            }
        });

        // 创建调整任务
        let adjustment_task = tokio::spawn({
            let config = config.clone();
            let batch_size = batch_size.clone();
            let wait_us = wait_us.clone();
            let latency_window = latency_window.clone();

            async move {
                let interval = Duration::from_millis(config.adjustment_interval_ms);
                loop {
                    tokio::time::sleep(interval).await;
                    Self::adjust_batch_parameters(
                        &config,
                        &batch_size,
                        &wait_us,
                        &latency_window,
                    ).await;
                }
            }
        });

        Self {
            config,
            queue,
            batch_size,
            wait_us,
            total_batches,
            total_items,
            total_latency_ms,
            batch_write_latency_ms,
            error_count,
            latency_window,
            write_tx: Arc::new(tx),
            write_task: Arc::new(Mutex::new(Some(write_task))),
            adjustment_task: Arc::new(Mutex::new(Some(adjustment_task))),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 调整批量参数
    async fn adjust_batch_parameters(
        config: &BatchWriteConfig,
        batch_size: &AtomicUsize,
        wait_us: &AtomicU64,
        latency_window: &Mutex<VecDeque<(Instant, Duration)>>,
    ) {
        let mut window = latency_window.lock().await;
        let now = Instant::now();

        // 清理过期的历史记录
        window.retain(|(time, _)| now.duration_since(*time) < Duration::from_secs(60));

        if window.is_empty() {
            return;
        }

        // 计算平均延迟
        let total_latency: Duration = window.iter().map(|(_, d)| *d).sum();
        let avg_latency = total_latency / window.len() as u32;
        let avg_latency_ms = avg_latency.as_millis() as f64;

        // 获取当前参数
        let current_size = batch_size.load(Ordering::Relaxed);
        let current_wait = wait_us.load(Ordering::Relaxed);

        // 根据延迟调整
        let new_size = if avg_latency_ms > config.latency_threshold_ms as f64 {
            // 延迟过高，减少批量
            ((current_size as f64) * config.shrink_factor) as usize
                .max(config.min_batch_size)
        } else if avg_latency_ms < config.latency_threshold_ms as f64 * 0.5 {
            // 延迟很低，增加批量
            ((current_size as f64) * config.growth_factor) as usize
                .min(config.max_batch_size)
        } else {
            current_size
        };

        // 调整等待时间
        let new_wait = if avg_latency_ms > config.latency_threshold_ms as f64 {
            ((current_wait as f64) * config.growth_factor) as u64
                .min(config.max_wait_us)
        } else {
            ((current_wait as f64) * config.shrink_factor) as u64
                .max(config.min_wait_us)
        };

        if new_size != current_size {
            batch_size.store(new_size, Ordering::Relaxed);
            debug!("Adjusted batch size: {} -> {}", current_size, new_size);
        }

        if new_wait != current_wait {
            wait_us.store(new_wait, Ordering::Relaxed);
            debug!("Adjusted wait_us: {}us -> {}us", current_wait, new_wait);
        }
    }

    /// 添加写入项
    ///
    /// # 参数
    /// * `item` - 要写入的项
    ///
    /// # 返回值
    /// * 写入项数量
    pub async fn push(&self, item: BatchItem<K, V>) -> usize {
        let mut queue = self.queue.lock().await;
        queue.push_back(item);

        // 检查是否达到批量大小
        let batch_size = self.batch_size.load(Ordering::Relaxed);
        if queue.len() >= batch_size {
            self.flush().await
        } else {
            queue.len()
        }
    }

    /// 批量添加写入项
    ///
    /// # 参数
    /// * `items` - 要写入的项列表
    ///
    /// # 返回值
    /// * 添加的项数量
    pub async fn push_batch(&self, items: Vec<BatchItem<K, V>>) -> usize {
        let mut queue = self.queue.lock().await;
        let count = items.len();

        for item in items {
            queue.push_back(item);
        }

        // 检查是否需要刷新
        let batch_size = self.batch_size.load(Ordering::Relaxed);
        if queue.len() >= batch_size {
            self.flush().await
        } else {
            queue.len()
        }
    }

    /// 刷新当前队列
    ///
    /// # 返回值
    /// * 刷新的项数量
    pub async fn flush(&self) -> usize {
        let mut queue = self.queue.lock().await;

        if queue.is_empty() {
            return 0;
        }

        let items: Vec<BatchItem<K, V>> = queue.drain(..).collect();
        let count = items.len();

        // 记录延迟
        let wait_us = self.wait_us.load(Ordering::Relaxed);

        if let Err(e) = self.write_tx.send(items).await {
            warn!("Failed to send batch write: {}", e);
        }

        debug!("Flushed {} items, wait_us: {}", count, wait_us);
        count
    }

    /// 记录延迟
    pub fn record_latency(&self, latency: Duration) {
        let mut window = self.latency_window.blocking_lock();
        let now = Instant::now();
        window.push_back((now, latency));

        // 保持窗口大小在 100 条以内
        if window.len() > 100 {
            window.pop_front();
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> BatchMetrics {
        let total = self.total_batches.load(Ordering::Relaxed);
        let items = self.total_items.load(Ordering::Relaxed);
        let latency = self.total_latency_ms.load(Ordering::Relaxed);
        let batch_latency = self.batch_write_latency_ms.load(Ordering::Relaxed);
        let errors = self.error_count.load(Ordering::Relaxed);

        let avg_batch_size = if total > 0 {
            items as f64 / total as f64
        } else {
            0.0
        };

        let avg_latency_ms = if total > 0 {
            latency as f64 / total as f64
        } else {
            0.0
        };

        let batch_latency_avg = if total > 0 {
            batch_latency as f64 / total as f64
        } else {
            0.0
        };

        let error_rate = if items > 0 {
            errors as f64 / items as f64 * 100.0
        } else {
            0.0
        };

        BatchMetrics {
            total_batches: total,
            total_items: items,
            avg_batch_size,
            avg_latency_ms,
            batch_write_latency_ms: batch_latency_avg,
            error_count: errors,
            error_rate,
            current_batch_size: self.batch_size.load(Ordering::Relaxed),
            current_wait_us: self.wait_us.load(Ordering::Relaxed),
            queue_depth: self.queue
            .blocking_lock()
            .expect("AdaptiveBatchWriter queue lock poisoned")
            .len(),
        }
    }

    /// 获取当前批量大小
    pub fn batch_size(&self) -> usize {
        self.batch_size.load(Ordering::Relaxed)
    }

    /// 获取当前等待时间
    pub fn wait_us(&self) -> u64 {
        self.wait_us.load(Ordering::Relaxed)
    }

    /// 停止写入器
    pub async fn shutdown(&self) {
        // 设置关闭标志
        self.shutdown_flag.store(true, Ordering::Release);

        // 先刷新剩余项
        self.flush().await;

        // 关闭发送端，这将导致写入任务自然结束
        drop(self.write_tx.clone());

        // 等待写入任务完成
        if let Some(handle) = self.write_task.lock().await.take() {
            debug!("Waiting for write task to complete...");
            let _ = tokio::time::timeout(
                Duration::from_secs(30),
                handle
            ).await;
            debug!("Write task completed");
        }

        // 等待调整任务完成
        if let Some(handle) = self.adjustment_task.lock().await.take() {
            debug!("Waiting for adjustment task to complete...");
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                handle
            ).await;
            debug!("Adjustment task completed");
        }

        debug!("AdaptiveBatchWriter shutdown completed");
    }
}

/// 批量写入装饰器
///
/// 为现有后端添加批量写入能力。
pub struct BatchWriteDecorator<B> {
    /// 底层后端
    backend: B,
    /// 批量写入器
    batch_writer: AdaptiveBatchWriter<String, Vec<u8>>,
}

impl<B> BatchWriteDecorator<B>
where
    B: Clone + Send + 'static,
{
    /// 创建新的批量写入装饰器
    pub fn new(backend: B, config: BatchWriteConfig) -> Self
    where
        B: crate::backend::strategy::traits::L2BackendStrategy,
    {
        let batch_writer = AdaptiveBatchWriter::new(config, {
            let backend = backend.clone();
            move |items| {
                let backend = backend.clone();
                Box::pin(async move {
                    let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
                    let values: Vec<&[u8]> = items.iter().map(|i| i.value.as_slice()).collect();
                    let ttls: Vec<Option<u64>> = items.iter().map(|i| i.ttl).collect();

                    backend.mset(&keys.iter().zip(values.iter()).collect::<Vec<_>>(), ttls.first().copied()).await
                })
            }
        });

        Self {
            backend,
            batch_writer,
        }
    }

    /// 添加写入操作
    pub async fn batch_set(
        &self,
        key: &str,
        value: &[u8],
        ttl: Option<u64>,
    ) {
        let item = BatchItem::new(key.to_string(), value.to_vec(), ttl);
        self.batch_writer.push(item).await;
    }

    /// 批量写入
    pub async fn batch_set_batch(
        &self,
        items: &[(&str, &[u8], Option<u64>)],
    ) {
        let batch_items: Vec<BatchItem<String, Vec<u8>>> = items
            .iter()
            .map(|(k, v, t)| BatchItem::new(k.to_string(), v.to_vec(), *t))
            .collect();

        self.batch_writer.push_batch(batch_items).await;
    }

    /// 刷新待写入项
    pub async fn flush(&self) {
        self.batch_writer.flush().await;
    }

    /// 获取统计信息
    pub fn stats(&self) -> BatchMetrics {
        self.batch_writer.stats()
    }
}
