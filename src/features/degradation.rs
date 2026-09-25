// Copyright (c) 2025-2026 Kirky.X🌠
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
//! # 双层降级/熔断语义
//!
//! L2 故障保护分两层，各自独立计时、独立恢复，互不联动：
//!
//! 1. **外层**（本模块）：[`DegradableBackend`] + [`DegradationController`]，
//!    工作在 L1/L2 链路层。内层任意错误（含内层熔断拒绝）都计为一次外层
//!    失败；降级期拦截返回 [`OxCacheError::Degraded`]，调用方回落 L1。
//!    阈值与恢复窗口由调用方在 [`DegradationController::new`] 指定。
//! 2. **内层**（redis 后端内建）：RedisBackend 自带轻量熔断器（默认连续
//!    5 次失败打开、30s 后半开探测，`backend/memory/redis/builder.rs`），
//!    打开时在连接层直接返回 [`OxCacheError::Degraded`]，不触达网络。
//!
//! **恢复时序叠加**：外层进入 HalfOpen 后放行的探测请求仍可能被尚处
//! Open 的内层熔断拒绝——探测错误随即把外层打回 Degraded（假恢复）。
//! 内层先恢复也不会缩短外层 Degraded 窗口；两层窗口必须分别超时。
//! 因此外层 `recovery_timeout` 应不小于内层 `reset_timeout`，否则外层
//! 每次半开探测都会撞上内层 Open，反复假恢复直到内层真恢复。
//! [`DegradationController::snapshot`] 与 `telemetry` feature 下的
//! [`DegradationTracing`] 提供对该时序的观测面。
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

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
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

    /// 当前状态机的不变更快照（观测用）
    ///
    /// 与 [`DegradationController::allow_l2`] 不同，snapshot 不产生任何
    /// 状态迁移（Degraded 不会因此转 HalfOpen），适合指标导出与健康上报。
    pub fn snapshot(&self) -> DegradationSnapshot {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        DegradationSnapshot {
            state: inner.state,
            consecutive_failures: inner.consecutive_failures,
            failure_threshold: self.failure_threshold,
            degraded_elapsed: if inner.state == DegradationState::Degraded {
                inner.degraded_since.map(|t| t.elapsed())
            } else {
                None
            },
            recovery_timeout: self.recovery_timeout,
        }
    }
}

/// 降级状态机的只读快照
///
/// 由 [`DegradationController::snapshot`] 生成，用于指标导出、健康上报等
/// 观测场景；字段在生成瞬间一致，不随状态机后续变化。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DegradationSnapshot {
    /// 状态机当前状态
    pub state: DegradationState,
    /// 当前连续失败计数（恢复 Active 时归零）
    pub consecutive_failures: u32,
    /// 触发降级的连续失败阈值
    pub failure_threshold: u32,
    /// 处于 Degraded 状态的持续时长；非 Degraded 状态为 `None`
    pub degraded_elapsed: Option<Duration>,
    /// 降级后半开探测前的等待时长配置
    pub recovery_timeout: Duration,
}

impl DegradationSnapshot {
    /// 是否处于 L1-only 降级
    pub fn is_l1_only(&self) -> bool {
        self.state == DegradationState::Degraded
    }

    /// 预测下一次 [`DegradationController::allow_l2`] 是否放行（不触发迁移）。
    ///
    /// Degraded 且降级时长已达 `recovery_timeout` 时为 `true`——下一次
    /// L2 访问将把状态机迁入 HalfOpen。
    pub fn l2_probe_due(&self) -> bool {
        match self.state {
            DegradationState::Active | DegradationState::HalfOpen => true,
            DegradationState::Degraded => {
                self.degraded_elapsed.unwrap_or_default() >= self.recovery_timeout
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

/// 降级状态迁移的 tracing 观测桥（`telemetry` feature）
///
/// 把 [`DegradationController::on_state_change`] 回调转成 tracing 事件：
/// 进入 Degraded 记 `warn`（告警面），HalfOpen/Active 记 `info`（恢复面）。
/// `telemetry` 关闭时不编译，零开销。
///
/// ```rust,ignore
/// use oxcache::features::degradation::{DegradationController, DegradationTracing};
///
/// let controller = DegradationController::new(5, Duration::from_secs(30))
///     .on_state_change(DegradationTracing::listener());
/// ```
#[cfg(feature = "telemetry")]
pub struct DegradationTracing;

#[cfg(feature = "telemetry")]
impl DegradationTracing {
    /// 生成可传入 [`DegradationController::on_state_change`] 的监听闭包
    pub fn listener() -> impl Fn(DegradationState) + Send + Sync + 'static {
        |state| match state {
            DegradationState::Degraded => tracing::warn!(
                target = "oxcache::degradation",
                state = state.as_str(),
                "L2 degradation entered, serving L1-only"
            ),
            DegradationState::HalfOpen => tracing::info!(
                target = "oxcache::degradation",
                state = state.as_str(),
                "L2 degradation half-open, probing recovery"
            ),
            DegradationState::Active => tracing::info!(
                target = "oxcache::degradation",
                state = state.as_str(),
                "L2 degradation recovered"
            ),
        }
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
        Arc::new(
            MockBackend::new("mock", 50, false)
                .with_fail_get()
                .with_fail_set(),
        )
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
        assert_eq!(
            controller.state(),
            DegradationState::Active,
            "探测成功应恢复"
        );
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
        let _ = backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await;
        let _ = backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await;
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

    // ========================================================================
    // DegradationSnapshot 观测快照
    // ========================================================================

    #[test]
    fn snapshot_reflects_lifecycle_without_mutating_state() {
        let controller = Arc::new(DegradationController::new(2, Duration::from_millis(60)));

        let snap = controller.snapshot();
        assert_eq!(snap.state, DegradationState::Active);
        assert_eq!(snap.consecutive_failures, 0);
        assert_eq!(snap.failure_threshold, 2);
        assert_eq!(snap.degraded_elapsed, None);
        assert!(!snap.is_l1_only());
        assert!(snap.l2_probe_due(), "Active 应预测放行 L2");
        assert_eq!(
            controller.state(),
            DegradationState::Active,
            "snapshot 不得触发状态迁移"
        );

        controller.record_failure();
        controller.record_failure();
        let snap = controller.snapshot();
        assert_eq!(snap.state, DegradationState::Degraded);
        assert_eq!(snap.consecutive_failures, 2);
        assert!(snap.degraded_elapsed.is_some());
        assert!(snap.is_l1_only());
        assert!(!snap.l2_probe_due(), "降级窗口内应预测拦截");
        assert_eq!(
            controller.state(),
            DegradationState::Degraded,
            "snapshot 不得把 Degraded 推进到 HalfOpen"
        );
        assert!(!controller.allow_l2(), "降级窗口内 allow_l2 应拦截");
        assert_eq!(controller.state(), DegradationState::Degraded);

        std::thread::sleep(Duration::from_millis(80));
        let snap = controller.snapshot();
        assert!(snap.l2_probe_due(), "超时后应预测放行,但不产生迁移");
        assert_eq!(controller.state(), DegradationState::Degraded);
        assert!(controller.allow_l2());
        assert_eq!(controller.state(), DegradationState::HalfOpen);
        let snap = controller.snapshot();
        assert_eq!(snap.state, DegradationState::HalfOpen);
        assert_eq!(
            snap.degraded_elapsed, None,
            "degraded_elapsed 仅在 Degraded 状态上报"
        );

        controller.record_success();
        let snap = controller.snapshot();
        assert_eq!(snap.state, DegradationState::Active);
        assert_eq!(snap.consecutive_failures, 0);
        assert_eq!(snap.degraded_elapsed, None);
    }

    // ========================================================================
    // 双层熔断恢复时序叠加(外层 DegradationController × 内层后端内建熔断)
    // ========================================================================

    /// 模拟后端内建熔断层的最小后端(与 RedisBackend 内建熔断同构)：
    /// 首次 get 视为真实故障并立即打开熔断(阈值 1)；打开期短路上抛
    /// `Degraded` 且不触达内层、不顺延恢复期限(与 client 层 Open 直接
    /// 返回语义一致)；reset 窗口过后放行探测，一次成功即闭合。
    struct InnerCircuitBackend {
        reset: Duration,
        open_until: Mutex<Option<Instant>>,
        first_failure_pending: std::sync::atomic::AtomicBool,
        admitted: std::sync::atomic::AtomicUsize,
    }

    impl InnerCircuitBackend {
        fn new(reset: Duration) -> Self {
            Self {
                reset,
                open_until: Mutex::new(None),
                first_failure_pending: std::sync::atomic::AtomicBool::new(true),
                admitted: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn admitted(&self) -> usize {
            self.admitted.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl crate::backend::CacheReader for InnerCircuitBackend {
        async fn get(&self, _key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            use std::sync::atomic::Ordering;

            let mut open_until = self.open_until.lock().unwrap();
            if let Some(t) = *open_until {
                if Instant::now() < t {
                    return Err(OxCacheError::Degraded("inner breaker is open".to_string()));
                }
                *open_until = None;
                self.admitted.fetch_add(1, Ordering::SeqCst);
                return Ok(Some(b"probe-ok".to_vec()));
            }
            if self.first_failure_pending.swap(false, Ordering::SeqCst) {
                *open_until = Some(Instant::now() + self.reset);
                return Err(OxCacheError::Operation("inner real failure".to_string()));
            }
            self.admitted.fetch_add(1, Ordering::SeqCst);
            Ok(Some(b"probe-ok".to_vec()))
        }

        async fn exists(&self, _key: &str) -> OxCacheResult<bool> {
            Ok(false)
        }

        async fn ttl(&self, _key: &str) -> OxCacheResult<Option<Duration>> {
            Ok(None)
        }

        async fn len(&self) -> OxCacheResult<u64> {
            Ok(0)
        }

        async fn capacity(&self) -> OxCacheResult<u64> {
            Ok(0)
        }

        async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
            Ok(HashMap::new())
        }

        async fn keys(&self, _pattern: &str) -> OxCacheResult<Vec<String>> {
            Ok(Vec::new())
        }
    }

    #[async_trait::async_trait]
    impl crate::backend::CacheWriter for InnerCircuitBackend {
        async fn set(
            &self,
            _key: Arc<str>,
            _value: Arc<Vec<u8>>,
            _ttl: Option<Duration>,
        ) -> OxCacheResult<()> {
            Ok(())
        }

        async fn delete(&self, _key: &str) -> OxCacheResult<()> {
            Ok(())
        }

        async fn clear(&self) -> OxCacheResult<()> {
            Ok(())
        }

        async fn expire(&self, _key: &str, _ttl: Duration) -> OxCacheResult<bool> {
            Ok(false)
        }

        async fn set_many(&self, _items: &[CacheSetItem]) -> OxCacheResult<()> {
            Ok(())
        }

        async fn delete_many(&self, _keys: &[String]) -> OxCacheResult<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl crate::backend::CacheConnector for InnerCircuitBackend {
        async fn health_check(&self) -> OxCacheResult<()> {
            Ok(())
        }

        async fn shutdown(&self) {}

        fn backend_kind(&self) -> BackendKind {
            BackendKind::Mock
        }
    }

    /// 外层 HalfOpen 探测被仍处 Open 的内层熔断拒绝 → 假恢复：
    /// 外层看似恢复(半开放行)却被探测失败即刻打回 Degraded，
    /// 直到内层 reset 也到期，探测才触达内层并完成真恢复。
    #[tokio::test]
    async fn outer_half_open_probe_rejected_by_inner_open_is_false_recovery() {
        // 内层 reset(400ms) > 外层恢复窗(80ms)：外层半开探测必然撞上内层 Open
        let outer = Arc::new(DegradationController::new(1, Duration::from_millis(80)));
        let inner = Arc::new(InnerCircuitBackend::new(Duration::from_millis(400)));
        let inner_dyn: Arc<dyn CacheBackend> = inner.clone();
        let backend = DegradableBackend::new(inner_dyn, outer.clone());

        // 首次访问：内层真实故障 → 内层熔断打开，外层同步降级
        let err = backend.get("k").await.expect_err("首次访问应失败");
        assert!(
            matches!(err, OxCacheError::Operation(_)),
            "首次应为真实故障而非熔断拒绝, got {err:?}"
        );
        assert_eq!(outer.state(), DegradationState::Degraded);
        assert_eq!(inner.admitted(), 0);

        // 降级窗口内：外层短路，不触达内层
        let err = backend.get("k").await.expect_err("降级窗口内应短路");
        assert!(matches!(err, OxCacheError::Degraded(_)));
        assert_eq!(inner.admitted(), 0);

        // 外层恢复窗先到：转半开，看似恢复
        std::thread::sleep(Duration::from_millis(120));
        let snap = outer.snapshot();
        assert!(snap.l2_probe_due(), "外层窗口已到应预测放行");
        assert!(outer.allow_l2());
        assert_eq!(outer.state(), DegradationState::HalfOpen, "外层看似恢复");

        // 半开探测被内层 Open 拒绝 → 外层打回 Degraded(假恢复)
        let err = backend.get("k").await.expect_err("探测应被内层熔断拒绝");
        assert!(
            matches!(err, OxCacheError::Degraded(_)),
            "内层 Open 拒绝应表现为 Degraded, got {err:?}"
        );
        assert_eq!(
            outer.state(),
            DegradationState::Degraded,
            "探测失败应打回降级(假恢复)"
        );
        assert_eq!(inner.admitted(), 0, "内层打开期探测不应触达内层");
        let snap = outer.snapshot();
        assert_eq!(
            snap.consecutive_failures, 2,
            "初始失败计数仅 Active 归零,半开探测失败再计 1"
        );
        assert!(
            snap.degraded_elapsed.unwrap() < Duration::from_millis(80),
            "重新降级后降级计时重启"
        );

        // 内层 reset 也到期后：再次半开，探测触达内层成功 → 真恢复
        std::thread::sleep(Duration::from_millis(340));
        assert!(backend.get("k").await.unwrap().is_some());
        assert_eq!(
            outer.state(),
            DegradationState::Active,
            "内层放行后应真恢复"
        );
        assert_eq!(inner.admitted(), 1);
        let snap = outer.snapshot();
        assert_eq!(snap.state, DegradationState::Active);
        assert_eq!(snap.consecutive_failures, 0);
        assert_eq!(snap.degraded_elapsed, None);
    }

    /// 内层先恢复不缩短外层 Degraded 窗口：外层窗口内继续短路，
    /// 两层窗口分别到期后才完成一次探测并恢复。
    #[tokio::test]
    async fn inner_recovery_does_not_shortcut_outer_degraded_window() {
        let outer = Arc::new(DegradationController::new(1, Duration::from_millis(200)));
        let inner = Arc::new(InnerCircuitBackend::new(Duration::from_millis(50)));
        let inner_dyn: Arc<dyn CacheBackend> = inner.clone();
        let backend = DegradableBackend::new(inner_dyn, outer.clone());

        let _ = backend.get("k").await;
        assert_eq!(outer.state(), DegradationState::Degraded);

        // 内层 reset(50ms)已过、外层窗(200ms)未到：外层仍短路
        std::thread::sleep(Duration::from_millis(90));
        let err = backend.get("k").await.expect_err("外层窗口内应继续短路");
        assert!(matches!(err, OxCacheError::Degraded(_)));
        assert_eq!(inner.admitted(), 0, "内层先恢复也不得绕过外层窗口");

        // 外层窗口也到：探测触达已恢复的内层 → 一次成功即恢复
        std::thread::sleep(Duration::from_millis(150));
        assert!(backend.get("k").await.unwrap().is_some());
        assert_eq!(outer.state(), DegradationState::Active);
        assert_eq!(inner.admitted(), 1);
    }

    // ========================================================================
    // DegradationTracing 观测桥(telemetry)
    // ========================================================================

    #[cfg(feature = "telemetry")]
    mod telemetry_bridge {
        use super::*;
        use std::fmt;
        use tracing::field::{Field, Visit};
        use tracing::span::{Attributes, Id, Record};
        use tracing::{Event, Metadata, Subscriber};

        struct CapturedEvent {
            level: tracing::Level,
            state: String,
        }

        struct StateVisitor {
            state: String,
        }

        impl Visit for StateVisitor {
            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "state" {
                    self.state = value.to_string();
                }
            }

            fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
                if field.name() == "state" {
                    self.state = format!("{value:?}");
                }
            }
        }

        struct CaptureSubscriber {
            events: Arc<Mutex<Vec<CapturedEvent>>>,
        }

        impl Subscriber for CaptureSubscriber {
            fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _attrs: &Attributes<'_>) -> Id {
                Id::from_u64(1)
            }

            fn record(&self, _span: &Id, _values: &Record<'_>) {}

            fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

            fn event(&self, event: &Event<'_>) {
                let mut visitor = StateVisitor {
                    state: String::new(),
                };
                event.record(&mut visitor);
                self.events.lock().unwrap().push(CapturedEvent {
                    level: *event.metadata().level(),
                    state: visitor.state,
                });
            }

            fn enter(&self, _span: &Id) {}

            fn exit(&self, _span: &Id) {}
        }

        #[test]
        fn tracing_listener_emits_state_transitions() {
            let events = Arc::new(Mutex::new(Vec::new()));
            let sink = events.clone();
            let controller = Arc::new(
                DegradationController::new(1, Duration::from_millis(40))
                    .on_state_change(DegradationTracing::listener()),
            );

            tracing::subscriber::with_default(Arc::new(CaptureSubscriber { events: sink }), || {
                controller.record_failure(); // → Degraded
                std::thread::sleep(Duration::from_millis(50));
                controller.allow_l2(); // → HalfOpen
                controller.record_success(); // → Active
            });

            let events = events.lock().unwrap();
            assert_eq!(events.len(), 3, "三次迁移应各发一条事件");
            assert_eq!(events[0].state, "degraded");
            assert_eq!(events[0].level, tracing::Level::WARN);
            assert_eq!(events[1].state, "half_open");
            assert_eq!(events[1].level, tracing::Level::INFO);
            assert_eq!(events[2].state, "active");
            assert_eq!(events[2].level, tracing::Level::INFO);
        }
    }
}
