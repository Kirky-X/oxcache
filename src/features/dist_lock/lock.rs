// Copyright (c) 2025-2026 Kirky.X🌠
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
mod tests;
