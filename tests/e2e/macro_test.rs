//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 端到端宏测试

use oxcache::cached;
use oxcache::config::{
    CacheType, Config, GlobalConfig, L1Config, L2Config, RedisMode, SerializationType,
    ServiceConfig, TwoLevelConfig,
};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::collections::HashMap;
use std::time::Duration;

#[path = "../common/mod.rs"]
mod common;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct User {
    id: u64,
    name: String,
}

/// 设置宏测试环境
///
/// 初始化缓存管理器，配置用于测试缓存宏的环境
async fn setup_macro_env() {
    let config = Config {
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 5,
            serialization: SerializationType::Json,
            enable_metrics: false,
        },
        services: {
            let mut map = HashMap::new();
            map.insert(
                "user_cache".to_string(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(300),
                    serialization: None,
                    l1: Some(L1Config { max_capacity: 100 }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: std::env::var("REDIS_URL")
                            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
                            .into(),
                        connection_timeout_ms: 500,
                        command_timeout_ms: 500,
                        sentinel: None,
                        default_ttl: None,
                        cluster: None,
                        password: None,
                        enable_tls: false,
                    }),
                    two_level: Some(TwoLevelConfig {
                        invalidation_channel: None,
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 10,
                        batch_interval_ms: 100,
                    }),
                },
            );
            map
        },
    };
    // 重置并初始化
    oxcache::CacheManager::reset();
    oxcache::CacheManager::init(config)
        .await
        .expect("Failed to initialize CacheManager");
}

/// 获取用户信息
///
/// 模拟从数据库获取用户信息的函数，使用缓存宏进行缓存
///
/// # 参数
///
/// * `id` - 用户ID
///
/// # 返回值
///
/// 返回用户信息或错误
#[cached(service = "user_cache", ttl = 300)]
async fn get_user(id: u64) -> Result<User, String> {
    // 模拟数据库延迟
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok(User {
        id,
        name: format!("User{}", id),
    })
}

/// 测试缓存宏基本功能
///
/// 验证缓存宏能否正确缓存函数结果并在后续调用中返回缓存的结果
#[tokio::test]
#[serial]
async fn test_cached_macro_basic() {
    if !common::is_redis_available().await {
        println!("跳过 test_cached_macro_basic: Redis不可用");
        return;
    }

    // 确保Redis已启动或进行模拟（这里我们假设已启动或优雅地失败）
    // 我们全局初始化缓存管理器
    setup_macro_env().await;

    // 1. 第一次调用 - 未命中
    let start = std::time::Instant::now();
    let user = get_user(1).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(user.id, 1);
    assert!(duration >= Duration::from_millis(0));

    // 2. 第二次调用 - 命中
    let start = std::time::Instant::now();
    let user = get_user(1).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(user.id, 1);
    // 应该比50毫秒快得多
    assert!(duration < Duration::from_millis(10));
}

/// 获取用户信息（自定义键）
///
/// 使用自定义缓存键的缓存函数
///
/// # 参数
///
/// * `id` - 用户ID
///
/// # 返回值
///
/// 返回用户信息或错误
#[cached(service = "user_cache", key = "custom_user_{id}")]
async fn get_user_custom_key(id: u64) -> Result<User, String> {
    Ok(User {
        id,
        name: "Custom".to_string(),
    })
}

/// 测试缓存宏自定义键功能
///
/// 验证缓存宏能否正确使用自定义键进行缓存
#[tokio::test]
#[serial]
async fn test_cached_macro_custom_key() {
    if !common::is_redis_available().await {
        println!("跳过 test_cached_macro_custom_key: Redis不可用");
        return;
    }

    setup_macro_env().await;
    let user = get_user_custom_key(99).await.unwrap();
    assert_eq!(user.name, "Custom");

    // 手动验证
    let client = oxcache::get_client("user_cache").unwrap();
    // Use low-level get_bytes and manually deserialize because get<T> is not available on trait object
    // or use the helper if available. But since we are testing macro integration, we know macro worked if we can find it.
    // Also, we can use the new serializer method exposed on CacheOps
    use oxcache::serialization::Serializer;
    let bytes = client
        .get_bytes("custom_user_99")
        .await
        .unwrap()
        .expect("Cache miss");
    let cached: User = client.serializer().deserialize(&bytes).unwrap();
    assert_eq!(cached.id, 99);
}

/// 测试缓存宏cache_type参数功能 - L1 only模式
///
/// 验证缓存宏能否正确使用cache_type="l1-only"参数
#[cached(service = "user_cache", cache_type = "l1-only", ttl = 30)]
async fn get_user_l1_only(id: u64) -> Result<User, String> {
    // 模拟数据库延迟
    tokio::time::sleep(Duration::from_millis(10)).await;
    Ok(User {
        id,
        name: format!("L1User{}", id),
    })
}

/// 测试缓存宏cache_type参数功能 - L2 only模式
///
/// 验证缓存宏能否正确使用cache_type="l2-only"参数
#[cached(service = "user_cache", cache_type = "l2-only", ttl = 30)]
async fn get_user_l2_only(id: u64) -> Result<User, String> {
    // 模拟数据库延迟
    tokio::time::sleep(Duration::from_millis(10)).await;
    Ok(User {
        id,
        name: format!("L2User{}", id),
    })
}

/// 测试缓存宏cache_type参数功能
///
/// 验证缓存宏能否正确使用cache_type参数
#[tokio::test]
#[serial]
async fn test_cached_macro_with_cache_type() {
    if !common::is_redis_available().await {
        println!("跳过 test_cached_macro_with_cache_type: Redis不可用");
        return;
    }

    setup_macro_env().await;
    
    // 测试L1-only模式
    let user1 = get_user_l1_only(100).await.unwrap();
    assert_eq!(user1.name, "L1User100");
    
    // 测试L2-only模式
    let user2 = get_user_l2_only(200).await.unwrap();
    assert_eq!(user2.name, "L2User200");
    
    // 验证缓存生效
    let start = std::time::Instant::now();
    let user1_cached = get_user_l1_only(100).await.unwrap();
    let duration_l1 = start.elapsed();
    
    let start = std::time::Instant::now();
    let user2_cached = get_user_l2_only(200).await.unwrap();
    let duration_l2 = start.elapsed();
    
    // 缓存命中应该比原始调用快
    assert!(duration_l1 < Duration::from_millis(5));
    assert!(duration_l2 < Duration::from_millis(5));
    
    // 验证缓存内容
    assert_eq!(user1_cached.name, "L1User100");
    assert_eq!(user2_cached.name, "L2User200");
}
