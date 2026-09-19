// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 该模块定义了缓存系统的指标收集和监控功能。

pub mod backend;
pub mod export;
pub mod recorder;
pub mod snapshot;
pub mod unified;

pub use export::{export_json_format, export_prometheus_format, get_enhanced_stats};
pub use snapshot::CacheStats;
pub use unified::{AtomicCounters, UnifiedMetrics};

// ============================================================================
// Unified Metrics Exports
// ============================================================================

// Re-export unified metrics
pub use unified::{
    CacheOpResult, CacheOpType, CacheOperation, CounterSnapshot, GLOBAL_UNIFIED_METRICS,
    HistogramData, HitRates, MetricValue, MetricsConfig, MetricsSnapshot, TimerData,
    convenience as unified_convenience,
};

// metrics recorder port + standard naming exports
pub use export::export_prometheus_standard;
pub use recorder::{MetricsRecorder, NoOpMetricsRecorder, UnifiedMetricsRecorder, noop_recorder};
pub use unified::{OPERATION_LATENCY_HISTOGRAM, PROMETHEUS_LATENCY_BUCKETS};

// Re-export convenience module for test access
pub use unified::convenience;

// 从 core 重新导出 CacheLayer
pub use crate::core::CacheLayer;
