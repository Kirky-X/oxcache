//!
//! MIT License
//!
//! Redis backend implementation with ConnectionManager

use crate::backend::interface::CacheBackend;
use crate::backend::score::{BackendScore, Scores};
use crate::error::{CacheError, Result};
use crate::security;
use async_trait::async_trait;
use redis::{Client, RedisError};
use std::any::Any;
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
/// Uses ConnectionManager for efficient connection pooling.
#[derive(Clone)]
pub struct RedisBackend {
    client: Arc<Client>,
    mode: RedisMode,
    /// Connection manager for automatic connection pooling
    connection_manager: redis::aio::ConnectionManager,
}

impl RedisBackend {
    /// Create a new Redis backend with connection string
    pub async fn new(connection_string: &str) -> Result<Self> {
        Self::builder().connection_string(connection_string).build().await
    }

    /// Create a new Redis backend with connection pool
    pub async fn with_pool(connection_string: &str, _pool_size: usize) -> Result<Self> {
        Self::builder().connection_string(connection_string).build().await
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
        let mut conn = self.connection_manager.clone();
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

    /// Set connection pool size (deprecated: ConnectionManager manages this automatically)
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = Some(size);
        self
    }

    /// Build the Redis backend
    pub async fn build(self) -> Result<RedisBackend> {
        let connection_string = self
            .connection_string
            .ok_or_else(|| CacheError::ConfigError("Connection string is required".to_string()))?;

        if !connection_string.starts_with("rediss://") {
            if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
                tracing::warn!(
                    "Using insecure Redis connection (TLS disabled). This is only allowed in development/testing."
                );
            } else {
                return Err(CacheError::ConfigError(
                    "Redis connection must use TLS (rediss://) in production. \
                    For development/testing, set OXCACHE_ALLOW_INSECURE_REDIS=1 to override."
                        .to_string(),
                ));
            }
        }

        let client = Client::open(connection_string).map_err(|e| CacheError::Connection(e.to_string()))?;

        let connection_timeout = std::time::Duration::from_secs(2);
        let connection_result = tokio::time::timeout(connection_timeout, client.get_connection_manager()).await;

        let connection_manager = match connection_result {
            Ok(Ok(mgr)) => mgr,
            Ok(Err(e)) => {
                return Err(CacheError::Connection(format!("Failed to connect to Redis: {}", e)));
            }
            Err(_) => {
                return Err(CacheError::Connection(
                    "Connection timeout - Redis server unavailable".to_string(),
                ));
            }
        };

        Ok(RedisBackend {
            client: Arc::new(client),
            mode: self.mode,
            connection_manager,
        })
    }
}

#[async_trait]
impl CacheBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        security::validate_redis_key(key)?;

        let mut conn = self.connection_manager.clone();
        let result: Option<Vec<u8>> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(result)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        security::validate_redis_key(key)?;

        let mut conn = self.connection_manager.clone();

        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| CacheError::Connection(e.to_string()))?;
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| CacheError::Connection(e.to_string()))?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        security::validate_redis_key(key)?;

        let mut conn = self.connection_manager.clone();
        redis::cmd("DEL")
            .arg(key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        security::validate_redis_key(key)?;

        let mut conn = self.connection_manager.clone();
        let n: i64 = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(n > 0)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        security::validate_redis_key(key)?;

        let mut conn = self.connection_manager.clone();
        let n: i64 = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        if n <= 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs(n as u64)))
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        security::validate_redis_key(key)?;

        let mut conn = self.connection_manager.clone();
        let result: i64 = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(result > 0)
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.connection_manager.clone();

        security::validate_scan_pattern("*")?;

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
                security::validate_redis_key(key)?;
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
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let mut conn = self.connection_manager.clone();
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(result == "PONG")
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut conn = self.connection_manager.clone();
        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        let mut stats = HashMap::new();
        stats.insert("memory_info".to_string(), info);
        Ok(stats)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn len(&self) -> Result<u64> {
        let mut conn = self.connection_manager.clone();
        let len: i64 = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(len as u64)
    }

    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await?.eq(&0))
    }

    async fn capacity(&self) -> Result<u64> {
        Ok(0)
    }
}

impl BackendScore for RedisBackend {
    fn score(&self) -> u8 {
        Scores::REDIS
    }

    fn is_persistent(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        "redis"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn is_connection_error(e: &RedisError) -> bool {
    e.is_timeout() || e.is_io_error()
}

#[cfg(feature = "lua-script")]
impl RedisBackend {
    pub async fn eval_lua(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value> {
        security::validate_lua_script(script, keys.len())?;

        let mut conn = self.connection_manager.clone();

        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }
        for arg in args {
            cmd.arg(arg);
        }

        let result = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        Ok(result)
    }

    pub async fn eval_sha(&self, sha: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value> {
        let mut conn = self.connection_manager.clone();

        let mut cmd = redis::cmd("EVALSHA");
        cmd.arg(sha).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }
        for arg in args {
            cmd.arg(arg);
        }

        let result = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        Ok(result)
    }

    pub async fn script_load(&self, script: &str) -> Result<String> {
        security::validate_lua_script(script, 0)?;

        let mut conn = self.connection_manager.clone();

        let sha: String = redis::cmd("SCRIPT")
            .arg("LOAD")
            .arg(script)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(sha)
    }
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

    #[cfg(feature = "lua-script")]
    mod lua_script_tests {
        use super::*;

        #[test]
        fn test_validate_lua_script_valid() {
            let result = security::validate_lua_script("return redis.call('GET', KEYS[1])", 1);
            assert!(result.is_ok());

            let result = security::validate_lua_script(
                "local a = redis.call('GET', KEYS[1]); \
                 local b = redis.call('GET', KEYS[2]); \
                 return a + b",
                2,
            );
            assert!(result.is_ok());
        }

        #[test]
        fn test_validate_lua_script_forbidden_commands() {
            let result = security::validate_lua_script("return redis.call('FLUSHALL')", 0);
            assert!(result.is_err());

            let result = security::validate_lua_script("return redis.call('FLUSHDB')", 0);
            assert!(result.is_err());

            let result = security::validate_lua_script("return redis.call('KEYS', '*')", 0);
            assert!(result.is_err());

            let result = security::validate_lua_script("return redis.call('SHUTDOWN')", 0);
            assert!(result.is_err());

            let result = security::validate_lua_script("return redis.call('DEBUG', 'SEGFAULT')", 0);
            assert!(result.is_err());
        }

        #[test]
        fn test_validate_lua_script_max_length() {
            let max_script = "x".repeat(10 * 1024);
            let result = security::validate_lua_script(&max_script, 1);
            assert!(result.is_ok());

            let over_max_script = "x".repeat(10 * 1024 + 1);
            let result = security::validate_lua_script(&over_max_script, 1);
            assert!(result.is_err());
        }

        #[test]
        fn test_validate_lua_script_max_keys() {
            let script = "return 1";
            let result = security::validate_lua_script(script, 100);
            assert!(result.is_ok());

            let result = security::validate_lua_script(script, 101);
            assert!(result.is_err());
        }

        #[test]
        fn test_validate_lua_script_case_insensitive() {
            let result = security::validate_lua_script("return redis.call('FLUSHALL')", 0);
            assert!(result.is_err());

            let result = security::validate_lua_script("return redis.call('flushall')", 0);
            assert!(result.is_err());

            let result = security::validate_lua_script("return redis.call('FlushAll')", 0);
            assert!(result.is_err());
        }

        #[test]
        fn test_validate_lua_script_safe_commands() {
            let result = security::validate_lua_script("return redis.call('GET', KEYS[1])", 1);
            assert!(result.is_ok());

            let result = security::validate_lua_script("return redis.call('SET', KEYS[1], 'value')", 1);
            assert!(result.is_ok());

            let result = security::validate_lua_script("return redis.call('INCR', KEYS[1])", 1);
            assert!(result.is_ok());

            let result = security::validate_lua_script(
                "local val = redis.call('HGET', KEYS[1], 'field'); \
                 if not val then redis.call('HSET', KEYS[1], 'field', 'default'); end; \
                 return val",
                1,
            );
            assert!(result.is_ok());
        }

        #[test]
        fn test_validate_lua_script_with_args() {
            let result = security::validate_lua_script(
                "local val = redis.call('GET', KEYS[1]); \
                 if val then \
                   local new_val = tonumber(val) + tonumber(ARGV[1]); \
                   redis.call('SET', KEYS[1], new_val); \
                   return new_val; \
                 end; \
                 return ARGV[1]",
                1,
            );
            assert!(result.is_ok());
        }
    }
}
