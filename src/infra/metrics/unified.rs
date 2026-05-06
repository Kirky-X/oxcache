//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified metrics collection system that consolidates all metrics functionality

use crate::core::types::CacheLayer;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{span, Level};

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
    /// Metrics retention period
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
        let span = span!(Level::INFO, "cache_operation",
            layer = ?operation.layer,
            op_type = ?operation.op_type,
            result = ?operation.result
        );
        let _enter = span.enter();

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
            // Remove oldest metric if at capacity
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

        let l1_total = counters.l1_hits + counters.l1_misses;
        let l2_total = counters.l2_hits + counters.l2_misses;
        let total_hits = counters.l1_hits + counters.l2_hits;
        let total_misses = counters.l1_misses + counters.l2_misses;
        let overall_total = total_hits + total_misses;

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
                1.0
            },
        }
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
}
