//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了Redis提供者接口和默认实现。

#[cfg(feature = "l2-redis")]
use crate::{
    config::legacy_config::L2Config,
    error::{CacheError, Result},
};
#[cfg(feature = "l2-redis")]
use async_trait::async_trait;
#[cfg(feature = "l2-redis")]
use redis::{aio::ConnectionManager, Client};
#[cfg(feature = "l2-redis")]
use secrecy::ExposeSecret;
#[cfg(feature = "l2-redis")]
use tokio::time::{timeout, Duration};

#[cfg(feature = "l2-redis")]
#[async_trait]
pub trait RedisProvider: Send + Sync {
    async fn get_standalone_client(&self, config: &L2Config)
        -> Result<(Client, ConnectionManager)>;
    async fn get_cluster_client(&self, config: &L2Config) -> Result<redis::cluster::ClusterClient>;
    async fn get_sentinel_client(
        &self,
        config: &L2Config,
    ) -> Result<(Client, ConnectionManager, Option<ConnectionManager>)>;
}

#[cfg(feature = "l2-redis")]
pub struct DefaultRedisProvider;

#[cfg(feature = "l2-redis")]
#[async_trait]
impl RedisProvider for DefaultRedisProvider {
    async fn get_standalone_client(
        &self,
        config: &L2Config,
    ) -> Result<(Client, ConnectionManager)> {
        let connection_string_secret = &config.connection_string;
        let connection_string = if config.enable_tls
            && !connection_string_secret
                .expose_secret()
                .starts_with("rediss://")
        {
            connection_string_secret
                .expose_secret()
                .replace("redis://", "rediss://")
        } else {
            connection_string_secret.expose_secret().to_string()
        };

        let client = Client::open(connection_string.as_str())?;
        let manager = match timeout(
            Duration::from_millis(config.connection_timeout_ms),
            client.get_connection_manager(),
        )
        .await
        {
            Ok(res) => res?,
            Err(_) => {
                // Try again with longer timeout?
                // Or maybe the sentinel gave us an internal IP that is not reachable?
                // In docker compose, containers share a network so 172.x.x.x should be reachable.
                // But connection_timeout_ms might be too short for initial handshake.

                // Let's print the error context if possible, but timeout doesn't give error.
                return Err(CacheError::L2Error(format!(
                    "Connection timed out after {}ms. Target: [REDACTED]",
                    config.connection_timeout_ms
                )));
            }
        };
        Ok((client, manager))
    }

    async fn get_cluster_client(&self, config: &L2Config) -> Result<redis::cluster::ClusterClient> {
        let cluster_config = config.cluster.as_ref().ok_or_else(|| {
            CacheError::Configuration("Cluster configuration is missing".to_string())
        })?;

        let mut builder = redis::cluster::ClusterClient::builder(cluster_config.nodes.clone());

        if let Some(password) = &config.password {
            let secret: &String = ExposeSecret::expose_secret(password);
            builder = builder.password(secret.to_string());
        }

        // Enable read from replicas for better read scalability
        builder = builder.read_from_replicas();

        let client = builder.build()?;

        timeout(
            Duration::from_millis(config.connection_timeout_ms),
            client.get_async_connection(),
        )
        .await
        .map_err(|_| {
            CacheError::L2Error(format!(
                "Connection timed out after {}ms",
                config.connection_timeout_ms
            ))
        })??;
        Ok(client)
    }

    async fn get_sentinel_client(
        &self,
        config: &L2Config,
    ) -> Result<(Client, ConnectionManager, Option<ConnectionManager>)> {
        let sentinel_config = config.sentinel.as_ref().ok_or_else(|| {
            CacheError::Configuration("Sentinel configuration is missing".to_string())
        })?;

        tracing::info!("Initializing Sentinel client with automatic failover support");

        // Add sentinel nodes
        let nodes: Vec<String> = sentinel_config
            .nodes
            .iter()
            .map(|n: &String| {
                // Strip scheme if present
                n.trim_start_matches("redis://")
                    .trim_start_matches("redis+sentinel://")
                    .trim_start_matches("http://")
                    .to_string()
            })
            .collect();

        if nodes.is_empty() {
            return Err(CacheError::Configuration(
                "No sentinel nodes provided".to_string(),
            ));
        }

        // 构建 Sentinel URL
        // redis-rs 0.27 需要使用 sentinel://host:port 格式
        // 然后使用 SentinelClient::builder()
        let first_node = &nodes[0];

        // 记录连接信息（不包含密码）
        tracing::info!(
            "Connecting to Sentinel: master={}, nodes={}, url={}",
            sentinel_config.master_name,
            nodes.len(),
            first_node
        );

        // 使用 redis://host:port 格式创建基础 client
        // Sentinel 逻辑通过 ConnectionManager 处理
        let redis_url = if first_node.contains("://") {
            first_node.clone()
        } else {
            format!("redis://{}", first_node)
        };

        let client = Client::open(redis_url.as_str())?;

        // Create connection manager which handles reconnection and failover automatically
        let manager = timeout(
            Duration::from_millis(config.connection_timeout_ms),
            client.get_connection_manager(),
        )
        .await
        .map_err(|_| {
            CacheError::L2Error(format!(
                "Sentinel connection timed out after {}ms",
                config.connection_timeout_ms
            ))
        })??;

        // 单独进行密码认证（如果配置了密码）
        // 这样避免了将密码包含在 URL 中，防止密码泄露到日志
        if let Some(password) = &config.password {
            let mut conn = manager.clone();
            let secret_val: &String = ExposeSecret::expose_secret(password);
            let _: String = redis::cmd("AUTH")
                .arg(secret_val)
                .query_async(&mut conn)
                .await
                .map_err(|e| CacheError::L2Error(format!("Redis authentication failed: {}", e)))?;
            tracing::info!("Redis authentication successful (sentinel mode)");
        }

        // For slave/replica connection, we can create a separate connection if needed.
        // Currently we return None as the primary requirement is master failover.

        Ok((client, manager, None))
    }
}
