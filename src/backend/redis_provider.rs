//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了Redis提供者接口和默认实现。

use crate::{
    config::L2Config,
    error::{CacheError, Result},
};
use async_trait::async_trait;
use redis::{aio::ConnectionManager, Client};
use secrecy::ExposeSecret;
use tokio::time::{timeout, Duration};

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

pub struct DefaultRedisProvider;

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
                    "Connection timed out after {}ms. Target: {}",
                    config.connection_timeout_ms, connection_string
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
            builder = builder.password(password.expose_secret().to_string());
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

        // Construct the Sentinel URL: redis+sentinel://[:password@]host:port[,host:port][/service_name]
        let mut url = "redis+sentinel://".to_string();

        // Add password if present (for Redis authentication)
        if let Some(password) = &config.password {
            url.push_str(&format!(":{}@", password.expose_secret()));
        }

        // Add sentinel nodes
        let nodes: Vec<String> = sentinel_config
            .nodes
            .iter()
            .map(|n| {
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

        url.push_str(&nodes.join(","));
        url.push('/');
        url.push_str(&sentinel_config.master_name);

        // Note: We removed the manual map_addr logic because using redis+sentinel://
        // is required for automatic failover support in ConnectionManager.
        // In test environments with NAT/Docker, ensure Sentinels report reachable IPs
        // or use host networking.

        let client = Client::open(url)?;

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

        // For slave/replica connection, we can create a separate connection if needed.
        // Currently we return None as the primary requirement is master failover.

        Ok((client, manager, None))
    }
}
