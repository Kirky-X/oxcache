// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Distributed lock module for oxcache.
//!
//! Provides Redis-based distributed locking with TTL, automatic renewal,
//! and reentrant support.
//!
//! # Feature Gate
//!
//! This module requires the `lock` feature (included in `full`).

mod builder;
mod lock;
mod provider;
#[cfg(feature = "red-lock")]
pub mod redlock;

pub use builder::DistLockBuilder;
pub use lock::DistributedLock;
pub use provider::LockProvider;
#[cfg(feature = "red-lock")]
pub use redlock::{InMemoryLockNode, RedisLockNode, RedLock, LockNode};

/// Type alias for the default lock provider implementation.
///
/// Downstream crates can depend on this alias for dependency injection
/// without coupling to the concrete `DistributedLock` type.
pub type DefaultLockProvider = DistributedLock;
