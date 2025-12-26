//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的健康检查和状态恢复机制。

use crate::backend::l2::L2Backend;
use crate::recovery::wal::WalEntry;
use crate::recovery::wal::WalManager;
use crate::recovery::wal::WalReplayableBackend;
pub use crate::recovery::wal::WalReplayableBackend as WalReplayableBackendTrait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;

/// 可健康检查的后端Trait
///
/// 定义健康检查所需的方法
#[allow(async_fn_in_trait)]
pub trait HealthCheckableBackend: Clone + Send + Sync + 'static {
    /// 检查连接是否正常
    async fn ping(&self) -> crate::error::Result<()>;
    /// 获取命令超时时间（毫秒）
    fn command_timeout_ms(&self) -> u64;
}

/// 为L2Backend实现WalReplayableBackend
impl WalReplayableBackend for L2Backend {
    async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> crate::error::Result<()> {
        L2Backend::pipeline_replay(self, entries).await
    }
}

/// 为L2Backend实现HealthCheckableBackend
impl HealthCheckableBackend for L2Backend {
    async fn ping(&self) -> crate::error::Result<()> {
        L2Backend::ping(self).await
    }

    fn command_timeout_ms(&self) -> u64 {
        L2Backend::command_timeout_ms(self)
    }
}

/// 健康状态枚举
///
/// 定义缓存系统的健康状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthState {
    /// 健康状态
    Healthy,
    /// 降级状态
    Degraded { since: Instant, failure_count: u32 },
    /// 恢复中状态
    Recovering { since: Instant, success_count: u32 },
}

/// 健康检查器
///
/// 负责定期检查L2缓存的健康状态，并在必要时进行恢复
pub struct HealthChecker<T: HealthCheckableBackend> {
    /// L2缓存后端
    l2: Arc<T>,
    /// 健康状态
    state: Arc<RwLock<HealthState>>,
    /// WAL管理器
    wal: Arc<WalManager>,
    /// 服务名称
    service_name: String,
    /// 命令超时时间（毫秒）
    command_timeout_ms: u64,
}

impl<T: HealthCheckableBackend + WalReplayableBackend> HealthChecker<T> {
    /// 创建新的健康检查器
    ///
    /// # 参数
    ///
    /// * `l2` - L2缓存后端
    /// * `state` - 健康状态
    /// * `wal` - WAL管理器
    /// * `service_name` - 服务名称
    ///
    /// # 返回值
    ///
    /// 返回新的健康检查器实例
    pub fn new(
        l2: Arc<T>,
        state: Arc<RwLock<HealthState>>,
        wal: Arc<WalManager>,
        service_name: String,
        command_timeout_ms: u64,
    ) -> Self {
        Self {
            l2,
            state,
            wal,
            service_name,
            command_timeout_ms,
        }
    }

    /// 启动健康检查
    ///
    /// 定期检查L2缓存的健康状态，并根据检查结果更新状态和执行相应操作
    pub async fn start(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            let is_healthy = match timeout(
                Duration::from_millis(self.command_timeout_ms),
                self.l2.ping(),
            )
            .await
            {
                Ok(Ok(())) => {
                    tracing::trace!("服务 {} ping成功", self.service_name);
                    true
                }
                Ok(Err(e)) => {
                    tracing::debug!("服务 {} ping失败: {}", self.service_name, e);
                    false
                }
                Err(_) => {
                    tracing::debug!(
                        "服务 {} ping超时 ({}ms)",
                        self.service_name,
                        self.command_timeout_ms
                    );
                    false
                }
            };

            let current_state = *self.state.read().await;
            tracing::debug!(
                "服务 {} 健康检查: is_healthy={}, 当前状态={:?}, 即将获取写锁",
                self.service_name,
                is_healthy,
                current_state
            );

            let mut state_guard = self.state.write().await;
            tracing::debug!(
                "服务 {} 获取写锁成功，当前状态={:?}",
                self.service_name,
                *state_guard
            );

            let new_state = match *state_guard {
                HealthState::Healthy => {
                    if !is_healthy {
                        tracing::warn!("服务 {} L2已降级", self.service_name);
                        HealthState::Degraded {
                            since: Instant::now(),
                            failure_count: 1,
                        }
                    } else {
                        tracing::debug!("服务 {} 保持健康状态", self.service_name);
                        HealthState::Healthy
                    }
                }
                HealthState::Degraded {
                    since,
                    failure_count,
                } => {
                    tracing::debug!(
                        "服务 {} Degraded状态检查: is_healthy={}, failure_count={}, since={:?}",
                        self.service_name,
                        is_healthy,
                        failure_count,
                        since
                    );
                    if is_healthy {
                        tracing::info!(
                            "服务 {} L2正在恢复 (failure_count={})",
                            self.service_name,
                            failure_count
                        );
                        tracing::debug!(
                            "服务 {} 状态转换: Degraded -> Recovering",
                            self.service_name
                        );
                        HealthState::Recovering {
                            since: Instant::now(),
                            success_count: 1,
                        }
                    } else if failure_count >= 3 {
                        tracing::debug!(
                            "服务 {} 保持降级状态 (failure_count={} >= 3)",
                            self.service_name,
                            failure_count
                        );
                        HealthState::Degraded {
                            since,
                            failure_count,
                        }
                    } else {
                        tracing::debug!(
                            "服务 {} 增加失败计数: {} -> {}",
                            self.service_name,
                            failure_count,
                            failure_count + 1
                        );
                        HealthState::Degraded {
                            since,
                            failure_count: failure_count + 1,
                        }
                    }
                }
                HealthState::Recovering {
                    since,
                    success_count,
                } => {
                    if !is_healthy {
                        tracing::info!(
                            "服务 {} 恢复失败，回到降级状态 (success_count={})",
                            self.service_name,
                            success_count
                        );
                        HealthState::Degraded {
                            since: Instant::now(),
                            failure_count: 1,
                        }
                    } else if success_count >= 3 {
                        tracing::info!(
                            "服务 {} 达到恢复条件，开始重放WAL (success_count={})",
                            self.service_name,
                            success_count
                        );
                        // 重放WAL
                        drop(state_guard); // 重放期间释放锁
                        match self.wal.replay_all(&self.l2).await {
                            Ok(count) => {
                                tracing::info!(
                                    "服务 {} WAL已重放: {} 条目",
                                    self.service_name,
                                    count
                                );
                                state_guard = self.state.write().await;
                                HealthState::Healthy
                            }
                            Err(e) => {
                                tracing::error!("服务 {} WAL重放失败: {}", self.service_name, e);
                                state_guard = self.state.write().await;
                                HealthState::Recovering {
                                    since,
                                    success_count,
                                }
                            }
                        }
                    } else {
                        tracing::debug!(
                            "服务 {} 增加恢复计数: {} -> {}",
                            self.service_name,
                            success_count,
                            success_count + 1
                        );
                        HealthState::Recovering {
                            since,
                            success_count: success_count + 1,
                        }
                    }
                }
            };

            if *state_guard != new_state {
                tracing::info!(
                    "服务 {} 健康状态变更: {:?} -> {:?}",
                    self.service_name,
                    *state_guard,
                    new_state
                );
                *state_guard = new_state;
                // 更新指标
                let status_code = match new_state {
                    HealthState::Healthy => 1,
                    HealthState::Recovering { .. } => 2,
                    HealthState::Degraded { .. } => 0,
                };
                crate::metrics::GLOBAL_METRICS.set_health(&self.service_name, status_code);
            } else {
                tracing::debug!(
                    "服务 {} 健康状态未变更: {:?} (ping结果={})",
                    self.service_name,
                    *state_guard,
                    is_healthy
                );
            }
        }
    }
}
