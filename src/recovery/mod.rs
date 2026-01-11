//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的恢复机制，包括健康检查和WAL日志。
//! 通过 `wal-recovery` feature 控制启用/禁用

#[cfg(feature = "wal-recovery")]
pub mod health;
#[cfg(feature = "wal-recovery")]
pub mod wal;

#[cfg(not(feature = "wal-recovery"))]
pub(crate) mod health;
#[cfg(not(feature = "wal-recovery"))]
pub(crate) mod wal;

// ============================================================================
// 当 wal-recovery 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "wal-recovery"))]
use crate::error::Result;
#[cfg(not(feature = "wal-recovery"))]
use std::sync::Arc;
#[cfg(not(feature = "wal-recovery"))]
use std::time::Duration;

/// WAL条目（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone)]
pub struct WalEntry {
    pub timestamp: std::time::SystemTime,
    pub operation: Operation,
    pub key: String,
    pub value: Option<Vec<u8>>,
    pub ttl: Option<i64>,
}

/// WAL操作类型（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Set,
    Delete,
}

/// 可重放后端Trait（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[async_trait::async_trait]
pub trait WalReplayableBackend: Clone + Send + Sync + 'static {
    async fn pipeline_replay(&self, _entries: Vec<WalEntry>) -> Result<()>;
}

/// WAL管理器（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone)]
pub struct WalManager;

#[cfg(not(feature = "wal-recovery"))]
impl WalManager {
    pub async fn new(_service_name: &str) -> Result<Self> {
        Ok(Self)
    }

    pub async fn add_entry(&self, _entry: &WalEntry) -> Result<()> {
        Ok(())
    }

    pub async fn append(&self, _entry: WalEntry) -> Result<()> {
        Ok(())
    }

    pub async fn get_entries(&self) -> Result<Vec<WalEntry>> {
        Ok(Vec::new())
    }

    pub async fn clear_entries(&self) -> Result<()> {
        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        Ok(())
    }

    pub async fn clear(&self) -> Result<()> {
        Ok(())
    }

    pub async fn replay_all<B: WalReplayableBackend>(&self, _backend: &B) -> Result<usize> {
        Ok(0)
    }
}

/// 健康状态枚举（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthState {
    Healthy,
    Degraded {
        since: std::time::Instant,
        failure_count: u32,
    },
    Recovering {
        since: std::time::Instant,
        success_count: u32,
    },
    WalReplaying {
        since: std::time::Instant,
    },
}

/// 可健康检查的后端Trait（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[async_trait::async_trait]
pub trait HealthCheckableBackend: Clone + Send + Sync + 'static {
    async fn ping(&self) -> Result<()>;
    fn command_timeout_ms(&self) -> u64;
}

/// 健康检查器（空实现）
#[cfg(not(feature = "wal-recovery"))]
#[derive(Debug)]
pub struct HealthChecker<T: HealthCheckableBackend> {
    l2: Arc<T>,
    state: Arc<tokio::sync::RwLock<HealthState>>,
    wal: Arc<WalManager>,
    service_name: String,
    command_timeout_ms: u64,
}

#[cfg(not(feature = "wal-recovery"))]
impl<T: HealthCheckableBackend + WalReplayableBackend> HealthChecker<T> {
    pub fn new(
        l2: Arc<T>,
        state: Arc<tokio::sync::RwLock<HealthState>>,
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

    pub async fn start(self) {}
}

// 重新导出需要的Trait（当功能禁用时）
#[cfg(not(feature = "wal-recovery"))]
pub use self::wal::WalReplayableBackend as WalReplayableBackendTrait;
