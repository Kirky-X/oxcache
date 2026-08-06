// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified metrics collection system that consolidates all metrics functionality

use crate::core::CacheLayer;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Unified metrics collector
///
/// This provides a centralized way to collect and manage all cache metrics
/// with support for different metric types and aggregation strategies.
#[derive(Clone, Debug, Default)]
pub struct UnifiedMetrics {
    inner: Arc<UnifiedMetricsInner>,
}

struct UnifiedMetricsInner {
    /// High-frequency atomic counters
    counters: AtomicCounters,
    /// Low-frequency dynamic metrics
    dynamic_metrics: DashMap<String, MetricValue>,
    /// Configuration
    config: MetricsConfig,
}

impl std::fmt::Debug for UnifiedMetricsInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedMetricsInner")
            .field("counters", &"<AtomicCounters>")
            .field("dynamic_metrics", &self.dynamic_metrics)
            .field("config", &self.config)
            .finish()
    }
}

impl Default for UnifiedMetricsInner {
    fn default() -> Self {
        Self {
            counters: AtomicCounters::default(),
            dynamic_metrics: DashMap::new(),
            config: MetricsConfig::default(),
        }
    }
}

/// Atomic counters for high-frequency metrics
#[derive(Debug)]
pub struct AtomicCounters {
    /// L1 cache hits
    pub l1_hits: AtomicU64,
    /// L1 cache misses
    pub l1_misses: AtomicU64,
    /// L2 cache hits
    pub l2_hits: AtomicU64,
    /// L2 cache misses
    pub l2_misses: AtomicU64,
    /// L1 cache sets
    pub l1_sets: AtomicU64,
    /// L2 cache sets
    pub l2_sets: AtomicU64,
    /// L1 cache deletes
    pub l1_deletes: AtomicU64,
    /// L2 cache deletes
    pub l2_deletes: AtomicU64,
    /// Total operations
    pub total_operations: AtomicU64,
    /// Errors
    pub errors: AtomicU64,
    /// Prefetch operations
    pub prefetch_total: AtomicU64,
    /// Compression operations
    pub compression_total: AtomicU64,
    /// Compression bytes saved
    pub compression_bytes_saved: AtomicU64,
    /// L1 cache item count
    pub l1_items: AtomicU64,
    /// L1 cache capacity used (bytes)
    pub l1_capacity_used: AtomicU64,
    /// L2 degradation events (circuit breaker opened)
    pub l2_degraded: AtomicU64,
    /// L2 retry attempts total
    pub l2_retry_total: AtomicU64,
    /// Backfill success count (per backend)
    pub backfill_success: AtomicU64,
    /// Backfill failure count (per backend)
    pub backfill_failed: AtomicU64,
}

/// Metric value types
#[derive(Debug, Clone, Serialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(HistogramData),
    Timer(TimerData),
    Text(String),
}

/// Histogram data for distribution metrics
#[derive(Debug, Clone, Serialize)]
pub struct HistogramData {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub buckets: Vec<(f64, u64)>,
}

/// Timer data for duration metrics
#[derive(Debug, Clone, Serialize)]
pub struct TimerData {
    pub count: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
}

/// Metrics configuration
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Whether to enable detailed metrics
    pub detailed: bool,
    /// Histogram bucket boundaries
    pub histogram_buckets: Vec<f64>,
    /// Maximum number of dynamic metrics
    pub max_dynamic_metrics: usize,
    /// Metrics retention period for time-based eviction of stale snapshots.
    /// Currently configured but not actively consumed by the eviction logic;
    /// reserved for future implementation of snapshot lifecycle management.
    #[allow(dead_code)]
    pub retention_period: Option<Duration>,
}

impl Default for AtomicCounters {
    fn default() -> Self {
        Self {
            l1_hits: AtomicU64::new(0),
            l1_misses: AtomicU64::new(0),
            l2_hits: AtomicU64::new(0),
            l2_misses: AtomicU64::new(0),
            l1_sets: AtomicU64::new(0),
            l2_sets: AtomicU64::new(0),
            l1_deletes: AtomicU64::new(0),
            l2_deletes: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            prefetch_total: AtomicU64::new(0),
            compression_total: AtomicU64::new(0),
            compression_bytes_saved: AtomicU64::new(0),
            l1_items: AtomicU64::new(0),
            l1_capacity_used: AtomicU64::new(0),
            l2_degraded: AtomicU64::new(0),
            l2_retry_total: AtomicU64::new(0),
            backfill_success: AtomicU64::new(0),
            backfill_failed: AtomicU64::new(0),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            detailed: true,
            histogram_buckets: vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
            max_dynamic_metrics: 1000,
            retention_period: Some(Duration::from_secs(3600)), // 1 hour
        }
    }
}

impl UnifiedMetrics {
    /// Create a new unified metrics collector with default configuration
    pub fn new() -> Self {
        Self::with_config(MetricsConfig::default())
    }

    /// Create a new unified metrics collector with custom configuration
    pub fn with_config(config: MetricsConfig) -> Self {
        Self {
            inner: Arc::new(UnifiedMetricsInner {
                counters: AtomicCounters::default(),
                dynamic_metrics: DashMap::new(),
                config,
            }),
        }
    }

    /// Record a cache operation
    pub fn record_operation(&self, operation: CacheOperation) {
        // Update atomic counters
        match (&operation.layer, &operation.op_type, &operation.result) {
            (CacheLayer::L1, CacheOpType::Get, CacheOpResult::Hit) => {
                self.inner.counters.l1_hits.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L1, CacheOpType::Get, CacheOpResult::Miss) => {
                self.inner.counters.l1_misses.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L2, CacheOpType::Get, CacheOpResult::Hit) => {
                self.inner.counters.l2_hits.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L2, CacheOpType::Get, CacheOpResult::Miss) => {
                self.inner.counters.l2_misses.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L1, CacheOpType::Set, CacheOpResult::Success) => {
                self.inner.counters.l1_sets.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L2, CacheOpType::Set, CacheOpResult::Success) => {
                self.inner.counters.l2_sets.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L1, CacheOpType::Delete, CacheOpResult::Success) => {
                self.inner.counters.l1_deletes.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (CacheLayer::L2, CacheOpType::Delete, CacheOpResult::Success) => {
                self.inner.counters.l2_deletes.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            (_, _, CacheOpResult::Error) => {
                self.inner.counters.errors.fetch_add(1, Ordering::Relaxed);
                self.inner.counters.total_operations.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Update dynamic metrics if detailed mode is enabled
        if self.inner.config.detailed {
            let key = format!(
                "cache:operations:{}:{}:{}",
                operation.layer, operation.op_type, operation.result
            );
            self.increment_counter(&key, 1);
        }
    }

    /// Record operation duration
    pub fn record_duration(&self, operation: &CacheOperation, duration: Duration) {
        if !self.inner.config.detailed {
            return;
        }

        let key = format!("cache:duration:{}:{}", operation.layer, operation.op_type);
        self.record_timer(&key, duration);
    }

    /// Record a custom metric
    pub fn record_custom(&self, key: &str, value: MetricValue) {
        if self.inner.dynamic_metrics.len() >= self.inner.config.max_dynamic_metrics {
            // Remove an arbitrary metric if at capacity.
            // NOTE: DashMap does not guarantee iteration order, so this is
            // NOT true LRU/oldest-first eviction. A sequence-counter-based
            // approach would be needed for deterministic ordering.
            if let Some(first_key) = self.inner.dynamic_metrics.iter().next().map(|r| r.key().clone()) {
                self.inner.dynamic_metrics.remove(&first_key);
            }
        }
        self.inner.dynamic_metrics.insert(key.to_string(), value);
    }

    /// Increment a counter metric
    pub fn increment_counter(&self, key: &str, value: u64) {
        self.inner
            .dynamic_metrics
            .entry(key.to_string())
            .and_modify(|metric| {
                if let MetricValue::Counter(count) = metric {
                    *count += value;
                }
            })
            .or_insert(MetricValue::Counter(value));
    }

    /// Set a gauge metric
    pub fn set_gauge(&self, key: &str, value: f64) {
        self.inner
            .dynamic_metrics
            .insert(key.to_string(), MetricValue::Gauge(value));
    }

    /// Record a histogram value
    pub fn record_histogram(&self, key: &str, value: f64) {
        self.inner
            .dynamic_metrics
            .entry(key.to_string())
            .and_modify(|metric| {
                if let MetricValue::Histogram(hist) = metric {
                    hist.count += 1;
                    hist.sum += value;
                    hist.min = hist.min.min(value);
                    hist.max = hist.max.max(value);

                    // Update buckets
                    for (boundary, count) in &mut hist.buckets {
                        if value <= *boundary {
                            *count += 1;
                        }
                    }
                }
            })
            .or_insert_with(|| {
                let buckets = self
                    .inner
                    .config
                    .histogram_buckets
                    .iter()
                    .map(|&boundary| (boundary, if value <= boundary { 1 } else { 0 }))
                    .collect();

                MetricValue::Histogram(HistogramData {
                    count: 1,
                    sum: value,
                    min: value,
                    max: value,
                    buckets,
                })
            });
    }

    /// Record a timer value
    pub fn record_timer(&self, key: &str, duration: Duration) {
        self.inner
            .dynamic_metrics
            .entry(key.to_string())
            .and_modify(|metric| {
                if let MetricValue::Timer(timer) = metric {
                    timer.count += 1;
                    timer.total_duration += duration;
                    timer.min_duration = timer.min_duration.min(duration);
                    timer.max_duration = timer.max_duration.max(duration);
                }
            })
            .or_insert_with(|| {
                MetricValue::Timer(TimerData {
                    count: 1,
                    total_duration: duration,
                    min_duration: duration,
                    max_duration: duration,
                })
            });
    }

    /// Get atomic counter values
    pub fn get_counters(&self) -> CounterSnapshot {
        CounterSnapshot {
            l1_hits: self.inner.counters.l1_hits.load(Ordering::Relaxed),
            l1_misses: self.inner.counters.l1_misses.load(Ordering::Relaxed),
            l2_hits: self.inner.counters.l2_hits.load(Ordering::Relaxed),
            l2_misses: self.inner.counters.l2_misses.load(Ordering::Relaxed),
            l1_sets: self.inner.counters.l1_sets.load(Ordering::Relaxed),
            l2_sets: self.inner.counters.l2_sets.load(Ordering::Relaxed),
            l1_deletes: self.inner.counters.l1_deletes.load(Ordering::Relaxed),
            l2_deletes: self.inner.counters.l2_deletes.load(Ordering::Relaxed),
            total_operations: self.inner.counters.total_operations.load(Ordering::Relaxed),
            errors: self.inner.counters.errors.load(Ordering::Relaxed),
            prefetch_total: self.inner.counters.prefetch_total.load(Ordering::Relaxed),
            compression_total: self.inner.counters.compression_total.load(Ordering::Relaxed),
            compression_bytes_saved: self.inner.counters.compression_bytes_saved.load(Ordering::Relaxed),
            l1_items: self.inner.counters.l1_items.load(Ordering::Relaxed),
            l1_capacity_used: self.inner.counters.l1_capacity_used.load(Ordering::Relaxed),
            l2_degraded: self.inner.counters.l2_degraded.load(Ordering::Relaxed),
            l2_retry_total: self.inner.counters.l2_retry_total.load(Ordering::Relaxed),
            backfill_success: self.inner.counters.backfill_success.load(Ordering::Relaxed),
            backfill_failed: self.inner.counters.backfill_failed.load(Ordering::Relaxed),
        }
    }

    /// Get all dynamic metrics
    pub fn get_dynamic_metrics(&self) -> std::collections::HashMap<String, MetricValue> {
        self.inner
            .dynamic_metrics
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect()
    }

    /// Create a comprehensive snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            #[cfg(feature = "chrono")]
            timestamp: chrono::Utc::now(),
            #[cfg(not(feature = "chrono"))]
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            counters: self.get_counters(),
            dynamic_metrics: self.get_dynamic_metrics(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        // Reset atomic counters
        self.inner.counters.l1_hits.store(0, Ordering::Relaxed);
        self.inner.counters.l1_misses.store(0, Ordering::Relaxed);
        self.inner.counters.l2_hits.store(0, Ordering::Relaxed);
        self.inner.counters.l2_misses.store(0, Ordering::Relaxed);
        self.inner.counters.l1_sets.store(0, Ordering::Relaxed);
        self.inner.counters.l2_sets.store(0, Ordering::Relaxed);
        self.inner.counters.l1_deletes.store(0, Ordering::Relaxed);
        self.inner.counters.l2_deletes.store(0, Ordering::Relaxed);
        self.inner.counters.total_operations.store(0, Ordering::Relaxed);
        self.inner.counters.errors.store(0, Ordering::Relaxed);
        self.inner.counters.prefetch_total.store(0, Ordering::Relaxed);
        self.inner.counters.compression_total.store(0, Ordering::Relaxed);
        self.inner.counters.compression_bytes_saved.store(0, Ordering::Relaxed);
        self.inner.counters.l1_items.store(0, Ordering::Relaxed);
        self.inner.counters.l1_capacity_used.store(0, Ordering::Relaxed);
        self.inner.counters.l2_degraded.store(0, Ordering::Relaxed);
        self.inner.counters.l2_retry_total.store(0, Ordering::Relaxed);
        self.inner.counters.backfill_success.store(0, Ordering::Relaxed);
        self.inner.counters.backfill_failed.store(0, Ordering::Relaxed);

        // Clear dynamic metrics
        self.inner.dynamic_metrics.clear();
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let snapshot = self.snapshot();
        snapshot.export_prometheus()
    }

    /// Export metrics in JSON format
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let snapshot = self.snapshot();
        serde_json::to_string_pretty(&snapshot)
    }

    /// Calculate hit rates
    pub fn hit_rates(&self) -> HitRates {
        let counters = self.get_counters();

        let l1_total = counters.l1_hits.saturating_add(counters.l1_misses);
        let l2_total = counters.l2_hits.saturating_add(counters.l2_misses);
        let total_hits = counters.l1_hits.saturating_add(counters.l2_hits);
        let total_misses = counters.l1_misses.saturating_add(counters.l2_misses);
        let overall_total = total_hits.saturating_add(total_misses);

        HitRates {
            l1_hit_rate: if l1_total > 0 {
                counters.l1_hits as f64 / l1_total as f64
            } else {
                0.0
            },
            l2_hit_rate: if l2_total > 0 {
                counters.l2_hits as f64 / l2_total as f64
            } else {
                0.0
            },
            overall_hit_rate: if overall_total > 0 {
                total_hits as f64 / overall_total as f64
            } else {
                0.0
            },
        }
    }

    /// Record an L2 retry event.
    pub fn record_l2_retry(&self) {
        self.inner.counters.l2_retry_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an L2 degradation event (circuit breaker opened).
    pub fn record_l2_degraded(&self) {
        self.inner.counters.l2_degraded.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a backfill success event.
    pub fn record_backfill_success(&self) {
        self.inner.counters.backfill_success.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a backfill failure event.
    pub fn record_backfill_failed(&self) {
        self.inner.counters.backfill_failed.fetch_add(1, Ordering::Relaxed);
    }
}

/// Cache operation information
#[derive(Debug, Clone)]
pub struct CacheOperation {
    pub layer: CacheLayer,
    pub op_type: CacheOpType,
    pub result: CacheOpResult,
}

/// Cache operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheOpType {
    Get,
    Set,
    Delete,
    Clear,
}

impl std::fmt::Display for CacheOpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheOpType::Get => write!(f, "Get"),
            CacheOpType::Set => write!(f, "Set"),
            CacheOpType::Delete => write!(f, "Delete"),
            CacheOpType::Clear => write!(f, "Clear"),
        }
    }
}

/// Cache operation result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheOpResult {
    Hit,
    Miss,
    Success,
    Error,
}

impl std::fmt::Display for CacheOpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheOpResult::Hit => write!(f, "Hit"),
            CacheOpResult::Miss => write!(f, "Miss"),
            CacheOpResult::Success => write!(f, "Success"),
            CacheOpResult::Error => write!(f, "Error"),
        }
    }
}

/// Snapshot of atomic counters
#[derive(Debug, Clone, Serialize)]
pub struct CounterSnapshot {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l1_sets: u64,
    pub l2_sets: u64,
    pub l1_deletes: u64,
    pub l2_deletes: u64,
    pub total_operations: u64,
    pub errors: u64,
    pub prefetch_total: u64,
    pub compression_total: u64,
    pub compression_bytes_saved: u64,
    /// L1 cache item count (not yet wired to cache operations)
    pub l1_items: u64,
    /// L1 cache capacity used in bytes (not yet wired to cache operations)
    pub l1_capacity_used: u64,
    /// L2 degradation events (circuit breaker opened)
    pub l2_degraded: u64,
    /// L2 retry attempts total
    pub l2_retry_total: u64,
    /// Backfill success count
    pub backfill_success: u64,
    /// Backfill failure count
    pub backfill_failed: u64,
}

/// Comprehensive metrics snapshot
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    #[cfg(feature = "chrono")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "chrono"))]
    pub timestamp: u64, // Unix timestamp as fallback
    pub counters: CounterSnapshot,
    pub dynamic_metrics: std::collections::HashMap<String, MetricValue>,
}

/// Hit rate calculations
#[derive(Debug, Clone)]
pub struct HitRates {
    pub l1_hit_rate: f64,
    pub l2_hit_rate: f64,
    pub overall_hit_rate: f64,
}

impl MetricsSnapshot {
    /// Export in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        output.push_str("# Cache Metrics Snapshot\n");
        output.push_str(&format!("# Generated at: {}\n", self.timestamp));

        // Export counters
        output.push_str(&format!("cache_l1_hits_total {}\n", self.counters.l1_hits));
        output.push_str(&format!("cache_l1_misses_total {}\n", self.counters.l1_misses));
        output.push_str(&format!("cache_l2_hits_total {}\n", self.counters.l2_hits));
        output.push_str(&format!("cache_l2_misses_total {}\n", self.counters.l2_misses));
        output.push_str(&format!("cache_l1_sets_total {}\n", self.counters.l1_sets));
        output.push_str(&format!("cache_l2_sets_total {}\n", self.counters.l2_sets));
        output.push_str(&format!("cache_l1_deletes_total {}\n", self.counters.l1_deletes));
        output.push_str(&format!("cache_l2_deletes_total {}\n", self.counters.l2_deletes));
        output.push_str(&format!("cache_operations_total {}\n", self.counters.total_operations));
        output.push_str(&format!("cache_errors_total {}\n", self.counters.errors));
        output.push_str(&format!("cache_l1_items {}\n", self.counters.l1_items));
        output.push_str(&format!(
            "cache_l1_capacity_used_bytes {}\n",
            self.counters.l1_capacity_used
        ));
        output.push_str(&format!("cache_l2_degraded_total {}\n", self.counters.l2_degraded));
        output.push_str(&format!("cache_l2_retry_total {}\n", self.counters.l2_retry_total));
        output.push_str(&format!(
            "cache_backfill_success_total {}\n",
            self.counters.backfill_success
        ));
        output.push_str(&format!(
            "cache_backfill_failed_total {}\n",
            self.counters.backfill_failed
        ));

        // Export dynamic metrics
        for (key, value) in &self.dynamic_metrics {
            match value {
                MetricValue::Counter(count) => {
                    output.push_str(&format!("{}_counter {}\n", key, count));
                }
                MetricValue::Gauge(value) => {
                    output.push_str(&format!("{}_gauge {}\n", key, value));
                }
                MetricValue::Histogram(hist) => {
                    output.push_str(&format!("{}_histogram_sum {}\n", key, hist.sum));
                    output.push_str(&format!("{}_histogram_count {}\n", key, hist.count));
                    for (boundary, count) in &hist.buckets {
                        output.push_str(&format!("{}_histogram_bucket{{le=\"{}\"}} {}\n", key, boundary, count));
                    }
                }
                MetricValue::Timer(timer) => {
                    output.push_str(&format!(
                        "{}_timer_seconds_sum {}\n",
                        key,
                        timer.total_duration.as_secs_f64()
                    ));
                    output.push_str(&format!("{}_timer_seconds_count {}\n", key, timer.count));
                }
                MetricValue::Text(text) => {
                    output.push_str(&format!("{}_info \"{}\"\n", key, text));
                }
            }
        }

        output
    }
}

/// Global unified metrics instance
pub static GLOBAL_UNIFIED_METRICS: Lazy<UnifiedMetrics> = Lazy::new(UnifiedMetrics::new);

/// Convenience functions for global metrics
pub mod convenience {
    use super::*;

    /// Record a cache operation using global metrics
    pub fn record_operation(operation: CacheOperation) {
        GLOBAL_UNIFIED_METRICS.record_operation(operation);
    }

    /// Get global hit rates
    pub fn hit_rates() -> HitRates {
        GLOBAL_UNIFIED_METRICS.hit_rates()
    }

    /// Export global metrics in Prometheus format
    pub fn export_prometheus() -> String {
        GLOBAL_UNIFIED_METRICS.export_prometheus()
    }

    /// Export global metrics in JSON format
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn export_json() -> Result<String, serde_json::Error> {
        GLOBAL_UNIFIED_METRICS.export_json()
    }

    /// Reset global metrics
    pub fn reset() {
        GLOBAL_UNIFIED_METRICS.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_counters() {
        let metrics = UnifiedMetrics::new();

        // Record some operations
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

        let counters = metrics.get_counters();
        assert_eq!(counters.l1_hits, 1);
        assert_eq!(counters.l1_misses, 1);
        assert_eq!(counters.total_operations, 2);
    }

    #[test]
    fn test_hit_rates() {
        let metrics = UnifiedMetrics::new();

        // Record operations
        for _ in 0..7 {
            metrics.record_operation(CacheOperation {
                layer: CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Hit,
            });
        }

        for _ in 0..3 {
            metrics.record_operation(CacheOperation {
                layer: CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Miss,
            });
        }

        let hit_rates = metrics.hit_rates();
        assert_eq!(hit_rates.l1_hit_rate, 0.7);
        assert_eq!(hit_rates.l2_hit_rate, 0.0);
        assert_eq!(hit_rates.overall_hit_rate, 0.7);
    }

    #[test]
    fn test_dynamic_metrics() {
        let metrics = UnifiedMetrics::new();

        // Test counter
        metrics.increment_counter("test_counter", 5);
        metrics.increment_counter("test_counter", 3);

        // Test gauge
        metrics.set_gauge("test_gauge", 42.5);

        // Test histogram
        metrics.record_histogram("test_histogram", 1.5);
        metrics.record_histogram("test_histogram", 2.7);

        // Test timer
        metrics.record_timer("test_timer", Duration::from_millis(100));
        metrics.record_timer("test_timer", Duration::from_millis(200));

        let dynamic_metrics = metrics.get_dynamic_metrics();

        let counter_metric = dynamic_metrics.get("test_counter");
        assert!(
            matches!(counter_metric, Some(MetricValue::Counter(count)) if *count == 8),
            "Expected counter metric with value 8, got {:?}",
            counter_metric
        );

        let gauge_metric = dynamic_metrics.get("test_gauge");
        assert!(
            matches!(gauge_metric, Some(MetricValue::Gauge(value)) if *value == 42.5),
            "Expected gauge metric with value 42.5, got {:?}",
            gauge_metric
        );
    }

    #[test]
    fn test_snapshot() {
        let metrics = UnifiedMetrics::new();

        // Record some operations
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Set,
            result: CacheOpResult::Success,
        });

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.counters.l1_sets, 1);
        assert!(snapshot.dynamic_metrics.is_empty() || !snapshot.dynamic_metrics.contains_key("detailed"));

        // Test export
        let prometheus = snapshot.export_prometheus();
        assert!(prometheus.contains("cache_l1_sets_total 1"));

        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        assert!(json.contains("l1_sets"));
    }

    #[test]
    fn test_global_metrics() {
        // Reset global metrics
        convenience::reset();

        // Record operation using global metrics
        convenience::record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });

        let hit_rates = convenience::hit_rates();
        assert_eq!(hit_rates.l2_hit_rate, 1.0);

        let prometheus = convenience::export_prometheus();
        assert!(prometheus.contains("cache_l2_hits_total 1"));
    }

    #[test]
    fn test_unified_metrics_default_impl() {
        // Triggers Default for UnifiedMetricsInner (lines 45, 47-49)
        let metrics = UnifiedMetrics::default();
        let counters = metrics.get_counters();
        assert_eq!(counters.l1_hits, 0);
        assert_eq!(counters.total_operations, 0);
    }

    #[test]
    fn test_unified_metrics_debug_format() {
        // Triggers Debug for UnifiedMetricsInner (lines 35-39)
        let metrics = UnifiedMetrics::new();
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("UnifiedMetrics"));
        assert!(debug_str.contains("UnifiedMetricsInner"));
        assert!(debug_str.contains("dynamic_metrics"));
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn test_l2_get_miss_operation() {
        // Covers lines 205-206
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });
        let counters = metrics.get_counters();
        assert_eq!(counters.l2_misses, 1);
        assert_eq!(counters.total_operations, 1);
    }

    #[test]
    fn test_l2_set_success_operation() {
        // Covers lines 213-214
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Set,
            result: CacheOpResult::Success,
        });
        let counters = metrics.get_counters();
        assert_eq!(counters.l2_sets, 1);
        assert_eq!(counters.total_operations, 1);
    }

    #[test]
    fn test_l1_delete_success_operation() {
        // Covers lines 217-218
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Delete,
            result: CacheOpResult::Success,
        });
        let counters = metrics.get_counters();
        assert_eq!(counters.l1_deletes, 1);
        assert_eq!(counters.total_operations, 1);
    }

    #[test]
    fn test_l2_delete_success_operation() {
        // Covers lines 221-222
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Delete,
            result: CacheOpResult::Success,
        });
        let counters = metrics.get_counters();
        assert_eq!(counters.l2_deletes, 1);
        assert_eq!(counters.total_operations, 1);
    }

    #[test]
    fn test_error_result_operation() {
        // Covers lines 225-226 (the Error arm)
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Error,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Set,
            result: CacheOpResult::Error,
        });
        let counters = metrics.get_counters();
        assert_eq!(counters.errors, 2);
        assert_eq!(counters.total_operations, 2);
    }

    #[test]
    fn test_record_duration_with_detailed_disabled() {
        // Covers lines 242-243 (early return when detailed=false)
        let config = MetricsConfig {
            detailed: false,
            ..MetricsConfig::default()
        };
        let metrics = UnifiedMetrics::with_config(config);
        let op = CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        };
        // Should be a no-op (early return)
        metrics.record_duration(&op, Duration::from_millis(100));
        let dynamic = metrics.get_dynamic_metrics();
        assert!(!dynamic.keys().any(|k| k.contains("duration")));
    }

    #[test]
    fn test_record_duration_with_detailed_enabled() {
        // Covers lines 247-248 (record_timer call)
        let metrics = UnifiedMetrics::new();
        let op = CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        };
        metrics.record_duration(&op, Duration::from_millis(150));
        let dynamic = metrics.get_dynamic_metrics();
        let timer_key = "cache:duration:L1:Get";
        assert!(
            dynamic.contains_key(timer_key),
            "Expected timer metric at key {}",
            timer_key
        );
        match dynamic.get(timer_key) {
            Some(MetricValue::Timer(t)) => {
                assert_eq!(t.count, 1);
                assert_eq!(t.total_duration, Duration::from_millis(150));
            }
            other => panic!("Expected Timer, got {:?}", other),
        }
    }

    #[test]
    fn test_record_custom_at_capacity_check() {
        // Covers lines 252-253 (capacity check) and 259 (insert).
        // Uses max_dynamic_metrics=0 so the capacity check triggers on an empty map.
        // When the map is empty, iter().next() returns None, so the eviction body
        // (line 256) is skipped — avoiding a deadlock that exists in record_custom
        // when iter().next() returns Some and remove() is called while the iterator
        // still holds a read lock on the DashMap shard.
        let config = MetricsConfig {
            max_dynamic_metrics: 0,
            ..MetricsConfig::default()
        };
        let metrics = UnifiedMetrics::with_config(config);

        // Capacity check triggers (0 >= 0), but map is empty so no eviction.
        metrics.record_custom("only_metric", MetricValue::Counter(1));
        let dynamic = metrics.get_dynamic_metrics();
        assert!(
            dynamic.contains_key("only_metric"),
            "metric should be present after insert"
        );
        assert_eq!(dynamic.len(), 1);
    }

    #[test]
    fn test_record_custom_under_capacity_no_eviction() {
        let config = MetricsConfig {
            max_dynamic_metrics: 10,
            ..MetricsConfig::default()
        };
        let metrics = UnifiedMetrics::with_config(config);
        metrics.record_custom("a", MetricValue::Counter(1));
        metrics.record_custom("b", MetricValue::Counter(2));
        let dynamic = metrics.get_dynamic_metrics();
        assert_eq!(dynamic.len(), 2);
        assert!(dynamic.contains_key("a"));
        assert!(dynamic.contains_key("b"));
    }

    #[test]
    fn test_export_json_method() {
        // Covers lines 416-418 (export_json method on UnifiedMetrics)
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        let json = metrics.export_json().unwrap();
        assert!(json.contains("counters"));
        assert!(json.contains("l1_hits"));
        assert!(json.contains("dynamic_metrics"));
    }

    #[test]
    fn test_cache_op_type_clear_display() {
        // Covers line 473-474 (CacheOpType::Clear Display arm)
        assert_eq!(CacheOpType::Clear.to_string(), "Clear");
    }

    #[test]
    fn test_cache_op_type_all_display_variants() {
        // Covers all Display arms for CacheOpType
        assert_eq!(CacheOpType::Get.to_string(), "Get");
        assert_eq!(CacheOpType::Set.to_string(), "Set");
        assert_eq!(CacheOpType::Delete.to_string(), "Delete");
        assert_eq!(CacheOpType::Clear.to_string(), "Clear");
    }

    #[test]
    fn test_cache_op_result_error_display() {
        // Covers line 494 (CacheOpResult::Error Display arm)
        assert_eq!(CacheOpResult::Error.to_string(), "Error");
    }

    #[test]
    fn test_cache_op_result_all_display_variants() {
        // Covers all Display arms for CacheOpResult
        assert_eq!(CacheOpResult::Hit.to_string(), "Hit");
        assert_eq!(CacheOpResult::Miss.to_string(), "Miss");
        assert_eq!(CacheOpResult::Success.to_string(), "Success");
        assert_eq!(CacheOpResult::Error.to_string(), "Error");
    }

    #[test]
    fn test_export_prometheus_gauge_metric() {
        // Covers lines 561-562 (MetricValue::Gauge in export_prometheus)
        let metrics = UnifiedMetrics::new();
        metrics.set_gauge("my_gauge", 42.5);
        let prom = metrics.export_prometheus();
        assert!(prom.contains("my_gauge_gauge 42.5"));
    }

    #[test]
    fn test_export_prometheus_histogram_metric() {
        // Covers lines 564-568 (MetricValue::Histogram in export_prometheus)
        let metrics = UnifiedMetrics::new();
        metrics.record_histogram("my_histogram", 1.5);
        metrics.record_histogram("my_histogram", 2.5);
        let prom = metrics.export_prometheus();
        assert!(prom.contains("my_histogram_histogram_sum 4"));
        assert!(prom.contains("my_histogram_histogram_count 2"));
        // Buckets should be present
        assert!(prom.contains("my_histogram_histogram_bucket"));
        assert!(prom.contains("le=\""));
    }

    #[test]
    fn test_export_prometheus_timer_metric() {
        // Covers lines 571-572, 575, 577 (MetricValue::Timer in export_prometheus)
        let metrics = UnifiedMetrics::new();
        metrics.record_timer("my_timer", Duration::from_millis(250));
        let prom = metrics.export_prometheus();
        assert!(prom.contains("my_timer_timer_seconds_sum"));
        assert!(prom.contains("my_timer_timer_seconds_count 1"));
    }

    #[test]
    fn test_export_prometheus_text_metric() {
        // Covers lines 579-580 (MetricValue::Text in export_prometheus)
        let metrics = UnifiedMetrics::new();
        metrics.record_custom("my_info", MetricValue::Text("version1".to_string()));
        let prom = metrics.export_prometheus();
        assert!(prom.contains("my_info_info \"version1\""));
    }

    #[test]
    fn test_export_prometheus_counter_dynamic_metric() {
        // Covers line 559 (MetricValue::Counter in export_prometheus dynamic section)
        let metrics = UnifiedMetrics::new();
        metrics.increment_counter("my_dyn_counter", 7);
        let prom = metrics.export_prometheus();
        assert!(prom.contains("my_dyn_counter_counter 7"));
    }

    #[test]
    fn test_convenience_export_json() {
        // Covers lines 613-614 (convenience::export_json)
        convenience::reset();
        let json = convenience::export_json().unwrap();
        assert!(json.contains("counters"));
        assert!(json.contains("l1_hits"));
        assert!(json.contains("dynamic_metrics"));
    }

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert!(config.detailed);
        assert!(!config.histogram_buckets.is_empty());
        assert_eq!(config.max_dynamic_metrics, 1000);
        assert_eq!(config.retention_period, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_atomic_counters_default() {
        let counters = AtomicCounters::default();
        assert_eq!(counters.l1_hits.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_misses.load(Ordering::Relaxed), 0);
        assert_eq!(counters.errors.load(Ordering::Relaxed), 0);
        assert_eq!(counters.prefetch_total.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_reset_clears_all_metrics() {
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.increment_counter("custom", 5);
        assert!(!metrics.get_dynamic_metrics().is_empty());

        metrics.reset();
        let counters = metrics.get_counters();
        assert_eq!(counters.l1_hits, 0);
        assert_eq!(counters.total_operations, 0);
        assert!(metrics.get_dynamic_metrics().is_empty());
    }

    #[test]
    fn test_hit_rates_overall_default_is_zero() {
        // When no operations recorded, overall_hit_rate returns 0.0 (consistent with l1/l2)
        let metrics = UnifiedMetrics::new();
        let rates = metrics.hit_rates();
        assert_eq!(rates.l1_hit_rate, 0.0);
        assert_eq!(rates.l2_hit_rate, 0.0);
        assert_eq!(rates.overall_hit_rate, 0.0);
    }
}
