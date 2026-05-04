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
