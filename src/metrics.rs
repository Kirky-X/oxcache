//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的指标收集和监控功能。

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{span, Level};

/// 指标收集器
///
/// 用于收集和存储缓存系统的各种运行时指标
#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// 请求总数统计
    /// key: "service:layer:op:result"
    pub requests_total: Arc<Mutex<HashMap<String, u64>>>,
    /// L2健康状态
    pub l2_health_status: Arc<Mutex<HashMap<String, u8>>>,
    /// WAL条目数
    pub wal_entries: Arc<Mutex<HashMap<String, usize>>>,
    /// 操作耗时（简单的累积时间和计数，用于计算平均值，更复杂的直方图建议使用OpenTelemetry Metrics）
    /// key: "service:layer:op" -> (total_duration_secs, count)
    pub operation_duration: Arc<Mutex<HashMap<String, (f64, u64)>>>,
    /// 批量写入缓冲区大小
    pub batch_buffer_size: Arc<Mutex<HashMap<String, usize>>>,
}

lazy_static! {
    /// 全局指标实例
    pub static ref GLOBAL_METRICS: Metrics = Metrics::default();
}

impl Metrics {
    /// 记录请求指标
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
        let key = format!("{}:{}:{}:{}", service, layer, op, result);
        let mut map = self.requests_total.lock().unwrap();
        *map.entry(key).or_insert(0) += 1;
    }

    /// 记录操作耗时
    pub fn record_duration(&self, service: &str, layer: &str, op: &str, duration_secs: f64) {
        let key = format!("{}:{}:{}", service, layer, op);
        let mut map = self.operation_duration.lock().unwrap();
        let entry = map.entry(key).or_insert((0.0, 0));
        entry.0 += duration_secs;
        entry.1 += 1;
    }

    /// 设置健康状态
    ///
    /// # 参数
    ///
    /// * `service` - 服务名称
    /// * `status` - 健康状态码（0: 不健康, 1: 健康, 2: 恢复中）
    pub fn set_health(&self, service: &str, status: u8) {
        let mut map = self.l2_health_status.lock().unwrap();
        map.insert(service.to_string(), status);
    }

    /// 设置WAL大小
    ///
    /// # 参数
    ///
    /// * `service` - 服务名称
    /// * `size` - WAL条目数量
    pub fn set_wal_size(&self, service: &str, size: usize) {
        let mut map = self.wal_entries.lock().unwrap();
        map.insert(service.to_string(), size);
    }

    /// 设置批量写入缓冲区大小
    pub fn set_batch_buffer_size(&self, service: &str, size: usize) {
        let mut map = self.batch_buffer_size.lock().unwrap();
        map.insert(service.to_string(), size);
    }
}

/// 获取指标字符串
///
/// 将所有指标格式化为字符串返回，用于监控系统采集
///
/// # 返回值
///
/// 返回包含所有指标的字符串
pub fn get_metrics_string() -> String {
    let metrics = &GLOBAL_METRICS;
    let reqs = metrics.requests_total.lock().unwrap();
    let health = metrics.l2_health_status.lock().unwrap();
    let wal = metrics.wal_entries.lock().unwrap();
    let dur = metrics.operation_duration.lock().unwrap();
    let batch = metrics.batch_buffer_size.lock().unwrap();

    let mut output = String::new();
    for (k, v) in reqs.iter() {
        output.push_str(&format!("cache_requests_total{{labels=\"{}\"}} {}\n", k, v));
    }
    for (k, v) in health.iter() {
        output.push_str(&format!(
            "cache_l2_health_status{{service=\"{}\"}} {}\n",
            k, v
        ));
    }
    for (k, v) in wal.iter() {
        output.push_str(&format!("cache_wal_entries{{service=\"{}\"}} {}\n", k, v));
    }
    for (k, (total, count)) in dur.iter() {
        let parts: Vec<&str> = k.split(':').collect();
        if parts.len() == 3 {
            output.push_str(&format!(
                "cache_operation_duration_seconds_sum{{service=\"{}\", layer=\"{}\", operation=\"{}\"}} {}\n",
                parts[0], parts[1], parts[2], total
            ));
            output.push_str(&format!(
                "cache_operation_duration_seconds_count{{service=\"{}\", layer=\"{}\", operation=\"{}\"}} {}\n",
                parts[0], parts[1], parts[2], count
            ));
        }
    }
    for (k, v) in batch.iter() {
        output.push_str(&format!(
            "cache_batch_write_buffer_size{{service=\"{}\"}} {}\n",
            k, v
        ));
    }
    output
}
