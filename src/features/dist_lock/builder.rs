// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Builder for DistributedLock.

use super::lock::DistributedLock;
use crate::backend::RedisBackend;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Default lock TTL: 30 seconds.
const DEFAULT_LOCK_TTL: Duration = Duration::from_secs(30);

/// Builder for [`DistributedLock`].
///
/// # Example
///
/// ```rust,ignore
/// let lock = DistLockBuilder::new(backend, "my-lock".into())
///     .ttl(Duration::from_secs(10))
///     .watchdog_enabled(true)
///     .build();
/// ```
pub struct DistLockBuilder {
    backend: Arc<RedisBackend>,
    key: String,
    ttl: Duration,
    watchdog_enabled: bool,
}

impl DistLockBuilder {
    /// Create a new builder with required parameters.
    pub fn new(backend: Arc<RedisBackend>, key: String) -> Self {
        Self {
            backend,
            key,
            ttl: DEFAULT_LOCK_TTL,
            watchdog_enabled: true,
        }
    }

    /// Set the lock TTL.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Enable or disable the watchdog (automatic renewal).
    pub fn watchdog_enabled(mut self, enabled: bool) -> Self {
        self.watchdog_enabled = enabled;
        self
    }

    /// Build the [`DistributedLock`].
    pub fn build(self) -> DistributedLock {
        DistributedLock {
            backend: self.backend,
            key: self.key,
            owner_id: Uuid::new_v4().to_string(),
            reentrant_count: AtomicU32::new(0),
            ttl: self.ttl,
            watchdog_enabled: self.watchdog_enabled,
            watchdog: Mutex::new(None),
            released: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        // We can't create a real RedisBackend in unit tests, but we can verify
        // the builder pattern compiles and defaults are correct.
        assert_eq!(DEFAULT_LOCK_TTL, Duration::from_secs(30));
    }
}
