//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存统计快照

use serde::Serialize;

#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
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

#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
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

#[cfg(any(feature = "enhanced-stats", feature = "metrics"))]
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
