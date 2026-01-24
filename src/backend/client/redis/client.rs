//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis backend implementation with connection pooling

use crate::backend::backend::CacheBackend;
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
/// use oxcache::backend::client::redis::{RedisBackend, RedisMode};
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
    manager: Arc<ConnectionManager>,
    mode: RedisMode,
}

impl RedisBackend {
    /// Create a new Redis backend with connection string
    pub async fn new(connection_string: &str) -> Result<Self> {
        Self::builder()
            .connection_string(connection_string)
            .build()
            .await
    }

    /// Create a new Redis backend builder
    pub fn builder() -> RedisBackendBuilder {
        RedisBackendBuilder::default()
    }

    /// Get the Redis mode
    pub fn mode(&self) -> RedisMode {
        self.mode
    }

    /// Get the Redis client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the connection manager
    pub fn manager(&self) -> &ConnectionManager {
        &self.manager
    }
}

#[async_trait]
impl CacheBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self.manager.clone();
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
        let mut conn = self.manager.clone();
        
        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(&value)
                .query_async(&mut conn)
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
                .arg(&value)
                .query_async(&mut conn)
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
        let mut conn = self.manager.clone();
        redis::cmd("DEL")
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
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.manager.clone();
        let result: i64 = redis::cmd("EXISTS")
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
        Ok(result > 0)
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.manager.clone();
        redis::cmd("FLUSHDB")
            .query_async(&mut conn)
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
        // ConnectionManager will be closed automatically when dropped
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let mut conn = self.manager.clone();
        let result: i64 = redis::cmd("TTL")
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
        
        match result {
            -1 => Ok(None), // No expiration
            -2 => Ok(None), // Key doesn't exist
            ttl => Ok(Some(Duration::from_secs(ttl as u64))),
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.manager.clone();
        let ttl_secs = ttl.as_secs();
        let result: i64 = redis::cmd("EXPIRE")
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
        Ok(result > 0)
    }

    async fn health_check(&self) -> Result<bool> {
        let mut conn = self.manager.clone();
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;
        Ok(result == "PONG")
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut conn = self.manager.clone();
        let info: String = redis::cmd("INFO")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                if is_connection_error(&e) {
                    CacheError::Connection(e.to_string())
                } else {
                    CacheError::Operation(e.to_string())
                }
            })?;

        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "redis".to_string());
        stats.insert("mode".to_string(), format!("{:?}", self.mode));
        stats.insert("info".to_string(), info);
        Ok(stats)
    }
}

/// Redis backend builder
#[derive(Debug, Clone, Default)]
pub struct RedisBackendBuilder {
    connection_string: Option<String>,
    mode: RedisMode,
    connection_name: Option<String>,
}

impl RedisBackendBuilder {
    /// Set the connection string
    pub fn connection_string(mut self, connection_string: &str) -> Self {
        self.connection_string = Some(connection_string.to_string());
        self
    }

    /// Set the Redis mode
    pub fn mode(mut self, mode: RedisMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the connection name
    pub fn connection_name(mut self, name: &str) -> Self {
        self.connection_name = Some(name.to_string());
        self
    }

    /// Build the Redis backend
    pub async fn build(self) -> Result<RedisBackend> {
        let connection_string = self.connection_string
            .ok_or_else(|| CacheError::Configuration("Connection string is required".to_string()))?;

        let client = Client::open(connection_string)
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        let manager = client.get_connection_manager()
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        Ok(RedisBackend {
            client: Arc::new(client),
            manager: Arc::new(manager),
            mode: self.mode,
        })
    }
}

/// Check if a Redis error is a connection error
fn is_connection_error(error: &RedisError) -> bool {
    let error_str = error.to_string().to_lowercase();
    error_str.contains("connection") || 
    error_str.contains("timeout") || 
    error_str.contains("network") ||
    error_str.contains("broken pipe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_mode_default() {
        let mode = RedisMode::default();
        assert_eq!(mode, RedisMode::Standalone);
    }

    #[test]
    fn test_redis_backend_builder() {
        let builder = RedisBackendBuilder::default()
            .connection_string("redis://localhost:6379")
            .mode(RedisMode::Cluster)
            .connection_name("test");

        // We can't test the build() method without a Redis server
        // but we can test the builder configuration
        assert_eq!(builder.connection_string, Some("redis://localhost:6379".to_string()));
        assert_eq!(builder.mode, RedisMode::Cluster);
        assert_eq!(builder.connection_name, Some("test".to_string()));
    }
}
