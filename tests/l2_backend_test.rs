//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! L2后端测试

use oxcache::{
    backend::l2::L2Backend,
    config::{ClusterConfig, L2Config, RedisMode, SentinelConfig},
    error::CacheError,
};

mod common;

#[tokio::test]
async fn test_sentinel_mode_success() {
    common::setup_logging();

    // 等待 Sentinel 环境就绪
    // 注意：26379 是 sentinel 的端口，我们连接它来确认
    if !common::wait_for_redis("redis://127.0.0.1:26379").await {
        println!("Skipping test_sentinel_mode_success: Redis Sentinel not available");
        return;
    }

    // 尝试使用 redis-sentinel:// 协议格式，这是 redis-rs 0.22+ 官方支持的格式
    // 格式: redis-sentinel://[:password@]host:port/master_name[/db_index]
    // 注意：redis-rs 的 Sentinel URL 解析期望把 master_name 作为 path 的一部分

    let config = L2Config {
        mode: RedisMode::Sentinel,
        sentinel: Some(SentinelConfig {
            master_name: "mymaster".to_string(),
            nodes: vec!["127.0.0.1:26379".to_string()],
        }),
        default_ttl: None,
        connection_timeout_ms: 10000,
        ..Default::default()
    };

    let backend = L2Backend::new(&config).await;
    assert!(
        backend.is_ok(),
        "Backend creation failed: {:?}",
        backend.err()
    );
}

#[tokio::test]
async fn test_cluster_mode_success() {
    common::setup_logging();

    // 等待 Cluster 环境就绪
    if !common::wait_for_redis("redis://127.0.0.1:7000").await {
        println!("Skipping test_cluster_mode_success: Redis Cluster not available");
        return;
    }

    let config = L2Config {
        mode: RedisMode::Cluster,
        connection_string: "".to_string().into(),
        connection_timeout_ms: 5000,
        command_timeout_ms: 5000,
        sentinel: None,
        cluster: Some(ClusterConfig {
            nodes: vec!["redis://127.0.0.1:7000".to_string()],
        }),
        password: None,
        enable_tls: false,
        default_ttl: None,
        ..Default::default()
    };

    // 重试几次，因为集群状态可能还没完全收敛
    let mut backend = Err(CacheError::Configuration("Init".to_string()));
    for i in 0..5 {
        backend = L2Backend::new(&config).await;
        if backend.is_ok() {
            break;
        }
        println!(
            "Attempt {} failed: {:?}, retrying...",
            i + 1,
            backend.as_ref().err()
        );
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    if let Err(e) = &backend {
        println!("Cluster backend creation failed after retries: {:?}", e);
        panic!("Failed to connect to Redis Cluster: {:?}", e);
    }
}

#[tokio::test]
async fn test_sentinel_missing_config() {
    let config = L2Config {
        mode: RedisMode::Sentinel,
        connection_string: "".to_string().into(),
        connection_timeout_ms: 1000,
        command_timeout_ms: 1000,
        sentinel: None,
        cluster: None,
        password: None,
        enable_tls: false,
        default_ttl: None,
        ..Default::default()
    };

    // 这里我们直接调用 new，不再注入 provider
    // 预期会因为配置缺失直接返回错误，甚至不需要尝试连接
    match L2Backend::new(&config).await {
        Err(CacheError::Configuration(msg)) => {
            assert!(msg.contains("Sentinel configuration is missing"));
        }
        Err(e) => panic!("Expected Configuration error, got: {:?}", e),
        Ok(_) => panic!("Should return configuration error"),
    }
}

#[tokio::test]
async fn test_cluster_missing_config() {
    let config = L2Config {
        mode: RedisMode::Cluster,
        connection_string: "".to_string().into(),
        connection_timeout_ms: 1000,
        command_timeout_ms: 1000,
        sentinel: None,
        cluster: None,
        password: None,
        enable_tls: false,
        default_ttl: None,
        ..Default::default()
    };

    match L2Backend::new(&config).await {
        Err(CacheError::Configuration(msg)) => {
            assert!(msg.contains("Cluster configuration is missing"));
        }
        Err(e) => panic!("Expected Configuration error, got: {:?}", e),
        Ok(_) => panic!("Should return configuration error"),
    }
}
