// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Aerospike 集成测试
//!
//! 使用 Docker 启动 Aerospike 容器（固定端口 3000），
//! 通过 `ClientPolicy.ip_map` 做 IP 地址转换来解决 Docker NAT 问题。

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::interface::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use oxcache::backend::{AerospikeBackend, AerospikeConfig};

/// Aerospike Docker 容器管理器
struct AerospikeContainer {
    container_name: String,
    port: u16,
    container_ip: String,
}

impl AerospikeContainer {
    /// 启动 Aerospike 容器（固定端口 3000）
    async fn start() -> Result<Self, String> {
        let container_name = format!("oxcache-as-test-{}", std::process::id());

        // 先清理可能存在的同名容器
        let _ = Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output();

        // 启动容器，固定端口映射 3000:3000
        let output = Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                &container_name,
                "-p",
                "3000:3000",
                "aerospike/aerospike-server:8.0",
            ])
            .output()
            .map_err(|e| format!("启动 Docker 失败: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "docker run 失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // 等待 Aerospike 就绪（最多 30 秒）
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(30);
        let mut ready = false;
        while start.elapsed() < timeout {
            let logs = Command::new("docker")
                .args(["logs", &container_name])
                .output()
                .map_err(|e| format!("获取日志失败: {}", e))?;
            let stderr = String::from_utf8_lossy(&logs.stderr);
            if stderr.contains("migrations: complete") || stderr.contains("service ready") {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        if !ready {
            // 清理
            let _ = Command::new("docker")
                .args(["rm", "-f", &container_name])
                .output();
            return Err("Aerospike 容器启动超时".to_string());
        }

        // 获取容器内部 IP
        let output = Command::new("docker")
            .args([
                "inspect",
                &container_name,
                "--format",
                "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
            ])
            .output()
            .map_err(|e| format!("获取容器 IP 失败: {}", e))?;

        let container_ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_ip.is_empty() {
            let _ = Command::new("docker")
                .args(["rm", "-f", &container_name])
                .output();
            return Err("无法获取容器 IP".to_string());
        }

        Ok(Self {
            container_name,
            port: 3000,
            container_ip,
        })
    }

    /// 构建带 ip_map 的 AerospikeConfig
    fn config(&self) -> AerospikeConfig {
        let mut ip_map = HashMap::new();
        ip_map.insert(self.container_ip.clone(), "127.0.0.1".to_string());

        AerospikeConfig {
            seed_nodes: vec![format!("127.0.0.1:{}", self.port)],
            namespace: "test".to_string(),
            set_name: "oxcache_test".to_string(),
            default_ttl: 0,
            ip_map: Some(ip_map),
        }
    }
}

impl Drop for AerospikeContainer {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.container_name])
            .output();
    }
}

/// 创建 Aerospike 后端（带重试）
async fn make_backend(config: AerospikeConfig) -> AerospikeBackend {
    let mut last_err = None;
    for _ in 0..5 {
        match AerospikeBackend::new(config.clone()).await {
            Ok(backend) => return backend,
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    panic!(
        "Failed to connect to Aerospike after retries: {}",
        last_err.unwrap()
    )
}

// ============================================================================
// Aerospike 基础集成测试
// ============================================================================

#[tokio::test]
async fn test_aerospike_backend_kind() {
    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let backend = make_backend(container.config()).await;

    assert_eq!(backend.backend_kind(), BackendKind::Aerospike);
}

#[tokio::test]
async fn test_aerospike_set_get_delete() {
    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let backend = make_backend(container.config()).await;

    // set
    backend
        .set(
            Arc::from("as:key1"),
            Arc::new(b"value1".to_vec()),
            None,
        )
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
    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let backend = make_backend(container.config()).await;

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
    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let backend = make_backend(container.config()).await;

    // set without TTL (Never expires)
    backend
        .set(
            Arc::from("as:exp_key"),
            Arc::new(b"exp_value".to_vec()),
            None,
        )
        .await
        .unwrap();

    // expire (set TTL)
    let result = backend
        .expire("as:exp_key", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(result);

    // ttl should now be set
    let ttl = backend.ttl("as:exp_key").await.unwrap();
    assert!(ttl.is_some());

    // expire nonexistent key
    let result = backend
        .expire("as:nonexistent", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_aerospike_set_many_delete_many() {
    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let backend = make_backend(container.config()).await;

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
    backend
        .delete_many(&keys)
        .await
        .expect("delete_many failed");

    // verify all deleted
    assert!(backend.get("as:batch1").await.unwrap().is_none());
    assert!(backend.get("as:batch2").await.unwrap().is_none());
    assert!(backend.get("as:batch3").await.unwrap().is_none());
}

#[tokio::test]
async fn test_aerospike_health_check_and_stats() {
    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let backend = make_backend(container.config()).await;

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

    let container = AerospikeContainer::start()
        .await
        .expect("Failed to start Aerospike");
    let aerospike = make_backend(container.config()).await;
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
    chain
        .health_check()
        .await
        .expect("chain health_check failed");
}
