// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 指标导出函数

use crate::infra::{CacheStats, GLOBAL_UNIFIED_METRICS};

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

/// 导出标准 Prometheus exposition 格式（全局，T302：`# HELP`/`# TYPE` + `oxcache_*` 命名）
#[cfg(feature = "metrics")]
pub fn export_prometheus_standard() -> String {
    GLOBAL_UNIFIED_METRICS.export_prometheus_standard()
}

/// 导出 JSON 格式（全局）
#[cfg(feature = "metrics")]
pub fn export_json_format() -> Result<String, serde_json::Error> {
    GLOBAL_UNIFIED_METRICS.export_json()
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    use super::*;
    use crate::infra::convenience;

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
    fn test_export_prometheus_standard_is_compliant() {
        convenience::reset();
        let prom = export_prometheus_standard();
        // 标准 exposition 头：每个指标族必须有 HELP/TYPE
        assert!(prom.contains("# HELP oxcache_hits_total "));
        assert!(prom.contains("# TYPE oxcache_hits_total counter\n"));
        assert!(prom.contains("# HELP oxcache_misses_total "));
        assert!(prom.contains("# TYPE oxcache_misses_total counter\n"));
        assert!(prom.contains("# HELP oxcache_evictions_total "));
        assert!(prom.contains("# TYPE oxcache_evictions_total counter\n"));
        assert!(prom.contains("# HELP oxcache_operation_duration_seconds "));
        assert!(prom.contains("# TYPE oxcache_operation_duration_seconds histogram\n"));
        // 标准命名计数行（含 layer 标签）
        assert!(prom.contains("oxcache_hits_total{layer=\"l1\"} "));
        assert!(prom.contains("oxcache_misses_total{layer=\"l1\"} "));
        assert!(prom.contains("oxcache_evictions_total "));
        // 直方图三件套：bucket/sum/count
        assert!(prom.contains("oxcache_operation_duration_seconds_bucket{le="));
        assert!(prom.contains("oxcache_operation_duration_seconds_bucket{le=\"+Inf\"} "));
        assert!(prom.contains("oxcache_operation_duration_seconds_sum "));
        assert!(prom.contains("oxcache_operation_duration_seconds_count "));
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
