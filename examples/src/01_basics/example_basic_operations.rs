// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 基本CRUD操作示例
//
// 本示例演示基本的缓存操作:
// - Get: 获取缓存值
// - Set: 在缓存中存储值
// - Delete: 从缓存中删除值

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
    println!("设置用户: {}", user.name);
    cache.set(&"user:1".to_string(), &user).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_some());

    // 从缓存中获取值
    println!("获取用户...");
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!(
            "获取的用户: {} ({})",
            cached_user.name, cached_user.email
        );
        assert_eq!(cached_user.id, 1);
    }

    // 从缓存中删除值
    println!("删除用户...");
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());

    // 更新值
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("更新用户...");
    cache.set(&"user:1".to_string(), &updated_user).await?;
    if let Some(cached) = cache.get(&"user:1".to_string()).await? {
        println!("更新的用户: {} ({})", cached.name, cached.email);
    }

    println!("\n✓ 基本操作示例完成!");
    println!("  - Set: 在缓存中存储值");
    println!("  - Get: 从缓存中获取值");
    println!("  - Delete: 从缓存中删除值");
    println!("  - Update: 覆盖现有值");
    Ok(())
}
