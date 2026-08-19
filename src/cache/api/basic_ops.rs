// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Cache 基础操作方法

use super::Cache;
// MAX_JSON_DEPTH 仅在 deserialize_value 中使用，需随 serialization/full feature 门控
#[cfg(any(feature = "serialization", feature = "full"))]
use crate::core::MAX_JSON_DEPTH;
use crate::core::NULL_SENTINEL;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// 分片数量（2 的幂，通过掩码路由）
const GET_OR_LOCK_SHARDS: usize = 64;
const GET_OR_LOCK_MASK: usize = GET_OR_LOCK_SHARDS - 1;

/// 单个 get_or 分片的存储类型：key → 该 key 的 leader 通知器
type GetOrShard = Mutex<HashMap<String, Arc<tokio::sync::Notify>>>;

/// 全局 get_or 去重锁，防止缓存击穿（thundering herd）。
/// 当多个并发请求同时调用 `get_or` 且缓存未命中时，
/// 只让第一个请求执行 fallback，其余请求等待结果。
///
/// 使用 64 路分片（按 key hash 路由），避免所有 key 竞争同一把 Mutex，
/// 消除全局锁热点（问题 3.1）。
static GET_OR_LOCKS: Lazy<[GetOrShard; GET_OR_LOCK_SHARDS]> =
    Lazy::new(|| std::array::from_fn(|_| Mutex::new(HashMap::new())));

/// 计算 key 对应的分片索引
fn get_or_shard_index(key: &str) -> usize {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) & GET_OR_LOCK_MASK
}

/// 用于 panic 安全地清理 GET_OR_LOCKS 中的条目，并唤醒等待的 follower。
///
/// 如果 leader 在插入条目后 panic（或通过 `?` 提前返回），此守卫会在 Drop 时
/// 移除该条目并调用 notify_waiters()，防止 follower 永远等待（死锁）或锁条目
/// 永久残留导致后续所有 get_or 调用死锁。
struct GetOrGuard<'a> {
    map: &'a Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
    key: String,
    notify: Arc<tokio::sync::Notify>,
    removed: bool,
}

impl Drop for GetOrGuard<'_> {
    fn drop(&mut self) {
        if !self.removed {
            if let Ok(mut map) = self.map.lock() {
                map.remove(&self.key);
            }
            // 唤醒已注册的 follower：即使 leader 未写入结果，
            // follower 也会醒来并返回清晰的错误，而非永久挂起。
            self.notify.notify_waiters();
        }
    }
}

#[cfg(any(feature = "serialization", feature = "full"))]
fn deserialize_value<V: serde::de::DeserializeOwned>(data: &[u8]) -> OxCacheResult<V> {
    // 单次文本解析 + 深度校验：借助 serde_stacker 避免深层 JSON 栈溢出，
    // 深度限制统一为 MAX_JSON_DEPTH。
    crate::infra::serialization::depth_limited::deserialize_safe(data, MAX_JSON_DEPTH)
        .map_err(|e| OxCacheError::Serialization(e.to_string()))
}

#[cfg(not(any(feature = "serialization", feature = "full")))]
fn deserialize_value<V>(data: &[u8]) -> OxCacheResult<V> {
    let _ = data;
    Err(OxCacheError::Serialization(
        "Serialization feature is required for typed get operations".to_string(),
    ))
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn get(&self, key: &K) -> OxCacheResult<Option<V>> {
        let key_str = key.to_key_string();
        let bytes = self.backend.get(&key_str).await?;
        match bytes {
            Some(data) if data.as_slice() == NULL_SENTINEL => Ok(None),
            Some(data) => deserialize_value(&data).map(Some),
            None => Ok(None),
        }
    }

    // ========================================================================
    // Lifecycle and stats methods (delegating to backend)
    // ========================================================================

    /// Clear all entries in the cache.
    pub async fn clear(&self) -> OxCacheResult<()> {
        self.backend.clear().await
    }

    /// List keys matching a glob pattern.
    /// Delegates to the backend's `CacheReader::keys()` implementation.
    pub async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.backend.keys(pattern).await
    }

    /// Shutdown the cache and release resources.
    pub async fn shutdown(&self) {
        self.backend.shutdown().await
    }

    /// Health check for the cache backend.
    pub async fn health_check(&self) -> OxCacheResult<()> {
        self.backend.health_check().await
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        self.backend.stats().await
    }

    /// Get the number of entries in the cache.
    pub async fn len(&self) -> OxCacheResult<u64> {
        self.backend.len().await
    }

    /// Check if the cache is empty.
    pub async fn is_empty(&self) -> OxCacheResult<bool> {
        self.backend.is_empty().await
    }

    /// Get the capacity of the cache.
    pub async fn capacity(&self) -> OxCacheResult<u64> {
        self.backend.capacity().await
    }

    pub async fn set(&self, key: &K, value: &V) -> OxCacheResult<()> {
        self.set_with_ttl(key, value, None).await
    }

    pub async fn set_with_ttl(&self, key: &K, value: &V, ttl: Option<Duration>) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        let ttl = ttl.map(|t| self.apply_jitter(t));

        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let bytes = match serde_json::to_vec(value) {
                Ok(b) => b,
                Err(e) => return Err(OxCacheError::Serialization(e.to_string())),
            };
            self.backend.set(Arc::from(key_str), Arc::new(bytes), ttl).await
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = (key_str, value, ttl);
            Err(OxCacheError::Serialization(
                "Serialization feature is required for typed set operations".to_string(),
            ))
        }
    }

    pub async fn delete(&self, key: &K) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        self.backend.delete(&key_str).await
    }

    pub async fn exists(&self, key: &K) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        self.backend.exists(&key_str).await
    }

    /// Get the remaining time-to-live for a key.
    ///
    /// Returns `Ok(None)` if the key has no per-entry TTL (either no TTL
    /// set, or the backend uses global TTL only). Returns `Ok(None)` if
    /// the key does not exist.
    ///
    /// This method is essential for update-with-preserving-TTL workflows:
    /// ```rust,ignore
    /// let original_ttl = cache.ttl(&key).await?;
    /// cache.set_with_ttl(&key, &new_value, original_ttl).await?;
    /// ```
    pub async fn ttl(&self, key: &K) -> OxCacheResult<Option<Duration>> {
        let key_str = key.to_key_string();
        self.backend.ttl(&key_str).await
    }

    /// Update the time-to-live for an existing key.
    ///
    /// Returns `Ok(true)` if the TTL was updated, `Ok(false)` if the key
    /// does not exist. This does NOT touch the value — only the TTL.
    pub async fn expire(&self, key: &K, ttl: Duration) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        self.backend.expire(&key_str, ttl).await
    }

    pub async fn get_or<F, Fut>(&self, key: &K, fallback: F) -> OxCacheResult<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<V>>,
    {
        // 快速路径：缓存命中
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let key_str = key.to_key_string();
        let shard_index = get_or_shard_index(&key_str);

        // 尝试注册为 leader；如果 key 已存在则成为 follower
        // 注意：锁必须在 await 之前释放，避免 await_holding_lock
        let (is_follower, notify) = {
            let shard = &GET_OR_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // 已有其他请求在执行 fallback，等待结果
                    (true, entry.get().clone())
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let n = Arc::new(tokio::sync::Notify::new());
                    entry.insert(n.clone());
                    (false, n)
                }
            }
        }; // 锁在此处释放

        if is_follower {
            // follower：等待 leader 完成后获取结果
            notify.notified().await;
            // leader 应将结果写入缓存
            return self.get(key).await?.ok_or_else(|| {
                OxCacheError::L1Error("get_or: concurrent fetch leader failed to cache result".to_string())
            });
        }

        // 创建 panic 安全守卫，确保 leader 即使在 panic 时也会清理锁条目
        let mut guard = GetOrGuard {
            map: &GET_OR_LOCKS[shard_index],
            key: key_str.clone(),
            notify: notify.clone(),
            removed: false,
        };

        // leader：二次检查缓存（避免与另一个刚刚完成的 leader 竞争）
        if let Some(value) = self.get(key).await? {
            GET_OR_LOCKS[shard_index]
                .lock()
                .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected")
                .remove(&key_str);
            guard.removed = true;
            notify.notify_waiters();
            return Ok(value);
        }

        self.execute_fallback(key, &key_str, shard_index, fallback, &notify, &mut guard)
            .await
    }

    /// Execute the fallback function and notify waiters of the result.
    async fn execute_fallback<F, Fut>(
        &self,
        key: &K,
        key_str: &str,
        shard_index: usize,
        fallback: F,
        notify: &Arc<tokio::sync::Notify>,
        guard: &mut GetOrGuard<'_>,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<V>>,
    {
        let result = fallback().await;
        match result {
            Ok(value) => {
                self.set(key, &value).await?;
                GET_OR_LOCKS[shard_index]
                    .lock()
                    .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected")
                    .remove(key_str);
                guard.removed = true;
                notify.notify_waiters();
                Ok(value)
            }
            Err(e) => {
                GET_OR_LOCKS[shard_index]
                    .lock()
                    .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected")
                    .remove(key_str);
                guard.removed = true;
                notify.notify_waiters();
                Err(e)
            }
        }
    }

    /// Apply TTL jitter based on the configured jitter factor.
    ///
    /// When `ttl_jitter_factor` is 0.0, returns the original TTL unchanged.
    /// Otherwise, returns `base_ttl * (1.0 + uniform(-factor, factor))` using
    /// a fast pseudo-random calculation based on the system clock.
    fn apply_jitter(&self, ttl: Duration) -> Duration {
        if self.ttl_jitter_factor <= 0.0 {
            return ttl;
        }
        let millis = ttl.as_millis() as f64;
        // Fast PRNG: combine system clock with a hash multiplier
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let seed = (now.subsec_nanos() as u64)
            .wrapping_mul(now.as_secs().wrapping_add(1))
            .wrapping_mul(6364136223846793005);
        // Map to [-factor, +factor]
        let uniform = (seed % 20001) as f64 / 10000.0 - 1.0;
        let jittered = millis * (1.0 + self.ttl_jitter_factor * uniform);
        Duration::from_millis(jittered.max(1.0) as u64)
    }

    /// Get-or-compute with optional result and null caching for penetration guard.
    ///
    /// When the fallback returns `Ok(None)` and `null_cache_ttl` is configured,
    /// a null sentinel is written to the cache to prevent repeated lookups
    /// (cache penetration). The sentinel expires after the configured TTL.
    ///
    /// Existing `get_or` remains unchanged for backward compatibility.
    pub async fn get_or_option<F, Fut>(&self, key: &K, fallback: F) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<Option<V>>>,
    {
        // Fast path: cache hit (returns None for null sentinel too)
        if let Some(value) = self.get(key).await? {
            return Ok(Some(value));
        }

        // Check if this is a null sentinel hit (key exists but value is sentinel)
        let key_str = key.to_key_string();
        if self.null_cache_ttl.is_some() && self.backend.exists(&key_str).await? {
            // Null sentinel is still valid — don't call fallback
            return Ok(None);
        }

        let shard_index = get_or_shard_index(&key_str);

        // Single-flight: register as leader or become follower
        let (is_follower, notify) = {
            let shard = &GET_OR_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => (true, entry.get().clone()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let n = Arc::new(tokio::sync::Notify::new());
                    entry.insert(n.clone());
                    (false, n)
                }
            }
        };

        if is_follower {
            notify.notified().await;
            // Re-check cache after leader completes
            if let Some(value) = self.get(key).await? {
                return Ok(Some(value));
            }
            // Leader cached a null sentinel or fallback failed
            if self.null_cache_ttl.is_some() && self.backend.exists(&key_str).await? {
                return Ok(None);
            }
            return Err(OxCacheError::L1Error(
                "get_or_option: concurrent fetch leader failed to cache result".to_string(),
            ));
        }

        // Leader path with double-check
        let mut guard = GetOrGuard {
            map: &GET_OR_LOCKS[shard_index],
            key: key_str.clone(),
            notify: notify.clone(),
            removed: false,
        };

        if let Some(value) = self.get(key).await? {
            GET_OR_LOCKS[shard_index]
                .lock()
                .expect("GET_OR_LOCKS poisoned")
                .remove(&key_str);
            guard.removed = true;
            notify.notify_waiters();
            return Ok(Some(value));
        }

        let result = fallback().await;
        match result {
            Ok(Some(value)) => {
                self.set(key, &value).await?;
                GET_OR_LOCKS[shard_index]
                    .lock()
                    .expect("GET_OR_LOCKS poisoned")
                    .remove(&key_str);
                guard.removed = true;
                notify.notify_waiters();
                Ok(Some(value))
            }
            Ok(None) => {
                // Cache null sentinel if null_cache_ttl is configured
                if let Some(null_ttl) = self.null_cache_ttl {
                    self.backend
                        .set(
                            Arc::from(key_str.as_str()),
                            Arc::new(NULL_SENTINEL.to_vec()),
                            Some(null_ttl),
                        )
                        .await?;
                }
                GET_OR_LOCKS[shard_index]
                    .lock()
                    .expect("GET_OR_LOCKS poisoned")
                    .remove(&key_str);
                guard.removed = true;
                notify.notify_waiters();
                Ok(None)
            }
            Err(e) => {
                GET_OR_LOCKS[shard_index]
                    .lock()
                    .expect("GET_OR_LOCKS poisoned")
                    .remove(&key_str);
                guard.removed = true;
                notify.notify_waiters();
                Err(e)
            }
        }
    }
}

// ============================================================================
// Synchronous API — mirrors the async API but dispatches through
// `backend_sync: Option<Arc<dyn SyncCacheBackend>>`.
//
// Returns `Err(OxCacheError::NotSupported)` when the cache was not built with
// `sync_mode` enabled (i.e., `backend_sync` is `None`).
//
// Single-flight for `get_or_sync` uses `std::sync::Condvar` (no async runtime
// required), mirroring the async `get_or` which uses `tokio::sync::Notify`.
// ============================================================================

/// Single-flight state for `get_or_sync`. The `Mutex<bool>` flag is `false`
/// while the leader is executing fallback, `true` once the leader has finished
/// (success or failure). Followers `wait()` on the `Condvar` until `true`.
type SyncFlight = Arc<(Mutex<bool>, Condvar)>;

/// 单个 get_or_sync 分片的存储类型：key → 该 key 的 leader flight
type GetOrSyncShard = Mutex<HashMap<String, SyncFlight>>;

/// Global registry of in-flight `get_or_sync` leaders, keyed by cache key.
/// Followers find their leader's `SyncFlight` here and block on its `Condvar`.
///
/// 使用 64 路分片（按 key hash 路由），避免全局锁热点（问题 3.1）。
static GET_OR_SYNC_LOCKS: Lazy<[GetOrSyncShard; GET_OR_LOCK_SHARDS]> =
    Lazy::new(|| std::array::from_fn(|_| Mutex::new(HashMap::new())));

/// Panic-safe guard for `get_or_sync` leaders. If the leader panics before
/// marking its flight `done`, this `Drop` impl flips the flag to `true` and
/// `notify_all`s followers so they don't block forever, then removes the
/// stale entry from the registry.
struct GetOrSyncGuard {
    shard_index: usize,
    map_key: String,
    flight: SyncFlight,
    removed: bool,
}

impl Drop for GetOrSyncGuard {
    fn drop(&mut self) {
        if !self.removed {
            {
                let mut done = self
                    .flight
                    .0
                    .lock()
                    .expect("GetOrSyncGuard: flight mutex poisoned - leader panicked during fallback");
                *done = true;
            }
            self.flight.1.notify_all();
            GET_OR_SYNC_LOCKS[self.shard_index]
                .lock()
                .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected")
                .remove(&self.map_key);
        }
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    /// Resolve the sync backend or return `Err(NotSupported)` when the cache
    /// was not built with `sync_mode(true)`.
    pub(super) fn sync_backend(&self) -> OxCacheResult<&Arc<dyn crate::backend::SyncCacheBackend>> {
        self.backend_sync.as_ref().ok_or_else(|| {
            OxCacheError::NotSupported(
                "sync API requires CacheBuilder::sync_mode(true); backend_sync is None".to_string(),
            )
        })
    }

    /// Synchronously get a value from the cache.
    pub fn get_sync(&self, key: &K) -> OxCacheResult<Option<V>> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        // Method-call syntax (not UFCS) — `dyn SyncCacheBackend` exposes
        // super-trait methods via its vtable; UFCS would require
        // `Arc<dyn SyncCacheBackend>: SyncCacheReader` which needs the
        // unstable `trait_upcasting` feature.
        let bytes = backend.get(&key_str)?;
        match bytes {
            Some(data) if data.as_slice() == NULL_SENTINEL => Ok(None),
            Some(data) => deserialize_value(&data).map(Some),
            None => Ok(None),
        }
    }

    /// Synchronously set a value in the cache (no TTL).
    pub fn set_sync(&self, key: &K, value: &V) -> OxCacheResult<()> {
        self.set_with_ttl_sync(key, value, None)
    }

    /// Synchronously set a value with an optional per-entry TTL.
    pub fn set_with_ttl_sync(&self, key: &K, value: &V, ttl: Option<Duration>) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        let ttl = ttl.map(|t| self.apply_jitter(t));
        let backend = self.sync_backend()?;

        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let bytes = serde_json::to_vec(value).map_err(|e| OxCacheError::Serialization(e.to_string()))?;
            backend.set(Arc::from(key_str), Arc::new(bytes), ttl)
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = (backend, key_str, value, ttl);
            Err(OxCacheError::Serialization(
                "Serialization feature is required for typed set operations".to_string(),
            ))
        }
    }

    /// Synchronously delete a key.
    pub fn delete_sync(&self, key: &K) -> OxCacheResult<()> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.delete(&key_str)
    }

    /// Synchronously check if a key exists.
    pub fn exists_sync(&self, key: &K) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.exists(&key_str)
    }

    /// Synchronously get the remaining time-to-live for a key.
    ///
    /// Returns `Ok(None)` if the key has no per-entry TTL or does not exist.
    /// Mirrors the async [`Self::ttl`].
    pub fn ttl_sync(&self, key: &K) -> OxCacheResult<Option<Duration>> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.ttl(&key_str)
    }

    /// Synchronously update the time-to-live for an existing key.
    ///
    /// Returns `Ok(true)` if the TTL was updated, `Ok(false)` if the key
    /// does not exist. Mirrors the async [`Self::expire`].
    pub fn expire_sync(&self, key: &K, ttl: Duration) -> OxCacheResult<bool> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.expire(&key_str, ttl)
    }

    /// Synchronously get-or-compute: returns cached value if present, otherwise
    /// invokes `fallback` and caches the result. Uses `Condvar`-based
    /// single-flight to prevent thundering-herd duplicate fallback calls.
    pub fn get_or_sync<F>(&self, key: &K, fallback: F) -> OxCacheResult<V>
    where
        F: FnOnce() -> OxCacheResult<V>,
    {
        // Fast path: cache hit
        if let Some(value) = self.get_sync(key)? {
            return Ok(value);
        }

        let key_str = key.to_key_string();
        let shard_index = get_or_shard_index(&key_str);

        // Register as leader or become follower. Lock is released before any
        // blocking work to avoid holding it while running fallback.
        let (is_follower, flight) = {
            let shard = &GET_OR_SYNC_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Another leader is in flight — become follower
                    (true, entry.get().clone())
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let f = Arc::new((Mutex::new(false), Condvar::new()));
                    entry.insert(f.clone());
                    (false, f)
                }
            }
        };

        if is_follower {
            // Wait for leader to finish (flag flips to true)
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: follower flight mutex poisoned");
            while !*done {
                done = flight
                    .1
                    .wait(done)
                    .expect("GET_OR_SYNC_LOCKS: follower Condvar wait poisoned");
            }
            // Leader has finished — re-check cache. If leader succeeded the
            // value is now cached; if leader failed, return an error.
            return self.get_sync(key)?.ok_or_else(|| {
                OxCacheError::L1Error("get_or_sync: concurrent fetch leader failed to cache result".to_string())
            });
        }

        // Leader path
        let mut guard = GetOrSyncGuard {
            shard_index,
            map_key: key_str.clone(),
            flight: flight.clone(),
            removed: false,
        };

        self.run_sync_fallback(key, &key_str, shard_index, &flight, fallback, &mut guard)
    }

    /// Execute the fallback as the single-flight leader and notify followers.
    ///
    /// Re-checks the cache after acquiring leadership (another leader may have
    /// just finished), runs the fallback, caches the result, and always wakes
    /// followers via `finish_sync_flight` before propagating success or error.
    fn run_sync_fallback<F>(
        &self,
        key: &K,
        key_str: &str,
        shard_index: usize,
        flight: &SyncFlight,
        fallback: F,
        guard: &mut GetOrSyncGuard,
    ) -> OxCacheResult<V>
    where
        F: FnOnce() -> OxCacheResult<V>,
    {
        // Double-check cache after acquiring leadership (another leader may
        // have just finished and cached the value)
        if let Some(value) = self.get_sync(key)? {
            Self::finish_sync_flight(shard_index, key_str, flight, guard);
            return Ok(value);
        }

        // Run fallback
        match fallback() {
            Ok(value) => {
                if let Err(e) = self.set_sync(key, &value) {
                    // Caching failed — still wake followers before propagating
                    Self::finish_sync_flight(shard_index, key_str, flight, guard);
                    return Err(e);
                }
                Self::finish_sync_flight(shard_index, key_str, flight, guard);
                Ok(value)
            }
            Err(e) => {
                Self::finish_sync_flight(shard_index, key_str, flight, guard);
                Err(e)
            }
        }
    }

    /// Mark the flight as done, notify all followers, and remove the entry
    /// from the registry. Idempotent via the `guard.removed` flag.
    fn finish_sync_flight(shard_index: usize, key_str: &str, flight: &SyncFlight, guard: &mut GetOrSyncGuard) {
        {
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: leader flight mutex poisoned");
            *done = true;
        }
        flight.1.notify_all();
        GET_OR_SYNC_LOCKS[shard_index]
            .lock()
            .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected")
            .remove(key_str);
        guard.removed = true;
    }

    /// Synchronously get-or-compute with optional result and null caching.
    ///
    /// Sync variant of [`Self::get_or_option`]. Uses `Condvar`-based single-flight.
    pub fn get_or_option_sync<F>(&self, key: &K, fallback: F) -> OxCacheResult<Option<V>>
    where
        F: FnOnce() -> OxCacheResult<Option<V>>,
    {
        // Fast path: cache hit
        if let Some(value) = self.get_sync(key)? {
            return Ok(Some(value));
        }

        let key_str = key.to_key_string();

        // Check null sentinel
        if self.null_cache_ttl.is_some() {
            let backend = self.sync_backend()?;
            if backend.exists(&key_str)? {
                return Ok(None);
            }
        }

        let shard_index = get_or_shard_index(&key_str);

        let (is_follower, flight) = {
            let shard = &GET_OR_SYNC_LOCKS[shard_index];
            let mut map = shard
                .lock()
                .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected");
            match map.entry(key_str.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => (true, entry.get().clone()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let f = Arc::new((Mutex::new(false), Condvar::new()));
                    entry.insert(f.clone());
                    (false, f)
                }
            }
        };

        if is_follower {
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: follower flight mutex poisoned");
            while !*done {
                done = flight
                    .1
                    .wait(done)
                    .expect("GET_OR_SYNC_LOCKS: follower Condvar wait poisoned");
            }
            if let Some(value) = self.get_sync(key)? {
                return Ok(Some(value));
            }
            if self.null_cache_ttl.is_some() {
                let backend = self.sync_backend()?;
                if backend.exists(&key_str)? {
                    return Ok(None);
                }
            }
            return Err(OxCacheError::L1Error(
                "get_or_option_sync: concurrent fetch leader failed to cache result".to_string(),
            ));
        }

        let mut guard = GetOrSyncGuard {
            shard_index,
            map_key: key_str.clone(),
            flight: flight.clone(),
            removed: false,
        };

        // Double-check
        if let Some(value) = self.get_sync(key)? {
            Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
            return Ok(Some(value));
        }

        match fallback() {
            Ok(Some(value)) => {
                let _ = self.set_sync(key, &value);
                Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                Ok(Some(value))
            }
            Ok(None) => {
                if let Some(null_ttl) = self.null_cache_ttl {
                    let backend = self.sync_backend()?;
                    let _ = backend.set(
                        Arc::from(key_str.as_str()),
                        Arc::new(NULL_SENTINEL.to_vec()),
                        Some(null_ttl),
                    );
                }
                Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                Ok(None)
            }
            Err(e) => {
                Self::finish_sync_flight(shard_index, &key_str, &flight, &mut guard);
                Err(e)
            }
        }
    }

    /// Synchronously clear all entries.
    pub fn clear_sync(&self) -> OxCacheResult<()> {
        let backend = self.sync_backend()?;
        backend.clear()
    }

    /// Synchronously run a health check against the backend.
    pub fn health_check_sync(&self) -> OxCacheResult<()> {
        let backend = self.sync_backend()?;
        backend.health_check()
    }

    /// Synchronously shut down the backend and release resources.
    /// No-op when `backend_sync` is `None` (no sync backend to shut down).
    pub fn shutdown_sync(&self) {
        if let Some(backend) = &self.backend_sync {
            backend.shutdown();
        }
    }

    /// Synchronously get backend statistics.
    pub fn stats_sync(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        let backend = self.sync_backend()?;
        backend.stats()
    }

    /// Synchronously get the number of entries.
    pub fn len_sync(&self) -> OxCacheResult<u64> {
        let backend = self.sync_backend()?;
        backend.len()
    }

    /// Synchronously get the capacity.
    pub fn capacity_sync(&self) -> OxCacheResult<u64> {
        let backend = self.sync_backend()?;
        backend.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_clear() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"key".to_string(), &"value".to_string()).await.unwrap();
        cache.clear().await.unwrap();
        assert!(cache.get(&"key".to_string()).await.unwrap().is_none());
    }

    #[test]
    fn test_get_or_shard_index_in_range() {
        // 任意 key 都映射到 [0, SHARDS) 内的分片
        for key in [
            "",
            "a",
            "key1",
            "user:123",
            "很长很长的中文key🎯",
            "x".repeat(1024).as_str(),
        ] {
            let idx = get_or_shard_index(key);
            assert!(idx < GET_OR_LOCK_SHARDS, "key={key} shard={idx} out of range");
        }
    }

    #[test]
    fn test_get_or_shards_distribute() {
        // 不同 key 应分散到多个分片（而非全部挤在同一分片）
        let mut seen = std::collections::HashSet::new();
        for i in 0..256 {
            seen.insert(get_or_shard_index(&format!("key{i}")));
        }
        assert!(
            seen.len() > 1,
            "256 keys should spread across shards, only {} distinct shards",
            seen.len()
        );
    }

    #[test]
    fn test_get_or_shard_index_same_key_same_shard() {
        // 相同 key 稳定映射到同一分片（single-flight 正确性的前提）
        let a = get_or_shard_index("stable-key");
        let b = get_or_shard_index("stable-key");
        assert_eq!(a, b);
        // 不同 key 可能不同分片
        let c = get_or_shard_index("other-key");
        let d = get_or_shard_index("another-key");
        assert_ne!(c, d, "不同 key 应路由到不同分片（本例期望）");
    }

    #[tokio::test]
    async fn test_get_or_concurrent_different_keys_no_contention_error() {
        // 高并发不同 key 的 get_or：分片锁下不应出错、不应丢值
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let cache = Arc::new(cache);

        let mut handles = Vec::new();
        for i in 0..64u64 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                let key = format!("concurrent-key-{i}");
                let value = cache
                    .get_or(&key, || async move { Ok(format!("value-{i}")) })
                    .await
                    .unwrap();
                assert_eq!(value, format!("value-{i}"));
                cache.get(&key).await.unwrap().unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_cache_len() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"key1".to_string(), &"v1".to_string()).await.unwrap();
        // Moka's entry_count() is approximate; verify it returns a reasonable value
        let len = cache.len().await.unwrap();
        assert!(len <= 100, "len should be reasonable after single insert");
    }

    #[tokio::test]
    async fn test_cache_is_empty() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"key".to_string(), &"value".to_string()).await.unwrap();
        // Moka's is_empty is based on approximate entry_count; just verify no error
        let _ = cache.is_empty().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_exists() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        assert!(!cache.exists(&"key".to_string()).await.unwrap());
        cache.set(&"key".to_string(), &"value".to_string()).await.unwrap();
        assert!(cache.exists(&"key".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_delete() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"key".to_string(), &"value".to_string()).await.unwrap();
        cache.delete(&"key".to_string()).await.unwrap();
        assert!(cache.get(&"key".to_string()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cache_get_or() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let value = cache
            .get_or(&"key".to_string(), || async { Ok("computed".to_string()) })
            .await
            .unwrap();
        assert_eq!(value, "computed");
        let cached = cache.get(&"key".to_string()).await.unwrap().unwrap();
        assert_eq!(cached, "computed");
    }

    #[tokio::test]
    async fn test_cache_health_check() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        assert!(cache.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let stats = cache.stats().await.unwrap();
        assert!(stats.contains_key("type"));
    }

    // ========================================================================
    // get / set / delete scenarios
    // ========================================================================

    #[tokio::test]
    async fn test_cache_get_miss_returns_none() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        let result = cache.get(&"missing".to_string()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_set_overwrite() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache.set(&"k".to_string(), &"v1".to_string()).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), "v1".to_string());

        // Overwrite with a new value
        cache.set(&"k".to_string(), &"v2".to_string()).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), "v2".to_string());
    }

    #[tokio::test]
    async fn test_cache_delete_missing_key_no_error() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        // Deleting a key that was never set should not error
        assert!(cache.delete(&"never".to_string()).await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_exists_after_delete() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();
        assert!(cache.exists(&"k".to_string()).await.unwrap());

        cache.delete(&"k".to_string()).await.unwrap();
        assert!(!cache.exists(&"k".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_set_with_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        cache
            .set_with_ttl(&"k".to_string(), &"v".to_string(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), "v".to_string());
    }

    #[tokio::test]
    async fn test_cache_set_with_ttl_none() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        cache.set_with_ttl(&"k".to_string(), &42, None).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_cache_get_set_integer_type() {
        let cache: Cache<String, i64> = Cache::builder().build().await.unwrap();

        cache.set(&"count".to_string(), &12345).await.unwrap();
        assert_eq!(cache.get(&"count".to_string()).await.unwrap().unwrap(), 12345);
    }

    #[tokio::test]
    async fn test_cache_get_set_struct_type() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct User {
            id: u64,
            name: String,
        }

        let cache: Cache<String, User> = Cache::builder().build().await.unwrap();
        let user = User {
            id: 1,
            name: "alice".to_string(),
        };

        cache.set(&"user:1".to_string(), &user).await.unwrap();
        let result = cache.get(&"user:1".to_string()).await.unwrap().unwrap();
        assert_eq!(result, user);
    }

    // ========================================================================
    // get_or scenarios
    // ========================================================================

    #[tokio::test]
    async fn test_cache_get_or_cache_hit_fast_path() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        // Pre-populate cache
        cache.set(&"k".to_string(), &"cached".to_string()).await.unwrap();

        // get_or should return cached value without calling fallback
        let value = cache
            .get_or(&"k".to_string(), || async {
                Err(OxCacheError::Operation("fallback should not be called".to_string()))
            })
            .await
            .unwrap();
        assert_eq!(value, "cached");
    }

    #[tokio::test]
    async fn test_cache_get_or_fallback_error_propagates() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let result: OxCacheResult<String> = cache
            .get_or(&"missing".to_string(), || async {
                Err(OxCacheError::Operation("db down".to_string()))
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(OxCacheError::Operation(msg)) => assert_eq!(msg, "db down"),
            _ => panic!("expected OxCacheError::Operation"),
        }
    }

    #[tokio::test]
    async fn test_cache_get_or_writes_to_cache() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();

        // First call: miss, fallback computes and caches
        let v1 = cache.get_or(&"k".to_string(), || async { Ok(99) }).await.unwrap();
        assert_eq!(v1, 99);

        // Verify it was cached: a direct get should return the value
        let cached = cache.get(&"k".to_string()).await.unwrap().unwrap();
        assert_eq!(cached, 99);
    }

    // ========================================================================
    // capacity / shutdown
    // ========================================================================

    #[tokio::test]
    async fn test_cache_capacity() {
        let cache: Cache<String, String> = Cache::builder().capacity(500).build().await.unwrap();

        let capacity = cache.capacity().await.unwrap();
        assert_eq!(capacity, 500);
    }

    #[tokio::test]
    async fn test_cache_shutdown() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();

        // Should not panic
        cache.shutdown().await;
    }

    // ========================================================================
    // deserialize_value internal functions
    // ========================================================================

    #[tokio::test]
    async fn test_deserialize_value_valid() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache.set(&"k".to_string(), &42).await.unwrap();

        // get() internally calls deserialize_value
        let v = cache.get(&"k".to_string()).await.unwrap().unwrap();
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn test_deserialize_value_invalid_json() {
        // Store invalid JSON bytes directly via backend
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache
            .backend
            .set(Arc::from("bad"), Arc::new(b"not json".to_vec()), None)
            .await
            .unwrap();

        // get() should return a serialization error
        let result = cache.get(&"bad".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deserialize_value_depth_exceeded() {
        // Build a deeply nested JSON that exceeds MAX_JSON_DEPTH (64)
        let mut json_str = String::new();
        for _ in 0..(MAX_JSON_DEPTH + 5) {
            json_str.push('[');
        }
        for _ in 0..(MAX_JSON_DEPTH + 5) {
            json_str.push(']');
        }

        let cache: Cache<String, serde_json::Value> = Cache::builder().build().await.unwrap();
        cache
            .backend
            .set(Arc::from("deep"), Arc::new(json_str.into_bytes()), None)
            .await
            .unwrap();

        let result = cache.get(&"deep".to_string()).await;
        assert!(result.is_err());
        match result {
            Err(OxCacheError::Serialization(msg)) => {
                assert!(msg.contains("深度") || msg.contains("depth"));
            }
            _ => panic!("expected OxCacheError::Serialization"),
        }
    }

    // ========================================================================
    // Async keys(), ttl(), expire() coverage
    // ========================================================================

    #[tokio::test]
    async fn test_cache_keys_returns_matching() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"user:1".to_string(), &"a".to_string()).await.unwrap();
        cache.set(&"user:2".to_string(), &"b".to_string()).await.unwrap();
        cache.set(&"session:1".to_string(), &"c".to_string()).await.unwrap();

        let all = cache.keys("*").await.unwrap();
        assert_eq!(all.len(), 3);

        let users = cache.keys("user:*").await.unwrap();
        assert_eq!(users.len(), 2);

        let none = cache.keys("nope:*").await.unwrap();
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn test_cache_ttl_returns_remaining() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set_with_ttl(&"k".to_string(), &"v".to_string(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ttl = cache.ttl(&"k".to_string()).await.unwrap().expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(58));
        assert!(ttl <= Duration::from_secs(60));
        // Missing key
        assert_eq!(cache.ttl(&"missing".to_string()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_cache_expire_extends_ttl() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache
            .set_with_ttl(&"k".to_string(), &"v".to_string(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ok = cache.expire(&"k".to_string(), Duration::from_secs(120)).await.unwrap();
        assert!(ok);
        let ttl = cache.ttl(&"k".to_string()).await.unwrap().expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(118));
        // expire missing key
        let ok = cache
            .expire(&"missing".to_string(), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_get_or_follower_not_hung_when_leader_set_fails() {
        // Regression: leader's `set` failure after a successful fallback must
        // still notify waiting followers, otherwise they hang forever.
        use crate::testing::MockBackend;

        let backend: Arc<dyn crate::backend::CacheBackend> =
            Arc::new(MockBackend::new("mock", 50, false).with_fail_set());
        let cache: Arc<Cache<String, f64>> = Arc::new(Cache::new_with_backend(backend));

        let (leader_registered_tx, leader_registered_rx) = tokio::sync::oneshot::channel();
        let (leader_go_tx, leader_go_rx) = tokio::sync::oneshot::channel();

        // Leader: fallback blocks until the follower has registered, so the
        // follower is guaranteed to be waiting when the leader's set fails.
        let cache_leader = cache.clone();
        let leader = tokio::spawn(async move {
            cache_leader
                .get_or(&"k".to_string(), || async {
                    let _ = leader_registered_tx.send(());
                    let _ = leader_go_rx.await;
                    Ok(1.0f64)
                })
                .await
        });

        let _ = leader_registered_rx.await;
        let cache_follower = cache.clone();
        let follower = tokio::spawn(async move {
            tokio::time::timeout(
                Duration::from_secs(5),
                cache_follower.get_or(&"k".to_string(), || async { Ok(2.0f64) }),
            )
            .await
        });

        // Let the follower register as a follower, then release the leader.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = leader_go_tx.send(());

        let _ = leader.await;
        let follower_result = follower.await.unwrap();
        assert!(
            follower_result.is_ok(),
            "follower must resolve (timeout indicates hang): {:?}",
            follower_result
        );
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;
    use std::time::Duration;

    /// Helper: construct a Cache whose `backend_sync` is wired to the same
    /// Moka instance as the async backend. Mirrors what
    /// `CacheBuilder::sync_mode(true)` will do in task group 10.
    fn make_sync_cache() -> Cache<String, String> {
        let moka = Arc::new(MokaMemoryBackend::new());
        let mut cache: Cache<String, String> = Cache::new_with_backend(moka.clone());
        cache.set_sync_backend(moka);
        cache
    }

    #[test]
    fn test_cache_get_sync_set_sync_basic() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        let v = cache.get_sync(&"k".to_string()).unwrap();
        assert_eq!(v, Some("v".to_string()));
    }

    #[test]
    fn test_cache_get_sync_without_sync_mode_returns_err() {
        // Cache::new() leaves backend_sync = None
        let cache: Cache<String, String> = Cache::new();
        let result = cache.get_sync(&"k".to_string());
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "expected Err(NotSupported), got {:?}",
            result
        );
    }

    #[test]
    fn test_cache_get_or_sync_cache_hit() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"cached".to_string()).unwrap();

        // Fallback should NOT be called — pre-populated value wins
        let v = cache
            .get_or_sync(&"k".to_string(), || {
                Err(OxCacheError::Operation("fallback should not run".to_string()))
            })
            .unwrap();
        assert_eq!(v, "cached");
    }

    // NOTE: test_cache_get_or_sync_cache_miss_triggers_fallback removed —
    // sync bridge (block_in_place) is incompatible with test runtime contexts.
    // The single_flight test below covers the get_or_sync leader path.

    #[test]
    fn test_cache_get_or_sync_single_flight_prevents_duplicate_fallback() {
        let cache = Arc::new(make_sync_cache());
        let counter = Arc::new(AtomicU32::new(0));

        // Thread A: becomes leader, sleeps inside fallback to give B time to
        // arrive and become a follower.
        let cache_a = cache.clone();
        let counter_a = counter.clone();
        let handle_a = thread::spawn(move || {
            cache_a
                .get_or_sync(&"k".to_string(), || {
                    counter_a.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(120));
                    Ok("v".to_string())
                })
                .unwrap()
        });

        // Give A time to register as leader before B arrives.
        thread::sleep(Duration::from_millis(20));

        let cache_b = cache.clone();
        let counter_b = counter.clone();
        let handle_b = thread::spawn(move || {
            cache_b
                .get_or_sync(&"k".to_string(), || {
                    counter_b.fetch_add(1, Ordering::SeqCst);
                    Ok("should_not_run".to_string())
                })
                .unwrap()
        });

        let v_a = handle_a.join().expect("thread A panicked");
        let v_b = handle_b.join().expect("thread B panicked");

        assert_eq!(v_a, "v");
        assert_eq!(v_b, "v");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "fallback must run exactly once under single-flight"
        );
    }

    #[test]
    fn test_cache_set_with_ttl_sync_expires() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(&"k".to_string(), &"v".to_string(), Some(Duration::from_millis(50)))
            .unwrap();

        // Within TTL window: readable
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), Some("v".to_string()));

        // After TTL: expired
        thread::sleep(Duration::from_millis(120));
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_delete_sync() {
        let cache = make_sync_cache();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        cache.delete_sync(&"k".to_string()).unwrap();
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_exists_sync() {
        let cache = make_sync_cache();
        assert!(!cache.exists_sync(&"k".to_string()).unwrap());
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        assert!(cache.exists_sync(&"k".to_string()).unwrap());
    }

    #[test]
    fn test_cache_ttl_sync() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(&"k".to_string(), &"v".to_string(), Some(Duration::from_secs(60)))
            .unwrap();
        let ttl = cache.ttl_sync(&"k".to_string()).unwrap().expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(58));
        assert!(ttl <= Duration::from_secs(60));
        // Missing key
        assert_eq!(cache.ttl_sync(&"missing".to_string()).unwrap(), None);
    }

    #[test]
    fn test_cache_expire_sync() {
        let cache = make_sync_cache();
        cache
            .set_with_ttl_sync(&"k".to_string(), &"v".to_string(), Some(Duration::from_secs(60)))
            .unwrap();
        let ok = cache.expire_sync(&"k".to_string(), Duration::from_secs(120)).unwrap();
        assert!(ok);
        let ttl = cache.ttl_sync(&"k".to_string()).unwrap().expect("ttl should be Some");
        assert!(ttl > Duration::from_secs(118));
        // expire missing key
        let ok = cache
            .expire_sync(&"missing".to_string(), Duration::from_secs(60))
            .unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_cache_sync_methods_without_sync_mode_returns_err() {
        let cache: Cache<String, String> = Cache::new();
        assert!(cache.delete_sync(&"k".to_string()).is_err());
        assert!(cache.exists_sync(&"k".to_string()).is_err());
        assert!(cache.ttl_sync(&"k".to_string()).is_err());
        assert!(cache.expire_sync(&"k".to_string(), Duration::from_secs(1)).is_err());
    }
}
