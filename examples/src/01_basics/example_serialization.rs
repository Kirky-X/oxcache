// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 序列化选项示例
//
// 本示例演示不同的序列化选项:
// - JSON序列化 (人类可读)
// - Bincode序列化 (二进制，更高效)

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

    println!("序列化选项示例");
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

    println!("1. JSON序列化 (默认):");
    println!("   - 人类可读格式");
    println!("   - 适合调试");
    println!("   - 略微更大的尺寸\n");

    // 使用JSON存储
    cache.set(&"user:json:1".to_string(), &user).await?;
    println!("   存储的用户: {} - {}", user.name, user.email);

    // 检索
    if let Some(cached) = cache.get(&"user:json:1".to_string()).await? {
        println!("   检索到: {} - {}", cached.name, cached.email);
        println!("   配置文件: {:?}\n", cached.profile);
    }

    println!("2. 序列化特性:");
    println!("   - JSON: 标准格式，可读");
    println!("   - Bincode: 二进制格式，高效 (启用时)");
    println!("   - 支持自定义序列化器\n");

    println!("✓ 序列化示例完成!");
    Ok(())
}
