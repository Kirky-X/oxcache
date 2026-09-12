// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `MetricsRecorder` 端口。
//!
//! 对象安全的指标注入端口：`Cache<K,V>` 纯 L1 路径（此前零指标）在
//! get/set/delete 后调用注入的 [`MetricsRecorder`] 记录 hit/miss 计数与
//! 延迟样本。默认 [`NoOpMetricsRecorder`] 零开销；[`UnifiedMetricsRecorder`]
//! 适配到 [`UnifiedMetrics`](super::UnifiedMetrics)（含标准 Prometheus
//! 命名导出）。

use super::unified::{
    CacheOpResult, CacheOpType, CacheOperation, UnifiedMetrics, GLOBAL_UNIFIED_METRICS,
};
use crate::core::CacheLayer;
use std::sync::Arc;
use std::time::Duration;

/// 缓存指标记录端口（对象安全，可 `Arc<dyn MetricsRecorder>` 注入）
///
/// 所有实现必须是 `Send + Sync`（热路径并发调用）；方法应非阻塞。
pub trait MetricsRecorder: Send + Sync + 'static {
    /// 记录一次命中（含延迟样本）
    fn record_hit(&self, _layer: CacheLayer, _latency: Duration) {}

    /// 记录一次未命中（含延迟样本）
    fn record_miss(&self, _layer: CacheLayer, _latency: Duration) {}

    /// 记录一次写入（含延迟样本）
    fn record_set(&self, _layer: CacheLayer, _latency: Duration) {}

    /// 记录一次删除（含延迟样本）
    fn record_delete(&self, _layer: CacheLayer, _latency: Duration) {}

    /// 记录淘汰/过期事件数量
    fn record_eviction(&self, _count: u64) {}
}

/// 空实现（默认）：零开销，不产生任何分配与计数
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpMetricsRecorder;

impl MetricsRecorder for NoOpMetricsRecorder {}

/// [`UnifiedMetrics`] 适配器：把端口调用映射到统一指标收集器
///
/// - hit/miss/set/delete → 原子计数器 + 延迟计时器 + 标准命名延迟直方图；
/// - eviction → `evictions` 计数器。
#[derive(Clone, Debug)]
pub struct UnifiedMetricsRecorder {
    metrics: UnifiedMetrics,
}

impl UnifiedMetricsRecorder {
    /// 创建独立实例的适配器
    pub fn new() -> Self {
        Self {
            metrics: UnifiedMetrics::new(),
        }
    }

    /// 适配到全局统一指标收集器（与 ChainCache / Redis 客户端共享）
    pub fn global() -> Self {
        Self {
            metrics: GLOBAL_UNIFIED_METRICS.clone(),
        }
    }

    /// 底层统一指标收集器（查询计数、导出 Prometheus/JSON）
    pub fn metrics(&self) -> &UnifiedMetrics {
        &self.metrics
    }
}

impl Default for UnifiedMetricsRecorder {
    fn default() -> Self {
        Self::new()
    }
}

fn record_with_latency(metrics: &UnifiedMetrics, op: CacheOperation, latency: Duration) {
    metrics.record_operation(op.clone());
    metrics.record_duration(&op, latency);
    metrics.record_latency_seconds(latency.as_secs_f64());
}

impl MetricsRecorder for UnifiedMetricsRecorder {
    fn record_hit(&self, layer: CacheLayer, latency: Duration) {
        record_with_latency(
            &self.metrics,
            CacheOperation {
                layer,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Hit,
            },
            latency,
        );
    }

    fn record_miss(&self, layer: CacheLayer, latency: Duration) {
        record_with_latency(
            &self.metrics,
            CacheOperation {
                layer,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Miss,
            },
            latency,
        );
    }

    fn record_set(&self, layer: CacheLayer, latency: Duration) {
        record_with_latency(
            &self.metrics,
            CacheOperation {
                layer,
                op_type: CacheOpType::Set,
                result: CacheOpResult::Success,
            },
            latency,
        );
    }

    fn record_delete(&self, layer: CacheLayer, latency: Duration) {
        record_with_latency(
            &self.metrics,
            CacheOperation {
                layer,
                op_type: CacheOpType::Delete,
                result: CacheOpResult::Success,
            },
            latency,
        );
    }

    fn record_eviction(&self, count: u64) {
        self.metrics.record_eviction(count);
    }
}

/// 便捷构造：`Arc<dyn MetricsRecorder>`
pub fn noop_recorder() -> Arc<dyn MetricsRecorder> {
    Arc::new(NoOpMetricsRecorder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_recorder_is_zero_cost() {
        let recorder = NoOpMetricsRecorder;
        // 调用不 panic、无副作用
        recorder.record_hit(CacheLayer::L1, Duration::from_micros(10));
        recorder.record_miss(CacheLayer::L1, Duration::from_micros(10));
        recorder.record_set(CacheLayer::L1, Duration::from_micros(10));
        recorder.record_delete(CacheLayer::L1, Duration::from_micros(10));
        recorder.record_eviction(3);
    }

    #[test]
    fn unified_recorder_counts_hits_misses_and_latency_samples() {
        let recorder = UnifiedMetricsRecorder::new();
        let counters = recorder.metrics().get_counters();
        assert_eq!(counters.l1_hits, 0);
        assert_eq!(counters.l1_misses, 0);

        recorder.record_hit(CacheLayer::L1, Duration::from_micros(120));
        recorder.record_hit(CacheLayer::L1, Duration::from_micros(80));
        recorder.record_miss(CacheLayer::L1, Duration::from_micros(200));

        let counters = recorder.metrics().get_counters();
        assert_eq!(counters.l1_hits, 2, "命中计数应被记录");
        assert_eq!(counters.l1_misses, 1, "未命中计数应被记录");
        assert_eq!(counters.l1_sets, 0);

        // 延迟直方图样本（标准命名 key）
        let dynamic = recorder.metrics().get_dynamic_metrics();
        assert!(
            dynamic
                .get(super::super::unified::OPERATION_LATENCY_HISTOGRAM)
                .is_some(),
            "延迟直方图样本应被记录"
        );
    }

    #[test]
    fn unified_recorder_counts_sets_deletes_and_evictions() {
        let recorder = UnifiedMetricsRecorder::new();
        recorder.record_set(CacheLayer::L1, Duration::from_micros(50));
        recorder.record_delete(CacheLayer::L1, Duration::from_micros(30));
        recorder.record_eviction(4);

        let counters = recorder.metrics().get_counters();
        assert_eq!(counters.l1_sets, 1);
        assert_eq!(counters.l1_deletes, 1);
        assert_eq!(counters.evictions, 4, "淘汰计数应被记录");
    }

    #[test]
    fn recorder_is_object_safe_and_injectable() {
        // Arc<dyn MetricsRecorder> 注入形态可用
        let recorder: Arc<dyn MetricsRecorder> = Arc::new(UnifiedMetricsRecorder::new());
        recorder.record_hit(CacheLayer::L1, Duration::from_micros(5));
        let noop: Arc<dyn MetricsRecorder> = noop_recorder();
        noop.record_eviction(1);
    }
}
