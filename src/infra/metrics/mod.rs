//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的指标收集和监控功能。

pub mod backend;
pub mod counters;
pub mod export;
pub mod snapshot;
pub mod unified;

pub use counters::AtomicCounters;
pub use export::{export_json_format, export_prometheus_format, get_enhanced_stats};
pub use snapshot::CacheStats;
pub use unified::UnifiedMetrics;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::{span, Level};

/// 指标收集器
///
/// 用于收集和存储缓存系统的各种运行时指标
/// 优化版本：高频指标使用原子计数器，低频指标使用DashMap
#[cfg(any(feature = "metrics", feature = "moka"))]
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// 原子计数器（高频指标，无锁）
    pub(crate) counters: Arc<AtomicCounters>,
    /// 请求总数统计（低频指标，保留DashMap用于动态服务名）
    /// key: "service:layer:op:result"
    pub(crate) requests_total: Arc<DashMap<String, u64>>,
    /// L2健康状态
    pub(crate) l2_health_status: Arc<DashMap<String, u8>>,
    /// WAL条目数
    pub(crate) wal_entries: Arc<DashMap<String, usize>>,
    /// 操作耗时（简单的累积时间和计数，用于计算平均值，更复杂的直方图建议使用OpenTelemetry Metrics）
    /// key: "service:layer:op" -> (total_duration_secs, count)
    pub(crate) operation_duration: Arc<DashMap<String, (f64, u64)>>,
    /// 批量写入缓冲区大小
    pub(crate) batch_buffer_size: Arc<DashMap<String, usize>>,
    /// 批量写入成功率
    pub(crate) batch_success_rate: Arc<DashMap<String, f64>>,
    /// 批量写入吞吐量 (ops/sec)
    pub(crate) batch_throughput: Arc<DashMap<String, f64>>,
}

/// 全局指标实例
pub static GLOBAL_METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

impl Metrics {
    /// 记录请求指标（优化版本）
    ///
    /// 对于高频操作（L1/L2 get/set/delete），使用原子计数器
    /// 对于其他操作，使用HashMap
    ///
    /// # 参数
    ///
    /// * `service` - 服务名称
    /// * `layer` - 缓存层（L1/L2）
    /// * `op` - 操作类型（get/set/delete）
    /// * `result` - 操作结果（attempt/hit/miss）
    pub fn record_request(&self, service: &str, layer: &str, op: &str, result: &str) {
        let span = span!(Level::INFO, "cache_request", service, layer, op, result);
        let _enter = span.enter();

        // 使用原子计数器处理高频指标
        match (layer, op, result) {
            ("L1", "get", "hit") => {
                self.counters.l1_get_hits.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L1", "get", "miss") => {
                self.counters.l1_get_misses.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "get", "hit") => {
                self.counters.l2_get_hits.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "get", "miss") => {
                self.counters.l2_get_misses.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L1", "set", "attempt") => {
                self.counters.l1_set_total.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "set", "attempt") => {
                self.counters.l2_set_total.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L1", "delete", "attempt") => {
                self.counters.l1_delete_total.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "delete", "attempt") => {
                self.counters.l2_delete_total.fetch_add(1, Ordering::Relaxed);
                self.counters.total_operations.fetch_add(1, Ordering::Relaxed);
                return;
            }
            _ => {}
        }

        // 其他操作使用DashMap（无锁）
        let key = format!("{}:{}:{}:{}", service, layer, op, result);
        self.requests_total.entry(key).and_modify(|v| *v += 1).or_insert(1);
    }

    /// 记录操作耗时
    pub fn record_duration(&self, service: &str, layer: &str, op: &str, duration_secs: f64) {
        let key = format!("{}:{}:{}", service, layer, op);
        self.operation_duration
            .entry(key)
            .and_modify(|entry| {
                entry.0 += duration_secs;
                entry.1 += 1;
            })
            .or_insert((duration_secs, 1));
    }

    /// 设置健康状态
    ///
    /// # 参数
    ///
    /// * `service` - 服务名称
    /// * `status` - 健康状态码（0: 不健康, 1: 健康, 2: 恢复中）
    pub fn set_health(&self, service: &str, status: u8) {
        self.l2_health_status.insert(service.to_string(), status);
    }

    /// 设置WAL大小
    ///
    /// # 参数
    ///
    /// * `service` - 服务名称
    /// * `size` - WAL条目数量
    pub fn set_wal_size(&self, service: &str, size: usize) {
        self.wal_entries.insert(service.to_string(), size);
    }

    /// 设置批量写入缓冲区大小
    pub fn set_batch_buffer_size(&self, service: &str, size: usize) {
        self.batch_buffer_size.insert(service.to_string(), size);
    }

    /// 设置批量写入成功率
    pub fn set_batch_success_rate(&self, service: &str, rate: f64) {
        self.batch_success_rate.insert(service.to_string(), rate);
    }

    /// 设置批量写入吞吐量
    pub fn set_batch_throughput(&self, service: &str, throughput: f64) {
        self.batch_throughput.insert(service.to_string(), throughput);
    }

    /// 获取原子计数器的值
    pub fn get_counters(&self) -> (u64, u64, u64, u64, u64, u64, u64, u64, u64) {
        (
            self.counters.l1_get_hits.load(Ordering::Relaxed),
            self.counters.l1_get_misses.load(Ordering::Relaxed),
            self.counters.l2_get_hits.load(Ordering::Relaxed),
            self.counters.l2_get_misses.load(Ordering::Relaxed),
            self.counters.l1_set_total.load(Ordering::Relaxed),
            self.counters.l2_set_total.load(Ordering::Relaxed),
            self.counters.l1_delete_total.load(Ordering::Relaxed),
            self.counters.l2_delete_total.load(Ordering::Relaxed),
            self.counters.total_operations.load(Ordering::Relaxed),
        )
    }
}

/// 获取指标字符串
///
/// 将所有指标格式化为字符串返回，用于监控系统采集
///
/// # 返回值
///
/// 返回包含所有指标的字符串
///
/// # 注意
///
/// DashMap 无锁，无需担心死锁
#[cfg(any(feature = "metrics", feature = "moka"))]
pub fn get_metrics_string() -> String {
    let metrics = &GLOBAL_METRICS;
    let mut output = String::new();

    // 输出原子计数器（高频指标，无锁）
    let counters = metrics.get_counters();
    output.push_str(&format!("cache_l1_get_hits_total {}\n", counters.0));
    output.push_str(&format!("cache_l1_get_misses_total {}\n", counters.1));
    output.push_str(&format!("cache_l2_get_hits_total {}\n", counters.2));
    output.push_str(&format!("cache_l2_get_misses_total {}\n", counters.3));
    output.push_str(&format!("cache_l1_set_total {}\n", counters.4));
    output.push_str(&format!("cache_l2_set_total {}\n", counters.5));
    output.push_str(&format!("cache_l1_delete_total {}\n", counters.6));
    output.push_str(&format!("cache_l2_delete_total {}\n", counters.7));
    output.push_str(&format!("cache_operations_total {}\n", counters.8));

    // DashMap 无锁迭代
    let requests: &DashMap<String, u64> = &metrics.requests_total;
    for entry in requests.iter() {
        let (key, value): (&String, &u64) = entry.pair();
        output.push_str(&format!("cache_requests_total{{labels=\"{}\"}} {}\n", key, value));
    }

    let health_status: &DashMap<String, u8> = &metrics.l2_health_status;
    for entry in health_status.iter() {
        let (key, value): (&String, &u8) = entry.pair();
        output.push_str(&format!("cache_l2_health_status{{service=\"{}\"}} {}\n", key, value));
    }

    let wal_entries: &DashMap<String, usize> = &metrics.wal_entries;
    for entry in wal_entries.iter() {
        let (key, value): (&String, &usize) = entry.pair();
        output.push_str(&format!("cache_wal_entries{{service=\"{}\"}} {}\n", key, value));
    }

    let durations: &DashMap<String, (f64, u64)> = &metrics.operation_duration;
    for entry in durations.iter() {
        let (key, (total_duration, count)): (&String, &(f64, u64)) = entry.pair();
        if *count > 0 {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 3 {
                let service = parts[0];
                let layer = parts[1];
                let op = parts[2];
                let avg_duration = total_duration / *count as f64;
                output.push_str(&format!(
                    "cache_operation_duration_seconds{{service=\"{}\",layer=\"{}\",op=\"{}\"}} {}\n",
                    service, layer, op, avg_duration
                ));
            }
        }
    }

    let buffer_sizes: &DashMap<String, usize> = &metrics.batch_buffer_size;
    for entry in buffer_sizes.iter() {
        let (key, value): (&String, &usize) = entry.pair();
        output.push_str(&format!("cache_batch_buffer_size{{service=\"{}\"}} {}\n", key, value));
    }

    let success_rates: &DashMap<String, f64> = &metrics.batch_success_rate;
    for entry in success_rates.iter() {
        let (key, value): (&String, &f64) = entry.pair();
        output.push_str(&format!("cache_batch_success_rate{{service=\"{}\"}} {}\n", key, value));
    }

    let throughputs: &DashMap<String, f64> = &metrics.batch_throughput;
    for entry in throughputs.iter() {
        let (key, value): (&String, &f64) = entry.pair();
        output.push_str(&format!("cache_batch_throughput{{service=\"{}\"}} {}\n", key, value));
    }

    output
}

/// 当 metrics 和 moka 功能都禁用时的空实现
#[cfg(not(any(feature = "metrics", feature = "moka")))]
#[derive(Debug, Clone, Default)]
pub struct Metrics;

#[cfg(not(any(feature = "metrics", feature = "moka")))]
impl Metrics {
    /// 记录请求指标（空实现）
    pub fn record_request(&self, _service: &str, _layer: &str, _op: &str, _result: &str) {}

    /// 记录操作耗时（空实现）
    pub fn record_duration(&self, _service: &str, _layer: &str, _op: &str, _duration_secs: f64) {}

    /// 设置健康状态（空实现）
    pub fn set_health(&self, _service: &str, _status: u8) {}

    /// 设置WAL大小（空实现）
    pub fn set_wal_size(&self, _service: &str, _size: usize) {}

    /// 设置批量写入缓冲区大小（空实现）
    pub fn set_batch_buffer_size(&self, _service: &str, _size: usize) {}

    /// 设置批量写入成功率（空实现）
    pub fn set_batch_success_rate(&self, _service: &str, _rate: f64) {}

    /// 设置批量写入吞吐量（空实现）
    pub fn set_batch_throughput(&self, _service: &str, _throughput: f64) {}

    /// 获取原子计数器的值（返回全0）
    pub fn get_counters(&self) -> (u64, u64, u64, u64, u64, u64, u64, u64, u64) {
        (0, 0, 0, 0, 0, 0, 0, 0, 0)
    }
}

#[cfg(not(any(feature = "metrics", feature = "moka")))]
lazy_static! {
    /// 全局空指标实例
    pub static ref GLOBAL_METRICS: Metrics = Metrics;
}

#[cfg(not(any(feature = "metrics", feature = "moka")))]
/// 当 metrics 功能禁用时返回空字符串
pub fn get_metrics_string() -> String {
    String::new()
}

// ============================================================================
// Enhanced Statistics (enhanced-stats feature) - Metrics Extensions
// ============================================================================

#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
impl Metrics {
    /// 创建统计快照
    pub fn snapshot(&self) -> CacheStats {
        let counters = &self.counters;
        CacheStats {
            l1_hits: counters.l1_get_hits.load(Ordering::Relaxed),
            l1_misses: counters.l1_get_misses.load(Ordering::Relaxed),
            l2_hits: counters.l2_get_hits.load(Ordering::Relaxed),
            l2_misses: counters.l2_get_misses.load(Ordering::Relaxed),
            l1_sets: counters.l1_set_total.load(Ordering::Relaxed),
            l2_sets: counters.l2_set_total.load(Ordering::Relaxed),
            l1_deletes: counters.l1_delete_total.load(Ordering::Relaxed),
            l2_deletes: counters.l2_delete_total.load(Ordering::Relaxed),
            total_operations: counters.total_operations.load(Ordering::Relaxed),
            l1_item_count: counters.l1_items.load(Ordering::Relaxed),
            l1_capacity_used: counters.l1_capacity_used.load(Ordering::Relaxed),
            prefetch_count: counters.prefetch_total.load(Ordering::Relaxed),
            compression_count: counters.compression_total.load(Ordering::Relaxed),
            compression_bytes_saved: counters.compression_bytes_saved.load(Ordering::Relaxed),
            timestamp: chrono::Utc::now(),
        }
    }

    /// 重置所有统计
    pub fn reset(&self) {
        let counters = &self.counters;
        counters.l1_get_hits.store(0, Ordering::Relaxed);
        counters.l1_get_misses.store(0, Ordering::Relaxed);
        counters.l2_get_hits.store(0, Ordering::Relaxed);
        counters.l2_get_misses.store(0, Ordering::Relaxed);
        counters.l1_set_total.store(0, Ordering::Relaxed);
        counters.l2_set_total.store(0, Ordering::Relaxed);
        counters.l1_delete_total.store(0, Ordering::Relaxed);
        counters.l2_delete_total.store(0, Ordering::Relaxed);
        counters.total_operations.store(0, Ordering::Relaxed);
        counters.l1_items.store(0, Ordering::Relaxed);
        counters.l1_capacity_used.store(0, Ordering::Relaxed);
        counters.prefetch_total.store(0, Ordering::Relaxed);
        counters.compression_total.store(0, Ordering::Relaxed);
        counters.compression_bytes_saved.store(0, Ordering::Relaxed);

        // 清空 DashMap
        self.requests_total.clear();
        self.operation_duration.clear();
        self.batch_buffer_size.clear();
        self.batch_success_rate.clear();
        self.batch_throughput.clear();
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let counters = &self.counters;
        let hits = counters.l1_get_hits.load(Ordering::Relaxed) + counters.l2_get_hits.load(Ordering::Relaxed);
        let misses = counters.l1_get_misses.load(Ordering::Relaxed) + counters.l2_get_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            1.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 获取命中率百分比
    pub fn hit_rate_percent(&self) -> String {
        format!("{:.2}%", self.hit_rate() * 100.0)
    }

    /// 记录预取操作
    pub fn record_prefetch(&self) {
        self.counters.prefetch_total.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录压缩操作
    pub fn record_compression(&self, bytes_saved: u64) {
        self.counters.compression_total.fetch_add(1, Ordering::Relaxed);
        self.counters
            .compression_bytes_saved
            .fetch_add(bytes_saved, Ordering::Relaxed);
    }

    /// 设置 L1 缓存项数量
    pub fn set_l1_item_count(&self, count: u64) {
        self.counters.l1_items.store(count, Ordering::Relaxed);
    }

    /// 设置 L1 容量使用
    pub fn set_l1_capacity_used(&self, bytes: u64) {
        self.counters.l1_capacity_used.store(bytes, Ordering::Relaxed);
    }

    /// 导出 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        self.snapshot().export_prometheus()
    }

    /// 导出 JSON 格式
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().export_json()
    }
}

// ============================================================================
// Unified Metrics Exports
// ============================================================================

// Re-export unified metrics
pub use unified::{
    convenience as unified_convenience, CacheOpResult, CacheOpType, CacheOperation, CounterSnapshot, HistogramData,
    HitRates, MetricValue, MetricsConfig, MetricsSnapshot, TimerData, GLOBAL_UNIFIED_METRICS,
};

// 从 core::types 重新导出 CacheLayer
pub use crate::core::types::CacheLayer;
