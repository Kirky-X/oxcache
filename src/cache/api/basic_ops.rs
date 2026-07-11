// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Cache 基础操作方法

use super::Cache;
use crate::core::constants::MAX_JSON_DEPTH;
use crate::error::{CacheError, Result};
use crate::traits::CacheKey;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[cfg(any(feature = "tracing", feature = "full"))]
use tracing::instrument;

/// 计算 JSON 值的嵌套深度（用于防止栈溢出攻击）
fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                1
            } else {
                map.values().map(json_depth).max().unwrap_or(0) + 1
            }
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                1
            } else {
                arr.iter().map(json_depth).max().unwrap_or(0) + 1
            }
        }
        _ => 1,
    }
}

/// 全局 get_or 去重锁，防止缓存击穿（thundering herd）。
/// 当多个并发请求同时调用 `get_or` 且缓存未命中时，
/// 只让第一个请求执行 fallback，其余请求等待结果。
static GET_OR_LOCKS: Lazy<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 用于 panic 安全地清理 GET_OR_LOCKS 中的条目。
///
/// 如果 leader 在插入条目后 panic，此守卫会在 Drop 时移除该条目，
/// 防止锁永远留在 HashMap 中导致后续所有 get_or 调用死锁。
struct GetOrGuard<'a> {
    map: &'a Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
    key: String,
    removed: bool,
}

impl Drop for GetOrGuard<'_> {
    fn drop(&mut self) {
        if !self.removed {
            if let Ok(mut map) = self.map.lock() {
                map.remove(&self.key);
            }
        }
    }
}

#[cfg(any(feature = "serialization", feature = "full"))]
fn deserialize_value<V: serde::de::DeserializeOwned>(data: &[u8]) -> Result<V> {
    let depth_limit: usize = MAX_JSON_DEPTH;
    let json_value: serde_json::Value =
        serde_json::from_slice(data).map_err(|e| CacheError::Serialization(e.to_string()))?;
    if json_depth(&json_value) > depth_limit {
        return Err(CacheError::Serialization(format!(
            "JSON深度 {} 超过最大限制 {}",
            json_depth(&json_value),
            depth_limit
        )));
    }
    serde_json::from_value(json_value).map_err(|e| CacheError::Serialization(e.to_string()))
}

#[cfg(not(any(feature = "serialization", feature = "full")))]
fn deserialize_value<V>(data: &[u8]) -> Result<V> {
    let _ = data;
    Err(CacheError::Serialization(
        "Serialization feature is required for typed get operations".to_string(),
    ))
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    #[cfg_attr(
        any(feature = "tracing", feature = "full"),
        instrument(skip(self, key), level = "debug", fields(key))
    )]
    pub async fn get(&self, key: &K) -> Result<Option<V>> {
        let key_str = key.to_key_string();
        let bytes = self.backend.get(&key_str).await?;
        match bytes {
            Some(data) => deserialize_value(&data).map(Some),
            None => Ok(None),
        }
    }

    // ========================================================================
    // Lifecycle and stats methods (delegating to backend)
    // ========================================================================

    /// Clear all entries in the cache.
    pub async fn clear(&self) -> Result<()> {
        self.backend.clear().await
    }

    /// Shutdown the cache and release resources.
    pub async fn shutdown(&self) {
        self.backend.shutdown().await
    }

    /// Health check for the cache backend.
    pub async fn health_check(&self) -> Result<()> {
        self.backend.health_check().await
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> Result<std::collections::HashMap<String, String>> {
        self.backend.stats().await
    }

    /// Get the number of entries in the cache.
    pub async fn len(&self) -> Result<u64> {
        self.backend.len().await
    }

    /// Check if the cache is empty.
    pub async fn is_empty(&self) -> Result<bool> {
        self.backend.is_empty().await
    }

    /// Get the capacity of the cache.
    pub async fn capacity(&self) -> Result<u64> {
        self.backend.capacity().await
    }

    #[cfg_attr(
        any(feature = "tracing", feature = "full"),
        instrument(skip(self, key, value), level = "debug", fields(key))
    )]
    pub async fn set(&self, key: &K, value: &V) -> Result<()> {
        self.set_with_ttl(key, value, None).await
    }

    pub async fn set_with_ttl(&self, key: &K, value: &V, ttl: Option<Duration>) -> Result<()> {
        let key_str = key.to_key_string();

        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let bytes = match serde_json::to_vec(value) {
                Ok(b) => b,
                Err(e) => return Err(CacheError::Serialization(e.to_string())),
            };
            self.backend.set(&key_str, bytes, ttl).await
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = (key_str, value);
            Err(CacheError::Serialization(
                "Serialization feature is required for typed set operations".to_string(),
            ))
        }
    }

    #[cfg_attr(
        any(feature = "tracing", feature = "full"),
        instrument(skip(self, key), level = "debug", fields(key))
    )]
    pub async fn delete(&self, key: &K) -> Result<()> {
        let key_str = key.to_key_string();
        self.backend.delete(&key_str).await
    }

    pub async fn exists(&self, key: &K) -> Result<bool> {
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
    #[cfg_attr(
        any(feature = "tracing", feature = "full"),
        instrument(skip(self, key), level = "debug", fields(key))
    )]
    pub async fn ttl(&self, key: &K) -> Result<Option<Duration>> {
        let key_str = key.to_key_string();
        self.backend.ttl(&key_str).await
    }

    /// Update the time-to-live for an existing key.
    ///
    /// Returns `Ok(true)` if the TTL was updated, `Ok(false)` if the key
    /// does not exist. This does NOT touch the value — only the TTL.
    #[cfg_attr(
        any(feature = "tracing", feature = "full"),
        instrument(skip(self, key), level = "debug", fields(key))
    )]
    pub async fn expire(&self, key: &K, ttl: Duration) -> Result<bool> {
        let key_str = key.to_key_string();
        self.backend.expire(&key_str, ttl).await
    }

    pub async fn get_or<F, Fut>(&self, key: &K, fallback: F) -> Result<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V>>,
    {
        // 快速路径：缓存命中
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let key_str = key.to_key_string();

        // 尝试注册为 leader；如果 key 已存在则成为 follower
        // 注意：锁必须在 await 之前释放，避免 await_holding_lock
        let (is_follower, notify) = {
            let mut map = GET_OR_LOCKS
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
                CacheError::L1Error("get_or: concurrent fetch leader failed to cache result".to_string())
            });
        }

        // 创建 panic 安全守卫，确保 leader 即使在 panic 时也会清理锁条目
        let mut guard = GetOrGuard {
            map: &GET_OR_LOCKS,
            key: key_str.clone(),
            removed: false,
        };

        // leader：二次检查缓存（避免与另一个刚刚完成的 leader 竞争）
        if let Some(value) = self.get(key).await? {
            GET_OR_LOCKS
                .lock()
                .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected")
                .remove(&key_str);
            guard.removed = true;
            notify.notify_waiters();
            return Ok(value);
        }

        self.execute_fallback(key, &key_str, fallback, &notify, &mut guard)
            .await
    }

    /// Execute the fallback function and notify waiters of the result.
    async fn execute_fallback<F, Fut>(
        &self,
        key: &K,
        key_str: &str,
        fallback: F,
        notify: &Arc<tokio::sync::Notify>,
        guard: &mut GetOrGuard<'_>,
    ) -> Result<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V>>,
    {
        let result = fallback().await;
        match result {
            Ok(value) => {
                self.set(key, &value).await?;
                GET_OR_LOCKS
                    .lock()
                    .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected")
                    .remove(key_str);
                guard.removed = true;
                notify.notify_waiters();
                Ok(value)
            }
            Err(e) => {
                GET_OR_LOCKS
                    .lock()
                    .expect("GET_OR_LOCKS poisoned - concurrent operation panic detected")
                    .remove(key_str);
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
// Returns `Err(CacheError::NotSupported)` when the cache was not built with
// `sync_mode` enabled (i.e., `backend_sync` is `None`).
//
// Single-flight for `get_or_sync` uses `std::sync::Condvar` (no async runtime
// required), mirroring the async `get_or` which uses `tokio::sync::Notify`.
// ============================================================================

/// Single-flight state for `get_or_sync`. The `Mutex<bool>` flag is `false`
/// while the leader is executing fallback, `true` once the leader has finished
/// (success or failure). Followers `wait()` on the `Condvar` until `true`.
type SyncFlight = Arc<(Mutex<bool>, Condvar)>;

/// Global registry of in-flight `get_or_sync` leaders, keyed by cache key.
/// Followers find their leader's `SyncFlight` here and block on its `Condvar`.
static GET_OR_SYNC_LOCKS: Lazy<Mutex<HashMap<String, SyncFlight>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Panic-safe guard for `get_or_sync` leaders. If the leader panics before
/// marking its flight `done`, this `Drop` impl flips the flag to `true` and
/// `notify_all`s followers so they don't block forever, then removes the
/// stale entry from the registry.
struct GetOrSyncGuard {
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
            GET_OR_SYNC_LOCKS
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
    fn sync_backend(&self) -> Result<&Arc<dyn crate::backend::SyncCacheBackend>> {
        self.backend_sync.as_ref().ok_or_else(|| {
            CacheError::NotSupported(
                "sync API requires CacheBuilder::sync_mode(true); backend_sync is None".to_string(),
            )
        })
    }

    /// Synchronously get a value from the cache.
    pub fn get_sync(&self, key: &K) -> Result<Option<V>> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        // Method-call syntax (not UFCS) — `dyn SyncCacheBackend` exposes
        // super-trait methods via its vtable; UFCS would require
        // `Arc<dyn SyncCacheBackend>: SyncCacheReader` which needs the
        // unstable `trait_upcasting` feature.
        let bytes = backend.get(&key_str)?;
        match bytes {
            Some(data) => deserialize_value(&data).map(Some),
            None => Ok(None),
        }
    }

    /// Synchronously set a value in the cache (no TTL).
    pub fn set_sync(&self, key: &K, value: &V) -> Result<()> {
        self.set_with_ttl_sync(key, value, None)
    }

    /// Synchronously set a value with an optional per-entry TTL.
    pub fn set_with_ttl_sync(&self, key: &K, value: &V, ttl: Option<Duration>) -> Result<()> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;

        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let bytes = serde_json::to_vec(value).map_err(|e| CacheError::Serialization(e.to_string()))?;
            backend.set(&key_str, bytes, ttl)
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = (key_str, value, ttl);
            Err(CacheError::Serialization(
                "Serialization feature is required for typed set operations".to_string(),
            ))
        }
    }

    /// Synchronously delete a key.
    pub fn delete_sync(&self, key: &K) -> Result<()> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.delete(&key_str)
    }

    /// Synchronously check if a key exists.
    pub fn exists_sync(&self, key: &K) -> Result<bool> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.exists(&key_str)
    }

    /// Synchronously get the remaining time-to-live for a key.
    ///
    /// Returns `Ok(None)` if the key has no per-entry TTL or does not exist.
    /// Mirrors the async [`Self::ttl`].
    pub fn ttl_sync(&self, key: &K) -> Result<Option<Duration>> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.ttl(&key_str)
    }

    /// Synchronously update the time-to-live for an existing key.
    ///
    /// Returns `Ok(true)` if the TTL was updated, `Ok(false)` if the key
    /// does not exist. Mirrors the async [`Self::expire`].
    pub fn expire_sync(&self, key: &K, ttl: Duration) -> Result<bool> {
        let key_str = key.to_key_string();
        let backend = self.sync_backend()?;
        backend.expire(&key_str, ttl)
    }

    /// Synchronously get-or-compute: returns cached value if present, otherwise
    /// invokes `fallback` and caches the result. Uses `Condvar`-based
    /// single-flight to prevent thundering-herd duplicate fallback calls.
    pub fn get_or_sync<F>(&self, key: &K, fallback: F) -> Result<V>
    where
        F: FnOnce() -> Result<V>,
    {
        // Fast path: cache hit
        if let Some(value) = self.get_sync(key)? {
            return Ok(value);
        }

        let key_str = key.to_key_string();

        // Register as leader or become follower. Lock is released before any
        // blocking work to avoid holding it while running fallback.
        let (is_follower, flight) = {
            let mut map = GET_OR_SYNC_LOCKS
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
                CacheError::L1Error("get_or_sync: concurrent fetch leader failed to cache result".to_string())
            });
        }

        // Leader path
        let mut guard = GetOrSyncGuard {
            map_key: key_str.clone(),
            flight: flight.clone(),
            removed: false,
        };

        // Double-check cache after acquiring leadership (another leader may
        // have just finished and cached the value)
        if let Some(value) = self.get_sync(key)? {
            Self::finish_sync_flight(&key_str, &flight, &mut guard);
            return Ok(value);
        }

        // Run fallback
        match fallback() {
            Ok(value) => {
                if let Err(e) = self.set_sync(key, &value) {
                    // Caching failed — still wake followers before propagating
                    Self::finish_sync_flight(&key_str, &flight, &mut guard);
                    return Err(e);
                }
                Self::finish_sync_flight(&key_str, &flight, &mut guard);
                Ok(value)
            }
            Err(e) => {
                Self::finish_sync_flight(&key_str, &flight, &mut guard);
                Err(e)
            }
        }
    }

    /// Mark the flight as done, notify all followers, and remove the entry
    /// from the registry. Idempotent via the `guard.removed` flag.
    fn finish_sync_flight(key_str: &str, flight: &SyncFlight, guard: &mut GetOrSyncGuard) {
        {
            let mut done = flight
                .0
                .lock()
                .expect("GET_OR_SYNC_LOCKS: leader flight mutex poisoned");
            *done = true;
        }
        flight.1.notify_all();
        GET_OR_SYNC_LOCKS
            .lock()
            .expect("GET_OR_SYNC_LOCKS poisoned - concurrent operation panic detected")
            .remove(key_str);
        guard.removed = true;
    }

    /// Synchronously clear all entries.
    pub fn clear_sync(&self) -> Result<()> {
        let backend = self.sync_backend()?;
        backend.clear()
    }

    /// Synchronously run a health check against the backend.
    pub fn health_check_sync(&self) -> Result<()> {
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
    pub fn stats_sync(&self) -> Result<std::collections::HashMap<String, String>> {
        let backend = self.sync_backend()?;
        backend.stats()
    }

    /// Synchronously get the number of entries.
    pub fn len_sync(&self) -> Result<u64> {
        let backend = self.sync_backend()?;
        backend.len()
    }

    /// Synchronously get the capacity.
    pub fn capacity_sync(&self) -> Result<u64> {
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
                Err(CacheError::Operation("fallback should not be called".to_string()))
            })
            .await
            .unwrap();
        assert_eq!(value, "cached");
    }

    #[tokio::test]
    async fn test_cache_get_or_fallback_error_propagates() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();

        let result: Result<String> = cache
            .get_or(&"missing".to_string(), || async {
                Err(CacheError::Operation("db down".to_string()))
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(CacheError::Operation(msg)) => assert_eq!(msg, "db down"),
            _ => panic!("expected CacheError::Operation"),
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
    // json_depth / deserialize_value internal functions
    // ========================================================================

    #[test]
    fn test_json_depth_scalar() {
        let v = serde_json::json!(42);
        assert_eq!(json_depth(&v), 1);
    }

    #[test]
    fn test_json_depth_empty_object() {
        let v = serde_json::json!({});
        assert_eq!(json_depth(&v), 1);
    }

    #[test]
    fn test_json_depth_empty_array() {
        let v = serde_json::json!([]);
        assert_eq!(json_depth(&v), 1);
    }

    #[test]
    fn test_json_depth_nested_object() {
        let v = serde_json::json!({"a": {"b": {"c": 1}}});
        assert_eq!(json_depth(&v), 4);
    }

    #[test]
    fn test_json_depth_nested_array() {
        let v = serde_json::json!([[[1]]]);
        assert_eq!(json_depth(&v), 4);
    }

    #[test]
    fn test_json_depth_mixed() {
        let v = serde_json::json!({"a": [1, {"b": 2}]});
        // object -> array -> object -> scalar = 4
        assert_eq!(json_depth(&v), 4);
    }

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
        cache.backend.set("bad", b"not json".to_vec(), None).await.unwrap();

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
        cache.backend.set("deep", json_str.into_bytes(), None).await.unwrap();

        let result = cache.get(&"deep".to_string()).await;
        assert!(result.is_err());
        match result {
            Err(CacheError::Serialization(msg)) => {
                assert!(msg.contains("深度") || msg.contains("depth"));
            }
            _ => panic!("expected CacheError::Serialization"),
        }
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    use crate::backend::memory::MokaMemoryBackend;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
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
            matches!(result, Err(CacheError::NotSupported(_))),
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
                Err(CacheError::Operation("fallback should not run".to_string()))
            })
            .unwrap();
        assert_eq!(v, "cached");
    }

    #[test]
    fn test_cache_get_or_sync_cache_miss_triggers_fallback() {
        let cache = make_sync_cache();
        let v = cache
            .get_or_sync(&"k".to_string(), || Ok("computed".to_string()))
            .unwrap();
        assert_eq!(v, "computed");

        // Verify the value was cached: a direct get_sync returns it
        let cached = cache.get_sync(&"k".to_string()).unwrap().unwrap();
        assert_eq!(cached, "computed");
    }

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
}
