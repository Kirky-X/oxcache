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
use crate::core::RedisCommand;
use crate::error::OxCacheResult;
use redis::Client;
use std::sync::Arc;

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
    ) -> Self {
        Self {
            client,
            mode,
            connection_manager,
            dangerous_clear_enabled,
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

            if rest.contains('@') {
                // pragma: allowlist secret
                if let Some(at_pos) = rest.find('@') {
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
