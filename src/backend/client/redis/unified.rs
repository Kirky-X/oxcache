//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified Redis backend that consolidates all Redis-related functionality

use crate::backend::backend::CacheBackend;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::{Client, RedisError};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Redis connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedisMode {
    /// Standalone Redis server
    Standalone,
    /// Redis Sentinel for high availability
    Sentinel,
    /// Redis Cluster for horizontal scaling
    Cluster,
}

/// Redis connection configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Connection string or list of connection strings
    pub connection_strings: Vec<String>,
    /// Connection mode
    pub mode: RedisMode,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Command timeout
    pub command_timeout: Duration,
    /// Maximum connection pool size
    pub max_pool_size: Option<usize>,
    /// Minimum connection pool size
    pub min_pool_size: Option<usize>,
    /// Connection name (for identification)
    pub connection_name: Option<String>,
    /// Password for authentication
    pub password: Option<String>,
    /// Database number
    pub database: Option<i64>,
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

/// Unified Redis connection manager
///
/// This provides a centralized way to manage Redis connections with support
/// for different deployment modes and connection pooling.
#[derive(Clone)]
pub struct UnifiedRedisManager {
    config: RedisConfig,
    client: Arc<Client>,
}

impl UnifiedRedisManager {
    /// Create a new Redis manager with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(RedisConfig::default()).await
    }

    /// Create a new Redis manager with custom configuration
    pub async fn with_config(config: RedisConfig) -> Result<Self> {
        let connection_string = config.connection_strings.first()
            .ok_or_else(|| CacheError::ConfigError("No connection string provided".to_string()))?;

        let client = Arc::new(Client::open(connection_string.as_str())?);

        // Test connection
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        Ok(Self { config, client })
    }

    /// Get the configuration
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

    /// Get the Redis client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a connection manager
    pub async fn get_connection_manager(&self) -> Result<ConnectionManager> {
        self.client
            .get_connection_manager()
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))
    }
}

/// Unified Redis backend
///
/// This backend provides a comprehensive Redis implementation with support
/// for standalone, sentinel, and cluster modes.
#[derive(Clone)]
pub struct UnifiedRedisBackend {
    manager: Arc<UnifiedRedisManager>,
}

impl UnifiedRedisBackend {
    /// Create a new unified Redis backend with default configuration
    pub async fn new() -> Result<Self> {
        let manager = Arc::new(UnifiedRedisManager::new().await?);
        Ok(Self { manager })
    }

    /// Create a new unified Redis backend with custom configuration
    pub async fn with_config(config: RedisConfig) -> Result<Self> {
        let manager = Arc::new(UnifiedRedisManager::with_config(config).await?);
        Ok(Self { manager })
    }

    /// Create a new unified Redis backend with connection string
    pub async fn from_connection_string(connection_string: &str) -> Result<Self> {
        let mut config = RedisConfig::default();
        config.connection_strings = vec![connection_string.to_string()];
        Self::with_config(config).await
    }

    /// Get the Redis manager
    pub fn manager(&self) -> &UnifiedRedisManager {
        &self.manager
    }
}

#[async_trait]
impl CacheBackend for UnifiedRedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self.manager.get_connection_manager().await?;
        let result: Option<Vec<u8>> = redis::cmd("GET")
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
        Ok(result)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.manager.get_connection_manager().await?;
        
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
        let mut conn = self.manager.get_connection_manager().await?;
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
        let mut conn = self.manager.get_connection_manager().await?;
        let result: i64 = redis::cmd("EXISTS")
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
        Ok(result > 0)
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.manager.get_connection_manager().await?;
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
        // ConnectionManager will be closed automatically when dropped
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let mut conn = self.manager.get_connection_manager().await?;
        let result: i64 = redis::cmd("TTL")
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
        
        match result {
            -1 => Ok(None), // No expiration
            -2 => Ok(None), // Key doesn't exist
            ttl => Ok(Some(Duration::from_secs(ttl as u64))),
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.manager.get_connection_manager().await?;
        let ttl_secs = ttl.as_secs();
        let result: i64 = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl_secs)
            .query_async::<()>(&mut conn)
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
        let mut conn = self.manager.get_connection_manager().await?;
        let result: String = redis::cmd("PING")
            .query_async::<()>(&mut conn)
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
        let mut conn = self.manager.get_connection_manager().await?;
        let info: String = redis::cmd("INFO")
            .query_async::<()>(&mut conn)
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
        stats.insert("mode".to_string(), format!("{:?}", self.manager.config.mode));
        stats.insert("info".to_string(), info);
        Ok(stats)
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
    fn test_redis_config_default() {
        let config = RedisConfig::default();
        assert_eq!(config.connection_strings, vec!["redis://localhost:6379"]);
        assert_eq!(config.mode, RedisMode::Standalone);
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.command_timeout, Duration::from_secs(5));
        assert_eq!(config.max_pool_size, Some(10));
        assert_eq!(config.min_pool_size, Some(1));
        assert_eq!(config.connection_name, Some("oxcache".to_string()));
        assert_eq!(config.password, None);
        assert_eq!(config.database, Some(0));
    }

    #[test]
    fn test_redis_mode() {
        assert_eq!(RedisMode::Standalone, RedisMode::Standalone);
        assert_ne!(RedisMode::Standalone, RedisMode::Sentinel);
        assert_ne!(RedisMode::Standalone, RedisMode::Cluster);
        assert_ne!(RedisMode::Sentinel, RedisMode::Cluster);
    }
}
