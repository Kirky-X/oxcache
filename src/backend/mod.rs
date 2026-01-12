//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的后端提供者，包括L1和L2缓存后端。

#[cfg(feature = "test")]
pub mod l1;

#[cfg(feature = "test")]
pub mod l2;

#[cfg(feature = "test")]
pub mod redis_provider;

#[cfg(not(feature = "test"))]
pub mod l1;

#[cfg(not(feature = "test"))]
pub mod l2;

#[cfg(not(feature = "test"))]
pub mod redis_provider;
