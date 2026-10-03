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
//! - [`OffloadManager::spawn`](crate::features::offload::OffloadManager::spawn)：fire-and-forget。同 key 已在飞或无可用许可
//!   时返回 `false`（任务体不会执行）；否则返回 `true` 并在后台执行。
//! - panic 隔离：任务体 panic 由 tokio 运行时捕获，私有守卫 `InFlightGuard` 在
//!   unwind 中照常 Drop，in_flight 表不泄漏（与 single-flight 守卫同模式）。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::Semaphore;

#[cfg(feature = "telemetry")]
use crate::i18n::messages::{
    MSG_LOG_OFFLOAD_LIFECYCLE_EVENT, MSG_LOG_OFFLOAD_TIMEOUT_POLICY_EXCEEDED, t,
};

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
    tracing::debug!(
        target: "oxcache::offload",
        key,
        event,
        "{}",
        t(MSG_LOG_OFFLOAD_LIFECYCLE_EVENT, &[])
    );
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn telemetry_event(_key: &str, _event: &str) {}

#[cfg(feature = "telemetry")]
#[inline]
fn telemetry_timeout(key: &str, elapsed: Duration, policy: &str) {
    tracing::warn!(
        target: "oxcache::offload",
        key,
        elapsed = ?elapsed,
        policy,
        "{}",
        t(MSG_LOG_OFFLOAD_TIMEOUT_POLICY_EXCEEDED, &[])
    );
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
mod tests;
