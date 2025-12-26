//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! Comprehensive example demonstrating basic usage, manual control, and serialization.

// Import common module
#[path = "common/mod.rs"]
mod common;

use oxcache::{config::SerializationType, get_client, CacheExt, CacheManager};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct AppConfig {
    theme: String,
    max_retries: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct LargeData {
    data: Vec<u8>,
}

// 模拟数据库查询
async fn fetch_user_from_db(id: u64) -> Result<User, String> {
    println!("Fetching user {} from database...", id);
    sleep(Duration::from_millis(100)).await; // 模拟延迟
    Ok(User {
        id,
        name: format!("User_{}", id),
        email: format!("user{}@example.com", id),
    })
}

// 模拟带缓存的函数（由于宏不可用，使用手动缓存逻辑）
async fn get_user(id: u64) -> Result<User, String> {
    let client = get_client("default_service").expect("Default service not found");

    // 先尝试从缓存获取
    if let Ok(Some(cached_user)) = client.get::<User>(&format!("user_{}", id)).await {
        return Ok(cached_user);
    }

    // 缓存未命中，从数据库获取
    let user = fetch_user_from_db(id).await?;

    // 存入缓存
    let _ = client.set(&format!("user_{}", id), &user, Some(60)).await;

    Ok(user)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Oxcache Comprehensive Example ---\n");

    // 1. 初始化配置
    // 我们创建两个服务配置：
    // - "default_service": 使用默认 JSON 序列化，用于基本演示和宏
    // - "binary_service": 使用 Bincode 序列化，用于演示大数据或二进制数据
    let mut config = common::create_default_config("default_service", 1000);

    // 添加第二个服务配置
    if let Some(mut service_config) = config.services.get("default_service").cloned() {
        // 修改为 Bincode 序列化
        service_config.serialization = Some(SerializationType::Bincode);
        config
            .services
            .insert("binary_service".to_string(), service_config);
    }

    println!("Initializing CacheManager...");
    if let Err(e) = CacheManager::init(config).await {
        eprintln!(
            "Failed to initialize cache manager: {}. Check if Redis is running.",
            e
        );
        // 如果连接失败，我们这里选择退出，或者可以演示降级策略
        // return Err(e.into());
        println!("Continuing with potential limitations...");
    }

    // === Part 1: 基本用法与宏 ===
    println!("\n=== Part 1: Basic Usage & Macro ===");

    // 第一次调用：缓存未命中
    println!("1. First Call (Cache Miss):");
    let start = std::time::Instant::now();
    let user1 = get_user(1).await?;
    println!("   Result: {:?}", user1);
    println!("   Time: {:?}", start.elapsed());

    // 第二次调用：缓存命中
    println!("2. Second Call (Cache Hit):");
    let start = std::time::Instant::now();
    let user2 = get_user(1).await?;
    println!("   Result: {:?}", user2);
    println!("   Time: {:?}", start.elapsed());

    assert_eq!(user1, user2);

    // === Part 2: 手动控制 (L1/L2) ===
    println!("\n=== Part 2: Manual Control (L1/L2) ===");
    let client = get_client("default_service").expect("Default service not found");

    let app_config = AppConfig {
        theme: "dark".to_string(),
        max_retries: 5,
    };

    // 仅写入 L1
    println!("1. Writing to L1 only (local session data)...");
    client
        .set_l1_only("local_session", &"temp_data", Some(60))
        .await?;
    let val: Option<String> = client.get("local_session").await?;
    println!("   Read from L1: {:?}", val);

    // 仅写入 L2
    println!("2. Writing to L2 only (shared config)...");
    client
        .set_l2_only("global_config", &app_config, Some(3600))
        .await?;
    // 读取 (会从 L2 拉取并回填 L1)
    let fetched_config: Option<AppConfig> = client.get("global_config").await?;
    println!("   Read from L2: {:?}", fetched_config);

    // 标准 Set (同时写 L1 和 L2)
    println!("3. Writing to both L1 and L2...");
    client.set("shared_key", &"shared_value", None).await?;
    let val: Option<String> = client.get("shared_key").await?;
    println!("   Read value: {:?}", val);

    // 删除
    println!("4. Deleting 'shared_key'...");
    client.delete("shared_key").await?;
    let val: Option<String> = client.get("shared_key").await?;
    println!("   Value after delete: {:?}", val);

    // === Part 3: 自定义序列化 (Bincode) ===
    println!("\n=== Part 3: Serialization (Bincode) ===");
    let bin_client = get_client("binary_service").expect("Binary service not found");

    let data = LargeData {
        data: vec![1, 2, 3, 4, 5, 255, 0],
    };

    println!("1. Setting data with Bincode serialization...");
    bin_client.set("bin_key", &data, None).await?;

    let retrieved: Option<LargeData> = bin_client.get("bin_key").await?;
    println!("   Retrieved: {:?}", retrieved);
    assert_eq!(Some(data), retrieved);

    // 等待异步任务完成
    sleep(Duration::from_millis(200)).await;

    println!("\nComprehensive example finished successfully.");
    Ok(())
}
