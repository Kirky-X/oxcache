// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了缓存系统的指标收集和监控功能。

pub mod backend;
pub mod export;
pub mod snapshot;
pub mod unified;

mod metrics_impl;

pub use export::{export_json_format, export_prometheus_format, get_enhanced_stats};
pub use snapshot::CacheStats;
pub use unified::{AtomicCounters, UnifiedMetrics};

// 当 metrics 和 moka 功能都禁用时的空实现
#[cfg(not(any(feature = "metrics", feature = "memory")))]
#[derive(Debug, Clone, Default)]
pub struct Metrics;

#[cfg(not(any(feature = "metrics", feature = "memory")))]
pub use metrics_impl::{get_metrics_string, GLOBAL_METRICS};

// ============================================================================
// Unified Metrics Exports
// ============================================================================

// Re-export unified metrics
pub use unified::{
    convenience as unified_convenience, CacheOpResult, CacheOpType, CacheOperation, CounterSnapshot, HistogramData,
    HitRates, MetricValue, MetricsConfig, MetricsSnapshot, TimerData, GLOBAL_UNIFIED_METRICS,
};

// Re-export convenience module for test access
pub use unified::convenience;

// 从 core 重新导出 CacheLayer
pub use crate::core::CacheLayer;
