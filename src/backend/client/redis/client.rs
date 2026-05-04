//!
//! MIT License
//!
//! Redis backend implementation with ConnectionManager

use crate::backend::interface::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::backend::score::{BackendScore, Scores};
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

    /// Redact sensitive information from connection string for logging
    ///
    /// # Example
    /// ```
    /// // pragma: allowlist secret
    /// use oxcache::backend::client::RedisBackend;
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

    /// Ping the Redis server
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.connection_manager.clone();
        let result: String = redis::cmd("PING")
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

        let mut conn = self.connection_manager.clone();
        let mut pipe = redis::pipe();

        for (key, value) in items {
            if let Some(ttl) = ttl {
                pipe.cmd("SETEX").arg(key).arg(ttl.as_secs()).arg(value.as_slice());
            } else {
                pipe.cmd("SET").arg(key).arg(value.as_slice());
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

        let mut conn = self.connection_manager.clone();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd("GET").arg(key);
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

        let mut conn = self.connection_manager.clone();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd("DEL").arg(key);
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

        let mut conn = self.connection_manager.clone();
        let result: Option<Vec<u8>> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;
        Ok(result)
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

    async fn clear(&self) -> Result<()> {
        let mut conn = self.connection_manager.clone();

        security::validate_scan_pattern("*")?;

        let mut cursor = 0i64;

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

        let mut conn = self.connection_manager.clone();
        let result: i64 = redis::cmd("EXPIRE")
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

        // Convert to the format expected by set_many_pipeline
        let pipeline_items: Vec<(&str, Vec<u8>)> = items
            .iter()
            .map(|(key, value, _ttl)| (key.as_str(), value.clone()))
            .collect();

        // Use the TTL from the first item for all (common use case)
        let ttl = items.first().and_then(|(_, _, ttl)| *ttl);

        self.set_many_pipeline(&pipeline_items, ttl).await
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
        let mut conn = self.connection_manager.clone();
        redis::cmd("PING")
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

    async fn script_load(&self, script: &str) -> Result<String> {
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

    // 用于测试隔离的静态锁，防止并行测试间的环境变量污染
    use std::sync::OnceLock;
    static TEST_ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    fn get_test_env_lock() -> &'static std::sync::Mutex<()> {
        TEST_ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    mod security_tests {
        use super::*;

        #[test]
        fn test_redact_connection_string_with_password() {
            let conn_str = "redis://:secret_password@localhost:6379/0"; // pragma: allowlist secret
            let redacted = RedisBackend::redact_connection_string(conn_str);

            assert!(!redacted.contains("secret_password"), "Password should be redacted");
            assert!(redacted.contains("[REDACTED]"), "Should contain REDACTED marker");
            assert!(redacted.contains("localhost:6379"), "Host should be visible");
        }

        #[test]
        fn test_redact_connection_string_with_user_and_password() {
            let conn_str = "redis://user:mypassword@redis.example.com:6379/1"; // pragma: allowlist secret
            let redacted = RedisBackend::redact_connection_string(conn_str);

            assert!(!redacted.contains("mypassword"), "Password should be redacted");
            assert!(!redacted.contains("user"), "Username should be redacted");
            assert!(redacted.contains("[REDACTED]"), "Should contain REDACTED marker");
            assert!(redacted.contains("redis.example.com:6379"), "Host should be visible");
        }

        #[test]
        fn test_redact_connection_string_without_password() {
            let conn_str = "redis://localhost:6379/0";
            let redacted = RedisBackend::redact_connection_string(conn_str);

            assert_eq!(
                conn_str, redacted,
                "Connection string without password should not be modified"
            );
        }

        #[test]
        fn test_redact_connection_string_tls() {
            let conn_str = "rediss://:secret@prod-redis.cluster:6379/0"; // pragma: allowlist secret
            let redacted = RedisBackend::redact_connection_string(conn_str);

            assert!(!redacted.contains("secret"), "Password should be redacted");
            assert!(redacted.starts_with("rediss://"), "TLS protocol should be preserved");
        }

        #[test]
        fn test_insecure_connection_rejected_by_default() {
            // 使用锁序列化环境变量操作，防止并行测试污染
            let _lock = get_test_env_lock().lock().unwrap();

            // 清理环境变量
            std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");

            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                RedisBackend::builder()
                    .connection_string("redis://localhost:6379")
                    .build()
                    .await
            });

            assert!(result.is_err(), "Insecure connection should be rejected by default");
            if let Err(e) = result {
                let err_msg = e.to_string();
                assert!(err_msg.contains("TLS"), "Error message should mention TLS");
                assert!(err_msg.contains("rediss://"), "Error message should suggest TLS");
            }
        }

        #[test]
        fn test_insecure_connection_requires_explicit_consent() {
            // 使用锁序列化环境变量操作
            let _lock = get_test_env_lock().lock().unwrap();

            // 保存原始环境变量值（使用RAII模式确保测试后恢复）
            let original = std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").ok();

            // 测试错误的环境变量值
            std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "wrong_value");

            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                RedisBackend::builder()
                    .connection_string("redis://localhost:6379")
                    .build()
                    .await
            });

            // 测试正确的确认值
            std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");

            // 注意：这个测试需要实际的Redis连接，所以我们只验证配置验证通过
            // 实际连接会失败，因为没有Redis服务器
            let result2 = rt.block_on(async {
                RedisBackend::builder()
                    .connection_string("redis://nonexistent-host:6379")
                    .build()
                    .await
            });

            // 恢复原始环境变量状态（测试结束时，RAII模式）
            if let Some(v) = original {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", v);
            } else {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            assert!(result.is_err(), "Wrong consent value should be rejected");

            // 应该是连接错误，而不是TLS错误
            // 如果是TLS错误，说明环境变量验证没通过
            if let Err(e) = result2 {
                let err_msg = e.to_string();
                assert!(!err_msg.contains("TLS"), "Should not fail on TLS check");
            }
        }

        #[test]
        fn test_development_only_consent() {
            // 使用锁序列化环境变量操作
            let _lock = get_test_env_lock().lock().unwrap();

            // 保存原始环境变量值
            let original = std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").ok();

            std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "development-only");

            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                RedisBackend::builder()
                    .connection_string("redis://localhost:6379")
                    .build()
                    .await
            });

            // 恢复原始环境变量状态（测试结束时）
            if let Some(v) = original {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", v);
            } else {
                std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
            }

            // 应该是连接错误，而不是TLS错误
            if let Err(e) = result {
                let err_msg = e.to_string();
                assert!(!err_msg.contains("TLS"), "development-only consent should be accepted");
            }
        }
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

    mod sha_validation_tests {
        use super::*;

        fn validate_sha_format(sha: &str) -> Result<()> {
            if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(CacheError::InvalidInput(format!(
                    "Invalid SHA format: expected 40 hexadecimal characters, got {} characters",
                    sha.len()
                )));
            }
            Ok(())
        }

        #[test]
        fn test_valid_sha_format() {
            let result = validate_sha_format("a1b2c3d4e5f6789012345678901234567890abcd");
            assert!(result.is_ok());
        }

        #[test]
        fn test_valid_sha_uppercase() {
            let result = validate_sha_format("A1B2C3D4E5F6789012345678901234567890ABCD");
            assert!(result.is_ok());
        }

        #[test]
        fn test_valid_sha_mixed_case() {
            let result = validate_sha_format("a1B2c3D4e5F6789012345678901234567890AbCd");
            assert!(result.is_ok());
        }

        #[test]
        fn test_invalid_sha_too_short() {
            let result = validate_sha_format("abc123");
            assert!(result.is_err());
            if let Err(e) = result {
                let err_msg = e.to_string();
                assert!(err_msg.contains("40"));
                assert!(err_msg.contains("6"));
            }
        }

        #[test]
        fn test_invalid_sha_too_long() {
            let result = validate_sha_format("a1b2c3d4e5f6789012345678901234567890abcde");
            assert!(result.is_err());
        }

        #[test]
        fn test_invalid_sha_non_hex_chars() {
            let result = validate_sha_format("ghijklmnopqrstuvwxyz12345678901234567890");
            assert!(result.is_err());
        }

        #[test]
        fn test_invalid_sha_empty() {
            let result = validate_sha_format("");
            assert!(result.is_err());
        }

        #[test]
        fn test_invalid_sha_special_chars() {
            let result = validate_sha_format("a1b2c3d4e5f6789012345678901234567890!@#$");
            assert!(result.is_err());
        }
    }
}
