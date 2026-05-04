//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存统计快照

use serde::Serialize;

/// 缓存统计快照
#[derive(Debug, Clone, Serialize)]
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
        output.push_str("# Cache Statistics\n");
        output.push_str(&format!("cache_l1_hits_total {}\n", self.l1_hits));
        output.push_str(&format!("cache_l1_misses_total {}\n", self.l1_misses));
        output.push_str(&format!("cache_l2_hits_total {}\n", self.l2_hits));
        output.push_str(&format!("cache_l2_misses_total {}\n", self.l2_misses));
        output.push_str(&format!("cache_l1_hit_rate {}\n", self.l1_hit_rate()));
        output.push_str(&format!("cache_l2_hit_rate {}\n", self.l2_hit_rate()));
        output.push_str(&format!("cache_overall_hit_rate {}\n", self.overall_hit_rate()));
        output.push_str(&format!("cache_l1_item_count {}\n", self.l1_item_count));
        output.push_str(&format!("cache_l1_capacity_used_bytes {}\n", self.l1_capacity_used));
        output
    }

    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
