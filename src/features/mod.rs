// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Features module

#[cfg(feature = "bloom")]
pub mod bloom_filter;

#[cfg(feature = "lock")]
pub mod dist_lock;

#[cfg(feature = "bloom")]
pub use bloom_filter::{BloomFilter, BloomFilterBackend, BloomFilterBackendBuilder};

#[cfg(feature = "lock")]
pub use dist_lock::{DistLockBuilder, DistributedLock};
