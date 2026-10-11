// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis 原生客户端示例
//!
//! 本示例演示如何使用 Oxcache 连接 Redis。
//!
//! 运行方式：
//! ```bash
//! # 本地开发需允许非 TLS 连接（生产请使用 rediss://）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --example redis_native
//! ```
//!（连接本机 Redis 时同样需要上述环境变量）

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RedisUser {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis native client example ===\n");

    println!("1. Connect to Redis");
    let cache: Cache<String, RedisUser> = Cache::redis("redis://127.0.0.1:6379").await?;
    println!("   ✓ Redis connected\n");

    println!("2. Basic CRUD operations");
    let user = RedisUser {
        id: 1,
        name: "Alice".to_string(),
        email: "zhangsan@example.com".to_string(),
    };

    // 创建
    println!("  adding user...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ user added");

    // 读取
    println!("  getting user...");
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!("   ✓ user retrieved: {} ({})", u.name, u.email),
        None => println!("   ✗ user not found"),
    }

    // 更新
    println!("  updating user...");
    let updated_user = RedisUser {
        id: 1,
        name: "Bob".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
    };
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   ✓ user updated");

    // 删除
    println!("  deleting user...");
    cache.delete(&"user:1".to_string()).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(_) => println!("   ✗ failed to delete the user"),
        None => println!("   ✓ user deleted"),
    }
    println!();

    println!("3. Batch operations");
    let users = vec![
        RedisUser {
            id: 2,
            name: "Charlie".to_string(),
            email: "lisi@example.com".to_string(),
        },
        RedisUser {
            id: 3,
            name: "David".to_string(),
            email: "wangwu@example.com".to_string(),
        },
    ];

    println!("  adding users in batch...");
    for user in &users {
        cache.set(&format!("user:{}", user.id), user).await?;
    }
    println!("   ✓ batch add succeeded");

    println!("  fetching users in batch...");
    for user in &users {
        if let Some(u) = cache.get(&format!("user:{}", user.id)).await? {
            println!("   ✓ user {}: {}", u.id, u.name);
        }
    }

    // 清空（安全方式：仅删除本示例写入的测试键，避免全库 clear）
    println!("  clearing test data...");
    for key in ["user:1", "user:2", "user:3"] {
        cache.delete(&key.to_string()).await?;
    }
    println!("   ✓ cleared\n");

    println!("4. Cache statistics");
    println!("  cache statistics are available through the metrics interface");
    println!("  (see the metrics_test example for detailed statistics)");
    println!();

    println!("=== Redis native client example complete ===");
    Ok(())
}
