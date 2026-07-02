//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存统计快照

use serde::Serialize;

#[cfg(feature = "metrics")]
use crate::infra::metrics::unified::MetricsSnapshot;

/// 缓存统计快照
#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l1_sets: u64,
    pub l2_sets: u64,
    pub l1_deletes: u64,
    pub l2_deletes: u64,
    pub total_operations: u64,
    pub l1_item_count: u64,
    pub l1_capacity_used: u64,
    pub prefetch_count: u64,
    pub compression_count: u64,
    pub compression_bytes_saved: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(feature = "metrics")]
impl From<MetricsSnapshot> for CacheStats {
    fn from(snapshot: MetricsSnapshot) -> Self {
        Self {
            l1_hits: snapshot.counters.l1_hits,
            l1_misses: snapshot.counters.l1_misses,
            l2_hits: snapshot.counters.l2_hits,
            l2_misses: snapshot.counters.l2_misses,
            l1_sets: snapshot.counters.l1_sets,
            l2_sets: snapshot.counters.l2_sets,
            l1_deletes: snapshot.counters.l1_deletes,
            l2_deletes: snapshot.counters.l2_deletes,
            total_operations: snapshot.counters.total_operations,
            l1_item_count: 0,
            l1_capacity_used: 0,
            prefetch_count: snapshot.counters.prefetch_total,
            compression_count: snapshot.counters.compression_total,
            compression_bytes_saved: snapshot.counters.compression_bytes_saved,
            #[cfg(feature = "chrono")]
            timestamp: snapshot.timestamp,
            #[cfg(not(feature = "chrono"))]
            timestamp: chrono::DateTime::from_timestamp(snapshot.timestamp as i64, 0).unwrap_or_else(chrono::Utc::now),
        }
    }
}

#[cfg(feature = "metrics")]
impl CacheStats {
    pub fn l1_hit_rate(&self) -> f64 {
        let total = self.l1_hits + self.l1_misses;
        if total == 0 {
            0.0
        } else {
            self.l1_hits as f64 / total as f64
        }
    }

    pub fn l2_hit_rate(&self) -> f64 {
        let total = self.l2_hits + self.l2_misses;
        if total == 0 {
            0.0
        } else {
            self.l2_hits as f64 / total as f64
        }
    }

    pub fn overall_hit_rate(&self) -> f64 {
        let total = self.l1_hits + self.l1_misses + self.l2_hits + self.l2_misses;
        if total == 0 {
            0.0
        } else {
            (self.l1_hits + self.l2_hits) as f64 / total as f64
        }
    }

    pub fn l1_hit_rate_percent(&self) -> String {
        format!("{:.2}%", self.l1_hit_rate() * 100.0)
    }
    pub fn l2_hit_rate_percent(&self) -> String {
        format!("{:.2}%", self.l2_hit_rate() * 100.0)
    }
    pub fn overall_hit_rate_percent(&self) -> String {
        format!("{:.2}%", self.overall_hit_rate() * 100.0)
    }

    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        output.push_str("# Cache Metrics Snapshot\n");
        output.push_str(&format!("# Generated at: {}\n", self.timestamp));

        // Export counters
        output.push_str(&format!("cache_l1_hits_total {}\n", self.l1_hits));
        output.push_str(&format!("cache_l1_misses_total {}\n", self.l1_misses));
        output.push_str(&format!("cache_l2_hits_total {}\n", self.l2_hits));
        output.push_str(&format!("cache_l2_misses_total {}\n", self.l2_misses));
        output.push_str(&format!("cache_l1_sets_total {}\n", self.l1_sets));
        output.push_str(&format!("cache_l2_sets_total {}\n", self.l2_sets));
        output.push_str(&format!("cache_l1_deletes_total {}\n", self.l1_deletes));
        output.push_str(&format!("cache_l2_deletes_total {}\n", self.l2_deletes));
        output.push_str(&format!("cache_operations_total {}\n", self.total_operations));

        // Export hit rates
        output.push_str(&format!("cache_l1_hit_rate {}\n", self.l1_hit_rate()));
        output.push_str(&format!("cache_l2_hit_rate {}\n", self.l2_hit_rate()));
        output.push_str(&format!("cache_overall_hit_rate {}\n", self.overall_hit_rate()));

        // Export gauges
        output.push_str(&format!("cache_l1_item_count {}\n", self.l1_item_count));
        output.push_str(&format!("cache_l1_capacity_used_bytes {}\n", self.l1_capacity_used));

        // Export extended metrics
        output.push_str(&format!("cache_prefetch_total {}\n", self.prefetch_count));
        output.push_str(&format!("cache_compression_total {}\n", self.compression_count));
        output.push_str(&format!(
            "cache_compression_bytes_saved {}\n",
            self.compression_bytes_saved
        ));

        output
    }

    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    use super::*;
    use crate::core::types::CacheLayer;
    use crate::infra::metrics::unified::{CacheOpResult, CacheOpType, CacheOperation, UnifiedMetrics};

    #[test]
    fn test_cache_stats_default_all_zero() {
        let stats = CacheStats::default();
        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l1_misses, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.l2_misses, 0);
        assert_eq!(stats.l1_sets, 0);
        assert_eq!(stats.l2_sets, 0);
        assert_eq!(stats.l1_deletes, 0);
        assert_eq!(stats.l2_deletes, 0);
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.l1_item_count, 0);
        assert_eq!(stats.l1_capacity_used, 0);
        assert_eq!(stats.prefetch_count, 0);
        assert_eq!(stats.compression_count, 0);
        assert_eq!(stats.compression_bytes_saved, 0);
    }

    #[test]
    fn test_hit_rates_zero_when_no_ops() {
        let stats = CacheStats::default();
        assert_eq!(stats.l1_hit_rate(), 0.0);
        assert_eq!(stats.l2_hit_rate(), 0.0);
        assert_eq!(stats.overall_hit_rate(), 0.0);
        assert_eq!(stats.l1_hit_rate_percent(), "0.00%");
        assert_eq!(stats.l2_hit_rate_percent(), "0.00%");
        assert_eq!(stats.overall_hit_rate_percent(), "0.00%");
    }

    #[test]
    fn test_l1_hit_rate() {
        let stats = CacheStats { l1_hits: 7, l1_misses: 3, ..Default::default() };
        assert_eq!(stats.l1_hit_rate(), 0.7);
        assert_eq!(stats.l1_hit_rate_percent(), "70.00%");
    }

    #[test]
    fn test_l2_hit_rate() {
        let stats = CacheStats { l2_hits: 4, l2_misses: 6, ..Default::default() };
        assert_eq!(stats.l2_hit_rate(), 0.4);
        assert_eq!(stats.l2_hit_rate_percent(), "40.00%");
    }

    #[test]
    fn test_overall_hit_rate() {
        let stats = CacheStats { l1_hits: 7, l1_misses: 3, l2_hits: 4, l2_misses: 6, ..Default::default() };
        // total hits = 11, total = 20
        assert_eq!(stats.overall_hit_rate(), 0.55);
        assert_eq!(stats.overall_hit_rate_percent(), "55.00%");
    }

    #[test]
    fn test_l1_hit_rate_all_hits() {
        let stats = CacheStats { l1_hits: 10, l1_misses: 0, ..Default::default() };
        assert_eq!(stats.l1_hit_rate(), 1.0);
        assert_eq!(stats.l1_hit_rate_percent(), "100.00%");
    }

    #[test]
    fn test_export_prometheus_contains_all_counters() {
        let stats = CacheStats {
            l1_hits: 5,
            l1_misses: 2,
            l2_hits: 3,
            l2_misses: 1,
            l1_sets: 4,
            l2_sets: 2,
            l1_deletes: 1,
            l2_deletes: 1,
            total_operations: 19,
            l1_item_count: 100,
            l1_capacity_used: 4096,
            prefetch_count: 7,
            compression_count: 3,
            compression_bytes_saved: 2048,
            ..Default::default()
        };

        let prom = stats.export_prometheus();
        assert!(prom.contains("# Cache Metrics Snapshot"));
        assert!(prom.contains("cache_l1_hits_total 5"));
        assert!(prom.contains("cache_l1_misses_total 2"));
        assert!(prom.contains("cache_l2_hits_total 3"));
        assert!(prom.contains("cache_l2_misses_total 1"));
        assert!(prom.contains("cache_l1_sets_total 4"));
        assert!(prom.contains("cache_l2_sets_total 2"));
        assert!(prom.contains("cache_l1_deletes_total 1"));
        assert!(prom.contains("cache_l2_deletes_total 1"));
        assert!(prom.contains("cache_operations_total 19"));
        assert!(prom.contains("cache_l1_hit_rate"));
        assert!(prom.contains("cache_l2_hit_rate"));
        assert!(prom.contains("cache_overall_hit_rate"));
        assert!(prom.contains("cache_l1_item_count 100"));
        assert!(prom.contains("cache_l1_capacity_used_bytes 4096"));
        assert!(prom.contains("cache_prefetch_total 7"));
        assert!(prom.contains("cache_compression_total 3"));
        assert!(prom.contains("cache_compression_bytes_saved 2048"));
    }

    #[test]
    fn test_export_json_serializes_all_fields() {
        let stats = CacheStats { l1_hits: 5, l1_misses: 2, l2_hits: 3, total_operations: 10, ..Default::default() };

        let json = stats.export_json().unwrap();
        assert!(json.contains("\"l1_hits\": 5"));
        assert!(json.contains("\"l1_misses\": 2"));
        assert!(json.contains("\"l2_hits\": 3"));
        assert!(json.contains("\"total_operations\": 10"));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_from_metrics_snapshot() {
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Set,
            result: CacheOpResult::Success,
        });

        let snapshot = metrics.snapshot();
        let stats: CacheStats = snapshot.into();
        assert_eq!(stats.l1_hits, 1);
        assert_eq!(stats.l2_sets, 1);
        // l1_item_count and l1_capacity_used are hardcoded to 0 in conversion
        assert_eq!(stats.l1_item_count, 0);
        assert_eq!(stats.l1_capacity_used, 0);
        assert_eq!(stats.total_operations, 2);
    }

    #[test]
    fn test_from_metrics_snapshot_preserves_all_counters() {
        let metrics = UnifiedMetrics::new();
        // L1 hit, L1 miss, L2 hit, L2 miss
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });
        // L1 delete, L2 delete
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Delete,
            result: CacheOpResult::Success,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Delete,
            result: CacheOpResult::Success,
        });

        let snapshot = metrics.snapshot();
        let stats: CacheStats = snapshot.into();
        assert_eq!(stats.l1_hits, 1);
        assert_eq!(stats.l1_misses, 1);
        assert_eq!(stats.l2_hits, 1);
        assert_eq!(stats.l2_misses, 1);
        assert_eq!(stats.l1_deletes, 1);
        assert_eq!(stats.l2_deletes, 1);
        assert_eq!(stats.total_operations, 6);
    }

    #[test]
    fn test_cache_stats_clone() {
        let stats = CacheStats { l1_hits: 42, ..Default::default() };
        let cloned = stats.clone();
        assert_eq!(cloned.l1_hits, 42);
    }

    #[test]
    fn test_cache_stats_debug() {
        let stats = CacheStats::default();
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("CacheStats"));
        assert!(debug_str.contains("l1_hits"));
    }
}
