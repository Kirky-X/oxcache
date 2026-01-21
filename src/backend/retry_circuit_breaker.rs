//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 重试与熔断器模块
//!
//! 提供指数退避重试和熔断器模式，增强系统容错能力。

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// 重试策略类型
#[derive(Debug, Clone, PartialEq)]
pub enum RetryStrategy {
    /// 立即重试
    Immediate,
    /// 固定间隔重试
    Fixed(Duration),
    /// 线性退避重试（每次增加固定时间）
    Linear {
        /// 初始间隔
        base_interval: Duration,
        /// 最大间隔
        max_interval: Duration,
        /// 最大重试次数
        max_retries: u32,
    },
    /// 指数退避重试（每次间隔翻倍）
    Exponential {
        /// 初始间隔
        base_interval: Duration,
        /// 最大间隔
        max_interval: Duration,
        /// 最大重试次数
        max_retries: u32,
        /// 是否添加随机抖动
        with_jitter: bool,
    },
}

impl Default for RetryStrategy {
    fn default() -> Self {
        RetryStrategy::Exponential {
            base_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            max_retries: 5,
            with_jitter: true,
        }
    }
}

/// 重试结果
#[derive(Debug, Clone)]
pub enum RetryResult<T> {
    /// 成功
    Success(T),
    /// 重试耗尽后失败
    Exhausted(T),
    /// 错误
    Error(String),
}

impl<T> RetryResult<T> {
    /// 检查是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, RetryResult::Success(_))
    }

    /// 获取成功值
    pub fn success(&self) -> Option<&T> {
        match self {
            RetryResult::Success(v) => Some(v),
            _ => None,
        }
    }
}

/// 可重试的操作
pub trait RetryableOp<T, E> {
    /// 执行操作
    fn execute(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>;
}

/// 重试器
///
/// 使用配置的策略执行重试逻辑。
#[derive(Clone)]
pub struct Retryer {
    strategy: RetryStrategy,
    /// 总重试次数
    total_retries: Arc<AtomicU64>,
    /// 成功次数
    success_count: Arc<AtomicU64>,
    /// 失败次数（重试耗尽）
    exhausted_count: Arc<AtomicU64>,
}

impl Retryer {
    /// 创建新的重试器
    pub fn new(strategy: RetryStrategy) -> Self {
        Self {
            strategy,
            total_retries: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            exhausted_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 使用默认策略创建重试器
    pub fn default() -> Self {
        Self::new(RetryStrategy::default())
    }

    /// 执行重试操作
    ///
    /// # 参数
    /// * `op` - 可重试的操作
    ///
    /// # 返回值
    /// 重试结果
    pub async fn retry<F, T, E>(&self, mut op: F) -> RetryResult<T>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let (max_retries, base_interval, max_interval, with_jitter) = match &self.strategy {
            RetryStrategy::Immediate => {
                return match op.await {
                    Ok(v) => {
                        self.success_count.fetch_add(1, Ordering::Relaxed);
                        RetryResult::Success(v)
                    }
                    Err(e) => {
                        self.exhausted_count.fetch_add(1, Ordering::Relaxed);
                        RetryResult::Error(e.to_string())
                    }
                };
            }
            RetryStrategy::Fixed(interval) => (u32::MAX, *interval, *interval, false),
            RetryStrategy::Linear {
                base_interval,
                max_interval,
                max_retries,
            } => (*max_retries, *base_interval, *max_interval, false),
            RetryStrategy::Exponential {
                base_interval,
                max_interval,
                max_retries,
                with_jitter,
            } => (*max_retries, *base_interval, *max_interval, *with_jitter),
        };

        let mut attempt = 0u32;
        let mut interval = base_interval;

        loop {
            match op.await {
                Ok(v) => {
                    self.success_count.fetch_add(1, Ordering::Relaxed);
                    return RetryResult::Success(v);
            }
            Err(e) => {
                attempt += 1;
                self.total_retries.fetch_add(1, Ordering::Relaxed);

                if attempt > max_retries {
                    self.exhausted_count.fetch_add(1, Ordering::Relaxed);
                    error!("Retry exhausted after {} attempts: {}", attempt, e);
                    return RetryResult::Exhausted(e);
                }

                // 计算延迟
                let delay = if with_jitter {
                    // 添加随机抖动（0.5x - 1.5x）
                    let jitter: f64 = rand::random();
                    let factor = 0.5 + jitter;
                    let mut d = interval.mul_f64(factor);
                    // 限制最大延迟
                    if d > max_interval {
                        d = max_interval;
                    }
                    d
                } else {
                    interval
                };

                debug!(
                    "Retry attempt {}/{} after error: {}, waiting {:?}",
                    attempt, max_retries, e, delay
                );

                tokio::time::sleep(delay).await;

                // 增加间隔（指数退避或线性增长）
                interval = (interval * 2).min(max_interval);
            }
        }
    }

    /// 统计信息
    pub fn stats(&self) -> RetryStats {
        let total = self.total_retries.load(Ordering::Relaxed);
        let success = self.success_count.load(Ordering::Relaxed);
        let exhausted = self.exhausted_count.load(Ordering::Relaxed);

        RetryStats {
            total_retries: total,
            success_count: success,
            exhausted_count: exhausted,
            success_rate: if success + exhausted > 0 {
                success as f64 / (success + exhausted) as f64
            } else {
                0.0
            },
        }
    }
}

/// 重试统计
#[derive(Debug, Clone)]
pub struct RetryStats {
    /// 总重试次数
    pub total_retries: u64,
    /// 成功次数
    pub success_count: u64,
    /// 失败次数（重试耗尽）
    pub exhausted_count: u64,
    /// 成功率
    pub success_rate: f64,
}

/// 熔断器状态
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    /// 关闭状态，正常运行
    Closed,
    /// 打开状态，快速失败
    Open,
    /// 半开状态，尝试恢复
    HalfOpen,
}

/// 熔断器事件
#[derive(Debug, Clone)]
pub enum CircuitBreakerEvent {
    /// 状态转换
    StateTransition(CircuitBreakerState, CircuitBreakerState),
    /// 成功
    Success,
    /// 失败
    Failure,
    /// 资源拒绝
    Rejected,
}

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 失败阈值（打开熔断器）
    pub failure_threshold: u64,
    /// 成功阈值（半开转关闭）
    pub success_threshold: u64,
    /// 打开状态持续时间
    pub open_duration: Duration,
    /// 半开状态最大尝试次数
    pub half_open_max_calls: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_duration: Duration::from_secs(60),
            half_open_max_calls: 10,
        }
    }
}

/// 熔断器
///
/// 实现熔断器模式，防止级联故障。
/// 状态转换：
/// - Closed -> Open: 连续失败超过阈值
/// - Open -> HalfOpen: 等待时间结束
/// - HalfOpen -> Closed: 连续成功超过阈值
/// - HalfOpen -> Open: 任何失败
#[derive(Clone)]
pub struct CircuitBreaker {
    /// 配置
    config: CircuitBreakerConfig,
    /// 当前状态
    state: Arc<Mutex<CircuitBreakerState>>,
    /// 状态开始时间
    state_since: Arc<Mutex<Instant>>,
    /// 连续失败计数
    failure_count: Arc<AtomicU64>,
    /// 连续成功计数
    success_count: Arc<AtomicU64>,
    /// 半开状态已尝试次数
    half_open_calls: Arc<AtomicU64>,
    /// 总调用次数
    total_calls: Arc<AtomicU64>,
    /// 被拒绝的调用次数
    rejected_calls: Arc<AtomicU64>,
    /// 成功次数
    success_calls: Arc<AtomicU64>,
    /// 失败次数
    failed_calls: Arc<AtomicU64>,
    /// 事件日志
    events: Arc<Mutex<Vec<CircuitBreakerEvent>>>,
}

impl CircuitBreaker {
    /// 创建新的熔断器
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(CircuitBreakerState::Closed)),
            state_since: Arc::new(Mutex::new(Instant::now())),
            failure_count: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            half_open_calls: Arc::new(AtomicU64::new(0)),
            total_calls: Arc::new(AtomicU64::new(0)),
            rejected_calls: Arc::new(AtomicU64::new(0)),
            success_calls: Arc::new(AtomicU64::new(0)),
            failed_calls: Arc::new(AtomicU64::new(0)),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 使用默认配置创建熔断器
    pub fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// 检查是否允许执行
    async fn check_permission(&self) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();

        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // 检查是否应该转换到半开状态
                let state_since = *self.state_since.lock().await;
                if now.duration_since(state_since) >= self.config.open_duration {
                    // 转换到半开状态
                    *state = CircuitBreakerState::HalfOpen;
                    *self.state_since.lock().await = now;
                    self.half_open_calls.store(0, Ordering::Relaxed);
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                    self.log_event(CircuitBreakerEvent::StateTransition(
                        CircuitBreakerState::Open,
                        CircuitBreakerState::HalfOpen,
                    ));
                    debug!("Circuit breaker: Open -> HalfOpen");
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => {
                // 检查是否还有尝试次数
                let calls = self.half_open_calls.load(Ordering::Relaxed);
                calls < self.config.half_open_max_calls
            }
        }
    }

    /// 记录事件
    fn log_event(&self, event: CircuitBreakerEvent) {
        let mut events = self.events.blocking_lock();
        // 保留最近 100 个事件
        if events.len() >= 100 {
            events.remove(0);
        }
        events.push(event);
    }

    /// 执行受保护的操作
    ///
    /// # 参数
    /// * `op` - 要执行的操作
    ///
    /// # 返回值
    /// 操作结果或熔断器错误
    pub async fn call<F, T, E>(&self, op: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        self.total_calls.fetch_add(1, Ordering::Relaxed);

        // 检查是否允许执行
        if !self.check_permission().await {
            self.rejected_calls.fetch_add(1, Ordering::Relaxed);
            self.log_event(CircuitBreakerEvent::Rejected);
            return Err(CircuitBreakerError::Rejected);
        }

        let state = self.state.lock().await;
        match *state {
            CircuitBreakerState::HalfOpen => {
                self.half_open_calls.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        drop(state);

        // 执行操作
        match op.await {
            Ok(v) => {
                self.on_success().await;
                Ok(v)
            }
            Err(e) => {
                self.on_failure().await;
                Err(CircuitBreakerError::CircuitOpen(e.to_string()))
            }
        }
    }

    /// 操作成功回调
    async fn on_success(&self) {
        self.success_calls.fetch_add(1, Ordering::Relaxed);
        self.log_event(CircuitBreakerEvent::Success);

        let mut state = self.state.lock().await;
        let mut success_count = self.success_count.fetch_add(1, Ordering::Relaxed);
        success_count += 1;

        match *state {
            CircuitBreakerState::Closed => {
                // 重置失败计数
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitBreakerState::HalfOpen => {
                if success_count >= self.config.success_threshold {
                    // 半开转关闭
                    *state = CircuitBreakerState::Closed;
                    *self.state_since.lock().await = Instant::now();
                    self.log_event(CircuitBreakerEvent::StateTransition(
                        CircuitBreakerState::HalfOpen,
                        CircuitBreakerState::Closed,
                    ));
                    debug!("Circuit breaker: HalfOpen -> Closed");
                }
            }
            _ => {}
        }
    }

    /// 操作失败回调
    async fn on_failure(&self) {
        self.failed_calls.fetch_add(1, Ordering::Relaxed);
        self.log_event(CircuitBreakerEvent::Failure);

        let mut state = self.state.lock().await;
        let mut failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed);
        failure_count += 1;

        match *state {
            CircuitBreakerState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    // 关闭转打开
                    *state = CircuitBreakerState::Open;
                    *self.state_since.lock().await = Instant::now();
                    self.log_event(CircuitBreakerEvent::StateTransition(
                        CircuitBreakerState::Closed,
                        CircuitBreakerState::Open,
                    ));
                    warn!(
                        "Circuit breaker: Closed -> Open ({} failures)",
                        failure_count
                    );
                }
            }
            CircuitBreakerState::HalfOpen => {
                // 半开转打开
                *state = CircuitBreakerState::Open;
                *self.state_since.lock().await = Instant::now();
                self.log_event(CircuitBreakerEvent::StateTransition(
                    CircuitBreakerState::HalfOpen,
                    CircuitBreakerState::Open,
                ));
                warn!("Circuit breaker: HalfOpen -> Open (failure in half-open state)");
            }
            _ => {}
        }
    }

    /// 获取当前状态
    pub async fn state(&self) -> CircuitBreakerState {
        *self.state.lock().await
    }

    /// 获取状态持续时间
    pub async fn state_duration(&self) -> Duration {
        let state_since = *self.state_since.lock().await;
        Instant::now().duration_since(state_since)
    }

    /// 统计信息
    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            total_calls: self.total_calls.load(Ordering::Relaxed),
            rejected_calls: self.rejected_calls.load(Ordering::Relaxed),
            success_calls: self.success_calls.load(Ordering::Relaxed),
            failed_calls: self.failed_calls.load(Ordering::Relaxed),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
        }
    }

    /// 手动重置熔断器
    pub async fn reset(&self) {
        let mut state = self.state.lock().await;
        *state = CircuitBreakerState::Closed;
        *self.state_since.lock().await = Instant::now();
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_calls.store(0, Ordering::Relaxed);
        self.events.lock().clear();
        debug!("Circuit breaker manually reset");
    }

    /// 手动强制打开熔断器
    pub async fn force_open(&self) {
        let mut state = self.state.lock().await;
        *state = CircuitBreakerState::Open;
        *self.state_since.lock().await = Instant::now();
        self.log_event(CircuitBreakerEvent::StateTransition(
            CircuitBreakerState::Closed,
            CircuitBreakerState::Open,
        ));
        debug!("Circuit breaker force opened");
    }
}

/// 熔断器错误
#[derive(Debug, Clone)]
pub enum CircuitBreakerError<E> {
    /// 熔断器打开，调用被拒绝
    Rejected,
    /// 熔断器打开，操作失败
    CircuitOpen(String),
    /// 内部错误
    Inner(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::Rejected => write!(f, "Circuit breaker is open, call rejected"),
            CircuitBreakerError::CircuitOpen(msg) => {
                write!(f, "Circuit breaker is open: {}", msg)
            }
            CircuitBreakerError::Inner(e) => write!(f, "Inner error: {}", e),
        }
    }
}

impl<E: std::fmt::Display> std::error::Error for CircuitBreakerError<E> {}

/// 熔断器统计
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerStats {
    /// 总调用次数
    pub total_calls: u64,
    /// 被拒绝次数
    pub rejected_calls: u64,
    /// 成功次数
    pub success_calls: u64,
    /// 失败次数
    pub failed_calls: u64,
    /// 连续失败计数
    pub failure_count: u64,
    /// 连续成功计数
    pub success_count: u64,
}

/// 带重试和熔断的客户端
///
/// 组合重试器和熔断器的包装器。
#[derive(Clone)]
pub struct ResilientClient {
    /// 重试器
    retryer: Retryer,
    /// 熔断器
    circuit_breaker: CircuitBreaker,
}

impl ResilientClient {
    /// 创建新的弹性客户端
    pub fn new(retry_strategy: RetryStrategy, circuit_config: CircuitBreakerConfig) -> Self {
        Self {
            retryer: Retryer::new(retry_strategy),
            circuit_breaker: CircuitBreaker::new(circuit_config),
        }
    }

    /// 使用默认配置创建
    pub fn default() -> Self {
        Self::new(RetryStrategy::default(), CircuitBreakerConfig::default())
    }

    /// 执行受保护的操作
    ///
    /// # 参数
    /// * `op` - 要执行的操作
    ///
    /// # 返回值
    /// 操作结果
    pub async fn execute<F, T, E>(&self, op: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display + Send + 'static,
    {
        self.circuit_breaker
            .call(async {
                // 使用重试器包装操作
                self.retryer.retry(op).await.into()
            })
            .await
    }

    /// 获取统计信息
    pub fn stats(&self) -> ResilientClientStats {
        let retry_stats = self.retryer.stats();
        let cb_stats = self.circuit_breaker.stats();

        ResilientClientStats {
            retry_total_retries: retry_stats.total_retries,
            retry_success_count: retry_stats.success_count,
            retry_exhausted_count: retry_stats.exhausted_count,
            retry_success_rate: retry_stats.success_rate,
            cb_total_calls: cb_stats.total_calls,
            cb_rejected_calls: cb_stats.rejected_calls,
            cb_success_calls: cb_stats.success_calls,
            cb_failed_calls: cb_stats.failed_calls,
        }
    }

    /// 获取当前状态
    pub async fn circuit_state(&self) -> CircuitBreakerState {
        self.circuit_breaker.state().await
    }
}

/// 将 RetryResult 转换为 Result
impl<T> From<RetryResult<T>> for Result<T, String> {
    fn from(result: RetryResult<T>) -> Self {
        match result {
            RetryResult::Success(v) => Ok(v),
            RetryResult::Exhausted(_) => Err("Retry exhausted".to_string()),
            RetryResult::Error(e) => Err(e),
        }
    }
}

/// 弹性客户端统计
#[derive(Debug, Clone, Default)]
pub struct ResilientClientStats {
    /// 重试总次数
    pub retry_total_retries: u64,
    /// 重试成功次数
    pub retry_success_count: u64,
    /// 重试耗尽次数
    pub retry_exhausted_count: u64,
    /// 重试成功率
    pub retry_success_rate: f64,
    /// 熔断器总调用次数
    pub cb_total_calls: u64,
    /// 熔断器被拒绝次数
    pub cb_rejected_calls: u64,
    /// 熔断器成功次数
    pub cb_success_calls: u64,
    /// 熔断器失败次数
    pub cb_failed_calls: u64,
}
