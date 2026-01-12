//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的指标收集和监控功能。
//! 通过 `metrics` 或 `l1-moka` feature 控制启用/禁用。

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{span, Level};

/// 原子计数器集合
///
/// 使用原子操作实现无锁的指标计数，大幅提升性能
#[derive(Debug)]
pub struct AtomicCounters {
    /// L1缓存命中次数
    pub l1_get_hits: AtomicU64,
    /// L1缓存未命中次数
    pub l1_get_misses: AtomicU64,
    /// L2缓存命中次数
    pub l2_get_hits: AtomicU64,
    /// L2缓存未命中次数
    pub l2_get_misses: AtomicU64,
    /// L1缓存设置次数
    pub l1_set_total: AtomicU64,
    /// L2缓存设置次数
    pub l2_set_total: AtomicU64,
    /// L1缓存删除次数
    pub l1_delete_total: AtomicU64,
    /// L2缓存删除次数
    pub l2_delete_total: AtomicU64,
    /// 总操作次数
    pub total_operations: AtomicU64,
}

impl Default for AtomicCounters {
    fn default() -> Self {
        Self {
            l1_get_hits: AtomicU64::new(0),
            l1_get_misses: AtomicU64::new(0),
            l2_get_hits: AtomicU64::new(0),
            l2_get_misses: AtomicU64::new(0),
            l1_set_total: AtomicU64::new(0),
            l2_set_total: AtomicU64::new(0),
            l1_delete_total: AtomicU64::new(0),
            l2_delete_total: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
        }
    }
}

/// 指标收集器
///
/// 用于收集和存储缓存系统的各种运行时指标
/// 优化版本：高频指标使用原子计数器，低频指标使用DashMap
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// 原子计数器（高频指标，无锁）
    pub counters: Arc<AtomicCounters>,
    /// 请求总数统计（低频指标，保留DashMap用于动态服务名）
    /// key: "service:layer:op:result"
    pub requests_total: Arc<DashMap<String, u64>>,
    /// L2健康状态
    pub l2_health_status: Arc<DashMap<String, u8>>,
    /// WAL条目数
    pub wal_entries: Arc<DashMap<String, usize>>,
    /// 操作耗时（简单的累积时间和计数，用于计算平均值，更复杂的直方图建议使用OpenTelemetry Metrics）
    /// key: "service:layer:op" -> (total_duration_secs, count)
    pub operation_duration: Arc<DashMap<String, (f64, u64)>>,
    /// 批量写入缓冲区大小
    pub batch_buffer_size: Arc<DashMap<String, usize>>,
    /// 批量写入成功率
    pub batch_success_rate: Arc<DashMap<String, f64>>,
    /// 批量写入吞吐量 (ops/sec)
    pub batch_throughput: Arc<DashMap<String, f64>>,
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
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L1", "get", "miss") => {
                self.counters.l1_get_misses.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "get", "hit") => {
                self.counters.l2_get_hits.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "get", "miss") => {
                self.counters.l2_get_misses.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L1", "set", "attempt") => {
                self.counters.l1_set_total.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "set", "attempt") => {
                self.counters.l2_set_total.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L1", "delete", "attempt") => {
                self.counters
                    .l1_delete_total
                    .fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            ("L2", "delete", "attempt") => {
                self.counters
                    .l2_delete_total
                    .fetch_add(1, Ordering::Relaxed);
                self.counters
                    .total_operations
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            _ => {}
        }

        // 其他操作使用DashMap（无锁）
        let key = format!("{}:{}:{}:{}", service, layer, op, result);
        self.requests_total
            .entry(key)
            .and_modify(|v| *v += 1)
            .or_insert(1);
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
        self.batch_throughput
            .insert(service.to_string(), throughput);
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
#[cfg(any(feature = "metrics", feature = "l1-moka"))]
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
        output.push_str(&format!(
            "cache_requests_total{{labels=\"{}\"}} {}\n",
            key, value
        ));
    }

    let health_status: &DashMap<String, u8> = &metrics.l2_health_status;
    for entry in health_status.iter() {
        let (key, value): (&String, &u8) = entry.pair();
        output.push_str(&format!(
            "cache_l2_health_status{{service=\"{}\"}} {}\n",
            key, value
        ));
    }

    let wal_entries: &DashMap<String, usize> = &metrics.wal_entries;
    for entry in wal_entries.iter() {
        let (key, value): (&String, &usize) = entry.pair();
        output.push_str(&format!(
            "cache_wal_entries{{service=\"{}\"}} {}\n",
            key, value
        ));
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
                            service,
                            layer,
                            op,
                            avg_duration
                        ));
            }
        }
    }

    let buffer_sizes: &DashMap<String, usize> = &metrics.batch_buffer_size;
    for entry in buffer_sizes.iter() {
        let (key, value): (&String, &usize) = entry.pair();
        output.push_str(&format!(
            "cache_batch_buffer_size{{service=\"{}\"}} {}\n",
            key, value
        ));
    }

    let success_rates: &DashMap<String, f64> = &metrics.batch_success_rate;
    for entry in success_rates.iter() {
        let (key, value): (&String, &f64) = entry.pair();
        output.push_str(&format!(
            "cache_batch_success_rate{{service=\"{}\"}} {}\n",
            key, value
        ));
    }

    let throughputs: &DashMap<String, f64> = &metrics.batch_throughput;
    for entry in throughputs.iter() {
        let (key, value): (&String, &f64) = entry.pair();
        output.push_str(&format!(
            "cache_batch_throughput{{service=\"{}\"}} {}\n",
            key, value
        ));
    }

    output
}

/// 当 metrics 和 l1-moka 功能都禁用时的空实现
#[cfg(not(any(feature = "metrics", feature = "l1-moka")))]
#[derive(Debug, Clone, Default)]
pub struct Metrics;

#[cfg(not(any(feature = "metrics", feature = "l1-moka")))]
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

#[cfg(not(any(feature = "metrics", feature = "l1-moka")))]
lazy_static! {
    /// 全局空指标实例
    pub static ref GLOBAL_METRICS: Metrics = Metrics;
}

#[cfg(not(any(feature = "metrics", feature = "l1-moka")))]
/// 当 metrics 功能禁用时返回空字符串
pub fn get_metrics_string() -> String {
    String::new()
}
