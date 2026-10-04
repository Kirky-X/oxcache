// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Redis Sentinel 集成测试

use crate::common::{get_sentinel_addr_map, get_sentinel_urls, wait_for_sentinel};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::memory::RedisMode;
use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

#[path = "../../common/mod.rs"]
mod common;

fn get_master_url() -> String {
    std::env::var("REDIS_SENTINEL_MASTER_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:16380".to_string())
}

fn is_sentinel_available() -> bool {
    std::env::var("REDIS_SENTINEL_AVAILABLE").is_ok()
}

/// compose 拓扑的定值 master（redis-master，静态 IP 172.26.0.2）。
fn original_master_addr(addr_map: &[(String, String)]) -> (String, u16) {
    for (from, to) in addr_map {
        if to.ends_with(":16380") {
            let (ip, port) = from.rsplit_once(':').expect("addr map key must be ip:port");
            return (ip.to_string(), port.parse().expect("port must be numeric"));
        }
    }
    panic!("addr map must contain the redis-master entry (host port 16380)");
}

fn translate_to_host(addr: &(String, u16), addr_map: &[(String, String)]) -> u16 {
    let key = format!("{}:{}", addr.0, addr.1);
    let mapped = addr_map
        .iter()
        .find(|(from, _)| *from == key)
        .map(|(_, to)| to.clone())
        .unwrap_or(key);
    mapped
        .rsplit_once(':')
        .expect("mapped address must be host:port")
        .1
        .parse()
        .expect("port must be numeric")
}

async fn open_conn(host: &str, port: u16) -> redis::aio::MultiplexedConnection {
    let client = redis::Client::open(format!("redis://{host}:{port}")).expect("valid URL");
    client
        .get_multiplexed_async_connection()
        .await
        .expect("connect to redis node")
}

/// 任一 sentinel 报告的当前 master 地址（容器网络视野）。
async fn sentinel_master_addr() -> (String, u16) {
    for port in [26382u16, 26383, 26384] {
        let mut conn = open_conn("127.0.0.1", port).await;
        let addr: Option<(String, u16)> = redis::cmd("SENTINEL")
            .arg("get-master-addr-by-name")
            .arg("mymaster")
            .query_async(&mut conn)
            .await
            .expect("SENTINEL get-master-addr-by-name");
        if let Some(addr) = addr {
            return addr;
        }
    }
    panic!("no sentinel could report the master address");
}

async fn trigger_failover() {
    let mut conn = open_conn("127.0.0.1", 26382).await;
    let result: redis::RedisResult<String> = redis::cmd("SENTINEL")
        .arg("failover")
        .arg("mymaster")
        .query_async(&mut conn)
        .await;
    if let Err(e) = result {
        // 已有 failover 进行中等场景下受理被拒不算失败，交给等待地址切换兜底
        println!("SENTINEL FAILOVER 未受理（继续等待地址切换）: {e}");
    }
}

async fn wait_master_addr_changes(from: &(String, u16), timeout: Duration) -> (String, u16) {
    let start = Instant::now();
    loop {
        let current = sentinel_master_addr().await;
        if &current != from {
            return current;
        }
        assert!(
            start.elapsed() < timeout,
            "master address did not switch away from {}:{} within {timeout:?}",
            from.0,
            from.1
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn value_as_string(value: &redis::Value) -> Option<String> {
    match value {
        redis::Value::SimpleString(s) => Some(s.clone()),
        redis::Value::BulkString(b) => String::from_utf8(b.clone()).ok(),
        _ => None,
    }
}

async fn node_role(host_port: u16) -> String {
    let mut conn = open_conn("127.0.0.1", host_port).await;
    let role: redis::Value = redis::cmd("ROLE")
        .query_async(&mut conn)
        .await
        .expect("ROLE");
    value_as_string(&role)
        .or_else(|| match &role {
            redis::Value::Array(items) => items.first().and_then(value_as_string),
            _ => None,
        })
        .unwrap_or_else(|| panic!("unexpected ROLE reply: {role:?}"))
}

async fn wait_node_role(host_port: u16, expected: &str, timeout: Duration) {
    let start = Instant::now();
    loop {
        if node_role(host_port).await == expected {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "node 127.0.0.1:{host_port} did not become '{expected}' within {timeout:?}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// 从节点与新 master 的复制链路就绪（晋升候选资格的前提）。
async fn wait_replica_in_sync(host_port: u16, timeout: Duration) {
    let start = Instant::now();
    loop {
        let mut conn = open_conn("127.0.0.1", host_port).await;
        let info: String = redis::cmd("INFO")
            .arg("replication")
            .query_async(&mut conn)
            .await
            .expect("INFO replication");
        if info.contains("role:slave") && info.contains("master_link_status:up") {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "node 127.0.0.1:{host_port} replication link did not come up within {timeout:?}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn set_config(host_port: u16, name: &str, value: &str) {
    let mut conn = open_conn("127.0.0.1", host_port).await;
    let _: String = redis::cmd("CONFIG")
        .arg("SET")
        .arg(name)
        .arg(value)
        .query_async(&mut conn)
        .await
        .unwrap_or_else(|e| panic!("CONFIG SET {name} {value} on {host_port}: {e}"));
}

/// 故障转移演练后把 master 恢复为拓扑定值节点（redis-master）。
///
/// 同文件其余测试与复跑都直连 master 宿主端口，不恢复会让它们打到降级
/// 后的从节点（READONLY）。做法：给其余节点临时设 replica-priority 0
/// （0 = 不参与晋升选举），使恢复转移只能晋升定值节点，完成后还原。
async fn restore_master_topology(original: &(String, u16), addr_map: &[(String, String)]) {
    let original_host_port = translate_to_host(original, addr_map);
    let other_host_ports: Vec<u16> = addr_map
        .iter()
        .filter_map(|(_, to)| to.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok()))
        .filter(|p| *p != original_host_port)
        .collect();

    for port in &other_host_ports {
        set_config(*port, "replica-priority", "0").await;
    }

    let current = sentinel_master_addr().await;
    if &current != original {
        wait_replica_in_sync(original_host_port, Duration::from_secs(60)).await;
        trigger_failover().await;
        wait_master_addr_changes(&current, Duration::from_secs(90)).await;
    }

    for port in &other_host_ports {
        set_config(*port, "replica-priority", "100").await;
    }

    let restored = sentinel_master_addr().await;
    assert_eq!(
        &restored, original,
        "master topology restore failed: still at {}:{}",
        restored.0, restored.1
    );
}

#[tokio::test]
async fn test_sentinel_connection() {
    println!("测试 Redis Sentinel 连接...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用 (设置 REDIS_SENTINEL_AVAILABLE=1 启用)");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    // 测试连接到 Sentinel 节点
    let urls = get_sentinel_urls();
    for url in &urls {
        let backend = RedisBackend::new(url).await;
        if backend.is_ok() {
            println!("✓ 成功连接到 Sentinel 节点: {}", url);
            return;
        }
    }

    panic!("无法连接到任何 Sentinel 节点");
}

#[tokio::test]
async fn test_sentinel_master_operations() {
    println!("测试 Redis Sentinel 主节点操作...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 测试基本操作
    backend
        .set(
            Arc::from("sentinel_key_1"),
            Arc::new(b"sentinel_value_1".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    let value = backend.get("sentinel_key_1").await.unwrap();
    assert_eq!(value, Some(b"sentinel_value_1".to_vec()));

    backend.delete("sentinel_key_1").await.unwrap();

    println!("✓ Redis Sentinel 主节点操作测试成功");
}

#[tokio::test]
async fn test_sentinel_ttl() {
    println!("测试 Redis Sentinel TTL...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 设置带 TTL 的键
    backend
        .set(
            Arc::from("sentinel_ttl_key"),
            Arc::new(b"ttl_value".to_vec()),
            Some(Duration::from_secs(2)),
        )
        .await
        .unwrap();

    // 验证键存在
    assert!(backend.exists("sentinel_ttl_key").await.unwrap());

    // 获取 TTL
    let ttl = backend.ttl("sentinel_ttl_key").await.unwrap();
    assert!(ttl.is_some());

    // 等待过期
    // ponytail: requires Docker, polling not feasible without container
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 验证键已过期
    assert!(!backend.exists("sentinel_ttl_key").await.unwrap());

    println!("✓ Redis Sentinel TTL 测试成功");
}

#[tokio::test]
async fn test_sentinel_expire() {
    println!("测试 Redis Sentinel EXPIRE...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 设置不带 TTL 的键
    backend
        .set(
            Arc::from("sentinel_expire_key"),
            Arc::new(b"expire_value".to_vec()),
            None,
        )
        .await
        .unwrap();

    // 设置过期时间
    let result = backend
        .expire("sentinel_expire_key", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(result);

    // 验证 TTL 已设置
    let ttl = backend.ttl("sentinel_expire_key").await.unwrap();
    assert!(ttl.is_some());

    backend.delete("sentinel_expire_key").await.unwrap();

    println!("✓ Redis Sentinel EXPIRE 测试成功");
}

#[tokio::test]
async fn test_sentinel_health_check() {
    println!("测试 Redis Sentinel 健康检查...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    backend.health_check().await.unwrap();

    println!("✓ Redis Sentinel 健康检查测试成功");
}

#[tokio::test]
async fn test_sentinel_stats() {
    println!("测试 Redis Sentinel 统计信息...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    backend
        .set(
            Arc::from("sentinel_stats_key"),
            Arc::new(b"stats_value".to_vec()),
            None,
        )
        .await
        .unwrap();

    let stats = backend.stats().await.unwrap();
    assert_eq!(stats.get("type"), Some(&"redis".to_string()));

    backend.delete("sentinel_stats_key").await.unwrap();

    println!("✓ Redis Sentinel 统计信息测试成功");
}

#[tokio::test]
async fn test_sentinel_many_keys() {
    println!("测试 Redis Sentinel 多键操作...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 设置 50 个键
    for i in 0..50 {
        let key = format!("sentinel_many_{}", i);
        let value = format!("value_{}", i);
        backend
            .set(
                Arc::from(key.as_str()),
                Arc::new(value.as_bytes().to_vec()),
                None,
            )
            .await
            .unwrap();
    }

    // 验证所有键
    for i in 0..50 {
        let key = format!("sentinel_many_{}", i);
        let expected = format!("value_{}", i);
        let value = backend.get(&key).await.unwrap();
        assert_eq!(value, Some(expected.as_bytes().to_vec()));
    }

    // 清理
    for i in 0..50 {
        let key = format!("sentinel_many_{}", i);
        backend.delete(&key).await.unwrap();
    }

    println!("✓ Redis Sentinel 多键操作测试成功");
}

#[tokio::test]
async fn test_sentinel_large_value() {
    println!("测试 Redis Sentinel 大值...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let master_url = get_master_url();

    let backend = match RedisBackend::new(&master_url).await {
        Ok(b) => b,
        Err(e) => {
            println!("跳过测试: 无法连接到主节点 - {}", e);
            return;
        }
    };

    // 1MB 数据
    let large_value = vec![0u8; 1024 * 1024];

    backend
        .set(
            Arc::from("sentinel_large_key"),
            Arc::new(large_value.clone()),
            None,
        )
        .await
        .unwrap();
    let retrieved = backend.get("sentinel_large_key").await.unwrap();
    assert_eq!(retrieved, Some(large_value));

    backend.delete("sentinel_large_key").await.unwrap();

    println!("✓ Redis Sentinel 大值测试成功");
}

/// 场景：Sentinel 模式构建 → 写入 → 触发 failover → 同一客户端重新发现
/// 新 master 继续读写。
///
/// 放在文件末尾：failover 会改变拓扑内 master 归属，串行执行时先跑完
/// 上面直连 master 的测试，测试结束后再把 master 恢复为拓扑定值节点。
#[tokio::test]
async fn test_sentinel_failover_rediscovery() {
    println!("测试 Redis Sentinel 主从故障转移（同一客户端重新发现新 master）...");

    if !is_sentinel_available() {
        println!("跳过测试: Redis Sentinel 不可用");
        return;
    }

    if !wait_for_sentinel().await {
        println!("跳过测试: Redis Sentinel 未就绪");
        return;
    }

    let addr_map = get_sentinel_addr_map();

    // Sentinel 模式构建：连接串是 sentinel 节点列表，master 由发现协议解析
    let backend = RedisBackend::builder()
        .mode(RedisMode::Sentinel)
        .connection_string(&get_sentinel_urls().join(","))
        .sentinel_master_name("mymaster")
        .sentinel_addr_map(addr_map.clone())
        .build()
        .await
        .expect("Sentinel 模式构建失败");
    assert_eq!(backend.mode(), RedisMode::Sentinel);

    // 清理上次运行遗留
    let _ = backend.delete("sentinel_failover_key").await;
    let _ = backend.delete("sentinel_failover_post_key").await;

    // 写入（经 sentinel 发现的当前 master）
    backend
        .set(
            Arc::from("sentinel_failover_key"),
            Arc::new(b"before_failover".to_vec()),
            Some(Duration::from_secs(300)),
        )
        .await
        .unwrap();
    assert_eq!(
        backend.get("sentinel_failover_key").await.unwrap(),
        Some(b"before_failover".to_vec())
    );

    // 触发 failover：等待 master 地址切换、旧 master 降级为从
    let old_master = sentinel_master_addr().await;
    println!(
        "failover 前 master: {}:{}（宿主端口 {}）",
        old_master.0,
        old_master.1,
        translate_to_host(&old_master, &addr_map)
    );
    trigger_failover().await;
    let new_master = wait_master_addr_changes(&old_master, Duration::from_secs(90)).await;
    println!(
        "failover 后 master: {}:{}（宿主端口 {}）",
        new_master.0,
        new_master.1,
        translate_to_host(&new_master, &addr_map)
    );
    let old_host_port = translate_to_host(&old_master, &addr_map);
    wait_node_role(old_host_port, "slave", Duration::from_secs(30)).await;

    // 同一客户端继续读写：旧连接指向已降级的旧 master，写入会触发
    // READONLY/断连，后端必须自动重新发现新 master
    backend
        .set(
            Arc::from("sentinel_failover_post_key"),
            Arc::new(b"after_failover".to_vec()),
            Some(Duration::from_secs(300)),
        )
        .await
        .unwrap();
    assert_eq!(
        backend.get("sentinel_failover_post_key").await.unwrap(),
        Some(b"after_failover".to_vec())
    );
    // failover 前写入的数据经复制保留，仍可读
    assert_eq!(
        backend.get("sentinel_failover_key").await.unwrap(),
        Some(b"before_failover".to_vec())
    );

    backend.delete("sentinel_failover_key").await.unwrap();
    backend.delete("sentinel_failover_post_key").await.unwrap();

    // 拓扑复原（其余测试与复跑依赖直连 redis-master 的宿主端口）
    let original = original_master_addr(&addr_map);
    restore_master_topology(&original, &addr_map).await;

    println!("✓ Redis Sentinel 主从故障转移测试成功");
}
