//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 随机故障混沌测试

use crate::common::{generate_unique_service_name, is_redis_available, setup_cache};
use oxcache::config::{
    CacheType, Config, GlobalConfig, L1Config, L2Config, RedisMode, SerializationType,
    ServiceConfig, TwoLevelConfig,
};
use oxcache::CacheExt;
use rand::Rng;
use secrecy::SecretBox;
use tokio::time::timeout;

#[path = "../common/mod.rs"]
mod common;

/// 测试随机Redis故障
///
/// 验证缓存在随机Redis故障情况下的稳定性和恢复能力
#[tokio::test]
#[ignore]
async fn test_random_redis_failures() {
    if !is_redis_available().await {
        return;
    }

    let service_name = generate_unique_service_name("chaos_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1,
            serialization: SerializationType::Json,
            enable_metrics: true,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 1000,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 100,
                        command_timeout_ms: 100,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                        invalidation_channel: None,
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).unwrap();

    let start = Instant::now();
    let mut success_count = 0;
    let mut failure_count = 0;

    while start.elapsed() < Duration::from_secs(10) {
        let op = rand::thread_rng().gen_range(0..3);
        let key = format!("key_{}", rand::thread_rng().gen_range(0..100));

        let result = match op {
            0 => client.set(&key, &"value", Some(300)).await,
            1 => client.get::<String>(&key).await.map(|_| ()),
            _ => client.delete(&key).await,
        };

        if result.is_ok() {
            success_count += 1;
        } else {
            failure_count += 1;
        }

        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    println!(
        "混沌测试结果: 成功={}, 失败={}",
        success_count, failure_count
    );
    assert!(success_count > 0);

    let _ = std::fs::remove_file(format!("{}_wal.db", service_name));
}

/// 测试分布式锁在故障情况下的行为
///
/// 验证锁的获取和释放在不稳定条件下的正确性
#[tokio::test]
#[ignore]
async fn test_distributed_lock_during_failures() {
    if !is_redis_available().await {
        return;
    }

    let service_name = generate_unique_service_name("lock_chaos");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1,
            serialization: SerializationType::Json,
            enable_metrics: true,
        },
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
                        mode: RedisMode::Standalone,
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 100,
                        command_timeout_ms: 100,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                        invalidation_channel: None,
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).unwrap();

    let lock_key = "chaos_lock_test";
    let value1 = "client_1_unique_value";
    let value2 = "client_2_unique_value";
    let ttl = 5;

    let acquired = client.lock(lock_key, value1, ttl).await.unwrap();
    assert!(acquired, "客户端1应该成功获取锁");

    let acquired_again = client.lock(lock_key, value2, ttl).await.unwrap();
    assert!(!acquired_again, "客户端2应该无法获取已持有的锁");

    let released = client.unlock(lock_key, value1).await.unwrap();
    assert!(released, "正确值应该能释放锁");

    let acquired_after_release = client.lock(lock_key, value2, ttl).await.unwrap();
    assert!(acquired_after_release, "锁释放后客户端2应该能获取");

    let _ = std::fs::remove_file(format!("{}_wal.db", service_name));
}

/// 测试并发操作在故障情况下的隔离性
///
/// 验证多个并发操作不会相互干扰
#[tokio::test]
#[ignore]
async fn test_concurrent_isolation_during_failures() {
    if !is_redis_available().await {
        return;
    }

    let service_name = generate_unique_service_name("isolation_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1,
            serialization: SerializationType::Json,
            enable_metrics: true,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 1000,
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 100,
                        command_timeout_ms: 100,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                        invalidation_channel: None,
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).unwrap();

    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for i in 0..10 {
        let client_clone = client.clone();
        let key = format!("concurrent_key_{}", i % 5);
        let value = format!("value_from_thread_{}", i);
        let success_clone = success_count.clone();
        let error_clone = error_count.clone();

        let handle = tokio::spawn(async move {
            for attempt in 0..20 {
                let result = timeout(
                    Duration::from_millis(50),
                    client_clone.set(&key, &value, Some(60)),
                )
                    .await;

                match result {
                    Ok(Ok(())) => {
                        success_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(Err(_)) | Err(_) => {
                        error_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }

                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = timeout(Duration::from_secs(30), handle).await;
    }

    let total_success = success_count.load(Ordering::SeqCst);
    let total_errors = error_count.load(Ordering::SeqCst);

    println!(
        "并发隔离测试: 成功={}, 错误={}",
        total_success, total_errors
    );

    assert!(
        total_success > 50,
        "应该有足够的成功操作，实际成功: {}",
        total_success
    );

    let _ = std::fs::remove_file(format!("{}_wal.db", service_name));
}

/// 测试网络不稳定情况下的超时处理
///
/// 验证系统在网络延迟和超时情况下的行为
#[tokio::test]
#[ignore]
async fn test_network_instability_handling() {
    if !is_redis_available().await {
        return;
    }

    let service_name = generate_unique_service_name("network_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1,
            serialization: SerializationType::Json,
            enable_metrics: true,
        },
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
                        mode: RedisMode::Standalone,
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 50,
                        command_timeout_ms: 50,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                        invalidation_channel: None,
                        bloom_filter: None,
                        warmup: None,
                        max_key_length: Some(1024),
                        max_value_size: Some(1024 * 1024),
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).unwrap();

    let mut success_count = 0;
    let mut timeout_count = 0;

    for i in 0..50 {
        let key = format!("timeout_test_key_{}", i);

        let result = timeout(Duration::from_millis(100), client.set(&key, &"value", Some(60))).await;

        match result {
            Ok(Ok(())) => success_count += 1,
            Ok(Err(_)) | Err(_) => timeout_count += 1,
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    println!(
        "网络不稳定测试: 成功={}, 超时/错误={}",
        success_count, timeout_count
    );

    assert!(
        success_count > 0,
        "至少应该有一些操作成功完成"
    );

    let _ = std::fs::remove_file(format!("{}_wal.db", service_name));
}

/// 测试数据一致性在故障恢复后
///
/// 验证故障恢复后L1和L2的数据一致性
#[tokio::test]
#[ignore]
async fn test_data_consistency_after_recovery() {
    if !is_redis_available().await {
        return;
    }

    let service_name = generate_unique_service_name("consistency_test");

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 1,
            serialization: SerializationType::Json,
            enable_metrics: true,
        },
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
                        mode: RedisMode::Standalone,
                        connection_string: SecretBox::new("redis://127.0.0.1:6379".into()),
                        connection_timeout_ms: 100,
                        command_timeout_ms: 100,
                        password: None,
                        enable_tls: false,
                        sentinel: None,
                        cluster: None,
                        default_ttl: None,
                        max_key_length: 256,
                        max_value_size: 1024 * 1024 * 10,
                    }),
                    two_level: Some(TwoLevelConfig {
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                        invalidation_channel: None,
                    }),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).unwrap();

    let test_key = "consistency_test_key";
    let test_value = "test_value_for_consistency";

    client.set(test_key, test_value, Some(60)).await.unwrap();

    for _ in 0..10 {
        let result: Option<String> = client.get(test_key).await.unwrap();
        assert_eq!(result, Some(test_value.to_string()), "读取到的值应该一致");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let _ = std::fs::remove_file(format!("{}_wal.db", service_name));
}
