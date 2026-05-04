//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 指标导出函数

use crate::infra::metrics::{CacheStats, GLOBAL_METRICS};

/// 获取增强统计快照（全局）
#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
pub fn get_enhanced_stats() -> CacheStats {
    GLOBAL_METRICS.snapshot()
}

/// 导出 Prometheus 格式（全局）
#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
pub fn export_prometheus_format() -> String {
    GLOBAL_METRICS.export_prometheus()
}

/// 导出 JSON 格式（全局）
#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
pub fn export_json_format() -> Result<String, serde_json::Error> {
    GLOBAL_METRICS.export_json()
}
