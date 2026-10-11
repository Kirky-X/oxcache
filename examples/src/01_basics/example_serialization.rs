// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 序列化选项示例：JSON（人类可读）与 Bincode（二进制、更高效）

use oxcache::Cache;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    profile: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, User> = Cache::builder().build().await?;

    println!("Serialization options example");
    println!("========================\n");

    // 使用JSON创建测试数据
    let user = User {
        id: 1,
        name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        profile: serde_json::json!({
            "age": 30,
            "city": "New York",
            "skills": ["Rust", "Redis", "Cache"]
        }),
    };

    println!("1. JSON serialization (default):");
    println!("  - human-readable format");
    println!("  - good for debugging");
    println!("  - slightly larger size\n");

    // 使用JSON存储
    cache.set(&"user:json:1".to_string(), &user).await?;
    println!("  stored user: {} - {}", user.name, user.email);

    // 检索
    if let Some(cached) = cache.get(&"user:json:1".to_string()).await? {
        println!("  retrieved: {} - {}", cached.name, cached.email);
        println!("  config: {:?}\n", cached.profile);
    }

    println!("2. Serialization features:");
    println!("  - JSON: standard, readable format");
    println!("  - Bincode: compact binary format, efficient (when enabled)");
    println!("  - supports custom serializers\n");

    println!("✓ Serialization example complete!");
    Ok(())
}
