//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的恢复机制，包括健康检查和WAL日志。

#[cfg(feature = "test")]
pub mod health;
#[cfg(feature = "test")]
pub mod wal;

#[cfg(not(feature = "test"))]
pub(crate) mod health;
#[cfg(not(feature = "test"))]
pub(crate) mod wal;
