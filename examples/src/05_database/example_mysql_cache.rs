//! MySQL 数据库缓存示例
//!
//! 本示例演示如何使用 Oxcache 缓存 MySQL 查询结果。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_mysql_cache
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    created_at: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MySQL 数据库缓存示例 ===\n");

    // 创建分层缓存 (L1: 内存, L2: Redis)
    println!("创建分层缓存...");
    let cache: Cache<String, User> = Cache::tiered(1000, "redis://127.0.0.1:6379").await?;
    println!("✓ 缓存创建成功\n");

    // 模拟用户数据
    println!("1. 模拟用户数据");
    let users = vec![
        User {
            id: 1,
            name: "张三".to_string(),
            email: "zhangsan@example.com".to_string(),
            created_at: "2024-01-01".to_string(),
        },
        User {
            id: 2,
            name: "李四".to_string(),
            email: "lisi@example.com".to_string(),
            created_at: "2024-01-02".to_string(),
        },
        User {
            id: 3,
            name: "王五".to_string(),
            email: "wangwu@example.com".to_string(),
            created_at: "2024-01-03".to_string(),
        },
    ];

    println!("   添加用户到缓存...");
    for user in &users {
        cache
            .set(&format!("user:{}", user.id), user, Some(3600))
            .await?;
        println!("   ✓ 用户 {}: {}", user.id, user.name);
    }
    println!();

    // 模拟查询
    println!("2. 模拟数据库查询 (带缓存)");
    for user_id in [1, 2, 3, 1, 2] {
        let key = format!("user:{}", user_id);
        let start = std::time::Instant::now();
        let user = cache.get(&key).await?;
        let elapsed = start.elapsed();

        match user {
            Some(u) => println!(
                "   ✓ 用户 {}: {} (耗时: {:?})",
                u.id, u.name, elapsed
            ),
            None => println!("   ✗ 用户 {} 未找到", user_id),
        }
    }
    println!();

    // 统计信息
    println!("3. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    if stats.hit_count() + stats.miss_count() > 0 {
        let hit_rate = stats.hit_count() as f64
            / (stats.hit_count() + stats.miss_count()) as f64
            * 100.0;
        println!("   - 命中率: {:.2}%", hit_rate);
    }
    println!();

    // 清理
    println!("4. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== MySQL 数据库缓存示例完成 ===");
    Ok(())
}