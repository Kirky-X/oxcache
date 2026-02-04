//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache builder for advanced configuration

use super::backend_builder::BackendBuilder;
use crate::backend::client::MokaMemoryBackend as MemoryBackend;
use crate::cache::Cache;
use crate::error::Result;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// Builder for creating configured Cache instances
///
/// This builder provides a fluent interface for configuring cache instances
/// with advanced options like TTL, capacity, batch writes, and auto-promote.
///
/// # Type Parameters
///
/// * `K` - Key type, must implement `CacheKey` trait
/// * `V` - Value type, must implement `Cacheable` trait
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::Cache;
/// use std::time::Duration;
///
/// let cache: Cache<String, User> = Cache::builder()
///     .ttl(Duration::from_secs(3600))
///     .capacity(10000)
///     .batch_writes(true)
///     .build()
///     .await?;
/// ```
pub struct CacheBuilder<K, V> {
    backend_builder: Option<BackendBuilder>,
    ttl: Option<Duration>,
    capacity: Option<u64>,
    batch_writes: bool,
    auto_promote: bool,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            backend_builder: None,
            ttl: None,
            capacity: None,
            batch_writes: false,
            auto_promote: true,
            _phantom: PhantomData,
        }
    }
}

impl<K, V> CacheBuilder<K, V>
where
    K: crate::traits::CacheKey,
    V: crate::traits::Cacheable,
{
    /// Set the default TTL for cache entries
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live duration
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// let builder = Cache::builder().ttl(Duration::from_secs(3600));
    /// ```
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the capacity for memory-based backends
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = Cache::builder().capacity(10000);
    /// ```
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// Enable or disable batch writes
    ///
    /// When enabled, multiple write operations are batched for better performance.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable batch writes
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = Cache::builder().batch_writes(true);
    /// ```
    pub fn batch_writes(mut self, enabled: bool) -> Self {
        self.batch_writes = enabled;
        self
    }

    /// Enable or disable auto-promote (for tiered backends)
    ///
    /// When enabled, values from L2 are automatically promoted to L1 on cache misses.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable auto-promote
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let builder = Cache::builder().auto_promote(true);
    /// ```
    pub fn auto_promote(mut self, enabled: bool) -> Self {
        self.auto_promote = enabled;
        self
    }

    /// 使用外部confers配置（DI支持）
    ///
    /// 此方法允许从confers配置实例读取缓存配置，
    /// 并与手动配置的参数（如TTL、capacity）合并。
    /// 手动配置的参数优先级高于confers配置。
    ///
    /// # Arguments
    ///
    /// * `config` - confers配置实例
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Configuration Keys
    ///
    /// 从confers读取以下配置项（如果未手动设置）：
    ///
    /// - `oxcache.backend`: 后端类型 ("memory" | "redis" | "tiered")
    /// - `oxcache.capacity`: 内存缓存容量（默认10000）
    /// - `oxcache.ttl`: 默认TTL（秒）
    /// - `oxcache.redis.url`: Redis连接URL
    /// - `oxcache.redis.mode`: Redis模式
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::Cache;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use serde_json::json;
    ///
    /// let config = json!({
    ///     "oxcache": {
    ///         "backend": "memory",
    ///         "capacity": 10000,
    ///         "ttl": 3600
    ///     }
    /// });
    ///
    /// // 使用confers配置，但覆盖TTL
    /// let cache = Cache::builder()
    ///     .with_confers(&config)
    ///     .ttl(Duration::from_secs(7200))  // 覆盖confers中的TTL
    ///     .build()
    ///     .await?;
    /// ```
    ///
    /// # Features
    ///
    /// 此方法仅在启用 `confers` feature 时可用。
    #[cfg(feature = "confers")]
    pub fn with_confers(mut self, config: &serde_json::Value) -> Self {
        use std::time::Duration;

        // 获取oxcache配置部分，如果没有则使用空对象
        let oxcache_config: &serde_json::Map<String, serde_json::Value> = match config
            .get("oxcache")
        {
            Some(serde_json::Value::Object(obj)) => obj,
            _ => {
                static EMPTY: once_cell::sync::Lazy<serde_json::Map<String, serde_json::Value>> =
                    once_cell::sync::Lazy::new(serde_json::Map::new);
                &EMPTY
            }
        };

        // 如果尚未设置TTL，从confers读取
        if self.ttl.is_none() {
            if let Some(ttl_secs) = oxcache_config.get("ttl").and_then(|v| v.as_i64()) {
                self.ttl = Some(Duration::from_secs(ttl_secs as u64));
            }
        }

        // 如果尚未设置capacity，从confers读取
        if self.capacity.is_none() {
            if let Some(cap) = oxcache_config
                .get("capacity")
                .and_then(|v| v.as_u64().or(v.as_i64().map(|i| i as u64)))
            {
                self.capacity = Some(cap);
            }
        }

        // 使用BackendBuilder::with_confers创建后端构建器
        self.backend_builder = Some(super::BackendBuilder::with_confers(config));

        self
    }

    /// Set the backend builder
    ///
    /// # Arguments
    ///
    /// * `builder` - Backend builder instance
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::builder::BackendBuilder;
    ///
    /// let builder = Cache::builder()
    ///     .backend(BackendBuilder::redis().connection_string("redis://localhost:6379"));
    /// ```
    pub fn backend(mut self, builder: BackendBuilder) -> Self {
        self.backend_builder = Some(builder);
        self
    }

    /// Build the cache instance
    ///
    /// # Returns
    ///
    /// Configured cache instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if configuration is invalid or backend creation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::builder()
    ///     .ttl(Duration::from_secs(3600))
    ///     .capacity(10000)
    ///     .build()
    ///     .await?;
    /// ```
    pub async fn build(self) -> Result<Cache<K, V>> {
        let backend = if let Some(backend_builder) = self.backend_builder {
            backend_builder.build().await?
        } else {
            // Default to memory backend
            let builder = MemoryBackend::builder();
            let backend = if let Some(capacity) = self.capacity {
                builder.capacity(capacity).build()
            } else {
                builder.build()
            };
            Arc::new(backend)
        };

        Ok(Cache::new_with_backend(backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestValue {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn test_cache_builder_default() {
        let cache: Cache<String, TestValue> = CacheBuilder::default().build().await.unwrap();
        assert!(cache.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_builder_with_capacity() {
        let cache: Cache<String, TestValue> = CacheBuilder::default()
            .capacity(1000)
            .build()
            .await
            .unwrap();
        assert!(cache.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_builder_with_ttl() {
        let cache: Cache<String, TestValue> = CacheBuilder::default()
            .ttl(Duration::from_secs(3600))
            .build()
            .await
            .unwrap();
        assert!(cache.health_check().await.unwrap());
    }
}
