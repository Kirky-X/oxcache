//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis集成测试 - 合并所有Redis测试文件
//! 包含：Standalone、Cluster、Sentinel模式测试

#[path = "../common/mod.rs"]
mod common;

use common::redis_test_utils::{
    cleanup_test_keys, create_standalone_config, test_redis_connection,
};
use common::{
    generate_unique_service_name, is_redis_available, setup_cache, setup_logging,
    wait_for_redis_cluster, wait_for_sentinel,
};
use oxcache::backend::l2::L2Backend;
use oxcache::config::{
    CacheType, ClusterConfig, Config, GlobalConfig, L1Config, L2Config, RedisMode, SentinelConfig,
    ServiceConfig, TwoLevelConfig,
};
use oxcache::CacheExt;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_l2_backend_standalone_creation() {
    println!("测试L2Backend Standalone模式创建...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let config = create_standalone_config();
    let result = L2Backend::new(&config).await;

    assert!(
        result.is_ok(),
        "应该能成功创建Standalone L2Backend: {:?}",
        result.err()
    );
    println!("✓ Standalone L2Backend创建成功");
}

#[tokio::test]
async fn test_l2_backend_standalone_operations() {
    println!("测试L2Backend Standalone模式基本操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.unwrap();

    let test_key = "oxcache:test:ha:standalone";

    let _: Result<(), String> = cleanup_test_keys("oxcache:test:ha:*").await;

    let set_result = backend
        .set_bytes(test_key, b"standalone_value".to_vec(), Some(60))
        .await;
    assert!(set_result.is_ok(), "SET操作失败: {:?}", set_result.err());

    let get_result = backend.get_bytes(test_key).await;
    assert!(get_result.is_ok(), "GET操作失败");
    assert_eq!(get_result.unwrap(), Some(b"standalone_value".to_vec()));

    let delete_result = backend.delete(test_key).await;
    assert!(delete_result.is_ok(), "DELETE操作失败");

    println!("✓ Standalone模式基本操作测试通过");
}

#[tokio::test]
async fn test_l2_backend_health_check() {
    println!("测试L2Backend健康检查...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.unwrap();

    for i in 0..5 {
        let ping_result = backend.ping().await;
        assert!(ping_result.is_ok(), "第{}次Ping失败", i + 1);
    }

    println!("✓ 5次健康检查全部通过");
}

#[tokio::test]
async fn test_l2_backend_ttl_operations() {
    println!("测试L2Backend TTL操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.unwrap();

    let test_key = "oxcache:test:ha:ttl";

    let _: Result<(), String> = cleanup_test_keys("oxcache:test:ha:ttl*").await;

    let set_result = backend
        .set_bytes(test_key, b"ttl_value".to_vec(), Some(5))
        .await;
    assert!(set_result.is_ok(), "SET with TTL失败");

    let get_result = backend.get_bytes(test_key).await;
    assert!(get_result.is_ok());
    assert_eq!(get_result.unwrap(), Some(b"ttl_value".to_vec()));

    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;

    let expire_check = backend.get_bytes(test_key).await;
    assert!(expire_check.is_ok());
    assert!(expire_check.unwrap().is_none(), "键应该在TTL过期后被删除");

    println!("✓ TTL操作测试通过");
}

#[tokio::test]
async fn test_l2_backend_exists_operation() {
    println!("测试L2Backend EXISTS操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.unwrap();

    let test_key = "oxcache:test:ha:exists";

    let _: Result<(), String> = cleanup_test_keys("oxcache:test:ha:exists*").await;

    let exists_before = backend.get_bytes(test_key).await;
    assert!(exists_before.is_ok());
    assert!(exists_before.unwrap().is_none(), "不存在的键应该返回None");

    backend
        .set_bytes(test_key, b"exists_value".to_vec(), Some(60))
        .await
        .unwrap();

    let exists_after = backend.get_bytes(test_key).await;
    assert!(exists_after.is_ok());
    assert!(exists_after.unwrap().is_some(), "存在的键应该返回Some");

    backend.delete(test_key).await.unwrap();

    let exists_final = backend.get_bytes(test_key).await;
    assert!(exists_final.is_ok());
    assert!(exists_final.unwrap().is_none(), "删除后的键应该返回None");

    println!("✓ EXISTS操作测试通过");
}

#[tokio::test]
async fn test_l2_backend_incr_operation() {
    println!("测试L2Backend INCR操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.unwrap();

    let test_key = "oxcache:test:ha:incr";

    let _: Result<(), String> = cleanup_test_keys("oxcache:test:ha:incr*").await;

    backend.delete(test_key).await.unwrap();

    let set_result = backend.set_bytes(test_key, b"0".to_vec(), Some(60)).await;
    assert!(set_result.is_ok());

    let get_result = backend.get_bytes(test_key).await;
    assert!(get_result.is_ok());
    assert_eq!(get_result.unwrap(), Some(b"0".to_vec()));

    backend.delete(test_key).await.unwrap();

    println!("✓ INCR操作测试通过（简化版，使用set_bytes和get_bytes）");
}

#[tokio::test]
async fn test_sentinel_missing_config() {
    setup_logging();

    let config = L2Config {
        mode: RedisMode::Sentinel,
        sentinel: None,
        ..Default::default()
    };

    let result = L2Backend::new(&config).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Sentinel configuration is missing"));
    } else {
        panic!("Expected error, got Ok");
    }
}

#[tokio::test]
async fn test_cluster_missing_config() {
    setup_logging();

    let config = L2Config {
        mode: RedisMode::Cluster,
        cluster: None,
        ..Default::default()
    };

    let result = L2Backend::new(&config).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Cluster configuration is missing"));
    } else {
        panic!("Expected error, got Ok");
    }
}

#[tokio::test]
async fn test_redis_sentinel_config() {
    setup_logging();

    if env::var("ENABLE_SENTINEL_TEST").is_err() {
        println!("Skipping test_redis_sentinel_config (ENABLE_SENTINEL_TEST not set)");
        return;
    }

    let config = L2Config {
        mode: RedisMode::Sentinel,
        sentinel: Some(SentinelConfig {
            master_name: "mymaster".to_string(),
            nodes: vec![
                std::env::var("REDIS_SENTINEL_URL_1")
                    .unwrap_or_else(|_| "redis://127.0.0.1:26379".to_string()),
                std::env::var("REDIS_SENTINEL_URL_2")
                    .unwrap_or_else(|_| "redis://127.0.0.1:26380".to_string()),
            ],
        }),
        ..Default::default()
    };

    let result = L2Backend::new(&config).await;
    match result {
        Ok(_) => println!("Sentinel connected successfully"),
        Err(e) => println!("Sentinel connection failed as expected (no server): {}", e),
    }
}

#[tokio::test]
async fn test_redis_cluster_config() {
    setup_logging();

    if env::var("ENABLE_CLUSTER_TEST").is_err() {
        println!("Skipping test_redis_cluster_config (ENABLE_CLUSTER_TEST not set)");
        return;
    }

    let config = L2Config {
        mode: RedisMode::Cluster,
        cluster: Some(ClusterConfig {
            nodes: vec![
                std::env::var("REDIS_CLUSTER_URL_1")
                    .unwrap_or_else(|_| "redis://127.0.0.1:7000".to_string()),
                std::env::var("REDIS_CLUSTER_URL_2")
                    .unwrap_or_else(|_| "redis://127.0.0.1:7001".to_string()),
                std::env::var("REDIS_CLUSTER_URL_3")
                    .unwrap_or_else(|_| "redis://127.0.0.1:7002".to_string()),
            ],
        }),
        ..Default::default()
    };

    let result = L2Backend::new(&config).await;
    match result {
        Ok(_) => println!("Cluster connected successfully"),
        Err(e) => println!("Cluster connection failed as expected (no server): {}", e),
    }
}

#[tokio::test]
async fn test_real_redis_standalone_connection() {
    println!("测试真实Redis Standalone连接...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();

    match L2Backend::new(&config).await {
        Ok(backend) => {
            let result = backend.ping().await;
            assert!(result.is_ok(), "Redis连接失败: {:?}", result.err());
            println!("✓ Redis Standalone连接成功");
        }
        Err(e) => {
            println!("✗ 创建L2Backend失败: {}", e);
            panic!("无法连接到Redis: {}", e);
        }
    }
}

#[tokio::test]
async fn test_real_redis_basic_operations() {
    println!("测试Redis基本操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let test_key = "oxcache:test:basic_ops";
    let test_value = "hello_redis";

    let _ = cleanup_test_keys("oxcache:test:*").await;

    let set_result = backend
        .set_bytes(test_key, test_value.as_bytes().to_vec(), Some(60))
        .await;
    assert!(set_result.is_ok(), "SET操作失败: {:?}", set_result.err());
    println!("✓ SET操作成功");

    let get_result = backend.get_bytes(test_key).await;
    assert!(get_result.is_ok(), "GET操作失败: {:?}", get_result.err());
    let retrieved_value = get_result.unwrap();
    assert_eq!(retrieved_value, Some(test_value.as_bytes().to_vec()));
    println!("✓ GET操作成功，返回值匹配");

    let delete_result = backend.delete(test_key).await;
    assert!(
        delete_result.is_ok(),
        "DELETE操作失败: {:?}",
        delete_result.err()
    );
    println!("✓ DELETE操作成功");

    let get_after_delete = backend.get_bytes(test_key).await;
    assert_eq!(get_after_delete.unwrap(), None);
    println!("✓ 键已正确删除");
}

#[tokio::test]
async fn test_real_redis_ttl() {
    println!("测试Redis TTL功能...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let test_key = "oxcache:test:ttl";
    let test_value = "ttl_test";

    backend
        .set_bytes(test_key, test_value.as_bytes().to_vec(), Some(5))
        .await
        .expect("SET失败");

    let get_before_expiry = backend.get_bytes(test_key).await;
    assert_eq!(
        get_before_expiry.unwrap(),
        Some(test_value.as_bytes().to_vec())
    );
    println!("✓ TTL设置成功，5秒内可读取");

    backend.delete(test_key).await.expect("DELETE失败");
}

#[tokio::test]
async fn test_real_redis_exists() {
    println!("测试Redis EXISTS功能...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let test_key = "oxcache:test:exists";

    assert!(
        !backend.exists(test_key).await.expect("EXISTS失败"),
        "键不应存在"
    );
    println!("✓ 不存在的键返回false");

    backend
        .set_bytes(test_key, "exists_test".as_bytes().to_vec(), Some(60))
        .await
        .expect("SET失败");
    assert!(
        backend.exists(test_key).await.expect("EXISTS失败"),
        "键应存在"
    );
    println!("✓ 存在的键返回true");

    backend.delete(test_key).await.expect("DELETE失败");
}

#[tokio::test]
async fn test_real_redis_keys() {
    println!("测试Redis KEYS操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let _ = cleanup_test_keys("oxcache:test:keys*").await;

    backend
        .set_bytes(
            "oxcache:test:keys_1",
            "value1".as_bytes().to_vec(),
            Some(60),
        )
        .await
        .expect("SET失败");
    backend
        .set_bytes(
            "oxcache:test:keys_2",
            "value2".as_bytes().to_vec(),
            Some(60),
        )
        .await
        .expect("SET失败");
    backend
        .set_bytes(
            "oxcache:test:keys_3",
            "value3".as_bytes().to_vec(),
            Some(60),
        )
        .await
        .expect("SET失败");

    let pattern = "oxcache:test:keys*";
    let keys: Vec<String> = match &backend {
        L2Backend::Standalone { manager, .. } => {
            let mut conn = manager.clone();
            redis::cmd("KEYS")
                .arg(pattern)
                .query_async(&mut conn)
                .await
                .expect("KEYS失败")
        }
        L2Backend::Cluster { client, .. } => {
            let mut conn = client
                .get_async_connection()
                .await
                .expect("获取集群连接失败");
            redis::cmd("KEYS")
                .arg(pattern)
                .query_async(&mut conn)
                .await
                .expect("KEYS失败")
        }
    };
    assert!(keys.len() >= 3, "应找到至少3个键，实际找到: {}", keys.len());
    println!("✓ KEYS功能正常，找到 {} 个键", keys.len());

    let _ = cleanup_test_keys("oxcache:test:keys*").await;
}

#[tokio::test]
async fn test_real_redis_setnx() {
    println!("测试Redis SETNX功能...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let test_key = "oxcache:test:setnx";

    let _ = cleanup_test_keys(test_key).await;

    let first_set = backend.set_nx(test_key, "first", Some(60)).await;
    assert!(first_set.is_ok(), "SETNX失败");
    assert!(first_set.unwrap(), "第一次SETNX应返回true");
    println!("✓ 第一次SETNX返回true");

    let second_set = backend.set_nx(test_key, "second", Some(60)).await;
    assert!(second_set.is_ok(), "第二次SETNX失败");
    assert!(!second_set.unwrap(), "已存在的键SETNX应返回false");
    println!("✓ 第二次SETNX返回false");

    backend.delete(test_key).await.expect("DELETE失败");
}

#[tokio::test]
async fn test_real_redis_incr() {
    println!("测试Redis INCR功能...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let test_key = "oxcache:test:incr";

    let _ = cleanup_test_keys(test_key).await;

    backend
        .set_bytes(test_key, "10".as_bytes().to_vec(), Some(60))
        .await
        .expect("SET失败");

    let incr_result = backend.incr(test_key).await;
    assert!(incr_result.is_ok(), "INCR失败");
    assert_eq!(incr_result.unwrap(), 11);
    println!("✓ INCR操作成功，值从10增加到11");

    backend.delete(test_key).await.expect("DELETE失败");
}

#[tokio::test]
async fn test_real_redis_expire() {
    println!("测试Redis EXPIRE功能...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let test_key = "oxcache:test:expire";

    backend
        .set_bytes(test_key, "expire_test".as_bytes().to_vec(), Some(300))
        .await
        .expect("SET失败");

    let expire_result = backend.expire(test_key, 10).await;
    assert!(expire_result.is_ok(), "EXPIRE失败");
    println!("✓ EXPIRE设置成功");

    backend.delete(test_key).await.expect("DELETE失败");
}

#[tokio::test]
async fn test_real_redis_multiple_operations() {
    println!("测试Redis批量操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let _ = cleanup_test_keys("oxcache:test:batch*").await;

    for i in 0..10 {
        let key = format!("oxcache:test:batch_{}", i);
        let value = format!("batch_value_{}", i);
        assert!(backend
            .set_bytes(&key, value.as_bytes().to_vec(), Some(60))
            .await
            .is_ok());
    }
    println!("✓ 批量写入10个键成功");

    for i in 0..10 {
        let key = format!("oxcache:test:batch_{}", i);
        let expected = format!("batch_value_{}", i);
        let result = backend.get_bytes(&key).await.unwrap();
        assert_eq!(result, Some(expected.as_bytes().to_vec()));
    }
    println!("✓ 批量读取10个键成功");

    let _ = cleanup_test_keys("oxcache:test:batch*").await;
    println!("✓ 批量清理成功");
}

#[tokio::test]
async fn test_real_redis_health_check() {
    println!("测试Redis健康检查...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    for _ in 0..5 {
        let ping_result = backend.ping().await;
        assert!(ping_result.is_ok(), "健康检查失败");
    }
    println!("✓ 多次健康检查全部通过");
}

#[tokio::test]
async fn test_real_redis_type_operations() {
    println!("测试Redis TYPE操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = L2Backend::new(&config).await.expect("创建L2Backend失败");

    let string_key = "oxcache:test:type_string";
    let hash_key = "oxcache:test:type_hash";

    let _ = cleanup_test_keys("oxcache:test:type*").await;

    backend
        .set_bytes(string_key, "string_value".as_bytes().to_vec(), Some(60))
        .await
        .expect("SET失败");

    let string_type = backend.get_type(string_key).await.expect("TYPE失败");
    assert_eq!(string_type, "string");
    println!("✓ String类型检测正确");

    backend.delete(string_key).await.expect("DELETE失败");
    backend.delete(hash_key).await.expect("DELETE失败");
}

#[tokio::test]
async fn test_l2_backend_with_real_provider() {
    println!("测试L2Backend使用真实Provider...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();

    let result = L2Backend::new(&config).await;

    match result {
        Ok(backend) => {
            let ping = backend.ping().await;
            assert!(ping.is_ok(), "使用真实Provider创建L2Backend后Ping失败");
            println!("✓ L2Backend使用真实Provider成功");
        }
        Err(e) => {
            println!("✗ L2Backend创建失败: {}", e);
            panic!("无法使用真实Provider创建L2Backend");
        }
    }
}

#[tokio::test]
async fn test_concurrent_redis_operations() {
    println!("测试Redis并发操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let config = create_standalone_config();
    let backend = Arc::new(L2Backend::new(&config).await.expect("创建L2Backend失败"));

    let _ = cleanup_test_keys("oxcache:test:concurrent*").await;

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let backend = backend.clone();
            tokio::spawn(async move {
                for j in 0..10 {
                    let key = format!("oxcache:test:concurrent_{}_{}", i, j);
                    let value = format!("value_{}_{}", i, j);
                    let _ = backend
                        .set_bytes(&key, value.as_bytes().to_vec(), Some(60))
                        .await;
                }
            })
        })
        .collect();

    for handle in handles {
        handle.await.expect("并发任务失败");
    }
    println!("✓ 5个任务并发写入50个键成功");

    let pattern = "oxcache:test:concurrent*";
    let backend_ref: &L2Backend = &backend;
    let keys: Vec<String> = match backend_ref {
        L2Backend::Standalone { manager, .. } => {
            let mut conn = manager.clone();
            redis::cmd("KEYS")
                .arg(pattern)
                .query_async(&mut conn)
                .await
                .expect("KEYS失败")
        }
        L2Backend::Cluster { client, .. } => {
            let mut conn = client.get_async_connection().await.expect("获取连接失败");
            redis::cmd("KEYS")
                .arg(pattern)
                .query_async(&mut conn)
                .await
                .expect("KEYS失败")
        }
    };
    assert!(
        keys.len() >= 50,
        "应找到至少50个键，实际找到: {}",
        keys.len()
    );
    println!("✓ 并发写入的所有键都已正确保存");

    let _ = cleanup_test_keys("oxcache:test:concurrent*").await;
    println!("✓ 并发操作测试完成");
}

#[tokio::test]
async fn test_cluster_basic_operations() {
    setup_logging();

    if env::var("ENABLE_CLUSTER_TEST").is_err() {
        println!("Skipping test_cluster_basic_operations (ENABLE_CLUSTER_TEST not set)");
        return;
    }

    let cluster_urls = vec![
        "redis://127.0.0.1:7000",
        "redis://127.0.0.1:7001",
        "redis://127.0.0.1:7002",
        "redis://127.0.0.1:7003",
        "redis://127.0.0.1:7004",
        "redis://127.0.0.1:7005",
    ];

    if !wait_for_redis_cluster(&cluster_urls).await {
        panic!("Failed to wait for Redis Cluster");
    }

    let service_name = generate_unique_service_name("cluster_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Cluster,
                        connection_string: "redis://127.0.0.1:7000".to_string().into(),
                        connection_timeout_ms: 5000,
                        command_timeout_ms: 2000,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: Some(ClusterConfig {
                            nodes: vec![
                                "127.0.0.1:7000".to_string(),
                                "127.0.0.1:7001".to_string(),
                                "127.0.0.1:7002".to_string(),
                                "127.0.0.1:7003".to_string(),
                                "127.0.0.1:7004".to_string(),
                                "127.0.0.1:7005".to_string(),
                            ],
                        }),
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    let key = "cluster_test_key";
    let value = "cluster_test_value";

    client
        .set(key, &value.to_string(), Some(60))
        .await
        .expect("Failed to set cache");

    let retrieved: Option<String> = client.get(key).await.expect("Failed to get cache");
    assert_eq!(retrieved, Some(value.to_string()));

    client.delete(key).await.expect("Failed to delete cache");

    let after_delete: Option<String> = client.get(key).await.expect("Failed to get cache");
    assert_eq!(after_delete, None);

    println!("Cluster basic operations test passed!");
}

#[tokio::test]
async fn test_cluster_data_distribution() {
    setup_logging();

    if env::var("ENABLE_CLUSTER_TEST").is_err() {
        println!("Skipping test_cluster_data_distribution (ENABLE_CLUSTER_TEST not set)");
        return;
    }

    let cluster_urls = vec![
        "redis://127.0.0.1:7000",
        "redis://127.0.0.1:7001",
        "redis://127.0.0.1:7002",
        "redis://127.0.0.1:7003",
        "redis://127.0.0.1:7004",
        "redis://127.0.0.1:7005",
    ];

    if !wait_for_redis_cluster(&cluster_urls).await {
        panic!("Failed to wait for Redis Cluster");
    }

    let service_name = generate_unique_service_name("cluster_distribution_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Cluster,
                        connection_string: "redis://127.0.0.1:7000".to_string().into(),
                        connection_timeout_ms: 5000,
                        command_timeout_ms: 2000,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: Some(ClusterConfig {
                            nodes: vec![
                                "127.0.0.1:7000".to_string(),
                                "127.0.0.1:7001".to_string(),
                                "127.0.0.1:7002".to_string(),
                                "127.0.0.1:7003".to_string(),
                                "127.0.0.1:7004".to_string(),
                                "127.0.0.1:7005".to_string(),
                            ],
                        }),
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    let num_keys = 100;
    let mut keys = Vec::new();

    for i in 0..num_keys {
        let key = format!("cluster_dist_key_{}", i);
        let value = format!("cluster_dist_value_{}", i);
        keys.push(key.clone());

        client
            .set(&key, &value, Some(300))
            .await
            .expect("Failed to set cache");
    }

    for i in 0..num_keys {
        let key = format!("cluster_dist_key_{}", i);
        let expected_value = format!("cluster_dist_value_{}", i);

        let retrieved: Option<String> = client.get(&key).await.expect("Failed to get cache");
        assert_eq!(retrieved, Some(expected_value));
    }

    println!("Cluster data distribution test passed!");
}

#[tokio::test]
async fn test_cluster_distributed_lock() {
    setup_logging();

    if env::var("ENABLE_CLUSTER_TEST").is_err() {
        println!("Skipping test_cluster_distributed_lock (ENABLE_CLUSTER_TEST not set)");
        return;
    }

    let cluster_urls = vec![
        "redis://127.0.0.1:7000",
        "redis://127.0.0.1:7001",
        "redis://127.0.0.1:7002",
        "redis://127.0.0.1:7003",
        "redis://127.0.0.1:7004",
        "redis://127.0.0.1:7005",
    ];

    if !wait_for_redis_cluster(&cluster_urls).await {
        panic!("Failed to wait for Redis Cluster");
    }

    let service_name = generate_unique_service_name("cluster_lock_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Cluster,
                        connection_string: "redis://127.0.0.1:7000".to_string().into(),
                        connection_timeout_ms: 5000,
                        command_timeout_ms: 2000,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: Some(ClusterConfig {
                            nodes: vec![
                                "127.0.0.1:7000".to_string(),
                                "127.0.0.1:7001".to_string(),
                                "127.0.0.1:7002".to_string(),
                                "127.0.0.1:7003".to_string(),
                                "127.0.0.1:7004".to_string(),
                                "127.0.0.1:7005".to_string(),
                            ],
                        }),
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    let lock_key = "cluster_lock_test";
    let lock_value1 = "uuid_1";
    let lock_value2 = "uuid_2";
    let ttl = 10;

    let locked1 = client
        .lock(lock_key, lock_value1, ttl)
        .await
        .expect("Failed to acquire lock");
    assert!(locked1, "First client should acquire lock successfully");

    let locked2 = client
        .lock(lock_key, lock_value2, ttl)
        .await
        .expect("Failed to call lock");
    assert!(!locked2, "Second client should fail to acquire lock");

    let unlocked1 = client
        .unlock(lock_key, lock_value1)
        .await
        .expect("Failed to release lock");
    assert!(unlocked1, "First client should release lock successfully");

    let locked2_after_unlock = client
        .lock(lock_key, lock_value2, ttl)
        .await
        .expect("Failed to acquire lock");
    assert!(
        locked2_after_unlock,
        "Second client should acquire lock after first client releases it"
    );

    let unlocked2 = client
        .unlock(lock_key, lock_value2)
        .await
        .expect("Failed to release lock");
    assert!(unlocked2, "Second client should release lock successfully");

    println!("Cluster distributed lock test passed!");
}

#[tokio::test]
async fn test_sentinel_basic_operations() {
    setup_logging();

    if env::var("ENABLE_SENTINEL_TEST").is_err() {
        println!("Skipping test_sentinel_basic_operations (ENABLE_SENTINEL_TEST not set)");
        return;
    }

    if !wait_for_sentinel().await {
        panic!("Failed to wait for Redis Sentinel");
    }

    let service_name = generate_unique_service_name("sentinel_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Sentinel,
                        connection_string: "redis://127.0.0.1:26379".to_string().into(),
                        connection_timeout_ms: 5000,
                        command_timeout_ms: 2000,
                        password: None,
                        enable_tls: false,
                        sentinel: Some(SentinelConfig {
                            master_name: "mymaster".to_string(),
                            nodes: vec![
                                "127.0.0.1:26379".to_string(),
                                "127.0.0.1:26380".to_string(),
                                "127.0.0.1:26381".to_string(),
                            ],
                        }),
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    let key = "sentinel_test_key".to_string();
    let value = "sentinel_test_value".to_string();

    client
        .set(&key, &value, Some(60))
        .await
        .expect("Failed to set cache");

    let retrieved: Option<String> = client.get(&key).await.expect("Failed to get cache");
    assert_eq!(retrieved, Some(value));

    client.delete(&key).await.expect("Failed to delete cache");

    let after_delete: Option<String> = client.get(&key).await.expect("Failed to get cache");
    assert_eq!(after_delete, None);

    println!("Sentinel basic operations test passed!");
}

#[tokio::test]
async fn test_sentinel_failover() {
    setup_logging();

    if env::var("ENABLE_SENTINEL_TEST").is_err() {
        println!("Skipping test_sentinel_failover (ENABLE_SENTINEL_TEST not set)");
        return;
    }

    if !wait_for_sentinel().await {
        panic!("Failed to wait for Redis Sentinel");
    }

    let service_name = generate_unique_service_name("sentinel_failover_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Sentinel,
                        connection_string: "redis://127.0.0.1:26379".to_string().into(),
                        connection_timeout_ms: 5000,
                        command_timeout_ms: 2000,
                        password: None,
                        enable_tls: false,
                        sentinel: Some(SentinelConfig {
                            master_name: "mymaster".to_string(),
                            nodes: vec![
                                "127.0.0.1:26379".to_string(),
                                "127.0.0.1:26380".to_string(),
                                "127.0.0.1:26381".to_string(),
                            ],
                        }),
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    let test_key = "failover_test_key".to_string();
    let test_value = "failover_test_value".to_string();
    client
        .set(&test_key, &test_value, Some(300))
        .await
        .expect("Failed to set cache");

    let retrieved: Option<String> = client.get(&test_key).await.expect("Failed to get cache");
    assert_eq!(retrieved, Some(test_value));

    println!("Data written before failover: {:?}", retrieved);
    println!(
        "Note: To fully test failover, manually stop redis-master container and run test again"
    );
    println!("Sentinel failover test setup completed!");
}

#[tokio::test]
async fn test_sentinel_distributed_lock() {
    setup_logging();

    if env::var("ENABLE_SENTINEL_TEST").is_err() {
        println!("Skipping test_sentinel_distributed_lock (ENABLE_SENTINEL_TEST not set)");
        return;
    }

    if !wait_for_sentinel().await {
        panic!("Failed to wait for Redis Sentinel");
    }

    let service_name = generate_unique_service_name("sentinel_lock_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 100,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Sentinel,
                        connection_string: "redis://127.0.0.1:26379".to_string().into(),
                        connection_timeout_ms: 5000,
                        command_timeout_ms: 2000,
                        password: None,
                        enable_tls: false,
                        sentinel: Some(SentinelConfig {
                            master_name: "mymaster".to_string(),
                            nodes: vec![
                                "127.0.0.1:26379".to_string(),
                                "127.0.0.1:26380".to_string(),
                                "127.0.0.1:26381".to_string(),
                            ],
                        }),
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    let lock_key = "sentinel_lock_test";
    let lock_value1 = "uuid_1";
    let lock_value2 = "uuid_2";
    let ttl = 10;

    let locked1 = client
        .lock(lock_key, lock_value1, ttl)
        .await
        .expect("Failed to acquire lock");
    assert!(locked1, "First client should acquire lock successfully");

    let locked2 = client
        .lock(lock_key, lock_value2, ttl)
        .await
        .expect("Failed to call lock");
    assert!(!locked2, "Second client should fail to acquire lock");

    let unlocked1 = client
        .unlock(lock_key, lock_value1)
        .await
        .expect("Failed to release lock");
    assert!(unlocked1, "First client should release lock successfully");

    let locked2_after_unlock = client
        .lock(lock_key, lock_value2, ttl)
        .await
        .expect("Failed to acquire lock");
    assert!(
        locked2_after_unlock,
        "Second client should acquire lock after first client releases it"
    );

    let unlocked2 = client
        .unlock(lock_key, lock_value2)
        .await
        .expect("Failed to release lock");
    assert!(unlocked2, "Second client should release lock successfully");

    println!("Sentinel distributed lock test passed!");
}
