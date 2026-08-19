// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis 原生客户端示例
//!
//! 本示例演示如何使用 Oxcache 连接 Redis。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example redis_native
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RedisUser {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis 原生客户端示例 ===\n");

    // 创建 Redis 缓存
    println!("1. 连接 Redis");
    let cache: Cache<String, RedisUser> = Cache::redis("redis://127.0.0.1:6379").await?;
    println!("   ✓ Redis 连接成功\n");

    // 2. 基本操作
    println!("2. 基本 CRUD 操作");
    let user = RedisUser {
        id: 1,
        name: "张三".to_string(),
        email: "zhangsan@example.com".to_string(),
    };

    // 创建
    println!("   添加用户...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ 用户添加成功");

    // 读取
    println!("   获取用户...");
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!("   ✓ 用户获取成功: {} ({})", u.name, u.email),
        None => println!("   ✗ 用户未找到"),
    }

    // 更新
    println!("   更新用户...");
    let updated_user = RedisUser {
        id: 1,
        name: "张三丰".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
    };
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   ✓ 用户更新成功");

    // 删除
    println!("   删除用户...");
    cache.delete(&"user:1".to_string()).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(_) => println!("   ✗ 用户删除失败"),
        None => println!("   ✓ 用户删除成功"),
    }
    println!();

    // 3. 批量操作
    println!("3. 批量操作");
    let users = vec![
        RedisUser {
            id: 2,
            name: "李四".to_string(),
            email: "lisi@example.com".to_string(),
        },
        RedisUser {
            id: 3,
            name: "王五".to_string(),
            email: "wangwu@example.com".to_string(),
        },
    ];

    println!("   批量添加用户...");
    for user in &users {
        cache.set(&format!("user:{}", user.id), user).await?;
    }
    println!("   ✓ 批量添加成功");

    println!("   批量获取用户...");
    for user in &users {
        if let Some(u) = cache.get(&format!("user:{}", user.id)).await? {
            println!("   ✓ 用户 {}: {}", u.id, u.name);
        }
    }

    // 清空（安全方式：仅删除本示例写入的测试键，避免全库 clear）
    println!("   清空测试数据...");
    for key in ["user:1", "user:2", "user:3"] {
        cache.delete(&key.to_string()).await?;
    }
    println!("   ✓ 清空完成\n");

    // 4. 统计信息
    println!("4. 缓存统计");
    println!("   缓存统计功能需要通过 metrics 接口获取");
    println!("   (详细统计信息请参考 metrics_test 示例)");
    println!();

    println!("=== Redis 原生客户端示例完成 ===");
    Ok(())
}
