// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 基本 CRUD 操作示例（get / set / delete）

use oxcache::Cache;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建一个简单的内存缓存
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // 创建测试用户
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    // 在缓存中设置值
    println!("Set user: {}", user.name);
    cache.set(&"user:1".to_string(), &user).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_some());

    // 从缓存中获取值
    println!("Getting user...");
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!("Got user: {} ({})", cached_user.name, cached_user.email);
        assert_eq!(cached_user.id, 1);
    }

    // 从缓存中删除值
    println!("Deleting user...");
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());

    // 更新值
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("Updating user...");
    cache.set(&"user:1".to_string(), &updated_user).await?;
    if let Some(cached) = cache.get(&"user:1".to_string()).await? {
        println!("Updated user: {} ({})", cached.name, cached.email);
    }

    println!("\n✓ Basic operations example complete!");
    println!("  - Set: store a value in the cache");
    println!("  - Get: read a value from the cache");
    println!("  - Delete: remove a value from the cache");
    println!("  - Update: overwrite the existing value");
    Ok(())
}
