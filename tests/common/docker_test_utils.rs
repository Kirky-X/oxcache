//! Docker test utilities using testcontainers 0.23+
//!
//! This module provides helper functions for setting up Docker-based test environments
//! using testcontainers for Redis.

#![allow(dead_code)]

use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;

/// Redis 容器类型别名
pub type RedisContainer = testcontainers::ContainerAsync<testcontainers_modules::redis::Redis>;

/// 创建 Redis 测试容器
///
/// # Example
/// ```ignore
/// #[tokio::test]
/// async fn test_with_redis() {
///     let (container, redis_url) = setup_redis_container().await.unwrap();
///     // 使用 redis_url 连接
/// }
/// ```
pub async fn setup_redis_container() -> Result<(RedisContainer, String), String> {
    let redis = testcontainers_modules::redis::Redis::default()
        .with_tag("7-alpine")
        .start()
        .await
        .map_err(|e| format!("启动 Redis 容器失败: {}", e))?;

    let port = redis
        .get_host_port_ipv4(6379)
        .await
        .map_err(|e| format!("获取端口失败: {}", e))?;

    let redis_url = format!("redis://127.0.0.1:{}", port);

    Ok((redis, redis_url))
}

/// 等待 Redis 就绪
pub async fn wait_for_redis(url: &str, timeout_secs: u64) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    while start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_secs(2), client.get_multiplexed_async_connection()).await {
            Ok(Ok(_)) => return true,
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    false
}

/// 检查 Redis 是否可用
pub async fn is_redis_available(url: &str) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    matches!(
        tokio::time::timeout(Duration::from_secs(2), client.get_multiplexed_async_connection()).await,
        Ok(Ok(_))
    )
}

/// 创建多个 Redis 容器用于集群测试
pub async fn setup_redis_cluster_nodes(count: usize) -> Result<Vec<(RedisContainer, String)>, String> {
    let mut containers = Vec::new();

    for i in 0..count {
        let (container, url) = setup_redis_container()
            .await
            .map_err(|e| format!("创建第 {} 个 Redis 容器失败: {}", i + 1, e))?;
        containers.push((container, url));
    }

    Ok(containers)
}
