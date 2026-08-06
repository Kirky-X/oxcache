// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Distributed lock implementation based on Redis.

use crate::backend::RedisBackend;
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Lua script for atomic release: only delete if the value matches owner.
const RELEASE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
else
    return 0
end
"#;

/// Lua script for atomic extend (renew TTL): only extend if the value matches owner.
const EXTEND_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('PEXPIRE', KEYS[1], ARGV[2])
else
    return 0
end
"#;

/// Distributed lock based on Redis.
///
/// Supports:
/// - TTL-based automatic expiry
/// - Automatic renewal via watchdog
/// - Reentrant acquisition (same instance)
/// - Safe release via Lua script (only owner can release)
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::features::dist_lock::{DistributedLock, DistLockBuilder};
/// use oxcache::backend::RedisBackend;
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// let backend = Arc::new(RedisBackend::new("redis://localhost:6379").await?);
/// let mut lock = DistLockBuilder::new(backend, "my-lock".into())
///     .ttl(Duration::from_secs(30))
///     .build();
///
/// if lock.acquire().await? {
///     // critical section
///     lock.release().await?;
/// }
/// ```
pub struct DistributedLock {
    pub(super) backend: Arc<RedisBackend>,
    pub(super) key: String,
    pub(super) owner_id: String,
    pub(super) reentrant_count: AtomicU32,
    pub(super) ttl: Duration,
    pub(super) watchdog_enabled: bool,
    pub(super) watchdog: Mutex<Option<JoinHandle<()>>>,
    pub(super) released: Arc<AtomicBool>,
}

impl DistributedLock {
    /// Acquire the distributed lock.
    ///
    /// Returns `Ok(true)` if the lock was newly acquired, `Ok(false)` if reentrant
    /// (already held by this instance). Returns `Err` if the lock is held by another owner.
    pub async fn acquire(&mut self) -> OxCacheResult<bool> {
        // Reentrant: already held by this instance
        let count = self.reentrant_count.load(Ordering::SeqCst);
        if count > 0 {
            self.reentrant_count.fetch_add(1, Ordering::SeqCst);
            return Ok(false);
        }

        // Reset released flag for new acquisition
        self.released.store(false, Ordering::SeqCst);

        let ttl_ms = self.ttl.as_millis() as u64;
        let mut conn = self.backend.conn();

        // SET key owner_id NX PX ttl_ms
        let result: Option<String> = redis::cmd(RedisCommand::Set.as_str())
            .arg(&self.key)
            .arg(&self.owner_id)
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("dist_lock acquire failed: {e}")))?;

        match result {
            Some(_) => {
                // Successfully acquired
                self.reentrant_count.store(1, Ordering::SeqCst);

                // Start watchdog if enabled
                if self.watchdog_enabled {
                    let handle = self.spawn_watchdog();
                    *self.watchdog.lock().await = Some(handle);
                }

                Ok(true)
            }
            None => {
                // Lock is held by another owner
                Err(OxCacheError::Operation(format!(
                    "dist_lock '{}' is already held by another owner",
                    self.key
                )))
            }
        }
    }

    /// Release the distributed lock.
    ///
    /// For reentrant locks, decrements the count and only releases when it reaches zero.
    pub async fn release(&mut self) -> OxCacheResult<()> {
        let count = self.reentrant_count.load(Ordering::SeqCst);
        if count == 0 {
            return Err(OxCacheError::Operation(
                "dist_lock not held, cannot release".to_string(),
            ));
        }

        let new_count = count - 1;
        self.reentrant_count.store(new_count, Ordering::SeqCst);

        if new_count > 0 {
            // Still reentrant, don't actually release
            return Ok(());
        }

        // Mark as released to stop watchdog
        self.released.store(true, Ordering::SeqCst);

        // Abort watchdog
        if let Some(handle) = self.watchdog.lock().await.take() {
            handle.abort();
        }

        // Execute Lua release script
        let mut conn = self.backend.conn();
        let result: i64 = redis::cmd(RedisCommand::Eval.as_str())
            .arg(RELEASE_SCRIPT)
            .arg(1)
            .arg(&self.key)
            .arg(&self.owner_id)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("dist_lock release failed: {e}")))?;

        if result == 0 {
            return Err(OxCacheError::Operation(format!(
                "dist_lock '{}' not held by owner (already expired or stolen)",
                self.key
            )));
        }

        Ok(())
    }

    /// Extend the lock TTL (renew).
    ///
    /// Returns `Ok(true)` if the lock was successfully renewed, `Ok(false)` if
    /// the lock is no longer held by this owner.
    pub async fn extend(&self) -> OxCacheResult<bool> {
        let ttl_ms = self.ttl.as_millis().to_string();
        let mut conn = self.backend.conn();

        let result: i64 = redis::cmd(RedisCommand::Eval.as_str())
            .arg(EXTEND_SCRIPT)
            .arg(1)
            .arg(&self.key)
            .arg(&self.owner_id)
            .arg(&ttl_ms)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("dist_lock extend failed: {e}")))?;

        Ok(result == 1)
    }

    /// Check if this lock is currently held by this owner.
    pub async fn is_held(&self) -> OxCacheResult<bool> {
        let mut conn = self.backend.conn();
        let result: Option<String> = redis::cmd(RedisCommand::Get.as_str())
            .arg(&self.key)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("dist_lock is_held check failed: {e}")))?;

        Ok(result.as_deref() == Some(&self.owner_id))
    }

    /// Spawn the watchdog background task.
    fn spawn_watchdog(&self) -> JoinHandle<()> {
        let backend = self.backend.clone();
        let key = self.key.clone();
        let owner_id = self.owner_id.clone();
        let ttl = self.ttl;
        let released = self.released.clone();

        let renew_interval = ttl / 3;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(renew_interval).await;

                // Check if lock has been released
                if released.load(Ordering::SeqCst) {
                    break;
                }

                // Attempt to extend
                let ttl_ms = ttl.as_millis().to_string();
                let mut conn = backend.conn();
                let result: Result<i64, _> = redis::cmd(RedisCommand::Eval.as_str())
                    .arg(EXTEND_SCRIPT)
                    .arg(1)
                    .arg(&key)
                    .arg(&owner_id)
                    .arg(&ttl_ms)
                    .query_async(&mut conn)
                    .await;

                match result {
                    Ok(1) => {
                        // Successfully renewed, continue
                    }
                    Ok(_) => {
                        // Lock no longer held (expired or stolen), stop watchdog
                        break;
                    }
                    Err(_) => {
                        // Redis error, stop watchdog to avoid infinite retries
                        break;
                    }
                }
            }
        })
    }
}

impl Drop for DistributedLock {
    fn drop(&mut self) {
        // Mark as released so watchdog exits
        self.released.store(true, Ordering::SeqCst);
        // Note: we can't abort the watchdog here because we're in a sync Drop.
        // The watchdog will exit on its next iteration when it sees `released == true`.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_reentrant_count_logic() {
        let count = AtomicU32::new(0);

        // First acquire: count 0 -> 1
        assert_eq!(count.load(Ordering::SeqCst), 0);
        count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Reentrant acquire: count 1 -> 2
        count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(count.load(Ordering::SeqCst), 2);

        // First release: count 2 -> 1 (don't actually release)
        let new_count = count.fetch_sub(1, Ordering::SeqCst) - 1;
        assert_eq!(new_count, 1);
        assert!(new_count > 0); // shouldn't release yet

        // Second release: count 1 -> 0 (actually release)
        let new_count = count.fetch_sub(1, Ordering::SeqCst) - 1;
        assert_eq!(new_count, 0);
    }

    #[test]
    fn test_owner_id_is_unique() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_lua_scripts_are_valid() {
        // Basic sanity: scripts should contain expected Redis commands
        assert!(RELEASE_SCRIPT.contains("GET"));
        assert!(RELEASE_SCRIPT.contains("DEL"));
        assert!(EXTEND_SCRIPT.contains("GET"));
        assert!(EXTEND_SCRIPT.contains("PEXPIRE"));
    }

    #[test]
    fn test_released_flag_default() {
        let released = AtomicBool::new(false);
        assert!(!released.load(Ordering::SeqCst));
        released.store(true, Ordering::SeqCst);
        assert!(released.load(Ordering::SeqCst));
    }
}
