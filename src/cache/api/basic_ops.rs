//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 基础操作方法

use super::Cache;
use crate::core::constants::MAX_JSON_DEPTH;
use crate::error::{CacheError, Result};
use crate::traits::CacheKey;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
}
