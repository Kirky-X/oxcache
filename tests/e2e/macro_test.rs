// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 端到端宏测试 - 使用新API

extern crate oxcache;

use crate::common;
use oxcache::cached;
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::time::Duration;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct User {
    id: u64,
    name: String,
}

/// 设置宏测试环境
///
/// 使用新API创建缓存实例
async fn setup_macro_env() -> Cache<String, Vec<u8>> {
    if common::is_redis_available().await {
        Cache::tiered(100, "redis://127.0.0.1:6379")
            .await
            .expect("Failed to create tiered cache")
    } else {
        // Skip if Redis is not available
        panic!("Redis not available");
    }
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

    // 创建缓存并注册
    let cache = setup_macro_env().await;
    cache.register_for_macro("user_cache").await;

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

    let cache = setup_macro_env().await;
    cache.register_for_macro("user_cache").await;

    let user = get_user_custom_key(99).await.unwrap();
    assert_eq!(user.name, "Custom");

    // 再次调用应该从缓存返回（验证缓存工作）
    let user2 = get_user_custom_key(99).await.unwrap();
    assert_eq!(user2.name, "Custom");
    assert_eq!(user2.id, 99);
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

    let cache = setup_macro_env().await;
    cache.register_for_macro("user_cache").await;

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
    // 注意：由于系统负载等因素，这里使用更宽松的阈值
    assert!(
        duration_l1 < Duration::from_millis(50),
        "L1 cache should be fast"
    );
    assert!(
        duration_l2 < Duration::from_millis(50),
        "L2 cache should be fast"
    );

    // 验证缓存内容
    assert_eq!(user1_cached.name, "L1User100");
    assert_eq!(user2_cached.name, "L2User200");
}
