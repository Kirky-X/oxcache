//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 策略模式模块
//!
//! 提供 L2 后端策略抽象，支持不同的 Redis 部署模式（单机、哨兵、集群）
//! 作为 L2Backend 的策略实现基础。

pub mod traits;
pub use traits::{HealthStatus, L2BackendStrategy, ScanResult};

#[cfg(feature = "l2-redis")]
pub mod standalone;

#[cfg(feature = "l2-redis")]
pub mod sentinel;

#[cfg(feature = "l2-redis")]
pub mod cluster;

#[cfg(feature = "l2-redis")]
pub mod facade;
