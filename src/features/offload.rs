// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Offload 后台任务子系统（absorb-hitbox-features，对标 hitbox OffloadManager）
//!
//! 进程内后台任务执行器：同 key 去重（在飞任务期间重复 spawn 被丢弃）、
//! 信号量并发上限、超时策略（Cancel 包裹 / Warn 告警）。去重范围为进程级，
//! 跨实例协调属 dist_lock 职责（与 single-flight 同口径）。
//!
//! # 语义
//!
//! - [`OffloadManager::spawn`]：fire-and-forget。同 key 已在飞或无可用许可
//!   时返回 `false`（任务体不会执行）；否则返回 `true` 并在后台执行。
//! - panic 隔离：任务体 panic 由 tokio 运行时捕获，[`InFlightGuard`] 在
//!   unwind 中照常 Drop，in_flight 表不泄漏（与 single-flight 守卫同模式）。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::Semaphore;

/// 全局 unified 指标（metrics feature 下启用计数）
#[cfg(feature = "metrics")]
fn record_counter(name: &str, value: u64) {
    crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS.increment_counter(name, value);
}

#[cfg(not(feature = "metrics"))]
fn record_counter(_name: &str, _value: u64) {}

#[cfg(feature = "metrics")]
fn set_active_gauge(value: usize) {
    crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS
        .set_gauge("oxcache_offload_active", value as f64);
}

#[cfg(not(feature = "metrics"))]
fn set_active_gauge(_value: usize) {}

// telemetry 双版本 inline 埋点（feature 关闭时零开销）
#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_event(key: &str, event: &str) {
    tracing::debug!(target: "oxcache::offload", key, event, "offload lifecycle event");
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_event(_key: &str, _event: &str) {}

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_timeout(key: &str, elapsed: Duration, policy: &str) {
    tracing::warn!(target: "oxcache::offload", key, elapsed = ?elapsed, policy, "offload task exceeded timeout policy");
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_timeout(_key: &str, _elapsed: Duration, _policy: &str) {}

/// 超时策略（v1：Cancel 包裹取消，Warn 仅告警不取消）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutPolicy {
    /// 不设超时
    None,
    /// 超过时限取消任务（`tokio::time::timeout` 包裹），计入 timeout 计数
    Cancel(Duration),
    /// 超过时限仅记录告警事件，任务继续执行
    Warn(Duration),
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        TimeoutPolicy::Warn(Duration::from_secs(30))
    }
}

/// in_flight 表清理守卫：Drop 时移除 key、刷新活跃 gauge。
/// panic unwind 路径同样执行（single-flight `AsyncSfGuard` 同模式）。
struct InFlightGuard {
    in_flight: Arc<DashMap<Arc<str>, ()>>,
    key: Arc<str>,
    completed: Arc<AtomicU64>,
    // 持有许可至任务结束；字段命名为保留所有权
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl InFlightGuard {
    fn release(&mut self) {
        if self.in_flight.remove(&self.key).is_some() {
            set_active_gauge(self.in_flight.len());
            self.completed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.release();
    }
}

/// Offload 后台任务管理器。
///
/// Clone 语义：共享同一 in_flight 表与许可池（内部全 Arc）。
#[derive(Clone)]
pub struct OffloadManager {
    in_flight: Arc<DashMap<Arc<str>, ()>>,
    permits: Arc<Semaphore>,
    max_concurrent_tasks: usize,
    timeout_policy: TimeoutPolicy,
    completed: Arc<AtomicU64>,
}

impl OffloadManager {
    /// 创建管理器（超时策略取默认 `Warn(30s)`）。
    pub fn new(max_concurrent_tasks: usize) -> Self {
        Self::with_policy(max_concurrent_tasks, TimeoutPolicy::default())
    }

    /// 创建管理器并指定超时策略。`max_concurrent_tasks` 为 0 时按 1 处理。
    pub fn with_policy(max_concurrent_tasks: usize, timeout_policy: TimeoutPolicy) -> Self {
        let max_concurrent_tasks = max_concurrent_tasks.max(1);
        Self {
            in_flight: Arc::new(DashMap::new()),
            permits: Arc::new(Semaphore::new(max_concurrent_tasks)),
            max_concurrent_tasks,
            timeout_policy,
            completed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 并发上限（0 已归一为 1）。
    pub fn max_concurrent_tasks(&self) -> usize {
        self.max_concurrent_tasks
    }

    /// 当前超时策略。
    pub fn timeout_policy(&self) -> TimeoutPolicy {
        self.timeout_policy
    }

    /// 提交后台任务。返回 `false` 表示未执行：同 key 已在飞（去重）或许可池
    /// 已满（超限丢弃，不排队）。
    pub fn spawn<F>(&self, key: impl Into<Arc<str>>, task: F) -> bool
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let key: Arc<str> = key.into();
        if self.in_flight.contains_key(&key) {
            record_counter("oxcache_offload_deduplicated_total", 1);
            telemetry_event(&key, "deduplicated");
            return false;
        }
        // try_acquire：无空闲许可即丢弃（超限不排队，与 hitbox 语义一致）
        let permit = match self.permits.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                telemetry_event(&key, "rejected_no_permit");
                return false;
            }
        };
        self.in_flight.insert(key.clone(), ());
        set_active_gauge(self.in_flight.len());
        record_counter("oxcache_offload_spawned_total", 1);
        telemetry_event(&key, "spawned");

        let guard = InFlightGuard {
            in_flight: self.in_flight.clone(),
            key: key.clone(),
            completed: self.completed.clone(),
            _permit: permit,
        };
        let policy = self.timeout_policy;
        tokio::spawn(async move {
            match policy {
                TimeoutPolicy::None => task.await,
                TimeoutPolicy::Cancel(limit) => {
                    if tokio::time::timeout(limit, task).await.is_err() {
                        record_counter("oxcache_offload_timeout_total", 1);
                        telemetry_timeout(&key, limit, "cancel");
                    }
                }
                TimeoutPolicy::Warn(limit) => {
                    let start = Instant::now();
                    task.await;
                    let elapsed = start.elapsed();
                    if elapsed > limit {
                        telemetry_timeout(&key, elapsed, "warn");
                    }
                }
            }
            drop(guard);
        });
        true
    }

    /// 同 key 任务是否在飞。
    pub fn is_in_flight(&self, key: &str) -> bool {
        self.in_flight.contains_key(key)
    }

    /// 当前在飞任务数。
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// 清空 in_flight 表（正在执行的任务自然跑完，不强制中止）。
    /// 返回被丢弃登记的任务数。
    pub fn cancel_all(&self) -> usize {
        let removed = self.in_flight.len();
        self.in_flight.clear();
        set_active_gauge(self.in_flight.len());
        removed
    }

    /// 等待全部在飞任务完成（轮询 in_flight 表，间隔 5ms）。超时则提前返回。
    /// 返回等待期间完成的任务数。
    pub async fn wait_all(&self, timeout: Duration) -> usize {
        let baseline = self.completed.load(Ordering::Relaxed);
        let start = Instant::now();
        while !self.in_flight.is_empty() {
            if start.elapsed() >= timeout {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        (self.completed.load(Ordering::Relaxed) - baseline) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn manager() -> OffloadManager {
        OffloadManager::new(8)
    }

    #[tokio::test]
    async fn spawn_deduplicates_same_key_while_in_flight() {
        static EXECUTED: AtomicUsize = AtomicUsize::new(0);
        let mgr = manager();
        let first = mgr.spawn("k", async {
            EXECUTED.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(80)).await;
        });
        assert!(first, "first spawn must run");
        // leader 任务体在其它 worker 上执行，轮询等待其到达计数点
        let deadline = Instant::now() + Duration::from_secs(1);
        while EXECUTED.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let second = mgr.spawn("k", async {
            EXECUTED.fetch_add(1, Ordering::SeqCst);
        });
        assert!(
            !second,
            "same-key spawn while in flight must be deduplicated"
        );
        assert_eq!(
            EXECUTED.load(Ordering::SeqCst),
            1,
            "deduped task must not run"
        );
        assert!(mgr.is_in_flight("k"));
        let completed = mgr.wait_all(Duration::from_secs(2)).await;
        assert_eq!(completed, 1);
        assert!(!mgr.is_in_flight("k"), "completion must clear in_flight");
        // 完成后可重新调度
        let third = mgr.spawn("k", async {});
        assert!(third, "spawn after completion must run");
    }

    #[tokio::test]
    async fn distinct_keys_spawn_concurrently() {
        let mgr = manager();
        assert!(mgr.spawn("a", async {
            tokio::time::sleep(Duration::from_millis(50)).await
        }));
        assert!(mgr.spawn("b", async {
            tokio::time::sleep(Duration::from_millis(50)).await
        }));
        assert_eq!(mgr.in_flight_count(), 2);
        assert!(mgr.wait_all(Duration::from_secs(2)).await >= 2);
    }

    #[tokio::test]
    async fn concurrency_limit_rejects_overflow() {
        let mgr = OffloadManager::new(1);
        assert!(mgr.spawn("slot", async {
            tokio::time::sleep(Duration::from_millis(80)).await
        }));
        let rejected = mgr.spawn("other", async {});
        assert!(
            !rejected,
            "spawn beyond max_concurrent_tasks must be rejected"
        );
        assert!(mgr.wait_all(Duration::from_secs(2)).await >= 1);
        // 许可释放后可再次调度
        assert!(mgr.spawn("other", async {}));
        mgr.wait_all(Duration::from_secs(2)).await;
    }

    #[tokio::test]
    async fn cancel_all_clears_registry_only() {
        let mgr = manager();
        assert!(mgr.spawn("c1", async {
            tokio::time::sleep(Duration::from_millis(60)).await
        }));
        assert_eq!(mgr.cancel_all(), 1);
        assert_eq!(mgr.in_flight_count(), 0);
        // 任务自然跑完后 guard 不二次计数（key 已被清走）
        mgr.wait_all(Duration::from_secs(2)).await;
    }

    #[tokio::test]
    async fn panic_in_task_does_not_leak_registry() {
        static AFTER_PANIC: AtomicUsize = AtomicUsize::new(0);
        let mgr = manager();
        // 吞掉 panic 输出以外的断言：guard 在 unwind 中必须清理注册表
        let _ = mgr.spawn("p", async {
            AFTER_PANIC.fetch_add(1, Ordering::SeqCst);
            panic!("simulated offload panic");
        });
        // 等待任务结束（panic 被 tokio 捕获）
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(
            !mgr.is_in_flight("p"),
            "panic must not leak the in_flight registry"
        );
        assert!(
            mgr.spawn("p", async {}),
            "spawn after panic must be accepted"
        );
        mgr.wait_all(Duration::from_secs(2)).await;
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn metrics_counters_record_lifecycle() {
        let mgr = manager();
        let metrics = &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
        let spawned_before = dynamic_counter(metrics, "oxcache_offload_spawned_total");
        let dedup_before = dynamic_counter(metrics, "oxcache_offload_deduplicated_total");

        assert!(mgr.spawn("m", async {
            tokio::time::sleep(Duration::from_millis(60)).await
        }));
        assert!(!mgr.spawn("m", async {}));
        mgr.wait_all(Duration::from_secs(2)).await;

        // 全局计数器与其它并行测试共享，用单调 delta 断言
        assert!(
            dynamic_counter(metrics, "oxcache_offload_spawned_total") > spawned_before,
            "spawn must increment its counter"
        );
        assert!(
            dynamic_counter(metrics, "oxcache_offload_deduplicated_total") > dedup_before,
            "dedup must increment its counter"
        );
    }

    #[cfg(feature = "metrics")]
    fn dynamic_counter(metrics: &crate::infra::metrics::unified::UnifiedMetrics, key: &str) -> u64 {
        metrics
            .get_dynamic_metrics()
            .get(key)
            .and_then(|v| match v {
                crate::infra::metrics::unified::MetricValue::Counter(c) => Some(*c),
                _ => None,
            })
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn cancel_policy_counts_timeout() {
        let mgr = OffloadManager::with_policy(2, TimeoutPolicy::Cancel(Duration::from_millis(30)));
        #[cfg(feature = "metrics")]
        let metrics = &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
        #[cfg(feature = "metrics")]
        let timeout_before = dynamic_counter(metrics, "oxcache_offload_timeout_total");

        assert!(mgr.spawn("slow", async {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }));
        let completed = mgr.wait_all(Duration::from_secs(2)).await;
        assert_eq!(completed, 1, "cancelled task must release its slot");

        #[cfg(feature = "metrics")]
        assert!(
            dynamic_counter(metrics, "oxcache_offload_timeout_total") > timeout_before,
            "cancel-policy elapse must increment timeout counter"
        );
    }
}
