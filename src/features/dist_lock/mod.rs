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

pub use builder::DistLockBuilder;
pub use lock::DistributedLock;
