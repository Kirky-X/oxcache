// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis backend core: struct definition and essential methods.
//!
//! Trait implementations are split into dedicated modules:
//! - `async_traits` — CacheReader / CacheWriter / CacheConnector / BackendScore / AtomicCacheWriter
//! - `sync_traits` — SyncCacheReader / SyncCacheWriter / SyncCacheConnector / SyncAtomicCacheWriter
//! - `pipeline` — batch pipeline operations
//! - `lua_executor` — Lua script execution
//! - `namespace` — prefix-scoped key deletion
//! - `builder` — RedisBackendBuilder with extended configuration

use super::builder::{RedisBackendBuilder, RedisMode};
use super::circuit_breaker::CircuitBreaker;
use super::retry::retry_with_backoff;
use super::sentinel::SentinelConnection;
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "metrics")]
use crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
use redis::Client;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// Redis async connection abstraction.
///
/// 单机场景走 [`redis::aio::ConnectionManager`]；集群场景走
/// [`redis::cluster_async::ClusterConnection`]（按槽位路由并自动跟随
/// MOVED/ASK 重定向）；哨兵场景由 [`SentinelConnection`] 持有当前 master
/// 连接，可恢复错误后重新发现 master（failover 跟随）。三者都实现
/// `aio::ConnectionLike`，调用方以 `cmd.query_async(&mut conn)` 统一驱动，
/// 无需感知分支。
#[derive(Clone)]
pub(crate) enum RedisConnection {
    Single(redis::aio::ConnectionManager),
    Cluster(redis::cluster_async::ClusterConnection),
    Sentinel(SentinelConnection),
}

impl redis::aio::ConnectionLike for RedisConnection {
    fn req_packed_command<'a>(
        &'a mut self,
        cmd: &'a redis::Cmd,
    ) -> redis::RedisFuture<'a, redis::Value> {
        Box::pin(async move {
            match self {
                RedisConnection::Single(conn) => conn.req_packed_command(cmd).await,
                RedisConnection::Cluster(conn) => conn.req_packed_command(cmd).await,
                RedisConnection::Sentinel(sentinel) => sentinel.exec_cmd(cmd).await,
            }
        })
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        pipeline: &'a redis::Pipeline,
        offset: usize,
        count: usize,
    ) -> redis::RedisFuture<'a, Vec<redis::Value>> {
        Box::pin(async move {
            match self {
                RedisConnection::Single(conn) => {
                    conn.req_packed_commands(pipeline, offset, count).await
                }
                RedisConnection::Cluster(conn) => {
                    conn.req_packed_commands(pipeline, offset, count).await
                }
                RedisConnection::Sentinel(sentinel) => {
                    sentinel.exec_pipeline(pipeline, offset, count).await
                }
            }
        })
    }

    fn get_db(&self) -> i64 {
        match self {
            RedisConnection::Single(conn) => conn.get_db(),
            RedisConnection::Cluster(conn) => conn.get_db(),
            RedisConnection::Sentinel(sentinel) => sentinel.db(),
        }
    }
}

/// Redis cache backend.
///
/// This backend provides a distributed cache using Redis.
/// It supports standalone, sentinel, and cluster modes.
/// 单机用 ConnectionManager；哨兵模式发现 master 且 failover 后自动重新
/// 发现；集群自动切换 ClusterConnection（显式 `mode(Cluster)` 或默认模式
/// 下探测到 `cluster_enabled`）。
#[derive(Clone)]
pub struct RedisBackend {
    client: Arc<Client>,
    mode: RedisMode,
    connection: RedisConnection,
    dangerous_clear_enabled: bool,
    /// Maximum retry attempts for recoverable operations.
    retry_count: u32,
    /// Base delay between retries (exponential backoff).
    retry_delay: Duration,
    /// Circuit breaker for cascading failure protection.
    circuit_breaker: Arc<CircuitBreaker>,
}

impl RedisBackend {
    /// Construct a `RedisBackend` from its constituent parts.
    ///
    /// Called by `RedisBackendBuilder::build()`.
    pub(crate) fn from_parts(
        client: Arc<Client>,
        mode: RedisMode,
        connection: RedisConnection,
        dangerous_clear_enabled: bool,
        retry_count: u32,
        retry_delay: Duration,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Self {
        Self {
            client,
            mode,
            connection,
            dangerous_clear_enabled,
            retry_count,
            retry_delay,
            circuit_breaker,
        }
    }

    /// Whether dangerous full-database `clear()` is enabled.
    pub(crate) fn dangerous_clear_enabled(&self) -> bool {
        self.dangerous_clear_enabled
    }

    /// Create a new Redis backend with connection string.
    pub async fn new(connection_string: &str) -> OxCacheResult<Self> {
        Self::builder()
            .connection_string(connection_string)
            .build()
            .await
    }

    /// Create a new Redis backend with connection pool.
    ///
    /// # Parameters
    ///
    /// - `connection_string`: Redis connection URL.
    /// - `_pool_size`: Currently unused. The underlying `redis` crate’s
    ///   `ConnectionManager` manages its own connection pool internally.
    ///   This parameter is retained for API compatibility and will be
    ///   wired through when a custom pool backend is introduced.
    pub async fn with_pool(connection_string: &str, _pool_size: usize) -> OxCacheResult<Self> {
        Self::builder()
            .connection_string(connection_string)
            .build()
            .await
    }

    /// Create a new Redis backend builder.
    pub fn builder() -> RedisBackendBuilder {
        RedisBackendBuilder::default()
    }

    /// Redact sensitive information from connection string for logging.
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
        if let Some(start) = conn_str.find("://") {
            let protocol = &conn_str[..start + 3];
            let rest = &conn_str[start + 3..];

            // Check for userinfo (username[:password]@host). Use the last '@':
            // passwords may legally contain '@' (and '/'); splitting at the
            // first '@' would leave password fragments in the clear. A path
            // '@' without userinfo is not a valid Redis URL; over-redacting
            // it errs toward hiding rather than leaking.
            if let Some(at_pos) = rest.rfind('@') {
                // Found userinfo section - redact it
                return format!("{}[REDACTED]@{}", protocol, &rest[at_pos + 1..]);
            }
        }
        conn_str.to_string()
    }

    /// Get the Redis mode.
    pub fn mode(&self) -> RedisMode {
        self.mode
    }

    /// Get the Redis client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a cloned connection handle.
    ///
    /// ConnectionManager / ClusterConnection 内部均为 Arc 共享，clone 廉价。
    pub(crate) fn conn(&self) -> RedisConnection {
        self.connection.clone()
    }

    /// Get a reference to the circuit breaker.
    pub(crate) fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    /// Execute an operation with circuit breaker protection and retry.
    ///
    /// 1. Checks circuit breaker — if Open, returns `Degraded` immediately
    /// 2. Wraps the operation in `retry_with_backoff`
    /// 3. Records success/failure on the circuit breaker
    pub(crate) async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> OxCacheResult<T>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = OxCacheResult<T>> + Send,
    {
        if self.circuit_breaker().is_open() {
            return Err(OxCacheError::Degraded(
                "Redis circuit breaker is open".to_string(),
            ));
        }

        let result = retry_with_backoff(operation, self.retry_count, self.retry_delay).await;

        match &result {
            Ok(_) => self.circuit_breaker.record_success(),
            Err(_) => {
                if self.circuit_breaker.record_failure() {
                    // Circuit breaker just transitioned to Open
                    #[cfg(feature = "metrics")]
                    GLOBAL_UNIFIED_METRICS.record_l2_degraded();
                }
            }
        }

        result
    }

    /// Ping the Redis server.
    pub async fn ping(&self) -> OxCacheResult<String> {
        let mut conn = self.conn();
        let result: String = redis::cmd(RedisCommand::Ping.as_str())
            .query_async(&mut conn)
            .await
            .map_err(super::error::map_redis_error)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_connection_string_password_with_slash() {
        // 密码含 / 时不得绕过脱敏
        let redacted = RedisBackend::redact_connection_string("redis://user:p@ss/w0rd@host:6379"); /* pragma: allowlist secret */
        assert!(!redacted.contains("p@ss/w0rd"));
        assert!(!redacted.contains("w0rd"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_connection_string_no_userinfo_unchanged() {
        let redacted = RedisBackend::redact_connection_string("redis://localhost:6379/0");
        assert_eq!(redacted, "redis://localhost:6379/0");
    }
}
