// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全信号响应器与检测点共享类型

use super::signal::SecuritySignal;
use crate::features::degradation::DegradationController;
use std::sync::Arc;

/// 同步安全信号回调；需异步动作时在实现内部 `tokio::spawn`
pub trait SecurityResponder: Send + Sync + 'static {
    /// 处理一个信号
    fn on_signal(&self, signal: &SecuritySignal);
}

/// 检测点钩子类型（integrity.rs / redlock.rs cfg 门控字段）
pub type SecurityHook = Arc<dyn Fn(SecuritySignal) + Send + Sync>;

/// 默认降级响应器
///
/// - `IntegrityCompromised` → `controller.record_failure()`（达阈值进入 Degraded）
/// - `FencingAnomaly` → warning 日志 + 计数（不改变降级状态）
struct DegradationResponder {
    controller: Arc<DegradationController>,
}

impl SecurityResponder for DegradationResponder {
    fn on_signal(&self, signal: &SecuritySignal) {
        match signal {
            SecuritySignal::IntegrityCompromised { .. } => {
                self.controller.record_failure();
            }
            SecuritySignal::FencingAnomaly { .. } => {
                #[cfg(feature = "metrics")]
                crate::infra::GLOBAL_UNIFIED_METRICS
                    .increment_counter("oxcache_security_fencing_anomaly_total", 1);
                #[cfg(feature = "telemetry")]
                tracing::warn!(
                    target: "oxcache::security_events",
                    "fencing anomaly observed: {:?}", signal
                );
            }
        }
    }
}

/// 构造默认降级响应器
pub fn degradation_responder(controller: Arc<DegradationController>) -> Arc<dyn SecurityResponder> {
    Arc::new(DegradationResponder { controller })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::degradation::DegradationState;
    use std::time::Duration;

    fn integrity() -> SecuritySignal {
        SecuritySignal::IntegrityCompromised {
            instance_id: "node-a".to_string(),
            key: "k".to_string(),
            consecutive_failures: 3,
            raised_at_unix_ms: 1,
        }
    }

    fn fencing() -> SecuritySignal {
        SecuritySignal::FencingAnomaly {
            instance_id: "node-a".to_string(),
            lock_key: "lease".to_string(),
            node_index: 1,
            observed_token: 2,
            last_token: 5,
            raised_at_unix_ms: 2,
        }
    }

    #[test]
    fn consecutive_failures_drive_controller_to_degraded() {
        let controller = Arc::new(DegradationController::new(3, Duration::from_secs(60)));
        assert_eq!(controller.state(), DegradationState::Active);
        for _ in 0..3 {
            controller.record_failure();
        }
        assert_eq!(controller.state(), DegradationState::Degraded);
    }

    #[test]
    fn integrity_signal_drives_degradation() {
        let controller = Arc::new(DegradationController::new(3, Duration::from_secs(60)));
        let responder = degradation_responder(controller.clone());
        for _ in 0..3 {
            responder.on_signal(&integrity());
        }
        assert_eq!(controller.state(), DegradationState::Degraded);
    }

    #[cfg(feature = "metrics")]
    #[test]
    #[serial_test::serial]
    fn fencing_signal_increments_counter_without_degrading() {
        use crate::infra::metrics::MetricValue;
        let controller = Arc::new(DegradationController::new(3, Duration::from_secs(60)));
        let responder = degradation_responder(controller.clone());
        let before = crate::infra::GLOBAL_UNIFIED_METRICS
            .snapshot()
            .dynamic_metrics
            .get("oxcache_security_fencing_anomaly_total")
            .and_then(|v| {
                if let MetricValue::Counter(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        responder.on_signal(&fencing());
        let after = crate::infra::GLOBAL_UNIFIED_METRICS
            .snapshot()
            .dynamic_metrics
            .get("oxcache_security_fencing_anomaly_total")
            .and_then(|v| {
                if let MetricValue::Counter(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        assert!(after > before, "before={before} after={after}");
        assert_eq!(controller.state(), DegradationState::Active);
    }

    #[test]
    fn fencing_signal_does_not_change_state() {
        let controller = Arc::new(DegradationController::new(3, Duration::from_secs(60)));
        let responder = degradation_responder(controller.clone());
        for _ in 0..5 {
            responder.on_signal(&fencing());
        }
        assert_eq!(controller.state(), DegradationState::Active);
    }
}
