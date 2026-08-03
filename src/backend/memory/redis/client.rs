// Copyright (c) 2025-2026 Kirky.X
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
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
use redis::Client;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// Redis cache backend.
///
/// This backend provides a distributed cache using Redis.
/// It supports standalone, sentinel, and cluster modes.
/// Uses ConnectionManager for efficient connection pooling.
#[derive(Clone)]
pub struct RedisBackend {
    client: Arc<Client>,
    mode: RedisMode,
    connection_manager: redis::aio::ConnectionManager,
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
        connection_manager: redis::aio::ConnectionManager,
        dangerous_clear_enabled: bool,
        retry_count: u32,
        retry_delay: Duration,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Self {
        Self {
            client,
            mode,
            connection_manager,
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
        Self::builder().connection_string(connection_string).build().await
    }

    /// Create a new Redis backend with connection pool.
    ///
    /// Note: `pool_size` is currently not used as the underlying `redis` crate's
    /// `ConnectionManager` manages its own connection pool internally.
    /// This parameter is reserved for future use.
    pub async fn with_pool(connection_string: &str, _pool_size: usize) -> OxCacheResult<Self> {
        Self::builder().connection_string(connection_string).build().await
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

            // Check for userinfo (username[:password]@host)
            if let Some(at_pos) = rest.find('@') {
                // Check if there's a slash before @ (which would mean @ is not part of userinfo)
                let before_at = &rest[..at_pos];
                if !before_at.contains('/') {
                    // Found userinfo section - redact it
                    return format!("{}[REDACTED]@{}", protocol, &rest[at_pos + 1..]);
                }
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
    /// ConnectionManager uses Arc internally, so clone is cheap.
    pub(crate) fn conn(&self) -> redis::aio::ConnectionManager {
        self.connection_manager.clone()
    }

    /// Get the configured retry count.
    pub(crate) fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Get the configured retry base delay.
    pub(crate) fn retry_delay(&self) -> Duration {
        self.retry_delay
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

        let result =
            retry_with_backoff(operation, self.retry_count, self.retry_delay).await;

        match &result {
            Ok(_) => self.circuit_breaker.record_success(),
            Err(_) => {
                if self.circuit_breaker.record_failure() {
                    // Circuit breaker just transitioned to Open
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
