//! Circuit Breaker 模块 - 熔断器模式实现
//!
//! 三态熔断器：Closed（关闭）/ Open（打开）/ Half-Open（半开）
//! 用于保护后端服务，防止级联故障。
//!
//! # 状态转换
//!
//! ```text
//! Closed → Open: 失败次数达到阈值
//! Open → Half-Open: 超过恢复超时
//! Half-Open → Closed: 探测请求成功
//! Half-Open → Open: 探测请求失败
//! ```

use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// 熔断器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitState {
    /// 关闭状态 - 正常执行请求
    Closed = 0,
    /// 打开状态 - 拒绝所有请求
    Open = 1,
    /// 半开状态 - 允许有限探测请求
    HalfOpen = 2,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "Closed"),
            CircuitState::Open => write!(f, "Open"),
            CircuitState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 失败次数阈值，达到后切换到 Open 状态
    pub failure_threshold: u32,
    /// 恢复超时，超过后从 Open 切换到 Half-Open
    pub recovery_timeout: Duration,
    /// Half-Open 状态允许的最大探测请求数
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            half_open_max_calls: 1,
        }
    }
}

/// 熔断器
///
/// 使用原子操作实现无锁状态转换，适用于高并发场景。
pub struct CircuitBreaker {
    /// 当前状态：0=Closed, 1=Open, 2=HalfOpen
    state: AtomicU8,
    /// 失败计数
    failure_count: AtomicU32,
    /// 切换到 Open 状态的时刻（monotonic 时间戳的纳秒偏移）
    opened_at_nanos: AtomicU64,
    /// Half-Open 状态下的探测请求计数
    half_open_call_count: AtomicU32,
    /// 实例创建时间点，用于计算相对时间
    created_at: Instant,
    /// 配置参数
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// 创建新的熔断器实例
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU32::new(0),
            opened_at_nanos: AtomicU64::new(0),
            half_open_call_count: AtomicU32::new(0),
            created_at: Instant::now(),
            config,
        }
    }

    /// 检查是否允许执行请求
    pub fn can_execute(&self) -> bool {
        match self.current_state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if self.should_attempt_reset() {
                    self.transition_to_half_open()
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                let calls = self.half_open_call_count.fetch_add(1, Ordering::Relaxed);
                calls < self.config.half_open_max_calls
            }
        }
    }

    /// 记录成功
    pub fn record_success(&self) {
        match self.current_state() {
            CircuitState::HalfOpen => {
                self.transition_to_closed();
            }
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                self.transition_to_closed();
            }
        }
    }

    /// 记录失败
    pub fn record_failure(&self) {
        match self.current_state() {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                self.transition_to_open();
            }
            CircuitState::Open => {}
        }
    }

    /// 获取当前状态
    pub fn state(&self) -> CircuitState {
        self.current_state()
    }

    /// 重置熔断器
    pub fn reset(&self) {
        self.transition_to_closed();
    }

    /// 获取失败计数
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// 获取配置
    pub fn config(&self) -> &CircuitBreakerConfig {
        &self.config
    }

    // =========================================================================
    // 内部方法
    // =========================================================================

    fn current_state(&self) -> CircuitState {
        match self.state.load(Ordering::Relaxed) {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    fn should_attempt_reset(&self) -> bool {
        let opened_at = self.opened_at_nanos.load(Ordering::Relaxed);
        if opened_at == 0 {
            return true;
        }

        // 计算自打开以来经过的时间
        let now_offset = self.created_at.elapsed().as_nanos() as u64;
        let elapsed = now_offset.saturating_sub(opened_at);
        let recovery_timeout_nanos = self.config.recovery_timeout.as_nanos() as u64;

        elapsed >= recovery_timeout_nanos
    }

    fn transition_to_open(&self) {
        self.state.store(CircuitState::Open as u8, Ordering::Release);
        // 记录当前时间点相对于创建时间的偏移
        self.opened_at_nanos
            .store(self.created_at.elapsed().as_nanos() as u64, Ordering::Relaxed);
        self.half_open_call_count.store(0, Ordering::Relaxed);
    }

    fn transition_to_half_open(&self) -> bool {
        let old = self.state.load(Ordering::Relaxed);
        if old == CircuitState::Open as u8 {
            self.state
                .compare_exchange(
                    old,
                    CircuitState::HalfOpen as u8,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
        } else {
            false
        }
    }

    fn transition_to_closed(&self) {
        self.state.store(CircuitState::Closed as u8, Ordering::Release);
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_call_count.store(0, Ordering::Relaxed);
        self.opened_at_nanos.store(0, Ordering::Relaxed);
    }
}

impl fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("state", &self.current_state())
            .field("failure_count", &self.failure_count.load(Ordering::Relaxed))
            .field("config", &self.config)
            .finish()
    }
}

// ============================================================================
// Feature-gated empty implementation
// ============================================================================

#[cfg(not(feature = "circuit-breaker"))]
pub struct CircuitBreaker {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(not(feature = "circuit-breaker"))]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub half_open_max_calls: u32,
}

#[cfg(not(feature = "circuit-breaker"))]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[cfg(not(feature = "circuit-breaker"))]
impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            half_open_max_calls: 1,
        }
    }
}

#[cfg(not(feature = "circuit-breaker"))]
impl CircuitBreaker {
    pub fn new(_config: CircuitBreakerConfig) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn can_execute(&self) -> bool {
        true
    }

    pub fn record_success(&self) {}

    pub fn record_failure(&self) {}

    pub fn state(&self) -> CircuitState {
        CircuitState::Closed
    }

    pub fn reset(&self) {}

    pub fn failure_count(&self) -> u32 {
        0
    }

    pub fn config(&self) -> &CircuitBreakerConfig {
        unimplemented!("Circuit breaker feature not enabled")
    }
}

#[cfg(not(feature = "circuit-breaker"))]
impl fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CircuitBreaker").finish()
    }
}
