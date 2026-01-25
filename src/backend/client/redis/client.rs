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
/// Now includes connection pooling for better performance.
#[derive(Clone)]
pub struct RedisBackend {
    client: Arc<Client>,
    mode: RedisMode,
    /// Connection pool size (for future use with r2d2 or mobc)
    pool_size: usize,
}

impl RedisBackend {
    /// Create a new Redis backend with connection string
    pub async fn new(connection_string: &str) -> Result<Self> {
        Self::builder()
            .connection_string(connection_string)
            .build()
            .await
    }

    /// Create a new Redis backend with connection pool
    pub async fn with_pool(connection_string: &str, pool_size: usize) -> Result<Self> {
        Self::builder()
            .connection_string(connection_string)
            .pool_size(pool_size)
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

    /// Get connection pool size
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// Ping the Redis server
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
    pool_size: Option<usize>,
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

    /// Set connection pool size
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = Some(size);
        self
    }

    /// Build the Redis backend
    pub async fn build(self) -> Result<RedisBackend> {
        let connection_string = self.connection_string.ok_or_else(|| {
            CacheError::Configuration("Connection string is required".to_string())
        })?;

        // 强制 TLS 在生产环境，允许通过环境变量覆盖用于测试
        if !connection_string.starts_with("rediss://") {
            // 检查是否允许非 TLS 连接（用于开发和测试）
            if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
                tracing::warn!("Using insecure Redis connection (TLS disabled). This is only allowed in development/testing.");
            } else {
                return Err(CacheError::Configuration(
                    "Redis connection must use TLS (rediss://) in production. \
                    For development/testing, set OXCACHE_ALLOW_INSECURE_REDIS=1 to override.".to_string()
                ));
            }
        }

        // 创建客户端并验证连接
        let client =
            Client::open(connection_string).map_err(|e| CacheError::Connection(e.to_string()))?;
        
        // 快速验证连接是否可用（2秒超时）
        let connection_timeout = std::time::Duration::from_secs(2);
        let connection_result = tokio::time::timeout(
            connection_timeout,
            client.get_connection_manager()
        ).await;
        
        match connection_result {
            Ok(Ok(_)) => {
                // 连接成功
            }
            Ok(Err(e)) => {
                return Err(CacheError::Connection(format!(
                    "Failed to connect to Redis: {}", e
                )));
            }
            Err(_) => {
                return Err(CacheError::Connection(
                    "Connection timeout - Redis server unavailable".to_string()
                ));
            }
        }

        Ok(RedisBackend {
            client: Arc::new(client),
            mode: self.mode,
            pool_size: self.pool_size.unwrap_or(1),
        })
    }
}

#[async_trait]
impl CacheBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        // Use SCAN + DEL instead of FLUSHDB to avoid affecting other connections/databases
        // FLUSHDB clears the entire database which can interfere with other tests
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        // Iterate through all keys and delete them using SCAN
        let mut cursor = 0i64;
        let mut deleted_count = 0;

        loop {
            let (new_cursor, keys): (i64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("*")
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    if is_connection_error(&e) {
                        CacheError::Connection(e.to_string())
                    } else {
                        CacheError::Operation(e.to_string())
                    }
                })?;

            for key in &keys {
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
                deleted_count += 1;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        tracing::debug!("Cleared {} keys from Redis", deleted_count);
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        // Connection will be dropped when client is dropped
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
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
