//! Testcontainers 测试工具
//!
//! 使用 testcontainers 0.23+ API 提供容器管理功能

#![allow(dead_code)]

use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};

/// Redis 容器包装器
pub struct RedisContainer {
    container: ContainerAsync<testcontainers_modules::redis::Redis>,
    port: u16,
}

impl RedisContainer {
    /// 启动一个新的 Redis 容器
    pub async fn start() -> Result<Self, String> {
        let redis = testcontainers_modules::redis::Redis::default()
            .with_tag("7-alpine")
            .start()
            .await
            .map_err(|e| format!("启动 Redis 容器失败: {}", e))?;

        let port = redis
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("获取端口失败: {}", e))?;

        Ok(Self { container: redis, port })
    }

    /// 获取 Redis 连接 URL
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 获取端口
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 等待 Redis 就绪
    pub async fn wait_ready(&self) -> Result<(), String> {
        let url = self.url();
        let client = redis::Client::open(url.as_str()).map_err(|e| format!("创建客户端失败: {}", e))?;

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(30);

        while start.elapsed() < timeout {
            match client.get_multiplexed_async_connection().await {
                Ok(_) => return Ok(()),
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }

        Err("等待 Redis 就绪超时".to_string())
    }
}

/// Redis Cluster 容器管理器
pub struct RedisClusterManager {
    nodes: Vec<ContainerAsync<testcontainers_modules::redis::Redis>>,
    ports: Vec<u16>,
}

impl RedisClusterManager {
    /// 启动 Redis Cluster (6 个节点)
    pub async fn start_cluster() -> Result<Self, String> {
        let mut nodes = Vec::new();
        let mut ports = Vec::new();

        for i in 0..6 {
            let redis = testcontainers_modules::redis::Redis::default()
                .with_tag("7-alpine")
                .start()
                .await
                .map_err(|e| format!("启动 Redis Cluster 节点 {} 失败: {}", i, e))?;

            let port = redis
                .get_host_port_ipv4(6379)
                .await
                .map_err(|e| format!("获取端口失败: {}", e))?;

            nodes.push(redis);
            ports.push(port);
        }

        Ok(Self { nodes, ports })
    }

    /// 获取所有节点的 URL
    pub fn urls(&self) -> Vec<String> {
        self.ports.iter().map(|p| format!("redis://127.0.0.1:{}", p)).collect()
    }

    /// 获取端口列表
    pub fn ports(&self) -> &[u16] {
        &self.ports
    }
}

/// 测试环境管理器
pub struct TestEnvironment {
    redis: Option<RedisContainer>,
}

impl TestEnvironment {
    /// 创建新的测试环境
    pub fn new() -> Self {
        Self { redis: None }
    }

    /// 启动 Redis 容器
    pub async fn start_redis(&mut self) -> Result<&RedisContainer, String> {
        if self.redis.is_none() {
            let container = RedisContainer::start().await?;
            container.wait_ready().await?;
            self.redis = Some(container);
        }
        Ok(self.redis.as_ref().unwrap())
    }

    /// 获取 Redis URL
    pub fn redis_url(&self) -> Option<String> {
        self.redis.as_ref().map(|r| r.url())
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// 便捷函数：启动 Redis 容器并返回 URL
pub async fn start_redis_container() -> Result<(RedisContainer, String), String> {
    let container = RedisContainer::start().await?;
    container.wait_ready().await?;
    let url = container.url();
    Ok((container, url))
}

/// 便捷函数：检查 Redis 是否可用
pub async fn is_redis_available(url: &str) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    match tokio::time::timeout(Duration::from_secs(2), client.get_multiplexed_async_connection()).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}
