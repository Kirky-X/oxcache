// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 性能指标模块
//!
//! 提供高性能的指标收集系统，支持延迟直方图、操作计数器和性能快照。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 最大延迟直方图桶数量
const MAX_HISTOGRAM_BUCKETS: usize = 100;

/// 延迟直方图桶配置
#[derive(Debug, Clone)]
pub struct HistogramBucket {
    /// 桶边界（微秒）
    pub upper_bound_us: u64,
    /// 计数
    pub count: u64,
    /// 累积百分比
    pub cumulative_percentile: f64,
}

impl HistogramBucket {
    fn new(upper_bound_us: u64) -> Self {
        Self {
            upper_bound_us,
            count: 0,
            cumulative_percentile: 0.0,
        }
    }
}

/// 操作类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationType {
    /// 获取操作
    Get,
    /// 设置操作
    Set,
    /// 删除操作
    Delete,
    /// 存在检查
    Exists,
    /// 批量操作
    Batch,
    /// 其他操作
    Other,
}

impl OperationType {
    /// 获取操作名称
    pub fn name(&self) -> &'static str {
        match self {
            OperationType::Get => "get",
            OperationType::Set => "set",
            OperationType::Delete => "delete",
            OperationType::Exists => "exists",
            OperationType::Batch => "batch",
            OperationType::Other => "other",
        }
    }
}

/// 延迟直方图
///
/// 用于跟踪操作延迟分布。
#[derive(Clone)]
pub struct LatencyHistogram {
    /// 桶边界配置（微秒）
    buckets: Vec<u64>,
    /// 桶计数
    bucket_counts: Vec<Arc<AtomicU64>>,
    /// 总计数
    total_count: Arc<AtomicU64>,
    /// 总延迟（微秒）
    total_latency_us: Arc<AtomicU64>,
    /// 最小延迟
    min_latency_us: Arc<AtomicU64>,
    /// 最大延迟
    max_latency_us: Arc<AtomicU64>,
}

impl LatencyHistogram {
    /// 创建新的延迟直方图
    ///
    /// # 参数
    /// * `bucket_bounds_us` - 桶边界配置（微秒）
    ///
    /// # 返回值
    /// * 新的 LatencyHistogram 实例
    pub fn new(bucket_bounds_us: Vec<u64>) -> Result<Self, crate::error::OxCacheError> {
        // 验证桶数量限制，防止内存过度分配
        if bucket_bounds_us.len() > MAX_HISTOGRAM_BUCKETS {
            return Err(crate::error::OxCacheError::InvalidInput(format!(
                "Histogram bucket bounds exceed maximum of {} (got {})",
                MAX_HISTOGRAM_BUCKETS,
                bucket_bounds_us.len()
            )));
        }

        let bucket_counts: Vec<_> = bucket_bounds_us.iter().map(|_| Arc::new(AtomicU64::new(0))).collect();

        let max_latency = u64::MAX;

        Ok(Self {
            buckets: bucket_bounds_us,
            bucket_counts,
            total_count: Arc::new(AtomicU64::new(0)),
            total_latency_us: Arc::new(AtomicU64::new(0)),
            min_latency_us: Arc::new(AtomicU64::new(max_latency)),
            max_latency_us: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 记录延迟
    ///
    /// # 参数
    /// * `latency` - 延迟时长
    pub fn record(&self, latency: Duration) {
        let latency_us = latency.as_micros() as u64;

        // 更新统计
        self.total_count.fetch_add(1, Ordering::Relaxed);
        self.total_latency_us.fetch_add(latency_us, Ordering::Relaxed);

        // 更新最小/最大
        loop {
            let current_min = self.min_latency_us.load(Ordering::Relaxed);
            // 仅当 min 已设置（非 u64::MAX）且新延迟不小于当前最小值时才跳过更新
            if current_min != u64::MAX && latency_us >= current_min {
                break;
            }
            if self
                .min_latency_us
                .compare_exchange(current_min, latency_us, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        loop {
            let current_max = self.max_latency_us.load(Ordering::Relaxed);
            if latency_us <= current_max {
                break;
            }
            if self
                .max_latency_us
                .compare_exchange(current_max, latency_us, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // 更新桶计数
        for (i, bound) in self.buckets.iter().enumerate() {
            if latency_us <= *bound {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        // 超过所有边界，放入最后一个桶
        if let Some(last) = self.bucket_counts.last() {
            last.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 获取桶统计
    pub fn buckets(&self) -> Vec<HistogramBucket> {
        let total = self.total_count.load(Ordering::Relaxed);
        if total == 0 {
            return self.buckets.iter().map(|b| HistogramBucket::new(*b)).collect();
        }

        let mut result = Vec::new();
        let mut cumulative = 0u64;

        for (i, bound) in self.buckets.iter().enumerate() {
            let count = self.bucket_counts[i].load(Ordering::Relaxed);
            cumulative += count;
            let percentile = cumulative as f64 / total as f64 * 100.0;

            result.push(HistogramBucket {
                upper_bound_us: *bound,
                count,
                cumulative_percentile: percentile,
            });
        }

        result
    }

    /// 获取统计摘要
    pub fn summary(&self) -> HistogramSummary {
        let total = self.total_count.load(Ordering::Relaxed);
        let total_latency = self.total_latency_us.load(Ordering::Relaxed);
        let min = self.min_latency_us.load(Ordering::Relaxed);
        let max = self.max_latency_us.load(Ordering::Relaxed);

        let avg_latency_us = if total > 0 {
            total_latency as f64 / total as f64
        } else {
            0.0
        };

        HistogramSummary {
            total_count: total,
            avg_latency_us,
            min_latency_us: if min == u64::MAX { 0 } else { min },
            max_latency_us: max,
        }
    }

    /// 重置直方图
    pub fn reset(&self) {
        for counter in &self.bucket_counts {
            counter.store(0, Ordering::Relaxed);
        }
        self.total_count.store(0, Ordering::Relaxed);
        self.total_latency_us.store(0, Ordering::Relaxed);
        self.min_latency_us.store(u64::MAX, Ordering::Relaxed);
        self.max_latency_us.store(0, Ordering::Relaxed);
    }
}

/// 直方图摘要
#[derive(Debug, Clone)]
pub struct HistogramSummary {
    /// 总计数
    pub total_count: u64,
    /// 平均延迟（微秒）
    pub avg_latency_us: f64,
    /// 最小延迟（微秒）
    pub min_latency_us: u64,
    /// 最大延迟（微秒）
    pub max_latency_us: u64,
}

/// 操作计数器
///
/// 跟踪各类操作的成功/失败次数。
#[derive(Clone)]
pub struct OperationCounter {
    /// 操作类型
    op_type: OperationType,
    /// 成功计数
    success_count: Arc<AtomicU64>,
    /// 失败计数
    failure_count: Arc<AtomicU64>,
    /// 延迟直方图
    latency_histogram: LatencyHistogram,
}

impl OperationCounter {
    /// 创建新的操作计数器
    pub fn new(op_type: OperationType, bucket_bounds_us: Vec<u64>) -> Result<Self, crate::error::OxCacheError> {
        Ok(Self {
            op_type,
            success_count: Arc::new(AtomicU64::new(0)),
            failure_count: Arc::new(AtomicU64::new(0)),
            latency_histogram: LatencyHistogram::new(bucket_bounds_us)?,
        })
    }

    /// 记录成功操作
    pub fn record_success(&self, latency: Duration) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.latency_histogram.record(latency);
    }

    /// 记录失败操作
    pub fn record_failure(&self, latency: Duration) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.latency_histogram.record(latency);
    }

    /// 获取统计信息
    pub fn stats(&self) -> OperationStats {
        let success = self.success_count.load(Ordering::Relaxed);
        let failure = self.failure_count.load(Ordering::Relaxed);
        let total = success + failure;
        let summary = self.latency_histogram.summary();

        OperationStats {
            op_type: self.op_type.name().to_string(),
            total_count: total,
            success_count: success,
            failure_count: failure,
            success_rate: if total > 0 {
                success as f64 / total as f64 * 100.0
            } else {
                0.0
            },
            avg_latency_us: summary.avg_latency_us,
            min_latency_us: summary.min_latency_us,
            max_latency_us: summary.max_latency_us,
        }
    }
}

/// 操作统计
#[derive(Debug, Clone)]
pub struct OperationStats {
    /// 操作类型
    pub op_type: String,
    /// 总计数
    pub total_count: u64,
    /// 成功计数
    pub success_count: u64,
    /// 失败计数
    pub failure_count: u64,
    /// 成功率
    pub success_rate: f64,
    /// 平均延迟（微秒）
    pub avg_latency_us: f64,
    /// 最小延迟（微秒）
    pub min_latency_us: u64,
    /// 最大延迟（微秒）
    pub max_latency_us: u64,
}

/// 性能指标收集器
///
/// 集中收集和报告性能指标。
#[derive(Clone)]
pub struct MetricsCollector {
    /// 操作计数器
    operation_counters: Arc<Vec<OperationCounter>>,
    /// L1 命中计数
    l1_hits: Arc<AtomicU64>,
    /// L1 未命中计数
    l1_misses: Arc<AtomicU64>,
    /// L2 命中计数
    l2_hits: Arc<AtomicU64>,
    /// L2 未命中计数
    l2_misses: Arc<AtomicU64>,
    /// 当前连接数
    connections: Arc<AtomicUsize>,
    /// 活跃任务数
    active_tasks: Arc<AtomicUsize>,
    /// 队列深度
    queue_depth: Arc<AtomicUsize>,
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new() -> Result<Self, crate::error::OxCacheError> {
        // 初始化操作计数器
        let op_types = vec![
            OperationType::Get,
            OperationType::Set,
            OperationType::Delete,
            OperationType::Exists,
            OperationType::Batch,
        ];

        let bucket_bounds = vec![100, 500, 1000, 5000, 10000, 50000, 100000, 500000, 1000000];

        let operation_counters: Vec<_> = op_types
            .into_iter()
            .map(|op| OperationCounter::new(op, bucket_bounds.clone()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            operation_counters: Arc::new(operation_counters),
            l1_hits: Arc::new(AtomicU64::new(0)),
            l1_misses: Arc::new(AtomicU64::new(0)),
            l2_hits: Arc::new(AtomicU64::new(0)),
            l2_misses: Arc::new(AtomicU64::new(0)),
            connections: Arc::new(AtomicUsize::new(0)),
            active_tasks: Arc::new(AtomicUsize::new(0)),
            queue_depth: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// 获取操作计数器
    pub fn operation_counter(&self, op_type: OperationType) -> Option<&OperationCounter> {
        self.operation_counters.iter().find(|c| c.op_type == op_type)
    }

    /// 记录 L1 命中
    pub fn record_l1_hit(&self) {
        self.l1_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 L1 未命中
    pub fn record_l1_miss(&self) {
        self.l1_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 L2 命中
    pub fn record_l2_hit(&self) {
        self.l2_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 L2 未命中
    pub fn record_l2_miss(&self) {
        self.l2_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 更新连接数
    pub fn set_connections(&self, count: usize) {
        self.connections.store(count, Ordering::Relaxed);
    }

    /// 更新活跃任务数
    pub fn set_active_tasks(&self, count: usize) {
        self.active_tasks.store(count, Ordering::Relaxed);
    }

    /// 更新队列深度
    pub fn set_queue_depth(&self, depth: usize) {
        self.queue_depth.store(depth, Ordering::Relaxed);
    }

    /// 获取完整统计信息
    pub fn full_stats(&self) -> FullMetrics {
        let l1_hits = self.l1_hits.load(Ordering::Relaxed);
        let l1_misses = self.l1_misses.load(Ordering::Relaxed);
        let l2_hits = self.l2_hits.load(Ordering::Relaxed);
        let l2_misses = self.l2_misses.load(Ordering::Relaxed);

        let l1_total = l1_hits + l1_misses;
        let l2_total = l2_hits + l2_misses;

        let op_stats: Vec<_> = self.operation_counters.iter().map(|c| c.stats()).collect();

        FullMetrics {
            l1_hits,
            l1_misses,
            l1_hit_rate: if l1_total > 0 {
                l1_hits as f64 / l1_total as f64 * 100.0
            } else {
                0.0
            },
            l2_hits,
            l2_misses,
            l2_hit_rate: if l2_total > 0 {
                l2_hits as f64 / l2_total as f64 * 100.0
            } else {
                0.0
            },
            connections: self.connections.load(Ordering::Relaxed),
            active_tasks: self.active_tasks.load(Ordering::Relaxed),
            queue_depth: self.queue_depth.load(Ordering::Relaxed),
            operation_stats: op_stats,
        }
    }

    /// 获取缓存命中率
    pub fn cache_hit_rates(&self) -> CacheHitRates {
        let l1_hits = self.l1_hits.load(Ordering::Relaxed);
        let l1_misses = self.l1_misses.load(Ordering::Relaxed);
        let l2_hits = self.l2_hits.load(Ordering::Relaxed);
        let l2_misses = self.l2_misses.load(Ordering::Relaxed);

        let l1_total = l1_hits + l1_misses;

        // 全局命中率（从 L1 获得的比例）
        let global_hit_rate = if l1_total > 0 {
            l1_hits as f64 / l1_total as f64 * 100.0
        } else {
            0.0
        };

        // L2 命中率（未命中 L1 后从 L2 获得的比例）
        let l2_hit_rate = if l1_misses > 0 {
            l2_hits as f64 / l1_misses as f64 * 100.0
        } else {
            0.0
        };

        CacheHitRates {
            l1_hit_rate: global_hit_rate,
            l2_hit_rate,
            l1_hits,
            l1_misses,
            l2_hits,
            l2_misses,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create default MetricsCollector")
    }
}

/// 缓存命中率
#[derive(Debug, Clone)]
pub struct CacheHitRates {
    /// L1 命中率（全局命中率）
    pub l1_hit_rate: f64,
    /// L2 命中率（在 L1 未命中的情况下）
    pub l2_hit_rate: f64,
    /// L1 命中次数
    pub l1_hits: u64,
    /// L1 未命中次数
    pub l1_misses: u64,
    /// L2 命中次数
    pub l2_hits: u64,
    /// L2 未命中次数
    pub l2_misses: u64,
}

/// 完整指标
#[derive(Debug, Clone)]
pub struct FullMetrics {
    /// L1 命中次数
    pub l1_hits: u64,
    /// L1 未命中次数
    pub l1_misses: u64,
    /// L1 命中率
    pub l1_hit_rate: f64,
    /// L2 命中次数
    pub l2_hits: u64,
    /// L2 未命中次数
    pub l2_misses: u64,
    /// L2 命中率
    pub l2_hit_rate: f64,
    /// 当前连接数
    pub connections: usize,
    /// 活跃任务数
    pub active_tasks: usize,
    /// 队列深度
    pub queue_depth: usize,
    /// 操作统计
    pub operation_stats: Vec<OperationStats>,
}

/// 性能快照
///
/// 在特定时间点捕获的性能指标。
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// 捕获时间
    pub timestamp: Instant,
    /// 完整指标
    pub metrics: FullMetrics,
    /// 快照间隔（秒）
    pub interval_secs: f64,
}

impl PerformanceSnapshot {
    /// 创建新的性能快照
    pub fn new(metrics: FullMetrics, interval_secs: f64) -> Self {
        Self {
            timestamp: Instant::now(),
            metrics,
            interval_secs,
        }
    }
}

/// 滑动窗口指标
///
/// 维护最近时间窗口内的性能指标。
#[derive(Clone)]
pub struct SlidingWindowMetrics {
    /// 指标收集器
    collector: Arc<MetricsCollector>,
    /// 历史快照
    snapshots: Arc<Mutex<VecDeque<PerformanceSnapshot>>>,
    /// 最大快照数
    max_snapshots: usize,
    /// 窗口大小（秒）
    window_secs: u64,
    /// 最后捕获时间
    last_capture: Arc<Mutex<Instant>>,
}

impl SlidingWindowMetrics {
    /// 创建新的滑动窗口指标
    pub fn new(collector: Arc<MetricsCollector>, window_secs: u64, max_snapshots: usize) -> Self {
        Self {
            collector,
            snapshots: Arc::new(Mutex::new(VecDeque::new())),
            max_snapshots,
            window_secs,
            last_capture: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 捕获当前指标
    pub fn capture(&self) {
        let mut last = self
            .last_capture
            .lock()
            .expect("metrics last_capture lock poisoned - previous panic detected");
        let now = Instant::now();
        let interval = now.duration_since(*last).as_secs_f64();

        let metrics = self.collector.full_stats();
        let snapshot = PerformanceSnapshot::new(metrics, interval);

        let mut snapshots = self
            .snapshots
            .lock()
            .expect("metrics snapshots lock poisoned - previous panic detected");
        snapshots.push_back(snapshot);

        // 清理过期的快照
        let now = Instant::now();
        snapshots.retain(|s| now.duration_since(s.timestamp) < Duration::from_secs(self.window_secs));

        // 保持最大数量
        while snapshots.len() > self.max_snapshots {
            snapshots.pop_front();
        }

        *last = now;
    }

    /// 获取窗口指标摘要
    pub async fn window_summary(&self) -> WindowMetricsSummary {
        let snapshots = self
            .snapshots
            .lock()
            .expect("metrics snapshots lock poisoned - previous panic detected");
        let count = snapshots.len();

        if count == 0 {
            return WindowMetricsSummary::default();
        }

        let mut total_l1_hits = 0;
        let mut total_l1_misses = 0;
        let mut total_l2_hits = 0;
        let mut total_l2_misses = 0;
        let mut total_ops = 0;
        let mut total_success = 0;

        for snapshot in snapshots.iter() {
            total_l1_hits += snapshot.metrics.l1_hits;
            total_l1_misses += snapshot.metrics.l1_misses;
            total_l2_hits += snapshot.metrics.l2_hits;
            total_l2_misses += snapshot.metrics.l2_misses;

            for op in &snapshot.metrics.operation_stats {
                total_ops += op.total_count;
                total_success += op.success_count;
            }
        }

        let l1_total = total_l1_hits + total_l1_misses;
        let l2_total = total_l2_hits + total_l2_misses;

        WindowMetricsSummary {
            snapshot_count: count,
            window_secs: self.window_secs,
            avg_l1_hit_rate: if l1_total > 0 {
                total_l1_hits as f64 / l1_total as f64 * 100.0
            } else {
                0.0
            },
            avg_l2_hit_rate: if l2_total > 0 {
                total_l2_hits as f64 / l2_total as f64 * 100.0
            } else {
                0.0
            },
            total_l1_hits,
            total_l1_misses,
            total_l2_hits,
            total_l2_misses,
            total_operations: total_ops,
            success_rate: if total_ops > 0 {
                total_success as f64 / total_ops as f64 * 100.0
            } else {
                0.0
            },
        }
    }
}

/// 窗口指标摘要
#[derive(Debug, Clone, Default)]
pub struct WindowMetricsSummary {
    /// 快照数量
    pub snapshot_count: usize,
    /// 窗口大小（秒）
    pub window_secs: u64,
    /// 平均 L1 命中率
    pub avg_l1_hit_rate: f64,
    /// 平均 L2 命中率
    pub avg_l2_hit_rate: f64,
    /// 总 L1 命中
    pub total_l1_hits: u64,
    /// 总 L1 未命中
    pub total_l1_misses: u64,
    /// 总 L2 命中
    pub total_l2_hits: u64,
    /// 总 L2 未命中
    pub total_l2_misses: u64,
    /// 总操作数
    pub total_operations: u64,
    /// 成功率
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // OperationType tests
    // ----------------------------------------------------------------

    #[test]
    fn test_operation_type_name() {
        assert_eq!(OperationType::Get.name(), "get");
        assert_eq!(OperationType::Set.name(), "set");
        assert_eq!(OperationType::Delete.name(), "delete");
        assert_eq!(OperationType::Exists.name(), "exists");
        assert_eq!(OperationType::Batch.name(), "batch");
        assert_eq!(OperationType::Other.name(), "other");
    }

    // ----------------------------------------------------------------
    // LatencyHistogram tests
    // ----------------------------------------------------------------

    #[test]
    fn test_latency_histogram_new() {
        let hist = LatencyHistogram::new(vec![100, 500, 1000]).unwrap();
        let summary = hist.summary();
        assert_eq!(summary.total_count, 0);
        assert_eq!(summary.avg_latency_us, 0.0);
        assert_eq!(summary.min_latency_us, 0);
        assert_eq!(summary.max_latency_us, 0);
    }

    #[test]
    fn test_latency_histogram_new_too_many_buckets() {
        let bounds: Vec<u64> = (0..MAX_HISTOGRAM_BUCKETS + 1).map(|i| i as u64 * 100).collect();
        let result = LatencyHistogram::new(bounds);
        assert!(result.is_err(), "Should reject too many buckets");
    }

    #[test]
    fn test_latency_histogram_record_within_bounds() {
        let hist = LatencyHistogram::new(vec![100, 500, 1000]).unwrap();
        // 50us <= 100 -> bucket 0
        hist.record(Duration::from_micros(50));
        // 200us <= 500 -> bucket 1
        hist.record(Duration::from_micros(200));
        // 800us <= 1000 -> bucket 2
        hist.record(Duration::from_micros(800));

        let summary = hist.summary();
        assert_eq!(summary.total_count, 3);
        assert_eq!(summary.min_latency_us, 50);
        assert_eq!(summary.max_latency_us, 800);
        // avg = (50 + 200 + 800) / 3 = 350
        assert!((summary.avg_latency_us - 350.0).abs() < 0.01);

        let buckets = hist.buckets();
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].count, 1);
        assert_eq!(buckets[1].count, 1);
        assert_eq!(buckets[2].count, 1);
        // cumulative percentiles
        assert!((buckets[0].cumulative_percentile - 100.0 / 3.0).abs() < 0.01);
        assert!((buckets[1].cumulative_percentile - 200.0 / 3.0).abs() < 0.01);
        assert!((buckets[2].cumulative_percentile - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_latency_histogram_record_exceeds_all_bounds() {
        let hist = LatencyHistogram::new(vec![100, 500]).unwrap();
        // 1000us exceeds all bounds -> falls into last bucket
        hist.record(Duration::from_micros(1000));

        let buckets = hist.buckets();
        assert_eq!(buckets.len(), 2);
        // Last bucket gets the overflow count
        assert_eq!(buckets[1].count, 1);
        assert_eq!(buckets[0].count, 0);
    }

    #[test]
    fn test_latency_histogram_buckets_empty() {
        let hist = LatencyHistogram::new(vec![100, 500]).unwrap();
        let buckets = hist.buckets();
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].count, 0);
        assert_eq!(buckets[0].upper_bound_us, 100);
        assert_eq!(buckets[0].cumulative_percentile, 0.0);
    }

    #[test]
    fn test_latency_histogram_reset() {
        let hist = LatencyHistogram::new(vec![100, 500]).unwrap();
        hist.record(Duration::from_micros(50));
        hist.record(Duration::from_micros(200));
        assert_eq!(hist.summary().total_count, 2);

        hist.reset();
        let summary = hist.summary();
        assert_eq!(summary.total_count, 0);
        assert_eq!(summary.min_latency_us, 0);
        assert_eq!(summary.max_latency_us, 0);
        assert_eq!(summary.avg_latency_us, 0.0);

        let buckets = hist.buckets();
        assert_eq!(buckets[0].count, 0);
        assert_eq!(buckets[1].count, 0);
    }

    #[test]
    fn test_latency_histogram_min_max_updates() {
        let hist = LatencyHistogram::new(vec![10000]).unwrap();
        hist.record(Duration::from_micros(500));
        assert_eq!(hist.summary().min_latency_us, 500);
        assert_eq!(hist.summary().max_latency_us, 500);

        // New min
        hist.record(Duration::from_micros(100));
        assert_eq!(hist.summary().min_latency_us, 100);
        assert_eq!(hist.summary().max_latency_us, 500);

        // New max
        hist.record(Duration::from_micros(5000));
        assert_eq!(hist.summary().min_latency_us, 100);
        assert_eq!(hist.summary().max_latency_us, 5000);
    }

    // ----------------------------------------------------------------
    // OperationCounter tests
    // ----------------------------------------------------------------

    #[test]
    fn test_operation_counter_record_success_and_failure() {
        let counter = OperationCounter::new(OperationType::Get, vec![100, 500]).unwrap();
        counter.record_success(Duration::from_micros(50));
        counter.record_success(Duration::from_micros(200));
        counter.record_failure(Duration::from_micros(400));

        let stats = counter.stats();
        assert_eq!(stats.op_type, "get");
        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.success_count, 2);
        assert_eq!(stats.failure_count, 1);
        // success_rate = 2/3 * 100 = 66.66...
        assert!((stats.success_rate - 66.6667).abs() < 0.01);
        assert_eq!(stats.min_latency_us, 50);
        assert_eq!(stats.max_latency_us, 400);
    }

    #[test]
    fn test_operation_counter_stats_empty() {
        let counter = OperationCounter::new(OperationType::Set, vec![100]).unwrap();
        let stats = counter.stats();
        assert_eq!(stats.op_type, "set");
        assert_eq!(stats.total_count, 0);
        assert_eq!(stats.success_count, 0);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.success_rate, 0.0);
        assert_eq!(stats.avg_latency_us, 0.0);
    }

    #[test]
    fn test_operation_counter_new_too_many_buckets() {
        let bounds: Vec<u64> = (0..MAX_HISTOGRAM_BUCKETS + 1).map(|i| i as u64).collect();
        let result = OperationCounter::new(OperationType::Get, bounds);
        assert!(result.is_err());
    }

    // ----------------------------------------------------------------
    // MetricsCollector tests
    // ----------------------------------------------------------------

    #[test]
    fn test_metrics_collector_new() {
        let collector = MetricsCollector::new().unwrap();
        // All operation counters should be present
        assert!(collector.operation_counter(OperationType::Get).is_some());
        assert!(collector.operation_counter(OperationType::Set).is_some());
        assert!(collector.operation_counter(OperationType::Delete).is_some());
        assert!(collector.operation_counter(OperationType::Exists).is_some());
        assert!(collector.operation_counter(OperationType::Batch).is_some());
        // Other is not in the predefined list
        assert!(collector.operation_counter(OperationType::Other).is_none());
    }

    #[test]
    fn test_metrics_collector_default() {
        let collector = MetricsCollector::default();
        let stats = collector.full_stats();
        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l1_misses, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.l2_misses, 0);
        assert_eq!(stats.connections, 0);
        assert_eq!(stats.active_tasks, 0);
        assert_eq!(stats.queue_depth, 0);
        assert_eq!(stats.operation_stats.len(), 5);
    }

    #[test]
    fn test_metrics_collector_l1_l2_hits_misses() {
        let collector = MetricsCollector::new().unwrap();
        collector.record_l1_hit();
        collector.record_l1_hit();
        collector.record_l1_miss();
        collector.record_l2_hit();
        collector.record_l2_miss();
        collector.record_l2_miss();

        let stats = collector.full_stats();
        assert_eq!(stats.l1_hits, 2);
        assert_eq!(stats.l1_misses, 1);
        assert_eq!(stats.l2_hits, 1);
        assert_eq!(stats.l2_misses, 2);
        // l1_hit_rate = 2 / 3 * 100 = 66.66...
        assert!((stats.l1_hit_rate - 66.6667).abs() < 0.01);
        // l2_hit_rate = 1 / 3 * 100 = 33.33...
        assert!((stats.l2_hit_rate - 33.3333).abs() < 0.01);
    }

    #[test]
    fn test_metrics_collector_set_state() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_connections(10);
        collector.set_active_tasks(5);
        collector.set_queue_depth(20);

        let stats = collector.full_stats();
        assert_eq!(stats.connections, 10);
        assert_eq!(stats.active_tasks, 5);
        assert_eq!(stats.queue_depth, 20);
    }

    #[test]
    fn test_metrics_collector_full_stats_zero_hit_rate() {
        let collector = MetricsCollector::new().unwrap();
        let stats = collector.full_stats();
        // No hits or misses -> hit rate is 0
        assert_eq!(stats.l1_hit_rate, 0.0);
        assert_eq!(stats.l2_hit_rate, 0.0);
    }

    #[test]
    fn test_metrics_collector_cache_hit_rates() {
        let collector = MetricsCollector::new().unwrap();
        // L1: 3 hits, 1 miss -> global_hit_rate = 75%
        collector.record_l1_hit();
        collector.record_l1_hit();
        collector.record_l1_hit();
        collector.record_l1_miss();
        // L2: 1 hit out of 1 L1 miss -> l2_hit_rate = 100%
        collector.record_l2_hit();

        let rates = collector.cache_hit_rates();
        assert!((rates.l1_hit_rate - 75.0).abs() < 0.01);
        assert!((rates.l2_hit_rate - 100.0).abs() < 0.01);
        assert_eq!(rates.l1_hits, 3);
        assert_eq!(rates.l1_misses, 1);
        assert_eq!(rates.l2_hits, 1);
        assert_eq!(rates.l2_misses, 0);
    }

    #[test]
    fn test_metrics_collector_cache_hit_rates_no_l1_misses() {
        let collector = MetricsCollector::new().unwrap();
        // Only L1 hits, no misses -> l2_hit_rate = 0 (no L1 misses to compute against)
        collector.record_l1_hit();
        collector.record_l1_hit();

        let rates = collector.cache_hit_rates();
        assert!((rates.l1_hit_rate - 100.0).abs() < 0.01);
        assert_eq!(rates.l2_hit_rate, 0.0);
    }

    #[test]
    fn test_metrics_collector_operation_stats_via_counter() {
        let collector = MetricsCollector::new().unwrap();
        if let Some(counter) = collector.operation_counter(OperationType::Get) {
            counter.record_success(Duration::from_micros(100));
            counter.record_failure(Duration::from_micros(200));
        }

        let stats = collector.full_stats();
        let get_stats = stats.operation_stats.iter().find(|s| s.op_type == "get").unwrap();
        assert_eq!(get_stats.total_count, 2);
        assert_eq!(get_stats.success_count, 1);
        assert_eq!(get_stats.failure_count, 1);
        assert!((get_stats.success_rate - 50.0).abs() < 0.01);
    }

    // ----------------------------------------------------------------
    // PerformanceSnapshot tests
    // ----------------------------------------------------------------

    #[test]
    fn test_performance_snapshot_new() {
        let metrics = FullMetrics {
            l1_hits: 10,
            l1_misses: 5,
            l1_hit_rate: 66.67,
            l2_hits: 3,
            l2_misses: 2,
            l2_hit_rate: 60.0,
            connections: 4,
            active_tasks: 2,
            queue_depth: 8,
            operation_stats: vec![],
        };
        let snapshot = PerformanceSnapshot::new(metrics.clone(), 1.5);
        assert_eq!(snapshot.metrics.l1_hits, 10);
        assert_eq!(snapshot.metrics.l2_hits, 3);
        assert!((snapshot.interval_secs - 1.5).abs() < 0.001);
    }

    // ----------------------------------------------------------------
    // SlidingWindowMetrics tests
    // ----------------------------------------------------------------

    #[test]
    fn test_sliding_window_metrics_new() {
        let collector = Arc::new(MetricsCollector::new().unwrap());
        let sw = SlidingWindowMetrics::new(collector, 60, 10);
        // No captures yet
        assert_eq!(sw.max_snapshots, 10);
        assert_eq!(sw.window_secs, 60);
    }

    #[tokio::test]
    async fn test_sliding_window_metrics_window_summary_empty() {
        let collector = Arc::new(MetricsCollector::new().unwrap());
        let sw = SlidingWindowMetrics::new(collector, 60, 10);
        let summary = sw.window_summary().await;
        // Empty window returns Default (snapshot_count=0, window_secs=0)
        assert_eq!(summary.snapshot_count, 0);
        assert_eq!(summary.avg_l1_hit_rate, 0.0);
        assert_eq!(summary.total_operations, 0);
    }

    #[tokio::test]
    async fn test_sliding_window_metrics_capture_and_summary() {
        let collector = Arc::new(MetricsCollector::new().unwrap());
        // Record some metrics before capture
        collector.record_l1_hit();
        collector.record_l1_hit();
        collector.record_l1_miss();
        collector.record_l2_hit();
        if let Some(c) = collector.operation_counter(OperationType::Get) {
            c.record_success(Duration::from_micros(100));
        }

        let sw = SlidingWindowMetrics::new(collector.clone(), 60, 10);
        sw.capture();

        let summary = sw.window_summary().await;
        assert_eq!(summary.snapshot_count, 1);
        assert_eq!(summary.window_secs, 60);
        assert_eq!(summary.total_l1_hits, 2);
        assert_eq!(summary.total_l1_misses, 1);
        assert_eq!(summary.total_l2_hits, 1);
        // l1_hit_rate = 2/3 * 100 = 66.66...
        assert!((summary.avg_l1_hit_rate - 66.6667).abs() < 0.01);
        // 1 operation total, 1 success -> 100%
        assert_eq!(summary.total_operations, 1);
        assert!((summary.success_rate - 100.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_sliding_window_metrics_max_snapshots() {
        let collector = Arc::new(MetricsCollector::new().unwrap());
        let sw = SlidingWindowMetrics::new(collector, 60, 2);

        sw.capture();
        sw.capture();
        sw.capture(); // should evict the oldest

        let summary = sw.window_summary().await;
        assert_eq!(summary.snapshot_count, 2, "Should cap at max_snapshots");
    }

    #[tokio::test]
    async fn test_sliding_window_metrics_window_summary_zero_division() {
        let collector = Arc::new(MetricsCollector::new().unwrap());
        let sw = SlidingWindowMetrics::new(collector, 60, 10);
        sw.capture();
        let summary = sw.window_summary().await;
        // No hits/misses -> rates are 0
        assert_eq!(summary.avg_l1_hit_rate, 0.0);
        assert_eq!(summary.avg_l2_hit_rate, 0.0);
        assert_eq!(summary.success_rate, 0.0);
    }
}
