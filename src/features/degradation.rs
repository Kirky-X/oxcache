// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 自动降级与恢复
//!
//! [`DegradationController`] 三态状态机：**Active → Degraded → HalfOpen → Active**
//!
//! - L2 连续故障计数超阈值 → 自动降级 L1-only（`Degraded`）；
//! - 降级期 L2 流量被拦截（装饰器返回 `Degraded` 错误，ChainCache 部分失败
//!   容忍机制使读回落 L1）；
//! - 半开探测：降级超过 `recovery_timeout` 后转 `HalfOpen` 放行 L2 探测
//!   流量（并发探测均放行）；
//! - 探测成功 → 自动恢复 `Active`；探测失败 → 重新 `Degraded`。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::degradation::{DegradableBackend, DegradationController};
//!
//! let controller = Arc::new(DegradationController::new(5, Duration::from_secs(30)));
//! let l2 = DegradableBackend::new(redis_backend, controller.clone());
//! // redis 故障累计 5 次 → controller.state() == Degraded（读走 L1）
//! // 30s 后 allow_l2() 放行探测 → 成功自动恢复 Active
//! ```

use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
use crate::backend::interface::{BackendKind, CacheSetItem};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 降级状态机状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationState {
    /// 正常：L2 流量放行
    Active,
    /// 已降级：L1-only，L2 流量被拦截
    Degraded,
    /// 半开：探测恢复中
    HalfOpen,
}

impl DegradationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DegradationState::Active => "active",
            DegradationState::Degraded => "degraded",
            DegradationState::HalfOpen => "half_open",
        }
    }
}

struct ControllerInner {
    state: DegradationState,
    consecutive_failures: u32,
    degraded_since: Option<Instant>,
}

/// L2 降级状态机
pub struct DegradationController {
    failure_threshold: u32,
    recovery_timeout: Duration,
    inner: Mutex<ControllerInner>,
    state_listeners: Vec<Box<dyn Fn(DegradationState) + Send + Sync>>,
}

impl DegradationController {
    /// 创建状态机
    ///
    /// - `failure_threshold`：连续失败次数阈值（≥1）
    /// - `recovery_timeout`：降级后半开探测前的等待时长
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            failure_threshold: failure_threshold.max(1),
            recovery_timeout,
            inner: Mutex::new(ControllerInner {
                state: DegradationState::Active,
                consecutive_failures: 0,
                degraded_since: None,
            }),
            state_listeners: Vec::new(),
        }
    }

    /// 注册状态变化监听（指标 / 审计 / 告警）
    pub fn on_state_change(
        mut self,
        listener: impl Fn(DegradationState) + Send + Sync + 'static,
    ) -> Self {
        self.state_listeners.push(Box::new(listener));
        self
    }

    fn transition(&self, inner: &mut ControllerInner, next: DegradationState) {
        if inner.state != next {
            inner.state = next;
            if next == DegradationState::Degraded {
                inner.degraded_since = Some(Instant::now());
                #[cfg(feature = "metrics")]
                crate::infra::GLOBAL_UNIFIED_METRICS.record_l2_degraded();
            }
            if next == DegradationState::Active {
                inner.consecutive_failures = 0;
                inner.degraded_since = None;
            }
            for listener in &self.state_listeners {
                listener(next);
            }
        }
    }

    /// 当前状态
    pub fn state(&self) -> DegradationState {
        // 与 record_failure/record_success/allow_l2 同口径：锁中毒时恢复
        // 内部数据（状态机数据本身一致），避免永久卡在降级态
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
    }

    /// 是否处于 L1-only 降级
    pub fn is_l1_only(&self) -> bool {
        self.state() == DegradationState::Degraded
    }

    /// 记录一次 L2 故障。
    ///
    /// Active：累计失败，达阈值转 Degraded；HalfOpen：探测失败，回 Degraded。
    pub fn record_failure(&self) -> DegradationState {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.state {
            DegradationState::Active | DegradationState::HalfOpen => {
                inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);
                if inner.consecutive_failures >= self.failure_threshold {
                    self.transition(&mut inner, DegradationState::Degraded);
                }
            }
            DegradationState::Degraded => {}
        }
        inner.state
    }

    /// 记录一次 L2 成功。
    ///
    /// Active：重置失败计数；HalfOpen：探测成功，恢复 Active；Degraded：忽略。
    pub fn record_success(&self) -> DegradationState {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.state {
            DegradationState::Active => {
                inner.consecutive_failures = 0;
            }
            DegradationState::HalfOpen => {
                self.transition(&mut inner, DegradationState::Active);
            }
            DegradationState::Degraded => {}
        }
        inner.state
    }

    /// 是否放行 L2 流量。
    ///
    /// Degraded 状态且降级时长已超过恢复超时 → 转 HalfOpen 并放行探测。
    /// HalfOpen 期间 L2 流量全部放行（并发探测均放行），由探测的
    /// 成功/失败反馈驱动恢复或重回降级。
    pub fn allow_l2(&self) -> bool {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.state {
            DegradationState::Active => true,
            DegradationState::HalfOpen => true,
            DegradationState::Degraded => {
                let elapsed = inner
                    .degraded_since
                    .map(|t| t.elapsed())
                    .unwrap_or(self.recovery_timeout);
                if elapsed >= self.recovery_timeout {
                    self.transition(&mut inner, DegradationState::HalfOpen);
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// L2 降级保护装饰器。
///
/// 包装 L2 后端：状态机关闭（Degraded 未到半开窗口）时直接返回
/// [`OxCacheError::Degraded`]，调用方（ChainCache / Cache 层）回落 L1；
/// 内层成功/失败自动反馈状态机。
pub struct DegradableBackend {
    inner: Arc<dyn CacheBackend>,
    controller: Arc<DegradationController>,
}

impl DegradableBackend {
    /// 包装 L2 后端与降级状态机
    pub fn new(inner: Arc<dyn CacheBackend>, controller: Arc<DegradationController>) -> Self {
        Self { inner, controller }
    }

    /// 状态机
    pub fn controller(&self) -> &Arc<DegradationController> {
        &self.controller
    }

    fn degraded_err() -> OxCacheError {
        OxCacheError::Degraded(
            "L2 degraded: serving L1-only until half-open probe succeeds".to_string(),
        )
    }
}

#[async_trait::async_trait]
impl CacheReader for DegradableBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.get(key).await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.exists(key).await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key).await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        self.inner.stats().await
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait::async_trait]
impl CacheWriter for DegradableBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.set(key, value, ttl).await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.delete(key).await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }

    async fn clear(&self) -> OxCacheResult<()> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.clear().await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.set_many(items).await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        if !self.controller.allow_l2() {
            return Err(Self::degraded_err());
        }
        match self.inner.delete_many(keys).await {
            Ok(v) => {
                self.controller.record_success();
                Ok(v)
            }
            Err(e) => {
                self.controller.record_failure();
                Err(e)
            }
        }
    }
}

#[async_trait::async_trait]
impl CacheConnector for DegradableBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    fn failing_backend() -> Arc<dyn CacheBackend> {
        Arc::new(MockBackend::new("mock", 50, false).with_fail_get().with_fail_set())
    }

    fn healthy_backend() -> Arc<dyn CacheBackend> {
        Arc::new(MockBackend::new("mock", 50, false))
    }

    #[test]
    fn threshold_triggers_degradation() {
        let controller = Arc::new(DegradationController::new(3, Duration::from_secs(60)));
        assert_eq!(controller.state(), DegradationState::Active);

        assert_eq!(controller.record_failure(), DegradationState::Active);
        assert_eq!(controller.record_failure(), DegradationState::Active);
        assert_eq!(
            controller.record_failure(),
            DegradationState::Degraded,
            "连续失败达阈值应降级"
        );
        assert!(controller.is_l1_only());
    }

    #[test]
    fn degraded_blocks_l2_until_timeout() {
        let controller = Arc::new(DegradationController::new(1, Duration::from_millis(80)));
        controller.record_failure();
        assert!(!controller.allow_l2(), "降级窗口内应拦截 L2");

        // 超时后半开放行探测
        std::thread::sleep(Duration::from_millis(100));
        assert!(controller.allow_l2(), "超时后应放行半开探测");
        assert_eq!(controller.state(), DegradationState::HalfOpen);
    }

    #[test]
    fn half_open_success_recovers() {
        let controller = Arc::new(DegradationController::new(1, Duration::from_millis(50)));
        controller.record_failure();
        std::thread::sleep(Duration::from_millis(60));
        assert!(controller.allow_l2());

        controller.record_success();
        assert_eq!(controller.state(), DegradationState::Active, "探测成功应恢复");
        assert!(controller.allow_l2());
    }

    #[test]
    fn half_open_failure_re_degrades() {
        let controller = Arc::new(DegradationController::new(1, Duration::from_millis(50)));
        controller.record_failure();
        std::thread::sleep(Duration::from_millis(60));
        assert!(controller.allow_l2());
        assert_eq!(controller.state(), DegradationState::HalfOpen);

        assert_eq!(
            controller.record_failure(),
            DegradationState::Degraded,
            "探测失败应重新降级"
        );
    }

    #[test]
    fn success_in_active_resets_failure_count() {
        let controller = Arc::new(DegradationController::new(3, Duration::from_secs(60)));
        controller.record_failure();
        controller.record_failure();
        controller.record_success();
        // 计数已重置：需要重新累计到阈值才降级
        assert_eq!(controller.record_failure(), DegradationState::Active);
        assert_eq!(controller.record_failure(), DegradationState::Active);
        assert_eq!(controller.record_failure(), DegradationState::Degraded);
    }

    #[test]
    fn state_change_listeners_notified() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let sink = states.clone();
        let controller = Arc::new(
            DegradationController::new(1, Duration::from_millis(40))
                .on_state_change(move |state| sink.lock().unwrap().push(state)),
        );

        controller.record_failure(); // → Degraded
        std::thread::sleep(Duration::from_millis(50));
        controller.allow_l2(); // → HalfOpen
        controller.record_success(); // → Active

        let observed = states.lock().unwrap().clone();
        assert_eq!(
            observed,
            vec![
                DegradationState::Degraded,
                DegradationState::HalfOpen,
                DegradationState::Active
            ]
        );
    }

    /// 装饰器集成：故障后端自动降级，健康后端保持 Active
    #[tokio::test]
    async fn degradable_backend_short_circuits_when_degraded() {
        let controller = Arc::new(DegradationController::new(2, Duration::from_millis(50)));
        let backend = DegradableBackend::new(failing_backend(), controller.clone());

        // 两次故障达到阈值
        let _ = backend.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await;
        let _ = backend.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await;
        assert_eq!(controller.state(), DegradationState::Degraded);

        // 降级后 get 直接短路返回 Degraded（不再触达内层）
        let err = backend.get("k").await.expect_err("降级期应拒绝 L2");
        assert!(
            matches!(err, OxCacheError::Degraded(_)),
            "应返回 Degraded 错误供上层回落 L1，got {err:?}"
        );

        // 半开探测成功恢复
        std::thread::sleep(Duration::from_millis(60));
        // 换一个健康后端模拟故障恢复（同一 controller）
        let recovered = DegradableBackend::new(healthy_backend(), controller.clone());
        assert!(recovered.get("k").await.unwrap().is_none());
        assert_eq!(controller.state(), DegradationState::Active);
    }

    #[tokio::test]
    async fn healthy_backend_stays_active() {
        let controller = Arc::new(DegradationController::new(2, Duration::from_secs(60)));
        let backend = DegradableBackend::new(healthy_backend(), controller.clone());
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
        assert_eq!(controller.state(), DegradationState::Active);
    }
}
