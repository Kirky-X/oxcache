// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `LockProvider` trait — backend-agnostic distributed lock abstraction.
//!
//! Defines a minimal lock interface (`try_lock` / `lock` / `unlock`) that
//! any distributed lock backend must satisfy.  The existing Redis-based
//! [`DistributedLock`](super::DistributedLock) implements this trait so
//! downstream crates can depend on the abstraction rather than the concrete
//! type.

use crate::error::OxCacheResult;
use async_trait::async_trait;
use std::time::Duration;

/// Backend-agnostic distributed lock interface.
///
/// # Contract
///
/// - `try_lock` performs a **single** acquisition attempt.  Returns
///   `Ok(true)` when the lock was acquired, `Ok(false)` when it is
///   currently held by another owner (non-error contention).
/// - `lock` blocks (retries) until the lock is acquired or an error
///   occurs.
/// - `unlock` releases the lock.
///
/// Implementations must be `Send + Sync` so they can be shared across
/// async tasks.
#[async_trait]
pub trait LockProvider: Send + Sync {
    /// Try to acquire the lock once without blocking.
    ///
    /// Returns `Ok(true)` if the lock was acquired, `Ok(false)` if it is
    /// held by another owner.
    async fn try_lock(&mut self) -> OxCacheResult<bool>;

    /// Acquire the lock, retrying with exponential back-off until
    /// successful or an error occurs.
    async fn lock(&mut self) -> OxCacheResult<()>;

    /// Release the lock.
    async fn unlock(&mut self) -> OxCacheResult<()>;

    /// Check whether this lock is currently held by the caller.
    async fn is_held(&self) -> OxCacheResult<bool>;
}

// ---------------------------------------------------------------------------
// Blanket impl for DistributedLock
// ---------------------------------------------------------------------------

use super::lock::DistributedLock;

#[async_trait]
impl LockProvider for DistributedLock {
    async fn try_lock(&mut self) -> OxCacheResult<bool> {
        match self.acquire().await {
            Ok(acquired) => Ok(acquired),
            // acquire() returns Err when another owner holds the lock;
            // translate to Ok(false) per the LockProvider contract.
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("already held") {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn lock(&mut self) -> OxCacheResult<()> {
        let mut delay = Duration::from_millis(50);
        let max_delay = Duration::from_secs(2);
        loop {
            match self.try_lock().await? {
                true => return Ok(()),
                false => {
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(max_delay);
                }
            }
        }
    }

    async fn unlock(&mut self) -> OxCacheResult<()> {
        self.release().await
    }

    async fn is_held(&self) -> OxCacheResult<bool> {
        DistributedLock::is_held(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T024: verify `LockProvider` is object-safe (can be used as
    /// `Box<dyn LockProvider>`).
    #[test]
    fn lock_provider_is_object_safe() {
        fn _assert_dyn_safe(_: &dyn LockProvider) {}
    }

    /// T024: verify `DefaultLockProvider` type alias resolves to
    /// `DistributedLock`.
    #[test]
    fn default_lock_provider_alias() {
        fn _assert_same<T: LockProvider>() {}
        fn _check() {
            _assert_same::<super::super::DefaultLockProvider>();
        }
    }
}
