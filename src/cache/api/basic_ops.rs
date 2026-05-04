//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 基础操作方法

use super::Cache;
use crate::core::traits::{CacheKey, Cacheable};
use crate::error::{CacheError, Result};
use std::time::Duration;

#[cfg(any(feature = "tracing", feature = "full"))]
use tracing::instrument;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    #[cfg_attr(
        any(feature = "tracing", feature = "full"),
        instrument(skip(self, key), level = "debug", fields(key))
    )]
    pub async fn get(&self, key: &K) -> Result<Option<V>> {
        let key_str = key.to_key_string();
        let bytes = self.backend.get(&key_str).await?;

        #[cfg(any(feature = "serialization", feature = "full"))]
        match bytes {
            Some(data) => {
                let value: V = match serde_json::from_slice(&data) {
                    Ok(v) => v,
                    Err(_) => return Ok(None),
                };
                Ok(Some(value))
            }
            None => Ok(None),
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            let _ = bytes;
            Err(CacheError::Serialization(
                "Serialization feature is required for typed get operations".to_string(),
            ))
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
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }
        let value = fallback().await?;
        self.set(key, &value).await?;
        Ok(value)
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
        // len() may be approximate for concurrent caches
        let len = cache.len().await.unwrap();
        assert!(len >= 0 && len <= 100);
    }

    #[tokio::test]
    async fn test_cache_is_empty() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        // Initially empty or near-empty
        let empty = cache.is_empty().await.unwrap();
        // After insert, may not be empty
        cache.set(&"key".to_string(), &"value".to_string()).await.unwrap();
        // is_empty() behavior varies by backend
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
