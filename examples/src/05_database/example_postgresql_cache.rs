//! PostgreSQL 数据库缓存示例
//!
//! 本示例演示如何使用 Oxcache 缓存 PostgreSQL 查询结果。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_postgresql_cache
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Order {
    id: u64,
    user_id: u64,
    product: String,
    quantity: u32,
    total: f64,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PostgreSQL 数据库缓存示例 ===\n");

    // 创建分层缓存
    println!("创建分层缓存...");
    let cache: Cache<String, Order> = Cache::tiered(1000, "redis://127.0.0.1:6379").await?;
    println!("✓ 缓存创建成功\n");

    // 模拟订单数据
    println!("1. 模拟订单数据");
    let orders = vec![
        Order {
            id: 1001,
            user_id: 1,
            product: "笔记本电脑".to_string(),
            quantity: 1,
            total: 5999.99,
            status: "completed".to_string(),
        },
        Order {
            id: 1002,
            user_id: 1,
            product: "鼠标".to_string(),
            quantity: 2,
            total: 199.98,
            status: "completed".to_string(),
        },
        Order {
            id: 1003,
            user_id: 2,
            product: "键盘".to_string(),
            quantity: 1,
            total: 499.99,
            status: "pending".to_string(),
        },
    ];

    println!("   添加订单到缓存...");
    for order in &orders {
        cache
            .set(&format!("order:{}", order.id), order, Some(7200))
            .await?;
        println!(
            "   ✓ 订单 #{}: {} x{} (¥{:.2}) - {}",
            order.id, order.product, order.quantity, order.total, order.status
        );
    }
    println!();

    // 模拟订单查询
    println!("2. 模拟订单查询 (带缓存)");
    let query_order_ids = [1001, 1002, 1003, 1001, 1002];

    for order_id in query_order_ids {
        let key = format!("order:{}", order_id);
        let start = std::time::Instant::new();
        let order = cache.get(&key).await?;
        let elapsed = start.elapsed();

        match order {
            Some(o) => println!(
                "   ✓ 订单 #{}: {} (¥{:.2}, {}) - 耗时: {:?}",
                o.id, o.product, o.total, o.status, elapsed
            ),
            None => println!("   ✗ 订单 #{} 未找到", order_id),
        }
    }
    println!();

    // 模拟用户订单查询
    println!("3. 模拟用户订单查询");
    let user_id = 1;
    println!("   查询用户 {} 的订单...", user_id);

    let start = std::time::Instant::new();
    let mut user_orders = Vec::new();
    for order in &orders {
        if order.user_id == user_id {
            if let Some(o) = cache.get(&format!("order:{}", order.id)).await? {
                user_orders.push(o);
            }
        }
    }
    let elapsed = start.elapsed();

    println!("   ✓ 找到 {} 个订单，耗时: {:?}", user_orders.len(), elapsed);
    for order in &user_orders {
        println!("     #{}: {} x{} (¥{:.2})", order.id, order.product, order.quantity, order.total);
    }
    println!();

    // 统计信息
    println!("4. 缓存统计");
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
    println!("5. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== PostgreSQL 数据库缓存示例完成 ===");
    Ok(())
}