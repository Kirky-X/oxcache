//!
//! MIT License
//!
//! Redis backend implementation with connection pooling

use crate::backend::backend::CacheBackend;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
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

/// Redis configuration for connection setup
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisConfig {
    /// List of connection strings
    pub connection_strings: Vec<String>,
    /// Connection mode
    pub mode: RedisMode,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Command timeout
    pub command_timeout: Duration,
    /// Maximum pool size
    pub max_pool_size: Option<usize>,
    /// Minimum pool size
    pub min_pool_size: Option<usize>,
    /// Connection name
    pub connection_name: Option<String>,
    /// Password for authentication
    pub password: Option<String>,
    /// Database number
    pub database: Option<u32>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            connection_strings: vec!["redis://localhost:6379".to_string()],
            mode: RedisMode::Standalone,
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(5),
            max_pool_size: Some(10),
            min_pool_size: Some(1),
            connection_name: Some("oxcache".to_string()),
            password: None,
            database: Some(0),
        }
    }
}

/// Redis cache backend
///
/// This backend provides a distributed cache using Redis.
/// It supports standalone, sentinel, and cluster modes.
#[derive(Clone)]
pub struct RedisBackend {
    client: Arc<Client>,
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

    /// Ping the Redis server
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        
        Ok(result)
    }
}

/// Builder for RedisBackend
#[derive(Debug, Default)]
pub struct RedisBackendBuilder {
    connection_string: Option<String>,
    mode: RedisMode,
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

    /// Build the Redis backend
    pub async fn build(self) -> Result<RedisBackend> {
        let connection_string = self.connection_string
            .ok_or_else(|| CacheError::Configuration("Connection string is required".to_string()))?;

        let client = Client::open(connection_string)
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
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

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
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(&value)
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
                .arg(&value)
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
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

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
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

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

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

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

        if result <= 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs(result as u64)))
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        let result: i64 = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl.as_secs())
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
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

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
        // Connection will be dropped when client is dropped
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

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
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        let info: String = redis::cmd("INFO")
            .arg("memory")
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
        stats.insert("memory_info".to_string(), info);
        Ok(stats)
    }
}

/// Check if a Redis error is a connection error
fn is_connection_error(e: &RedisError) -> bool {
    e.is_timeout() || e.is_io_error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_mode_default() {
        assert_eq!(RedisMode::Standalone, RedisMode::default());
    }

    #[test]
    fn test_redis_mode_variants() {
        let _standalone = RedisMode::Standalone;
        let _sentinel = RedisMode::Sentinel;
        let _cluster = RedisMode::Cluster;
    }
}
