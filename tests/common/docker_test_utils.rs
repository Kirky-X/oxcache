// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Docker test utilities using testcontainers 0.28+
//!
//! This module provides helper functions for setting up Docker-based test environments
//! using testcontainers for Redis.

// 多个测试二进制共享本模块，助手函数按二进制各有取舍，统一放行死代码告警。
#![allow(dead_code)]

use std::time::Duration;
use testcontainers::GenericImage;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;

/// Redis 容器类型别名
pub type RedisContainer = testcontainers::ContainerAsync<GenericImage>;

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
    let redis = GenericImage::new("redis", "7-alpine")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
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

/// 在固定宿主端口启动 Redis 容器。
///
/// 与 [`setup_redis_container`] 的区别：宿主端口固定，容器 `stop`/`start`
/// 后地址不变（Docker 对临时绑定端口在重启时会重新分配），供断连恢复
/// 测试在同一地址上验证连接池自动重连。
pub async fn setup_redis_container_on_port(
    host_port: u16,
) -> Result<(RedisContainer, String), String> {
    use testcontainers::ImageExt;

    let redis = GenericImage::new("redis", "7-alpine")
        .with_exposed_port(6379.tcp())
        .with_mapped_port(host_port, 6379.tcp())
        .with_ready_conditions(vec![WaitFor::message_on_stdout(
            "Ready to accept connections",
        )])
        .start()
        .await
        .map_err(|e| format!("启动 Redis 容器失败: {}", e))?;

    Ok((redis, format!("redis://127.0.0.1:{}", host_port)))
}

/// 预分配一个空闲宿主端口（bind :0 取端口后立即释放，调用方随即使用）。
pub fn free_local_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
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
        match tokio::time::timeout(
            Duration::from_secs(2),
            client.get_multiplexed_async_connection(),
        )
        .await
        {
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
        tokio::time::timeout(
            Duration::from_secs(2),
            client.get_multiplexed_async_connection()
        )
        .await,
        Ok(Ok(_))
    )
}

/// 创建多个 Redis 容器用于集群测试
pub async fn setup_redis_cluster_nodes(
    count: usize,
) -> Result<Vec<(RedisContainer, String)>, String> {
    let mut containers = Vec::new();

    for i in 0..count {
        let (container, url) = setup_redis_container()
            .await
            .map_err(|e| format!("创建第 {} 个 Redis 容器失败: {}", i + 1, e))?;
        containers.push((container, url));
    }

    Ok(containers)
}
