// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Features module

#[cfg(feature = "bloom-filter")]
pub mod bloom_filter;

#[cfg(feature = "dist-lock")]
pub mod dist_lock;

#[cfg(feature = "bloom-filter")]
pub use bloom_filter::{BloomFilter, BloomFilterBackend, BloomFilterBackendBuilder};

#[cfg(feature = "dist-lock")]
pub use dist_lock::{DistributedLock, DistLockBuilder};
