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

    /// Validate a Redis key before operations
    fn validate_key(key: &str) -> Result<()> {
        security::validate_redis_key(key)
    }

    /// Map Redis connection error
    fn conn_err(e: RedisError) -> CacheError {
        CacheError::Connection(e.to_string())
    }

    /// Map Redis operation error
    fn op_err(e: RedisError) -> CacheError {
        CacheError::Operation(e.to_string())
    }

    /// Ping the Redis server
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.conn();
        let result: String = redis::cmd(RedisCommand::Ping.as_str())
            .query_async(&mut conn)
            .await
            .map_err(Self::conn_err)?;
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
            Self::validate_key(key)?;
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

        pipe.query_async::<()>(&mut conn).await.map_err(Self::op_err)?;

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
            Self::validate_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd(RedisCommand::Get.as_str()).arg(key);
        }

        let results: Vec<Option<Vec<u8>>> = pipe.query_async(&mut conn).await.map_err(Self::op_err)?;

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
            Self::validate_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd(RedisCommand::Del.as_str()).arg(key);
        }

        pipe.query_async::<()>(&mut conn).await.map_err(Self::op_err)?;

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

        let client = Client::open(connection_string).map_err(RedisBackend::conn_err)?;

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
        Self::validate_key(key)?;

        let mut conn = self.conn();
        let result: Option<Vec<u8>> = redis::cmd(RedisCommand::Get.as_str())
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(Self::conn_err)?;
        Ok(result)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Self::validate_key(key)?;

        let mut conn = self.conn();
        let n: i64 = redis::cmd(RedisCommand::Exists.as_str())
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(Self::conn_err)?;
        Ok(n > 0)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        Self::validate_key(key)?;

        let mut conn = self.conn();
        let n: i64 = redis::cmd(RedisCommand::Ttl.as_str())
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(Self::conn_err)?;

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
            .map_err(Self::conn_err)?;
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
            .map_err(Self::op_err)?;

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
        Self::validate_key(key)?;

        let mut conn = self.conn();

        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            redis::cmd(RedisCommand::SetEx.as_str())
                .arg(key)
                .arg(ttl_secs)
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(Self::conn_err)?;
        } else {
            redis::cmd(RedisCommand::Set.as_str())
                .arg(key)
                .arg(&value)
                .query_async::<()>(&mut conn)
                .await
                .map_err(Self::conn_err)?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        Self::validate_key(key)?;

        let mut conn = self.conn();
        redis::cmd(RedisCommand::Del.as_str())
            .arg(key)
            .query_async::<()>(&mut conn)
            .await
            .map_err(Self::conn_err)?;
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
                Self::validate_key(key)?;
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
        Self::validate_key(key)?;

        let mut conn = self.conn();
        let result: i64 = redis::cmd(RedisCommand::Expire.as_str())
            .arg(key)
            .arg(ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(Self::op_err)?;

        Ok(result > 0)
    }

    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Validate all keys first
        for (key, _, _) in items {
            Self::validate_key(key)?;
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

        pipe.query_async::<()>(&mut conn).await.map_err(Self::op_err)?;

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
            .map_err(Self::conn_err)?;
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

// ============================================================================
// Synchronous Trait Implementations (via block_in_place)
// ============================================================================
//
// Sync counterparts of the async `CacheReader`/`CacheWriter`/`CacheConnector`
// traits. Because Redis is inherently async, these bridge to the async impl
// using `tokio::task::block_in_place`, which requires a multi-thread runtime.
//
// Behavior:
// - Outside any Tokio runtime: return `Err(NotSupported(...))`.
// - On a current-thread runtime: return `Err(NotSupported(...))` (block_in_place
//   would panic).
// - On a multi-thread runtime: `block_in_place(|| handle.block_on(async {...}))`.

impl RedisBackend {
    /// 获取当前 Tokio runtime handle，要求是 multi-thread runtime。
    ///
    /// - 不在任何 Tokio runtime 中：返回 `Err(NotSupported)`。
    /// - current-thread runtime：返回 `Err(NotSupported)`，因为 `block_in_place`
    ///   在 current-thread runtime 上会 panic。
    fn multi_thread_handle() -> Result<tokio::runtime::Handle> {
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|e| CacheError::NotSupported(format!("sync API requires a Tokio runtime: {}", e)))?;
        if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
            return Err(CacheError::NotSupported(
                "sync API requires a multi-thread runtime; block_in_place is unavailable on current_thread runtime"
                    .to_string(),
            ));
        }
        Ok(handle)
    }
}

impl crate::backend::interface::SyncCacheReader for RedisBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::get(self, key)))
    }

    fn exists(&self, key: &str) -> Result<bool> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::exists(self, key)))
    }

    fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::ttl(self, key)))
    }

    fn len(&self) -> Result<u64> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::len(self)))
    }

    fn capacity(&self) -> Result<u64> {
        // Redis 后端 capacity 固定为 0，与 async 实现一致，无需 runtime 调用。
        Ok(0)
    }

    fn stats(&self) -> Result<HashMap<String, String>> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::stats(self)))
    }
}

impl crate::backend::interface::SyncCacheWriter for RedisBackend {
    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::set(self, key, value, ttl)))
    }

    fn delete(&self, key: &str) -> Result<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::delete(self, key)))
    }

    fn clear(&self) -> Result<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::clear(self)))
    }

    fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::expire(self, key, ttl)))
    }
}

impl crate::backend::interface::SyncCacheConnector for RedisBackend {
    fn health_check(&self) -> Result<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheConnector::health_check(self)))
    }

    fn shutdown(&self) {
        // Redis 连接由 ConnectionManager 管理，无需显式关闭。
        // 与 async CacheConnector::shutdown 一致（no-op）。
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Redis
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

        let result = cmd.query_async(&mut conn).await.map_err(Self::op_err)?;
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
            Self::validate_key(key)?;
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

        let result = cmd.query_async(&mut conn).await.map_err(Self::op_err)?;
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
            .map_err(Self::op_err)?;

        Ok(sha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::interface::{CacheConnector, CacheReader, CacheWriter};
    use crate::backend::score::BackendScore;
    use crate::core::types::RedisModeType;
    use serial_test::serial;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Redis 测试连接地址（Docker 中运行的 Redis）
    const REDIS_URL: &str = "redis://127.0.0.1:6379";
    /// 使用独立数据库用于 clear 测试，避免影响其他并行测试
    const REDIS_URL_DB1: &str = "redis://127.0.0.1:6379/1";
    /// 测试 key 前缀，确保测试间互不干扰
    const KEY_PREFIX: &str = "test_client:";

    /// 全局唯一 ID 生成器，保证每个测试 key 唯一
    static UID: AtomicU64 = AtomicU64::new(0);

    /// 生成唯一测试 key
    fn unique_key(suffix: &str) -> String {
        let id = UID.fetch_add(1, Ordering::SeqCst);
        format!("{}{}_{}", KEY_PREFIX, id, suffix)
    }

    /// 设置允许非 TLS 连接的环境变量并创建 RedisBackend
    async fn make_backend() -> RedisBackend {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        RedisBackend::new(REDIS_URL).await.expect("Failed to connect to Redis")
    }

    /// 创建连接到指定数据库的 RedisBackend
    async fn make_backend_with_url(url: &str) -> RedisBackend {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        RedisBackend::new(url).await.expect("Failed to connect to Redis")
    }

    /// 清理指定 key
    async fn cleanup(backend: &RedisBackend, key: &str) {
        let _ = backend.delete(key).await;
    }

    // =========================================================================
    // redact_connection_string 测试（不需要 Redis 连接）
    // =========================================================================

    #[test]
    fn test_redact_connection_string_with_password() {
        // pragma: allowlist secret
        let conn_str = "redis://:secret_password@localhost:6379/0";
        let redacted = RedisBackend::redact_connection_string(conn_str);
        assert!(!redacted.contains("secret_password"));
        assert!(redacted.contains("[REDACTED]"));
        assert!(redacted.contains("localhost:6379/0"));
    }

    #[test]
    fn test_redact_connection_string_without_password() {
        let conn_str = "redis://localhost:6379/0";
        let redacted = RedisBackend::redact_connection_string(conn_str);
        // 没有密码时，原样返回
        assert_eq!(redacted, conn_str);
    }

    #[test]
    fn test_redact_connection_string_no_protocol() {
        let conn_str = "localhost:6379";
        let redacted = RedisBackend::redact_connection_string(conn_str);
        assert_eq!(redacted, conn_str);
    }

    #[test]
    fn test_redact_connection_string_rediss_protocol() {
        // pragma: allowlist secret
        let conn_str = "rediss://:mypw@example.com:6380/2";
        let redacted = RedisBackend::redact_connection_string(conn_str);
        assert!(!redacted.contains("mypw"));
        assert!(redacted.starts_with("rediss://[REDACTED]@"));
        assert!(redacted.contains("example.com:6380/2"));
    }

    // =========================================================================
    // Builder 测试
    // =========================================================================

    #[tokio::test]
    async fn test_builder_missing_connection_string() {
        let result = RedisBackend::builder().build().await;
        assert!(result.is_err());
        if let Err(CacheError::InvalidInput(msg)) = result {
            assert!(msg.contains("Connection string is required"));
        } else {
            panic!("Expected InvalidInput error");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_builder_insecure_rejected_without_env() {
        // 临时移除环境变量，验证非 TLS 连接被拒绝
        std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
        let result = RedisBackend::builder()
            .connection_string("redis://127.0.0.1:6379")
            .build()
            .await;
        assert!(result.is_err());
        if let Err(CacheError::InvalidInput(msg)) = result {
            assert!(msg.contains("TLS") || msg.contains("insecure"));
        } else {
            panic!("Expected InvalidInput error");
        }
        // 恢复环境变量，避免影响后续测试
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    }

    #[tokio::test]
    #[serial]
    async fn test_builder_insecure_allowed_with_env() {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        let backend = RedisBackend::builder().connection_string(REDIS_URL).build().await;
        assert!(backend.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_builder_insecure_allowed_with_dev_value() {
        // "development-only" 也应被接受
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "development-only");
        let backend = RedisBackend::builder().connection_string(REDIS_URL).build().await;
        assert!(backend.is_ok());
        // 恢复环境变量
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
    }

    #[tokio::test]
    #[serial]
    async fn test_builder_with_mode() {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        let backend = RedisBackend::builder()
            .connection_string(REDIS_URL)
            .mode(RedisModeType::Standalone)
            .build()
            .await;
        assert!(backend.is_ok());
        assert_eq!(backend.unwrap().mode(), RedisModeType::Standalone);
    }

    #[tokio::test]
    async fn test_builder_default_mode_is_standalone() {
        let backend = make_backend().await;
        assert_eq!(backend.mode(), RedisModeType::Standalone);
    }

    // =========================================================================
    // new() / with_pool() 连接测试
    // =========================================================================

    #[tokio::test]
    async fn test_new_connects_to_redis() {
        let backend = make_backend().await;
        backend.health_check().await.expect("health check failed");
    }

    #[tokio::test]
    async fn test_with_pool_connects_to_redis() {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        let backend = RedisBackend::with_pool(REDIS_URL, 4).await;
        assert!(backend.is_ok());
    }

    #[tokio::test]
    async fn test_new_invalid_url_returns_error() {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        let result = RedisBackend::new("redis://127.0.0.1:1/0").await;
        assert!(result.is_err());
        if let Err(CacheError::Connection(msg)) = result {
            assert!(msg.contains("Redis") || msg.contains("timeout") || msg.contains("connect"));
        } else {
            panic!("Expected Connection error");
        }
    }

    #[tokio::test]
    async fn test_new_unreachable_host_times_out() {
        std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        // 使用不可路由的地址触发超时
        let result = RedisBackend::new("redis://10.255.255.1:6379/0").await;
        assert!(result.is_err());
        if let Err(CacheError::Connection(msg)) = result {
            assert!(msg.contains("timeout") || msg.contains("Redis"));
        } else {
            panic!("Expected Connection/timeout error");
        }
    }

    // =========================================================================
    // ping / health_check 测试
    // =========================================================================

    #[tokio::test]
    async fn test_ping_returns_pong() {
        let backend = make_backend().await;
        let result = backend.ping().await.expect("ping failed");
        assert_eq!(result, "PONG");
    }

    #[tokio::test]
    async fn test_health_check_ok() {
        let backend = make_backend().await;
        backend.health_check().await.expect("health check failed");
    }

    // =========================================================================
    // CacheReader: get / exists / ttl 测试
    // =========================================================================

    #[tokio::test]
    async fn test_get_nonexistent_returns_none() {
        let backend = make_backend().await;
        let key = unique_key("no_such_key");
        let result = backend.get(&key).await.expect("get failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_set_then_get() {
        let backend = make_backend().await;
        let key = unique_key("set_get");
        backend
            .set(&key, b"hello world".to_vec(), None)
            .await
            .expect("set failed");
        let value = backend.get(&key).await.expect("get failed");
        assert_eq!(value, Some(b"hello world".to_vec()));
        cleanup(&backend, &key).await;
    }

    #[tokio::test]
    async fn test_set_empty_value() {
        let backend = make_backend().await;
        let key = unique_key("empty_val");
        backend.set(&key, vec![], None).await.expect("set failed");
        let value = backend.get(&key).await.expect("get failed");
        assert_eq!(value, Some(vec![]));
        cleanup(&backend, &key).await;
    }

    #[tokio::test]
    async fn test_set_binary_value() {
        let backend = make_backend().await;
        let key = unique_key("binary");
        let data: Vec<u8> = (0..=255).collect();
        backend.set(&key, data.clone(), None).await.expect("set failed");
        let value = backend.get(&key).await.expect("get failed");
        assert_eq!(value, Some(data));
        cleanup(&backend, &key).await;
    }

    #[tokio::test]
    async fn test_exists_true_after_set() {
        let backend = make_backend().await;
        let key = unique_key("exists_yes");
        backend.set(&key, b"v".to_vec(), None).await.expect("set failed");
        assert!(backend.exists(&key).await.expect("exists failed"));
        cleanup(&backend, &key).await;
    }

    #[tokio::test]
    async fn test_exists_false_for_missing() {
        let backend = make_backend().await;
        let key = unique_key("exists_no");
        assert!(!backend.exists(&key).await.expect("exists failed"));
    }

    #[tokio::test]
    async fn test_ttl_returns_none_for_key_without_expiry() {
        let backend = make_backend().await;
        let key = unique_key("no_ttl");
        backend.set(&key, b"v".to_vec(), None).await.expect("set failed");
        let ttl = backend.ttl(&key).await.expect("ttl failed");
        assert_eq!(ttl, None);
        cleanup(&backend, &key).await;
    }

    #[tokio::test]
    async fn test_ttl_returns_none_for_missing_key() {
        let backend = make_backend().await;
        let key = unique_key("missing_ttl");
        let ttl = backend.ttl(&key).await.expect("ttl failed");
        assert_eq!(ttl, None);
    }

    #[tokio::test]
    async fn test_ttl_returns_some_after_set_with_ttl() {
        let backend = make_backend().await;
        let key = unique_key("with_ttl");
        backend
            .set(&key, b"v".to_vec(), Some(Duration::from_secs(100)))
            .await
            .expect("set failed");
        let ttl = backend.ttl(&key).await.expect("ttl failed");
        assert!(ttl.is_some());
        let secs = ttl.unwrap().as_secs();
        // 允许少量误差
        assert!(secs > 90 && secs <= 100, "ttl secs = {}", secs);
        cleanup(&backend, &key).await;
    }

    // =========================================================================
    // CacheWriter: set / delete / expire 测试
    // =========================================================================

    #[tokio::test]
    async fn test_delete_removes_key() {
        let backend = make_backend().await;
        let key = unique_key("del");
        backend.set(&key, b"v".to_vec(), None).await.expect("set failed");
        assert!(backend.exists(&key).await.unwrap());
        backend.delete(&key).await.expect("delete failed");
        assert!(!backend.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_is_ok() {
        let backend = make_backend().await;
        let key = unique_key("del_missing");
        // 删除不存在的 key 不应报错
        backend.delete(&key).await.expect("delete missing key should be ok");
    }

    #[tokio::test]
    async fn test_expire_sets_ttl_on_existing_key() {
        let backend = make_backend().await;
        let key = unique_key("expire_ok");
        backend.set(&key, b"v".to_vec(), None).await.expect("set failed");
        let ok = backend
            .expire(&key, Duration::from_secs(50))
            .await
            .expect("expire failed");
        assert!(ok, "expire should return true for existing key");
        let ttl = backend.ttl(&key).await.unwrap();
        assert!(ttl.is_some());
        let secs = ttl.unwrap().as_secs();
        assert!(secs > 40 && secs <= 50, "ttl secs = {}", secs);
        cleanup(&backend, &key).await;
    }

    #[tokio::test]
    async fn test_expire_returns_false_for_missing_key() {
        let backend = make_backend().await;
        let key = unique_key("expire_missing");
        let ok = backend
            .expire(&key, Duration::from_secs(50))
            .await
            .expect("expire call failed");
        assert!(!ok, "expire should return false for missing key");
    }

    #[tokio::test]
    async fn test_set_with_ttl_expires() {
        let backend = make_backend().await;
        let key = unique_key("short_ttl");
        backend
            .set(&key, b"v".to_vec(), Some(Duration::from_secs(1)))
            .await
            .expect("set failed");
        assert!(backend.exists(&key).await.unwrap());
        // 等待过期
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(!backend.exists(&key).await.unwrap());
    }

    // =========================================================================
    // 批量操作: set_many / get_many / delete_many
    // =========================================================================

    #[tokio::test]
    async fn test_set_many_and_get_many() {
        let backend = make_backend().await;
        let k1 = unique_key("m1");
        let k2 = unique_key("m2");
        let k3 = unique_key("m3");
        let items = vec![
            (k1.clone(), b"v1".to_vec(), None),
            (k2.clone(), b"v2".to_vec(), None),
            (k3.clone(), b"v3".to_vec(), None),
        ];
        backend.set_many(&items).await.expect("set_many failed");

        let keys = vec![k1.clone(), k2.clone(), k3.clone()];
        let values = backend.get_many(&keys).await.expect("get_many failed");
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], Some(b"v1".to_vec()));
        assert_eq!(values[1], Some(b"v2".to_vec()));
        assert_eq!(values[2], Some(b"v3".to_vec()));

        backend.delete_many(&keys).await.expect("delete_many failed");
        for k in &keys {
            assert!(!backend.exists(k).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_set_many_empty_is_ok() {
        let backend = make_backend().await;
        backend.set_many(&[]).await.expect("set_many empty should be ok");
    }

    #[tokio::test]
    async fn test_get_many_empty_returns_empty() {
        let backend = make_backend().await;
        let result = backend.get_many(&[]).await.expect("get_many empty failed");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_delete_many_empty_is_ok() {
        let backend = make_backend().await;
        backend.delete_many(&[]).await.expect("delete_many empty should be ok");
    }

    #[tokio::test]
    async fn test_get_many_with_missing_keys() {
        let backend = make_backend().await;
        let k1 = unique_key("gm_present");
        let k2 = unique_key("gm_absent");
        backend.set(&k1, b"v".to_vec(), None).await.unwrap();
        let keys = vec![k1.clone(), k2.clone()];
        let values = backend.get_many(&keys).await.expect("get_many failed");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], Some(b"v".to_vec()));
        assert_eq!(values[1], None);
        cleanup(&backend, &k1).await;
    }

    #[tokio::test]
    async fn test_set_many_with_ttl() {
        let backend = make_backend().await;
        let k1 = unique_key("mttl1");
        let k2 = unique_key("mttl2");
        let items = vec![
            (k1.clone(), b"v1".to_vec(), Some(Duration::from_secs(100))),
            (k2.clone(), b"v2".to_vec(), Some(Duration::from_secs(100))),
        ];
        backend.set_many(&items).await.expect("set_many failed");
        let ttl1 = backend.ttl(&k1).await.unwrap();
        let ttl2 = backend.ttl(&k2).await.unwrap();
        assert!(ttl1.is_some() && ttl1.unwrap().as_secs() > 90);
        assert!(ttl2.is_some() && ttl2.unwrap().as_secs() > 90);
        cleanup(&backend, &k1).await;
        cleanup(&backend, &k2).await;
    }

    // =========================================================================
    // Pipeline 批量操作: set_many_pipeline / get_many_pipeline / delete_many_pipeline
    // =========================================================================

    #[tokio::test]
    async fn test_set_many_pipeline_and_get_many_pipeline() {
        let backend = make_backend().await;
        let k1 = unique_key("p1");
        let k2 = unique_key("p2");
        let items: Vec<(&str, Vec<u8>)> = vec![(k1.as_str(), b"pv1".to_vec()), (k2.as_str(), b"pv2".to_vec())];
        backend
            .set_many_pipeline(&items, None)
            .await
            .expect("set_many_pipeline failed");

        let keys: Vec<&str> = vec![k1.as_str(), k2.as_str()];
        let values = backend
            .get_many_pipeline(&keys)
            .await
            .expect("get_many_pipeline failed");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], Some(b"pv1".to_vec()));
        assert_eq!(values[1], Some(b"pv2".to_vec()));

        backend
            .delete_many_pipeline(&keys)
            .await
            .expect("delete_many_pipeline failed");
        for k in &keys {
            assert!(!backend.exists(k).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_set_many_pipeline_empty_is_ok() {
        let backend = make_backend().await;
        backend
            .set_many_pipeline(&[], None)
            .await
            .expect("set_many_pipeline empty should be ok");
    }

    #[tokio::test]
    async fn test_get_many_pipeline_empty_returns_empty() {
        let backend = make_backend().await;
        let result = backend
            .get_many_pipeline(&[])
            .await
            .expect("get_many_pipeline empty failed");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_delete_many_pipeline_empty_is_ok() {
        let backend = make_backend().await;
        backend
            .delete_many_pipeline(&[])
            .await
            .expect("delete_many_pipeline empty should be ok");
    }

    #[tokio::test]
    async fn test_set_many_pipeline_with_ttl() {
        let backend = make_backend().await;
        let k1 = unique_key("pttl1");
        let items: Vec<(&str, Vec<u8>)> = vec![(k1.as_str(), b"v".to_vec())];
        backend
            .set_many_pipeline(&items, Some(Duration::from_secs(80)))
            .await
            .expect("set_many_pipeline failed");
        let ttl = backend.ttl(&k1).await.unwrap();
        assert!(ttl.is_some() && ttl.unwrap().as_secs() > 70);
        cleanup(&backend, &k1).await;
    }

    // =========================================================================
    // stats / len / is_empty / capacity 测试
    // =========================================================================

    #[tokio::test]
    async fn test_stats_returns_memory_info() {
        let backend = make_backend().await;
        let stats = backend.stats().await.expect("stats failed");
        let info = stats.get("memory_info").expect("memory_info key missing");
        assert!(!info.is_empty());
        // INFO memory 应包含内存相关字段
        assert!(info.contains("memory") || info.contains("used_memory"));
    }

    #[tokio::test]
    async fn test_len_returns_u64() {
        let backend = make_backend().await;
        let len = backend.len().await.expect("len failed");
        // 数据库不为空时 len > 0（其他测试会写入数据）
        // 这里只验证返回类型正确且不报错
        let _ = len;
    }

    #[tokio::test]
    async fn test_is_empty_returns_bool() {
        let backend = make_backend().await;
        let _ = backend.is_empty().await.expect("is_empty failed");
    }

    #[tokio::test]
    async fn test_capacity_returns_zero() {
        let backend = make_backend().await;
        let cap = backend.capacity().await.expect("capacity failed");
        // Redis 后端 capacity 固定为 0
        assert_eq!(cap, 0);
    }

    // =========================================================================
    // 访问器: mode / client / backend_kind / BackendScore 测试
    // =========================================================================

    #[tokio::test]
    async fn test_mode_accessor() {
        let backend = make_backend().await;
        assert_eq!(backend.mode(), RedisModeType::Standalone);
    }

    #[tokio::test]
    async fn test_client_accessor() {
        let backend = make_backend().await;
        let _client: &Client = backend.client();
    }

    #[tokio::test]
    async fn test_backend_kind_is_redis() {
        let backend = make_backend().await;
        assert_eq!(backend.backend_kind(), BackendKind::Redis);
        assert!(backend.backend_kind().is_distributed());
        assert!(!backend.backend_kind().is_memory());
    }

    #[tokio::test]
    async fn test_backend_score() {
        let backend = make_backend().await;
        assert_eq!(backend.score(), 50);
    }

    #[tokio::test]
    async fn test_is_persistent_true() {
        let backend = make_backend().await;
        assert!(backend.is_persistent());
    }

    #[tokio::test]
    async fn test_backend_name() {
        let backend = make_backend().await;
        assert_eq!(backend.backend_name(), "redis");
    }

    // =========================================================================
    // shutdown 测试
    // =========================================================================

    #[tokio::test]
    async fn test_shutdown_is_noop() {
        let backend = make_backend().await;
        // shutdown 是 no-op，不应 panic
        backend.shutdown().await;
        // shutdown 后仍可正常使用
        backend
            .health_check()
            .await
            .expect("health check after shutdown failed");
    }

    // =========================================================================
    // 错误用例：键校验
    // =========================================================================

    #[tokio::test]
    async fn test_get_empty_key_rejected() {
        let backend = make_backend().await;
        let result = backend.get("").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CacheError::InvalidInput(_) => {}
            other => panic!("Expected InvalidInput, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_set_empty_key_rejected() {
        let backend = make_backend().await;
        let result = backend.set("", b"v".to_vec(), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_empty_key_rejected() {
        let backend = make_backend().await;
        let result = backend.delete("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exists_empty_key_rejected() {
        let backend = make_backend().await;
        let result = backend.exists("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ttl_empty_key_rejected() {
        let backend = make_backend().await;
        let result = backend.ttl("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_expire_empty_key_rejected() {
        let backend = make_backend().await;
        let result = backend.expire("", Duration::from_secs(10)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_key_with_newline_rejected() {
        let backend = make_backend().await;
        let result = backend.get("key\nwith\nnewline").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_key_with_null_rejected() {
        let backend = make_backend().await;
        let result = backend.get("key\0null").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_key_with_command_injection_char_rejected() {
        let backend = make_backend().await;
        // 分号是命令注入字符
        let result = backend.get("key;rm -rf").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_key_with_pipe_rejected() {
        let backend = make_backend().await;
        let result = backend.get("key|pipe").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_key_with_path_traversal_rejected() {
        let backend = make_backend().await;
        let result = backend.get("../etc/passwd").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_many_with_invalid_key_rejected() {
        let backend = make_backend().await;
        let items = vec![
            ("valid_key".to_string(), b"v".to_vec(), None),
            ("bad;key".to_string(), b"v".to_vec(), None),
        ];
        let result = backend.set_many(&items).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_many_pipeline_with_invalid_key_rejected() {
        let backend = make_backend().await;
        let items: Vec<(&str, Vec<u8>)> = vec![("bad;key", b"v".to_vec())];
        let result = backend.set_many_pipeline(&items, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_many_pipeline_with_invalid_key_rejected() {
        let backend = make_backend().await;
        let keys: Vec<&str> = vec!["bad;key"];
        let result = backend.get_many_pipeline(&keys).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_many_pipeline_with_invalid_key_rejected() {
        let backend = make_backend().await;
        let keys: Vec<&str> = vec!["bad;key"];
        let result = backend.delete_many_pipeline(&keys).await;
        assert!(result.is_err());
    }

    // =========================================================================
    // clear 测试（使用独立数据库避免干扰其他并行测试）
    // =========================================================================

    #[tokio::test]
    async fn test_clear_removes_all_keys() {
        // 使用数据库 1 隔离 clear 测试
        let backend = make_backend_with_url(REDIS_URL_DB1).await;
        let key = unique_key("clear_target");
        backend.set(&key, b"v".to_vec(), None).await.expect("set failed");
        assert!(backend.exists(&key).await.unwrap());

        backend.clear().await.expect("clear failed");

        // clear 后 key 应不存在
        assert!(!backend.exists(&key).await.unwrap());
    }

    // =========================================================================
    // Lua 脚本测试（需要 lua-script feature）
    // =========================================================================

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_eval_lua_simple_return() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        // 返回常量字符串
        let script = "return 'hello'";
        let result = backend.eval_lua(script, &[], &[]).await.expect("eval_lua failed");
        match result {
            redis::Value::BulkString(s) => assert_eq!(s, b"hello"),
            redis::Value::SimpleString(s) => assert_eq!(s, "hello"),
            other => panic!("Expected string, got {:?}", other),
        }
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_eval_lua_returns_int() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        let script = "return 42";
        let result = backend.eval_lua(script, &[], &[]).await.expect("eval_lua failed");
        match result {
            redis::Value::Int(n) => assert_eq!(n, 42),
            other => panic!("Expected Int(42), got {:?}", other),
        }
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_eval_lua_with_keys_and_args() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        let key = unique_key("lua_key");
        backend.set(&key, b"100".to_vec(), None).await.unwrap();

        // 读取 key 的值并加 arg
        let script = "local v = redis.call('GET', KEYS[1]); return tonumber(v) + tonumber(ARGV[1])";
        let result = backend
            .eval_lua(script, &[&key], &["5"])
            .await
            .expect("eval_lua failed");
        match result {
            redis::Value::Int(n) => assert_eq!(n, 105),
            other => panic!("Expected Int(105), got {:?}", other),
        }
        cleanup(&backend, &key).await;
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_script_load_and_eval_sha() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        let script = "return 1 + 1";
        let sha = backend.script_load(script).await.expect("script_load failed");
        // SHA 应为 40 位十六进制
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));

        let result = backend.eval_sha(&sha, &[], &[]).await.expect("eval_sha failed");
        match result {
            redis::Value::Int(n) => assert_eq!(n, 2),
            other => panic!("Expected Int(2), got {:?}", other),
        }
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_eval_sha_invalid_format_rejected() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        // 太短
        let result = backend.eval_sha("abc123", &[], &[]).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CacheError::InvalidInput(msg) => assert!(msg.contains("SHA")),
            other => panic!("Expected InvalidInput, got {:?}", other),
        }

        // 长度对但含非十六进制字符
        let result = backend
            .eval_sha("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", &[], &[])
            .await;
        assert!(result.is_err());
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_eval_lua_forbidden_command_rejected() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        // FLUSHALL 是被禁止的命令
        let script = "redis.call('FLUSHALL')";
        let result = backend.eval_lua(script, &[], &[]).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CacheError::InvalidInput(_) => {}
            other => panic!("Expected InvalidInput, got {:?}", other),
        }
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_eval_lua_too_many_keys_rejected() {
        use crate::backend::interface::LuaExecutor;
        let backend = make_backend().await;
        let keys: Vec<&str> = (0..200).map(|_| "k").collect();
        let result = backend.eval_lua("return 1", &keys, &[]).await;
        assert!(result.is_err());
    }

    #[cfg(feature = "lua-script")]
    #[tokio::test]
    async fn test_as_lua_executor_returns_some() {
        use crate::backend::interface::CacheConnector;
        let backend = make_backend().await;
        let executor = backend.as_lua_executor();
        assert!(executor.is_some());
    }

    #[cfg(not(feature = "lua-script"))]
    #[tokio::test]
    async fn test_as_lua_executor_returns_none_without_feature() {
        use crate::backend::interface::CacheConnector;
        let backend = make_backend().await;
        assert!(backend.as_lua_executor().is_none());
    }

    // =========================================================================
    // CacheBackend blanket impl 测试
    // =========================================================================

    #[tokio::test]
    async fn test_redis_backend_implements_all_traits() {
        use crate::backend::interface::CacheBackend;
        let backend = make_backend().await;
        // 验证可作为 CacheBackend 使用（blanket impl）
        let _: &dyn CacheBackend = &backend;
        let _: &dyn CacheReader = &backend;
        let _: &dyn CacheWriter = &backend;
        let _: &dyn CacheConnector = &backend;
    }
}
