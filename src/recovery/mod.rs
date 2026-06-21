//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的恢复机制，包括 WAL 日志。
//! 通过 `wal-recovery` feature 控制启用/禁用

#[cfg(feature = "wal-recovery")]
pub mod wal;

#[cfg(not(feature = "wal-recovery"))]
pub(crate) mod wal;

// ============================================================================
// 当 wal-recovery 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "wal-recovery"))]
use crate::error::Result;

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
