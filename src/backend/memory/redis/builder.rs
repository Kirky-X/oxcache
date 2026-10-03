// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis backend builder with extended configuration.

use super::circuit_breaker::CircuitBreaker;
use super::client::RedisBackend;
use super::client::RedisConnection;
use super::error::map_redis_error;
use crate::config::DistributedConfig;
use crate::core::RedisModeType;
use crate::error::{OxCacheError, OxCacheResult};
use std::sync::Arc;
use std::time::Duration;

/// Type alias for Redis mode, maintaining API compatibility.
pub type RedisMode = RedisModeType;

/// Builder for `RedisBackend` with extended configuration.
///
/// # Example
///
/// ```rust,ignore
/// let backend = RedisBackend::builder()
///     .connection_string("rediss://localhost:6379")
///     .pool_size(16)
///     .connection_timeout(Duration::from_secs(5))
///     .build()
///     .await?;
/// ```
#[derive(Debug)]
pub struct RedisBackendBuilder {
    connection_string: Option<String>,
    mode: RedisMode,
    /// mode 是否被显式设置：默认模式下连接目标若探测为集群实例，
    /// 自动切换 ClusterConnection；显式 Standalone/Sentinel 则尊重用户意图。
    mode_explicit: bool,
    pool_size: usize,
    connection_timeout: Duration,
    retry_count: u32,
    retry_delay: Duration,
    circuit_breaker_threshold: u32,
    circuit_breaker_reset_timeout: Duration,
    database: Option<u16>,
    pub(crate) dangerous_clear_enabled: bool,
}

impl Default for RedisBackendBuilder {
    fn default() -> Self {
        Self {
            connection_string: None,
            mode: RedisMode::default(),
            mode_explicit: false,
            pool_size: 8,
            connection_timeout: Duration::from_secs(2),
            retry_count: 3,
            retry_delay: Duration::from_millis(100),
            circuit_breaker_threshold: 5,
            circuit_breaker_reset_timeout: Duration::from_secs(30),
            database: None,
            dangerous_clear_enabled: false,
        }
    }
}

impl RedisBackendBuilder {
    /// Set the connection string.
    pub fn connection_string(mut self, connection_string: &str) -> Self {
        self.connection_string = Some(connection_string.to_string());
        self
    }

    /// Set the Redis mode.
    pub fn mode(mut self, mode: RedisMode) -> Self {
        self.mode = mode;
        self.mode_explicit = true;
        self
    }

    /// Set the connection pool size (default: 8).
    pub fn pool_size(mut self, pool_size: usize) -> Self {
        self.pool_size = pool_size;
        self
    }

    /// Set the connection timeout (default: 2s).
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set the retry count for reconnection (default: 3).
    pub fn retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    /// Set the retry delay between reconnection attempts (default: 100ms).
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Set the Redis database index (appended as `/N` to connection string).
    pub fn database(mut self, db: u16) -> Self {
        self.database = Some(db);
        self
    }

    /// Set the circuit breaker failure threshold (default: 5).
    ///
    /// After this many consecutive recoverable failures, the circuit breaker
    /// transitions to Open and subsequent operations return `Degraded` immediately.
    pub fn circuit_breaker_threshold(mut self, threshold: u32) -> Self {
        self.circuit_breaker_threshold = threshold;
        self
    }

    /// Set the circuit breaker reset timeout (default: 30s).
    ///
    /// Time to wait in Open state before transitioning to HalfOpen.
    pub fn circuit_breaker_reset_timeout(mut self, timeout: Duration) -> Self {
        self.circuit_breaker_reset_timeout = timeout;
        self
    }

    /// Apply all distributed parameters from a `DistributedConfig`.
    ///
    /// This is a convenience method that sets retry_count, retry_delay,
    /// circuit_breaker_threshold, and circuit_breaker_reset_timeout at once.
    /// Individual setters called after this will override specific values.
    pub fn distributed_config(mut self, config: DistributedConfig) -> Self {
        self.retry_count = config.retry_count;
        self.retry_delay = config.retry_base_delay;
        self.circuit_breaker_threshold = config.circuit_breaker_threshold;
        self.circuit_breaker_reset_timeout = config.circuit_breaker_reset_timeout;
        self
    }

    /// Enable or disable dangerous full-database `clear()` (default: false).
    ///
    /// When `false` (default), `CacheWriter::clear()` returns `Err(NotSupported)`.
    /// Set to `true` only for dedicated Redis instances or isolated DBs.
    pub fn dangerous_clear_enabled(mut self, enabled: bool) -> Self {
        self.dangerous_clear_enabled = enabled;
        self
    }

    /// Build the Redis backend.
    pub async fn build(self) -> OxCacheResult<RedisBackend> {
        // Validate pool_size
        if self.pool_size == 0 {
            return Err(OxCacheError::InvalidInput(
                "Connection pool size must be at least 1".to_string(),
            ));
        }

        let mut connection_string = self.connection_string.ok_or_else(|| {
            OxCacheError::InvalidInput("Connection string is required".to_string())
        })?;

        // Append database index if specified
        if let Some(db) = self.database {
            // Remove trailing slash if present, then append /N
            connection_string = connection_string.trim_end_matches('/').to_string();
            connection_string.push('/');
            connection_string.push_str(&db.to_string());
        }

        // Security check: enforce TLS connection
        if !connection_string.starts_with("rediss://") {
            let allow_insecure = std::env::var("OXCACHE_ALLOW_INSECURE_REDIS")
                .map(|v| v == "I_UNDERSTAND_THE_RISKS" || v == "development-only")
                .unwrap_or(false);

            if !allow_insecure {
                return Err(OxCacheError::InvalidInput(
                    "Redis connection must use TLS (rediss://) in production. \
                     To allow insecure connections for development only, \
                     set OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS"
                        .to_string(),
                ));
            }
        }

        let client = redis::Client::open(connection_string.as_str()).map_err(map_redis_error)?;

        let (mode, connection) = if self.mode == RedisMode::Cluster {
            (
                self.mode,
                Self::connect_cluster(&connection_string, self.connection_timeout).await?,
            )
        } else {
            let connection_result =
                tokio::time::timeout(self.connection_timeout, client.get_connection_manager())
                    .await;

            let mut connection_manager = match connection_result {
                Ok(Ok(mgr)) => mgr,
                Ok(Err(e)) => {
                    return Err(OxCacheError::Connection(format!(
                        "Failed to connect to Redis: {}",
                        e
                    )));
                }
                Err(_) => {
                    return Err(OxCacheError::Connection(
                        "Connection timeout - Redis server unavailable".to_string(),
                    ));
                }
            };

            // 默认模式下探测目标是否为集群实例（显式 Standalone/Sentinel 不探测，
            // 尊重用户意图）。非集群实例对 CLUSTER INFO 直接报错（cluster support
            // disabled），成功应答（cluster_state:...）即集群实例。建群未完成
            // （cluster_state:fail）交由后续操作显性报错。
            if !self.mode_explicit {
                let probe: redis::RedisResult<String> = redis::cmd("CLUSTER")
                    .arg("INFO")
                    .query_async(&mut connection_manager)
                    .await;
                if probe
                    .map(|info| info.contains("cluster_state:"))
                    .unwrap_or(false)
                {
                    (
                        RedisMode::Cluster,
                        Self::connect_cluster(&connection_string, self.connection_timeout).await?,
                    )
                } else {
                    (self.mode, RedisConnection::Single(connection_manager))
                }
            } else {
                (self.mode, RedisConnection::Single(connection_manager))
            }
        };

        Ok(RedisBackend::from_parts(
            std::sync::Arc::new(client),
            mode,
            connection,
            self.dangerous_clear_enabled,
            self.retry_count,
            self.retry_delay,
            Arc::new(CircuitBreaker::new(
                self.circuit_breaker_threshold,
                self.circuit_breaker_reset_timeout,
            )),
        ))
    }

    /// Connect through a cluster client seeded with the given node.
    ///
    /// ClusterConnection 按 CLUSTER SLOTS 路由并自动跟随 MOVED/ASK 重定向，
    /// 初始节点只需一个，其余节点自动发现。
    async fn connect_cluster(
        connection_string: &str,
        connection_timeout: Duration,
    ) -> OxCacheResult<RedisConnection> {
        let cluster_client =
            redis::cluster::ClusterClient::new(vec![connection_string]).map_err(map_redis_error)?;

        let connect = cluster_client.get_async_connection();
        let conn = match tokio::time::timeout(connection_timeout, connect).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                return Err(OxCacheError::Connection(format!(
                    "Failed to connect to Redis Cluster: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(OxCacheError::Connection(
                    "Connection timeout - Redis Cluster unavailable".to_string(),
                ));
            }
        };

        Ok(RedisConnection::Cluster(conn))
    }
}
