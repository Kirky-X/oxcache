//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 锁预热功能集成测试

use crate::common::{
    cleanup_service, generate_unique_service_name, is_redis_available, setup_cache,
};
use oxcache::backend::l1::L1Backend;
use oxcache::backend::l2::L2Backend;
use oxcache::client::two_level::TwoLevelClient;
use oxcache::config::{
    CacheType, Config, GlobalConfig, L1Config, L2Config, RedisMode, ServiceConfig, TwoLevelConfig,
};
use oxcache::serialization::json::JsonSerializer;
use oxcache::serialization::SerializerEnum;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[path = "../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_distributed_lock() {
    if !is_redis_available().await {
        println!("Skipping test_distributed_lock: Redis not available");
        return;
    }

    let service_name = generate_unique_service_name("lock_test");

    // 初始化配置
    let config = Config {
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config { max_capacity: 100 }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: "redis://127.0.0.1:6379".to_string().into(),
                        connection_timeout_ms: 500,
                        command_timeout_ms: 500,
                        sentinel: None,
                        default_ttl: None,
                        cluster: None,
                        password: None,
                        enable_tls: false,
                    }),
                    two_level: Some(TwoLevelConfig::default()),
                },
            );
            map
        },
    };

    setup_cache(config).await;
    let client = oxcache::get_client(&service_name).expect("Failed to get client");

    // 1. 测试获取锁
    let lock_key = "test_lock";
    let lock_val = "uuid_1";
    let ttl = 5;

    let locked = client
        .lock(lock_key, lock_val, ttl)
        .await
        .expect("Failed to acquire lock");
    assert!(locked, "Should acquire lock successfully");

    // 2. 测试重复获取锁（应失败）
    let locked_again = client
        .lock(lock_key, "uuid_2", ttl)
        .await
        .expect("Failed to call lock");
    assert!(!locked_again, "Should fail to acquire lock again");

    // 3. 测试释放锁
    let unlocked = client
        .unlock(lock_key, lock_val)
        .await
        .expect("Failed to unlock");
    assert!(unlocked, "Should unlock successfully");

    // 4. 测试释放不存在的锁（应失败）
    let unlocked_again = client
        .unlock(lock_key, lock_val)
        .await
        .expect("Failed to call unlock");
    assert!(
        !unlocked_again,
        "Should fail to unlock already released lock"
    );

    // 5. 测试锁过期
    let _ = client
        .lock(lock_key, lock_val, 1)
        .await
        .expect("Failed to acquire lock");
    sleep(Duration::from_secs(2)).await;
    let locked_after_expire = client
        .lock(lock_key, "uuid_2", ttl)
        .await
        .expect("Failed to acquire lock");
    assert!(locked_after_expire, "Should acquire lock after expiration");

    cleanup_service(&service_name).await;
}

#[tokio::test]
async fn test_cache_preheating() {
    if !is_redis_available().await {
        println!("Skipping test_cache_preheating: Redis not available");
        return;
    }

    let service_name = generate_unique_service_name("warmup_test");

    // 手动构建 TwoLevelClient 以访问 warmup 方法
    // 注意：oxcache::get_client 返回 Arc<dyn CacheOps>，不包含 warmup 方法
    // 所以我们需要直接构建 TwoLevelClient 或将其转型

    let l1 = Arc::new(L1Backend::new(100));
    let l2_config = L2Config {
        mode: RedisMode::Standalone,
        connection_string: "redis://127.0.0.1:6379".to_string().into(),
        connection_timeout_ms: 500,
        command_timeout_ms: 500,
        sentinel: None,
        default_ttl: None,
        cluster: None,
        password: None,
        enable_tls: false,
    };
    let l2 = Arc::new(
        L2Backend::new(&l2_config)
            .await
            .expect("Failed to create L2"),
    );

    let client = TwoLevelClient::new(
        service_name.clone(),
        TwoLevelConfig::default(),
        l1,
        l2,
        SerializerEnum::Json(JsonSerializer),
    )
    .await
    .expect("Failed to create client");

    let keys = vec!["warm_1".to_string(), "warm_2".to_string()];

    // 模拟数据加载器
    let loader = |keys: Vec<String>| async move {
        let mut res = Vec::new();
        for k in keys {
            res.push((k.clone(), format!("value_of_{}", k)));
        }
        Ok(res)
    };

    // 执行预热
    client
        .warmup(keys, loader, Some(60))
        .await
        .expect("Warmup failed");

    // 验证数据
    let val1: Option<String> = client.get("warm_1").await.expect("Get failed");
    assert_eq!(val1, Some("value_of_warm_1".to_string()));

    let val2: Option<String> = client.get("warm_2").await.expect("Get failed");
    assert_eq!(val2, Some("value_of_warm_2".to_string()));

    cleanup_service(&service_name).await;
}
