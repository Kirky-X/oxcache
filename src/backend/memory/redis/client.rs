//!
//! MIT License
//!
//! Redis backend implementation with ConnectionManager

use crate::backend::interface::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::backend::score::{BackendScore, Scores};
use crate::core::command::RedisCommand;
use crate::core::types::RedisModeType;
use crate::error::{CacheError, Result};
use crate::security;
use async_trait::async_trait;
use redis::{Client, RedisError};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 类型别名，保持 API 兼容性
pub type RedisMode = RedisModeType;

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

    /// Redact sensitive information from connection string for logging
    ///
    /// # Example
    /// ```
    /// // pragma: allowlist secret
    /// use oxcache::backend::memory::RedisBackend;
    /// let conn_str = "redis://:secret_password@localhost:6379/0";
    /// let redacted = RedisBackend::redact_connection_string(conn_str);
    /// assert!(!redacted.contains("secret_password"));
    /// ```
    pub fn redact_connection_string(conn_str: &str) -> String {
        // 移除密码等敏感信息
        if let Some(start) = conn_str.find("://") {
            let protocol = &conn_str[..start + 3];
            let rest = &conn_str[start + 3..];

            if rest.contains('@') {
                // Support format: redis-with-password at host port db
                // pragma: allowlist secret
                if let Some(at_pos) = rest.find('@') {
                    return format!("{}[REDACTED]@{}", protocol, &rest[at_pos + 1..]);
                }
            }
        }
        conn_str.to_string()
    }

    /// Get the Redis mode
    pub fn mode(&self) -> RedisMode {
        self.mode
    }

    /// Get the Redis client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a cloned connection handle
    ///
    /// ConnectionManager uses Arc internally, so clone is cheap.
    fn conn(&self) -> redis::aio::ConnectionManager {
        self.connection_manager.clone()
    }

    /// Ping the Redis server
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.conn();
        let result: String = redis::cmd(RedisCommand::Ping.as_str())
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(result)
    }

    /// Batch set multiple key-value pairs using Redis Pipeline
    ///
    /// This is significantly faster than individual SET commands when setting many keys,
    /// as it reduces network round trips from N to 1.
    ///
    /// # Arguments
    ///
    /// * `items` - Slice of (key, value) tuples
    /// * `ttl` - Optional TTL for all keys
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success, or an error if the operation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let items = vec![
    ///     ("key1", b"value1".to_vec()),
    ///     ("key2", b"value2".to_vec()),
    /// ];
    /// backend.set_many_pipeline(&items, Some(Duration::from_secs(60))).await?;
    /// ```
    pub async fn set_many_pipeline(&self, items: &[(&str, Vec<u8>)], ttl: Option<Duration>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Validate all keys first
        for (key, _) in items {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for (key, value) in items {
            if let Some(ttl) = ttl {
                pipe.cmd(RedisCommand::SetEx.as_str())
                    .arg(key)
                    .arg(ttl.as_secs())
                    .arg(value.as_slice());
            } else {
                pipe.cmd(RedisCommand::Set.as_str()).arg(key).arg(value.as_slice());
            }
        }

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    /// Batch get multiple keys using Redis Pipeline
    ///
    /// This is significantly faster than individual GET commands when fetching many keys,
    /// as it reduces network round trips from N to 1.
    ///
    /// # Arguments
    ///
    /// * `keys` - Slice of keys to fetch
    ///
    /// # Returns
    ///
    /// Returns a Vec of Option<Vec<u8>> where each element corresponds to the key at the same index.
    /// None indicates the key does not exist.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let keys = vec!["key1", "key2", "key3"];
    /// let values = backend.get_many_pipeline(&keys).await?;
    /// assert_eq!(values.len(), 3);
    /// ```
    pub async fn get_many_pipeline(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        // Validate all keys first
        for key in keys {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd(RedisCommand::Get.as_str()).arg(key);
        }

        let results: Vec<Option<Vec<u8>>> = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(results)
    }

    /// Batch delete multiple keys using Redis Pipeline
    ///
    /// This is significantly faster than individual DEL commands when deleting many keys,
    /// as it reduces network round trips from N to 1.
    ///
    /// # Arguments
    ///
    /// * `keys` - Slice of keys to delete
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success, or an error if the operation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let keys = vec!["key1", "key2", "key3"];
    /// backend.delete_many_pipeline(&keys).await?;
    /// ```
    pub async fn delete_many_pipeline(&self, keys: &[&str]) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        // Validate all keys first
        for key in keys {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd(RedisCommand::Del.as_str()).arg(key);
        }

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
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
        let connection_string = self
            .connection_string
            .ok_or_else(|| CacheError::InvalidInput("Connection string is required".to_string()))?;

        // 安全检查：强制使用TLS连接
        if !connection_string.starts_with("rediss://") {
            let allow_insecure = std::env::var("OXCACHE_ALLOW_INSECURE_REDIS")
                .map(|v| {
                    // 要求明确确认风险
                    v == "I_UNDERSTAND_THE_RISKS" || v == "development-only"
                })
                .unwrap_or(false);

            if !allow_insecure {
                return Err(CacheError::InvalidInput(
                    "Redis connection must use TLS (rediss://) in production. \
                     To allow insecure connections for development only, \
                     set OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS"
                        .to_string(),
                ));
            }
            // 安全警告：使用非 TLS 连接，允许在开发环境中使用
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
impl CacheReader for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        security::validate_redis_key(key)?;

        let mut conn = self.conn();
        let result: Option<Vec<u8>> = redis::cmd(RedisCommand::Get.as_str())
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(result)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        security::validate_redis_key(key)?;

        let mut conn = self.conn();
        let n: i64 = redis::cmd(RedisCommand::Exists.as_str())
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(n > 0)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        security::validate_redis_key(key)?;

        let mut conn = self.conn();
        let n: i64 = redis::cmd(RedisCommand::Ttl.as_str())
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

    async fn len(&self) -> Result<u64> {
        let mut conn = self.conn();
        let len: i64 = redis::cmd(RedisCommand::Dbsize.as_str())
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

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut conn = self.conn();
        let info: String = redis::cmd(RedisCommand::Info.as_str())
            .arg("memory")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        let mut stats = HashMap::new();
        stats.insert("memory_info".to_string(), info);
        Ok(stats)
    }

    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        // Convert to the format expected by get_many_pipeline
        let keys_slice: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();

        self.get_many_pipeline(&keys_slice).await
    }
}

#[async_trait]
impl CacheWriter for RedisBackend {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        security::validate_redis_key(key)?;

        let mut conn = self.conn();

        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            redis::cmd(RedisCommand::SetEx.as_str())
                .arg(key)
                .arg(ttl_secs)
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| CacheError::Connection(e.to_string()))?;
        } else {
            redis::cmd(RedisCommand::Set.as_str())
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

        let mut conn = self.conn();
        redis::cmd(RedisCommand::Del.as_str())
            .arg(key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.conn();

        security::validate_scan_pattern("*")?;

        let mut cursor = 0i64;

        loop {
            let (new_cursor, keys): (i64, Vec<String>) = redis::cmd(RedisCommand::Scan.as_str())
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
                redis::cmd(RedisCommand::Del.as_str())
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
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        security::validate_redis_key(key)?;

        let mut conn = self.conn();
        let result: i64 = redis::cmd(RedisCommand::Expire.as_str())
            .arg(key)
            .arg(ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(result > 0)
    }

    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Validate all keys first
        for (key, _, _) in items {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for (key, value, ttl) in items {
            if let Some(ttl) = ttl {
                pipe.cmd(RedisCommand::SetEx.as_str())
                    .arg(key.as_str())
                    .arg(ttl.as_secs())
                    .arg(value.as_slice());
            } else {
                pipe.cmd(RedisCommand::Set.as_str())
                    .arg(key.as_str())
                    .arg(value.as_slice());
            }
        }

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        // Convert to the format expected by delete_many_pipeline
        let keys_slice: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();

        self.delete_many_pipeline(&keys_slice).await
    }
}

#[async_trait]
impl CacheConnector for RedisBackend {
    async fn health_check(&self) -> Result<()> {
        let mut conn = self.conn();
        redis::cmd(RedisCommand::Ping.as_str())
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(())
    }

    async fn shutdown(&self) {
        // Redis connection is managed by the connection pool; no explicit close needed
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Redis
    }

    #[cfg(feature = "lua-script")]
    fn as_lua_executor(&self) -> Option<&dyn crate::backend::interface::LuaExecutor> {
        Some(self)
    }
}

// CacheBackend is automatically implemented via blanket implementation

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
}

fn is_connection_error(e: &RedisError) -> bool {
    e.is_timeout() || e.is_io_error()
}

#[cfg(feature = "lua-script")]
#[async_trait::async_trait]
impl crate::backend::interface::LuaExecutor for RedisBackend {
    async fn eval_lua(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value> {
        security::validate_lua_script(script, keys.len())?;

        let mut conn = self.conn();

        let mut cmd = redis::cmd(RedisCommand::Eval.as_str());
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

    /// Execute a Lua script by its SHA1 hash
    ///
    /// # Arguments
    ///
    /// * `sha` - The SHA1 hash of the script (must be exactly 40 hexadecimal characters)
    /// * `keys` - The keys that the script will access
    /// * `args` - Additional arguments to pass to the script
    ///
    /// # Errors
    ///
    /// Returns `CacheError::InvalidInput` if:
    /// - SHA is not exactly 40 hexadecimal characters
    /// - Any key fails validation
    async fn eval_sha(&self, sha: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value> {
        // SHA 格式验证：必须是40位十六进制字符
        if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid SHA format: expected 40 hexadecimal characters, got {} characters",
                sha.len()
            )));
        }

        // 验证所有 keys
        for key in keys {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();

        let mut cmd = redis::cmd(RedisCommand::EvalSha.as_str());
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

    async fn script_load(&self, script: &str) -> Result<String> {
        security::validate_lua_script(script, 0)?;

        let mut conn = self.conn();

        let sha: String = redis::cmd(RedisCommand::Script.as_str())
            .arg("LOAD")
            .arg(script)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(sha)
    }
}
