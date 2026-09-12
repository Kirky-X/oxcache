// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Distributed lock implementation based on Redis.

use crate::backend::RedisBackend;
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
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
    /// fencing token：acquire 成功时经 `INCR <key>:fence` 取单调递增值
    pub(super) fencing_token: AtomicU64,
}

/// Backoff delay for watchdog renewal after consecutive Redis errors.
///
/// Grows exponentially from 200ms and caps at ~6.4s so transient outages are
/// retried without hot-looping, while a lost lock still exits promptly via
/// the `Ok(_)` arm.
fn watchdog_retry_delay(consecutive_errors: u32) -> Duration {
    let shift = consecutive_errors.min(5);
    Duration::from_millis(200 * (1 << shift))
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

                // fencing token — 单调递增，供下游资源做 staleness 检测
                match self.acquire_fence_token().await {
                    Ok(token) => {
                        self.fencing_token.store(token, Ordering::SeqCst);
                    }
                    Err(_) => {
                        // fence INCR 失败不回滚锁本身（锁已持有）；token 为 0
                        // 表示无 fencing 保护，下游按未带 token 处理。
                    }
                }

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

    /// `INCR <key>:fence` 取单调递增 fencing token
    async fn acquire_fence_token(&self) -> OxCacheResult<u64> {
        let mut conn = self.backend.conn();
        let token: i64 = redis::cmd(RedisCommand::Incr.as_str())
            .arg(format!("{}:fence", self.key))
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("dist_lock fence incr failed: {e}")))?;
        Ok(u64::try_from(token).unwrap_or(0))
    }

    /// 当前 fencing token（0 = 尚未获取或不支持）
    ///
    /// # 下游使用契约
    ///
    /// fencing token 单调递增：锁的主从切换丢锁场景下，旧持有者的 token
    /// 小于新持有者，下游资源（存储/队列）应拒绝 stale token 的写入。
    pub fn token(&self) -> u64 {
        self.fencing_token.load(Ordering::SeqCst)
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
        if new_count > 0 {
            // Still reentrant, don't actually release
            self.reentrant_count.store(new_count, Ordering::SeqCst);
            return Ok(());
        }

        // Final release: run the remote Lua script first. Only on success do
        // we mutate local state, so a failed release stays retryable instead
        // of leaking the remote lock while appearing released locally.
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

        self.reentrant_count.store(0, Ordering::SeqCst);

        // Mark as released to stop watchdog
        self.released.store(true, Ordering::SeqCst);

        // Abort watchdog
        if let Some(handle) = self.watchdog.lock().await.take() {
            handle.abort();
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
            let mut consecutive_errors: u32 = 0;
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
                        consecutive_errors = 0;
                    }
                    Ok(_) => {
                        // Lock no longer held (expired or stolen), stop watchdog
                        break;
                    }
                    Err(_) => {
                        // Transient Redis error: back off and retry instead of
                        // abandoning the lock, which would let it expire early.
                        consecutive_errors = consecutive_errors.saturating_add(1);
                        tokio::time::sleep(watchdog_retry_delay(consecutive_errors)).await;
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

    #[test]
    fn test_watchdog_retry_delay_grows_and_caps() {
        assert!(watchdog_retry_delay(0) <= watchdog_retry_delay(1));
        assert!(watchdog_retry_delay(1) <= watchdog_retry_delay(5));
        assert!(watchdog_retry_delay(u32::MAX) <= Duration::from_secs(10));
    }

    #[allow(unsafe_code)]
    async fn live_test_backend() -> Arc<RedisBackend> {
        // SAFETY: idempotent set of the same value; matches redis test helper pattern.
        unsafe {
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        }
        Arc::new(
            RedisBackend::new("redis://127.0.0.1:6379")
                .await
                .expect("live Redis required at 127.0.0.1:6379"),
        )
    }

    #[tokio::test]
    #[ignore = "needs live Redis at 127.0.0.1:6379"]
    async fn test_release_stolen_lock_keeps_retryable_state() {
        use super::super::DistLockBuilder;

        let backend = live_test_backend().await;
        let key = format!("test-lock-stolen-{}", Uuid::new_v4());
        let mut lock = DistLockBuilder::new(backend.clone(), key.clone())
            .ttl(Duration::from_secs(30))
            .watchdog_enabled(false)
            .build();
        assert!(lock.acquire().await.expect("acquire"));
        // Simulate expiry/steal: delete the key out-of-band.
        let mut conn = backend.conn();
        let _: i64 = redis::cmd(RedisCommand::Del.as_str())
            .arg(&key)
            .query_async(&mut conn)
            .await
            .expect("DEL");
        let err = lock
            .release()
            .await
            .expect_err("release of stolen lock must fail");
        assert!(err.to_string().contains("not held by owner"));
        assert_eq!(
            lock.reentrant_count.load(Ordering::SeqCst),
            1,
            "failed release must keep retryable state"
        );
    }

    #[tokio::test]
    #[ignore = "needs live Redis at 127.0.0.1:6379"]
    async fn test_release_conn_error_keeps_retryable_state() {
        use super::super::DistLockBuilder;
        use std::process::Command;

        let backend = live_test_backend().await;
        let key = format!("test-lock-connerr-{}", Uuid::new_v4());
        let mut lock = DistLockBuilder::new(backend.clone(), key.clone())
            .ttl(Duration::from_secs(30))
            .watchdog_enabled(false)
            .build();
        assert!(lock.acquire().await.expect("acquire"));
        // Kill the server to force a connection error on release.
        let _ = Command::new("redis-cli")
            .args(["-p", "6379", "shutdown", "nosave"])
            .status();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let err = lock
            .release()
            .await
            .expect_err("release against dead server must fail");
        assert!(err.to_string().contains("dist_lock release failed"));
        assert_eq!(
            lock.reentrant_count.load(Ordering::SeqCst),
            1,
            "failed release must keep retryable state"
        );
        assert!(
            !lock.released.load(Ordering::SeqCst),
            "failed release must not flag the lock as released"
        );
        // Restore the server for subsequent tests.
        let _ = Command::new("redis-server")
            .args([
                "--port",
                "6379",
                "--daemonize",
                "yes",
                "--save",
                "",
                "--appendonly",
                "no",
            ])
            .status();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    #[tokio::test]
    #[ignore = "needs live Redis at 127.0.0.1:6379"]
    async fn test_watchdog_renews_past_ttl() {
        use super::super::DistLockBuilder;

        let backend = live_test_backend().await;
        let key = format!("test-lock-watchdog-{}", Uuid::new_v4());
        let mut lock = DistLockBuilder::new(backend, key)
            .ttl(Duration::from_secs(3))
            .watchdog_enabled(true)
            .build();
        assert!(lock.acquire().await.expect("acquire"));
        // Sleep past the TTL; the watchdog (renew every ~1s) must keep it held.
        tokio::time::sleep(Duration::from_millis(4500)).await;
        assert!(
            lock.is_held().await.expect("is_held"),
            "watchdog must renew the lock past TTL"
        );
        lock.release().await.expect("release");
    }
}
