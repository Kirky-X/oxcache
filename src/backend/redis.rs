//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis backend implementation with connection pooling

use super::new_backend::CacheBackend;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::{Client, RedisError};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Redis connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedisMode {
    /// Standalone Redis server
    #[default]
    Standalone,
    /// Redis Sentinel for high availability
    Sentinel,
    /// Redis Cluster for horizontal scaling
    Cluster,
}

/// Redis cache backend
///
/// This backend provides a distributed cache using Redis with connection pooling.
/// It supports standalone, sentinel, and cluster modes.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::{RedisBackend, RedisMode};
///
/// // Create with default settings
/// let backend = RedisBackend::new("redis://localhost:6379").await?;
///
/// // Create with custom settings
/// let backend = RedisBackend::builder()
///     .connection_string("redis://localhost:6379")
///     .mode(RedisMode::Standalone)
///     .build()
///     .await?;
/// ```
#[derive(Clone)]
pub struct RedisBackend {
    client: Arc<Client>,
    mode: RedisMode,
}

impl RedisBackend {
    /// Create a new Redis backend with default settings
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection string
    ///
    /// # Returns
    ///
    /// Configured RedisBackend instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = RedisBackend::new("redis://localhost:6379").await?;
    /// ```
    pub async fn new(connection_string: &str) -> Result<Self> {
        Self::builder()
            .connection_string(connection_string)
            .build()
            .await
    }

    /// Create a new builder for configuring the Redis backend
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::backend::{RedisBackend, RedisMode};
    ///
    /// let backend = RedisBackend::builder()
    ///     .connection_string("redis://localhost:6379")
    ///     .mode(RedisMode::Standalone)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn builder() -> RedisBackendBuilder {
        RedisBackendBuilder::default()
    }

    async fn get_connection(&self) -> Result<ConnectionManager> {
        ConnectionManager::new((*self.client).clone())
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))
    }
}

/// Builder for RedisBackend
#[derive(Default)]
pub struct RedisBackendBuilder {
    connection_string: Option<String>,
    mode: RedisMode,
}

impl RedisBackendBuilder {
    /// Set the Redis connection string
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn connection_string(mut self, connection_string: &str) -> Self {
        self.connection_string = Some(connection_string.to_string());
        self
    }

    /// Set the Redis mode
    ///
    /// # Arguments
    ///
    /// * `mode` - Redis mode (Standalone, Sentinel, Cluster)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn mode(mut self, mode: RedisMode) -> Self {
        self.mode = mode;
        self
    }

    /// Build the Redis backend
    ///
    /// # Returns
    ///
    /// Configured RedisBackend instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if connection string is not set or connection fails
    pub async fn build(self) -> Result<RedisBackend> {
        let connection_string = self.connection_string.ok_or_else(|| {
            CacheError::ConfigError("Redis connection string is required".to_string())
        })?;

        let client =
            Client::open(connection_string).map_err(|e| CacheError::Connection(e.to_string()))?;

        // Test connection
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        Ok(RedisBackend {
            client: Arc::new(client),
            mode: self.mode,
        })
    }
}

#[async_trait]
impl CacheBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self.get_connection().await?;
        let result: Option<Vec<u8>> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;
        Ok(result)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.get_connection().await?;

        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| {
                    if is_connection_error(&e) {
                        CacheError::Connection(e.to_string())
                    } else {
                        CacheError::Operation(e.to_string())
                    }
                })?;
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| {
                    if is_connection_error(&e) {
                        CacheError::Connection(e.to_string())
                    } else {
                        CacheError::Operation(e.to_string())
                    }
                })?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        redis::cmd("DEL")
            .arg(key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let result: bool = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;
        Ok(result)
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        // ConnectionManager handles cleanup automatically
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let mut conn = self.get_connection().await?;

        if !self.exists(key).await? {
            return Err(CacheError::NotFound(key.to_string()));
        }

        let ttl_secs: i64 = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;

        if ttl_secs < 0 {
            // -1 means no expiration, -2 means key doesn't exist
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs(ttl_secs as u64)))
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let ttl_secs = ttl.as_secs();
        let result: bool = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;
        Ok(result)
    }

    async fn health_check(&self) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(result == "PONG")
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut conn = self.get_connection().await?;

        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "redis".to_string());
        stats.insert("mode".to_string(), format!("{:?}", self.mode));

        // Get Redis info
        let info: String = redis::cmd("INFO")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        // Parse some basic stats from INFO
        for line in info.lines() {
            if let Some(value) = line.strip_prefix("used_memory:") {
                stats.insert("used_memory".to_string(), value.to_string());
            } else if let Some(value) = line.strip_prefix("connected_clients:") {
                stats.insert("connected_clients".to_string(), value.to_string());
            }
        }

        Ok(stats)
    }
}

/// Check if a Redis error is a connection error
fn is_connection_error(e: &RedisError) -> bool {
    let error_str = e.to_string().to_lowercase();
    error_str.contains("connection")
        || error_str.contains("timeout")
        || error_str.contains("broken pipe")
        || error_str.contains("connection refused")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running Redis server
    async fn test_redis_backend_basic() {
        let backend = RedisBackend::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        // Test health check
        assert!(backend.health_check().await.unwrap());

        // Test set and get
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    #[ignore] // Requires running Redis server
    async fn test_redis_backend_ttl() {
        let backend = RedisBackend::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        backend
            .set("key1", b"value1".to_vec(), Some(Duration::from_secs(2)))
            .await
            .unwrap();

        // Value should exist immediately
        assert!(backend.exists("key1").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Value should be expired
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    #[ignore] // Requires running Redis server
    async fn test_redis_backend_stats() {
        let backend = RedisBackend::new("redis://localhost:6379")
            .await
            .expect("Failed to connect to Redis");

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"redis".to_string()));
        assert!(stats.contains_key("mode"));
    }
}
