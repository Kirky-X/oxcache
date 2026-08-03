// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Metrics impl blocks extracted from mod.rs

// 当 metrics 和 memory 功能都禁用时的空实现
#[cfg(not(any(feature = "metrics", feature = "memory")))]
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

/// 全局空指标实例
#[cfg(not(any(feature = "metrics", feature = "memory")))]
pub static GLOBAL_METRICS: ::once_cell::sync::Lazy<Metrics> = ::once_cell::sync::Lazy::new(|| Metrics);

#[cfg(not(any(feature = "metrics", feature = "memory")))]
/// 当 metrics 功能禁用时返回空字符串
pub fn get_metrics_string() -> String {
    String::new()
}
