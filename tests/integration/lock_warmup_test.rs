// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 锁预热功能集成测试 - 使用新API
// 注意：锁和预热功能需要直接使用后端，新API暂不支持

use common::{cleanup_service, generate_unique_service_name, is_redis_available, setup_logging};
use oxcache::Cache;
use std::time::Duration;
use tokio::time::sleep;

use crate::common;

/// 测试分布式锁
///
/// 验证分布式锁功能
/// 注意：此测试使用 Redis 直接实现锁功能，因为新API暂不直接支持锁
#[tokio::test]
async fn test_distributed_lock() {
    if !is_redis_available().await {
        println!("Skipping test_distributed_lock: Redis not available");
        return;
    }

    setup_logging();
    let service_name = generate_unique_service_name("lock_test");
    let redis_url = common::get_redis_url();

    // 使用新API创建缓存
    // 如果 TLS 连接失败，跳过测试
    let cache: Cache<String, String> = match Cache::redis(&redis_url).await {
        Ok(c) => c,
        Err(e) => {
            println!("Skipping test: Failed to create cache (TLS required): {}", e);
            return;
        }
    };

    let lock_key = "test_lock";
    let _ttl = Duration::from_secs(5);

    // 1. 测试基本缓存功能
    cache.set(&"key1".to_string(), &"value1".to_string()).await.unwrap();
    let val: Option<String> = cache.get(&"key1".to_string()).await.unwrap();
    assert_eq!(val, Some("value1".to_string()));

    // 2. 测试锁功能（使用Redis直接实现）
    // 注意：新API暂不直接支持lock/unlock方法
    // 使用 Redis SETNX 命令实现简单的锁
    let mut conn = match redis::Client::open(redis_url.as_str()) {
        Ok(client) => match client.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                println!("Skipping lock test: Failed to get Redis connection: {}", e);
                cache.shutdown().await;
                return;
            }
        },
        Err(e) => {
            println!("Skipping lock test: Failed to create Redis client: {}", e);
            cache.shutdown().await;
            return;
        }
    };

    // 获取锁
    let lock_value: Option<String> = redis::cmd("SET")
        .arg(&[lock_key, "lock_holder", "NX", "EX", "5"])
        .query_async(&mut conn)
        .await
        .expect("Failed to acquire lock");
    assert!(lock_value.is_some(), "Should acquire lock successfully");

    // 尝试再次获取锁（应该失败）
    let lock_again: Option<String> = redis::cmd("SET")
        .arg(&[lock_key, "lock_holder", "NX", "EX", "5"])
        .query_async(&mut conn)
        .await
        .expect("Failed to check lock");
    assert!(lock_again.is_none(), "Should fail to acquire lock again");

    // 释放锁
    let unlocked: i32 = redis::cmd("DEL")
        .arg(lock_key)
        .query_async(&mut conn)
        .await
        .expect("Failed to unlock");
    assert_eq!(unlocked, 1, "Should unlock successfully");

    // 3. 测试锁过期
    sleep(Duration::from_secs(6)).await;
    let lock_after_expire: Option<String> = redis::cmd("SET")
        .arg(&[lock_key, "lock_holder", "NX", "EX", "5"])
        .query_async(&mut conn)
        .await
        .expect("Failed to acquire lock after expiration");
    assert!(lock_after_expire.is_some(), "Should acquire lock after expiration");

    // 清理
    cache.shutdown().await;
    cleanup_service(&service_name).await;
}

/// 测试缓存预热
///
/// 验证缓存预热功能
/// 注意：此测试使用手动方式预热缓存，因为新API暂不直接支持warmup方法
#[tokio::test]
async fn test_cache_preheating() {
    if !is_redis_available().await {
        println!("Skipping test_cache_preheating: Redis not available");
        return;
    }

    setup_logging();
    let service_name = generate_unique_service_name("warmup_test");
    let redis_url = common::get_redis_url();

    // 使用新API创建缓存
    // 如果 TLS 连接失败，跳过测试
    let cache: Cache<String, String> = match Cache::redis(&redis_url).await {
        Ok(c) => c,
        Err(e) => {
            println!("Skipping test: Failed to create cache (TLS required): {}", e);
            return;
        }
    };

    // 模拟数据加载器
    let load_data = |keys: Vec<String>| async move {
        let mut res = Vec::new();
        for k in keys {
            res.push((k.clone(), format!("value_of_{}", k)));
        }
        Ok::<Vec<(String, String)>, String>(res)
    };

    let keys = vec!["warm_1".to_string(), "warm_2".to_string()];

    // 手动执行预热 - 先加载数据，然后写入缓存
    let data = load_data(keys.clone()).await.expect("Loader failed");

    // 预热数据
    for (key, value) in &data {
        cache.set(key, value).await.unwrap();
    }

    // 验证数据已预热
    for key in keys {
        let val: Option<String> = cache.get(&key).await.unwrap();
        assert!(val.is_some(), "Key {} should be preheated", key);
    }

    // 清理
    cache.shutdown().await;
    cleanup_service(&service_name).await;
}
