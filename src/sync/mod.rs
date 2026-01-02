//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的同步机制，包括批量写入、失效和提升功能。

#[cfg(feature = "test")]
pub mod batch_writer;
#[cfg(feature = "test")]
pub mod common;
#[cfg(feature = "test")]
pub mod invalidation;
#[cfg(feature = "test")]
pub mod optimized_batch_writer;
#[cfg(feature = "test")]
pub mod promotion;
pub mod warmup;

#[cfg(not(feature = "test"))]
pub(crate) mod batch_writer;
#[cfg(not(feature = "test"))]
pub(crate) mod common;
#[cfg(not(feature = "test"))]
pub(crate) mod invalidation;
#[cfg(not(feature = "test"))]
pub(crate) mod optimized_batch_writer;
#[cfg(not(feature = "test"))]
pub(crate) mod promotion;
