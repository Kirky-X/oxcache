//! Redis Standalone 模式示例
//!
//! 本示例演示如何使用 Oxcache 连接 Redis Standalone 模式。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_standalone
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis Standalone 模式示例 ===\n");

    // 创建 Redis 缓存
    println!("创建 Redis 缓存连接...");
    let cache: Cache<String, User> = Cache::redis("redis://127.0.0.1:6379").await?;
    println!("✓ Redis 连接成功\n");

    // 基本操作
    println!("1. 基本 CRUD 操作");
    let user = User {
        id: 1,
        name: "张三".to_string(),
        email: "zhangsan@example.com".to_string(),
    };

    // 创建
    println!("   添加用户...");
    cache.set("user:1", &user, Some(3600)).await?;
    println!("   ✓ 用户添加成功");

    // 读取
    println!("   获取用户...");
    let retrieved = cache.get("user:1").await?;
    match retrieved {
        Some(u) => println!("   ✓ 用户获取成功: {:?}", u.name),
        None => println!("   ✗ 用户未找到"),
    }

    // 更新
    println!("   更新用户...");
    let updated_user = User {
        id: 1,
        name: "张三丰".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
    };
    cache.set("user:1", &updated_user, Some(3600)).await?;
    println!("   ✓ 用户更新成功");

    // 验证更新
    let retrieved = cache.get("user:1").await?;
    match retrieved {
        Some(u) => println!("   ✓ 更新后用户: {:?}", u.name),
        None => println!("   ✗ 用户未找到"),
    }

    // 删除
    println!("   删除用户...");
    cache.delete("user:1").await?;
    let retrieved = cache.get("user:1").await?;
    match retrieved {
        Some(_) => println!("   ✗ 用户删除失败"),
        None => println!("   ✓ 用户删除成功"),
    }
    println!();

    // 批量操作
    println!("2. 批量操作");
    let users = vec![
        User {
            id: 2,
            name: "李四".to_string(),
            email: "lisi@example.com".to_string(),
        },
        User {
            id: 3,
            name: "王五".to_string(),
            email: "wangwu@example.com".to_string(),
        },
    ];

    println!("   批量添加用户...");
    for user in &users {
        cache
            .set(&format!("user:{}", user.id), user, Some(3600))
            .await?;
    }
    println!("   ✓ 批量添加成功");

    println!("   批量获取用户...");
    for user in &users {
        if let Some(u) = cache.get(&format!("user:{}", user.id)).await? {
            println!("   ✓ 用户 {}: {}", u.id, u.name);
        }
    }

    // 清空测试数据
    println!("   清空测试数据...");
    cache.clear().await?;
    println!("   ✓ 清空完成\n");

    println!("=== Redis Standalone 模式示例完成 ===");
    Ok(())
}