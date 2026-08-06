// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Aerospike 集成测试
//!
//! 使用 Docker 启动 Aerospike 容器（端口 3001→3000），
//! 服务器配置了 `access-address 127.0.0.1`，客户端直连 `127.0.0.1:3001`。
//! 所有测试共享同一个容器（通过 `OnceCell`），避免端口冲突。

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::interface::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use oxcache::backend::{AerospikeBackend, AerospikeConfig};
use tokio::sync::OnceCell;

/// Aerospike 容器名
const CONTAINER_NAME: &str = "oxcache-as-integration";
/// 宿主机映射端口
const HOST_PORT: u16 = 3001;

/// 全局共享的 Aerospike 配置（所有测试复用同一个容器）
static SHARED_CONFIG: OnceCell<Option<AerospikeConfig>> = OnceCell::const_new();

/// 获取共享的 Aerospike 配置（首次调用时启动容器）
async fn shared_config() -> Option<&'static AerospikeConfig> {
    SHARED_CONFIG.get_or_init(start_container).await.as_ref()
}

/// 启动 Aerospike 容器并返回配置；Docker 不可用时返回 None
async fn start_container() -> Option<AerospikeConfig> {
    // 先清理可能存在的同名容器
    let _ = Command::new("docker").args(["rm", "-f", CONTAINER_NAME]).output();

    // 启动容器，端口映射 HOST_PORT:3000
    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            CONTAINER_NAME,
            "-p",
            &format!("{HOST_PORT}:3000"),
            "aerospike/aerospike-server:8.0",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        eprintln!("skip: docker run failed: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    // 等待 Aerospike 初始就绪
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(60);
    let mut ready = false;
    while start.elapsed() < timeout {
        let logs = Command::new("docker").args(["logs", CONTAINER_NAME]).output().ok()?;
        let stderr = String::from_utf8_lossy(&logs.stderr);
        if stderr.contains("migrations: complete") || stderr.contains("service ready") {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if !ready {
        eprintln!("skip: Aerospike container start timeout");
        return None;
    }

    // 注入 access-address/access-port 配置（用 sed 修改默认配置）
    let _ = Command::new("docker")
        .args([
            "exec", CONTAINER_NAME, "sh", "-c",
            &format!(
                "sed -i 's|# access-address <IPADDR>|access-address 127.0.0.1\\n\\t\\taccess-port {HOST_PORT}|' /etc/aerospike/aerospike.conf"
            ),
        ])
        .output();

    // 停止并重新启动容器（不用 restart，避免 entrypoint 重新处理模板）
    let _ = Command::new("docker").args(["stop", CONTAINER_NAME]).output();
    let _ = Command::new("docker").args(["start", CONTAINER_NAME]).output();

    // 等待再次就绪
    tokio::time::sleep(Duration::from_secs(5)).await;
    let start = std::time::Instant::now();
    let mut ready = false;
    while start.elapsed() < timeout {
        let logs = Command::new("docker")
            .args(["logs", "--since", "10s", CONTAINER_NAME])
            .output()
            .ok()?;
        let stderr = String::from_utf8_lossy(&logs.stderr);
        if stderr.contains("migrations: complete") || stderr.contains("service ready") {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if !ready {
        eprintln!("skip: Aerospike container restart timeout");
        return None;
    }

    Some(AerospikeConfig {
        seed_nodes: vec![format!("127.0.0.1:{HOST_PORT}")],
        namespace: "test".to_string(),
        set_name: "oxcache_test".to_string(),
        default_ttl: 0,
        ip_map: None,
    })
}

/// 创建 Aerospike 后端（带重试）；容器不可用时返回 None
async fn make_backend() -> Option<AerospikeBackend> {
    let config = shared_config().await?.clone();
    let mut last_err = None;
    for _ in 0..5 {
        match AerospikeBackend::new(config.clone()).await {
            Ok(backend) => return Some(backend),
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    eprintln!("skip: Aerospike connect failed: {}", last_err.unwrap());
    None
}

// ============================================================================
// Aerospike 基础集成测试
// ============================================================================

#[tokio::test]
async fn test_aerospike_backend_kind() {
    let Some(backend) = make_backend().await else { return };
    assert_eq!(backend.backend_kind(), BackendKind::Aerospike);
}

#[tokio::test]
async fn test_aerospike_set_get_delete() {
    let Some(backend) = make_backend().await else { return };

    // set
    backend
        .set(Arc::from("as:key1"), Arc::new(b"value1".to_vec()), None)
        .await
        .expect("set failed");

    // get
    let val = backend.get("as:key1").await.unwrap();
    assert_eq!(val, Some(b"value1".to_vec()));

    // exists
    assert!(backend.exists("as:key1").await.unwrap());

    // delete
    backend.delete("as:key1").await.expect("delete failed");

    // get after delete
    let val = backend.get("as:key1").await.unwrap();
    assert_eq!(val, None);

    // exists after delete
    assert!(!backend.exists("as:key1").await.unwrap());
}

#[tokio::test]
async fn test_aerospike_set_with_ttl() {
    let Some(backend) = make_backend().await else { return };

    // set with TTL
    backend
        .set(
            Arc::from("as:ttl_key"),
            Arc::new(b"ttl_value".to_vec()),
            Some(Duration::from_secs(120)),
        )
        .await
        .expect("set with TTL failed");

    // get
    let val = backend.get("as:ttl_key").await.unwrap();
    assert_eq!(val, Some(b"ttl_value".to_vec()));

    // ttl should be Some
    let ttl = backend.ttl("as:ttl_key").await.unwrap();
    assert!(ttl.is_some());
    let ttl = ttl.unwrap();
    assert!(ttl > Duration::from_secs(100));
}

#[tokio::test]
async fn test_aerospike_expire() {
    let Some(backend) = make_backend().await else { return };

    // set without TTL (Never expires)
    backend
        .set(Arc::from("as:exp_key"), Arc::new(b"exp_value".to_vec()), None)
        .await
        .unwrap();

    // expire (set TTL)
    let result = backend.expire("as:exp_key", Duration::from_secs(60)).await.unwrap();
    assert!(result);

    // ttl should now be set
    let ttl = backend.ttl("as:exp_key").await.unwrap();
    assert!(ttl.is_some());

    // expire nonexistent key
    let result = backend.expire("as:nonexistent", Duration::from_secs(60)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_aerospike_set_many_delete_many() {
    let Some(backend) = make_backend().await else { return };

    // set_many
    let items = vec![
        (Arc::from("as:batch1"), Arc::new(b"b1".to_vec()), None),
        (Arc::from("as:batch2"), Arc::new(b"b2".to_vec()), None),
        (Arc::from("as:batch3"), Arc::new(b"b3".to_vec()), None),
    ];
    backend.set_many(&items).await.expect("set_many failed");

    // verify all exist
    assert!(backend.get("as:batch1").await.unwrap().is_some());
    assert!(backend.get("as:batch2").await.unwrap().is_some());
    assert!(backend.get("as:batch3").await.unwrap().is_some());

    // delete_many
    let keys = vec![
        "as:batch1".to_string(),
        "as:batch2".to_string(),
        "as:batch3".to_string(),
    ];
    backend.delete_many(&keys).await.expect("delete_many failed");

    // verify all deleted
    assert!(backend.get("as:batch1").await.unwrap().is_none());
    assert!(backend.get("as:batch2").await.unwrap().is_none());
    assert!(backend.get("as:batch3").await.unwrap().is_none());
}

#[tokio::test]
async fn test_aerospike_health_check_and_stats() {
    let Some(backend) = make_backend().await else { return };

    // health_check
    backend.health_check().await.expect("health_check failed");

    // stats
    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("backend_kind").unwrap(), "aerospike");
    assert_eq!(stats.get("namespace").unwrap(), "test");
    assert_eq!(stats.get("connected").unwrap(), "true");

    // unsupported operations
    assert!(backend.len().await.is_err());
    assert!(backend.capacity().await.is_err());
    assert!(backend.keys("*").await.is_err());
    assert!(backend.clear().await.is_err());

    // shutdown (should not panic)
    backend.shutdown().await;
}

// ============================================================================
// ChainCache 集成测试
// ============================================================================

#[tokio::test]
async fn test_aerospike_chain_cache_basic() {
    use oxcache::backend::MokaMemoryBackend;
    use oxcache::cache::chain::{ChainCacheBuilder, ChainLink};

    let Some(aerospike) = make_backend().await else { return };
    let moka = MokaMemoryBackend::new();

    // Moka(L1, score=100) + Aerospike(L2, score=30)
    let chain = ChainCacheBuilder::default()
        .link(ChainLink::new(moka, 100, false, "moka"))
        .link(ChainLink::new(aerospike, 30, true, "aerospike"))
        .build();

    // Write through chain
    chain
        .set("chain:as_key1", b"chain_value".to_vec(), None)
        .await
        .expect("chain set failed");

    // Read from chain
    let val = chain.get("chain:as_key1").await.unwrap();
    assert_eq!(val, Some(b"chain_value".to_vec()));

    // Health check
    chain.health_check().await.expect("chain health_check failed");
}
