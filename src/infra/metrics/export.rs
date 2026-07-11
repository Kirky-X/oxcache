// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 指标导出函数

use crate::infra::metrics::{CacheStats, GLOBAL_UNIFIED_METRICS};

/// 获取增强统计快照（全局）
#[cfg(feature = "metrics")]
pub fn get_enhanced_stats() -> CacheStats {
    GLOBAL_UNIFIED_METRICS.snapshot().into()
}

/// 导出 Prometheus 格式（全局）
#[cfg(feature = "metrics")]
pub fn export_prometheus_format() -> String {
    GLOBAL_UNIFIED_METRICS.export_prometheus()
}

/// 导出 JSON 格式（全局）
#[cfg(feature = "metrics")]
pub fn export_json_format() -> Result<String, serde_json::Error> {
    GLOBAL_UNIFIED_METRICS.export_json()
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    use super::*;
    use crate::infra::metrics::unified::convenience;

    #[test]
    fn test_get_enhanced_stats_returns_cache_stats() {
        convenience::reset();
        let stats = get_enhanced_stats();
        // After reset, all counters should be 0
        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l1_misses, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.l2_misses, 0);
        assert_eq!(stats.total_operations, 0);
    }

    #[test]
    fn test_export_prometheus_format_returns_valid_output() {
        convenience::reset();
        let prom = export_prometheus_format();
        // Should contain the standard Prometheus header
        assert!(prom.contains("# Cache Metrics Snapshot"));
        // Should contain counter lines
        assert!(prom.contains("cache_l1_hits_total"));
        assert!(prom.contains("cache_l1_misses_total"));
        assert!(prom.contains("cache_l2_hits_total"));
        assert!(prom.contains("cache_l2_misses_total"));
        assert!(prom.contains("cache_operations_total"));
        assert!(prom.contains("cache_errors_total"));
    }

    #[test]
    fn test_export_json_format_returns_valid_json() {
        convenience::reset();
        let json = export_json_format().unwrap();
        // Should be valid JSON containing expected fields
        assert!(json.contains("counters"));
        assert!(json.contains("l1_hits"));
        assert!(json.contains("l1_misses"));
        assert!(json.contains("l2_hits"));
        assert!(json.contains("l2_misses"));
        assert!(json.contains("total_operations"));
        assert!(json.contains("dynamic_metrics"));
    }

    #[test]
    fn test_export_json_format_is_pretty_printed() {
        let json = export_json_format().unwrap();
        // Pretty-printed JSON contains newlines and indentation
        assert!(json.contains("\n"));
        assert!(json.contains("  "));
    }
}
