// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

//! #[cached] 宏使用示例
//!
//! 本示例演示了 Oxcache 的 #[cached] 宏功能：
//! - 零样板缓存装饰
//! - 自动序列化/反序列化
//! - 灵活的键生成策略
//! - TTL 控制
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_cached_macro
//! ```

use oxcache::macros::cached;
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

// 使用 #[cached] 宏一行代码启用缓存
#[cached(service = "user_cache", ttl = 600)]
async fn get_user(id: u64) -> Result<User, String> {
    // 模拟耗时的数据库查询
    println!("   执行原始函数逻辑 (数据库查询)...");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

// 使用自定义键格式的缓存函数
#[cached(service = "user_cache", ttl = 600, key_prefix = "user")]
async fn get_user_custom_key(id: u64) -> Result<User, String> {
    println!("   执行原始函数逻辑 (自定义键)...");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("Custom Key User {}", id),
    })
}

// 使用不同缓存策略的函数
#[cached(service = "user_cache", ttl = 300, cache_type = "l1-only")]
async fn get_hot_data(id: u64) -> Result<String, String> {
    println!("   执行原始函数逻辑 (热点数据)...");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(format!("Hot Data for {}", id))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== #[cached] 宏使用示例 ===
");

    // 初始化缓存
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // 注册缓存实例到全局管理器（供宏使用）
    cache.register_for_macro("user_cache").await;
    println!("✓ 缓存已注册供 #[cached] 宏使用
");

    // 1. 基础缓存功能演示
    println!("1. 基础缓存功能演示");
    println!("   第一次调用 get_user(1) - 执行函数逻辑 + 缓存结果...");
    let user = get_user(1).await?;
    println!("   第一次调用结果: {:?}", user);

    println!("   第二次调用 get_user(1) - 直接从缓存返回...");
    let cached_user = get_user(1).await?;
    println!("   第二次调用结果: {:?}", cached_user);
    println!();

    // 2. 自定义键格式演示
    println!("2. 自定义键格式演示");
    println!("   调用 get_user_custom_key(2) - 使用 'user_' 前缀键格式...");
    let user_custom = get_user_custom_key(2).await?;
    println!("   结果: {:?}", user_custom);

    println!("   再次调用 get_user_custom_key(2) - 从缓存返回...");
    let cached_custom = get_user_custom_key(2).await?;
    println!("   缓存结果: {:?}", cached_custom);
    println!();

    // 3. L1-only 缓存策略演示
    println!("3. L1-only 缓存策略演示");
    println!("   调用 get_hot_data(1) - 使用 L1-only 策略...");
    let hot_data = get_hot_data(1).await?;
    println!("   结果: {}", hot_data);

    println!("   再次调用 get_hot_data(1) - 从 L1 缓存返回...");
    let cached_hot_data = get_hot_data(1).await?;
    println!("   缓存结果: {}", cached_hot_data);
    println!();

    // 4. 性能对比演示
    println!("4. 性能对比演示");
    println!("   缓存命中通常比原始函数执行快得多");

    // 首次调用（未缓存）
    let start = std::time::Instant::now();
    let _first_call = get_user(3).await?;
    let first_duration = start.elapsed();
    println!("   首次调用耗时: {:?}", first_duration);

    // 第二次调用（缓存命中）
    let start = std::time::Instant::now();
    let _second_call = get_user(3).await?;
    let second_duration = start.elapsed();
    println!("   缓存命中耗时: {:?}", second_duration);

    println!("   性能提升: {:.2}x", first_duration.as_millis() as f64 / second_duration.as_millis() as f64);
    println!();

    // 5. 多参数函数缓存演示
    #[cached(service = "user_cache", ttl = 600)]
    async fn get_user_with_role(user_id: u64, role: String) -> Result<String, String> {
        println!("   执行原始函数逻辑 (多参数)...");
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        Ok(format!("User {} with role {}", user_id, role))
    }


    println!("5. 多参数函数缓存演示");
    println!("   调用 get_user_with_role(4, \"admin\")...");
    let result = get_user_with_role(4, "admin".to_string()).await?;
    println!("   结果: {}", result);

    println!("   再次调用相同参数 - 从缓存返回...");
    let cached_result = get_user_with_role(4, "admin".to_string()).await?;
    println!("   缓存结果: {}", cached_result);

    println!("   调用不同参数 - 执行原始函数...");
    let diff_result = get_user_with_role(4, "user".to_string()).await?;
    println!("   不同参数结果: {}", diff_result);
    println!();

    println!("=== #[cached] 宏示例完成 ===");
    println!("   #[cached] 宏的主要优势:");
    println!("   - 零样板代码：自动处理缓存逻辑");
    println!("   - 类型安全：编译时检查");
    println!("   - 灵活配置：支持多种缓存策略");
    println!("   - 自动序列化：无需手动处理");
    println!("   - 性能优化：显著提升缓存命中性能");

    Ok(())
}
