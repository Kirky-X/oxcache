// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

//! 缓存失效策略示例
//!
//! 本示例演示了 Oxcache 的各种缓存失效策略：
//! - 单个 key 失效
//! - 批量失效
//! - 模式匹配失效
//! - 基于时间的失效
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_invalidation
//!

use oxcache::Cache;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Order {
    id: u64,
    user_id: u64,
    product: String,
    quantity: u32,
    total: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "=== 缓存失效策略示例 ===
"
    );

    // 创建分层缓存用于演示
    let cache: Arc<Cache<String, Order>> = Arc::new(Cache::builder().build().await?);

    // 1. 准备测试数据
    println!("1. 准备测试数据");
    let orders = vec![
        Order {
            id: 1,
            user_id: 100,
            product: "笔记本电脑".to_string(),
            quantity: 1,
            total: 5999.99,
        },
        Order {
            id: 2,
            user_id: 100,
            product: "鼠标".to_string(),
            quantity: 2,
            total: 199.98,
        },
        Order {
            id: 3,
            user_id: 200,
            product: "键盘".to_string(),
            quantity: 1,
            total: 299.99,
        },
    ];

    for order in &orders {
        cache.set(&format!("order:{}", order.id), order).await?;
    }
    println!(
        "   ✓ 添加了 {} 个订单
",
        orders.len()
    );

    // 2. 单个 key 失效
    println!("2. 单个 key 失效");
    println!("   删除 order:1");
    cache.delete(&"order:1".to_string()).await?;
    let result = cache.get(&"order:1".to_string()).await?;
    match result {
        Some(_) => println!("   ✗ 订单仍然存在"),
        None => println!("   ✓ 订单已删除"),
    }
    println!();

    // 3. 批量失效
    println!("3. 批量失效 (通过清空缓存)");
    // 清空整个缓存
    cache.clear().await?;
    let remaining = cache.get(&"order:2".to_string()).await?;
    match remaining {
        Some(_) => println!("   ✗ order:2 仍然存在"),
        None => println!("   ✓ 所有订单已清空"),
    }
    println!();

    // 4. 基于用户 ID 的失效模式
    println!("4. 基于用户 ID 的失效模式");
    let user_cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await?);

    // 添加用户 100 的多个购物车项目
    user_cache
        .set(&"cart:100:item1".to_string(), &"笔记本电脑".to_string())
        .await?;
    user_cache
        .set(&"cart:100:item2".to_string(), &"鼠标".to_string())
        .await?;
    user_cache
        .set(&"cart:200:item1".to_string(), &"键盘".to_string())
        .await?;

    println!("   原始购物车:");
    println!(
        "     cart:100:item1 = {:?}",
        user_cache.get(&"cart:100:item1".to_string()).await?
    );
    println!(
        "     cart:100:item2 = {:?}",
        user_cache.get(&"cart:100:item2".to_string()).await?
    );
    println!(
        "     cart:200:item1 = {:?}",
        user_cache.get(&"cart:200:item1".to_string()).await?
    );

    // 模拟用户 100 结账，需要清空其购物车
    // 注意：Oxcache 没有直接的模式匹配删除，需要手动遍历
    println!(
        "
   用户 100 结账，清空其购物车..."
    );

    // 实际应用中应该维护 key 的索引列表
    // 这里演示概念，实际需要应用层维护关联
    user_cache.delete(&"cart:100:item1".to_string()).await?;
    user_cache.delete(&"cart:100:item2".to_string()).await?;

    println!("   清空后:");
    println!(
        "     cart:100:item1 = {:?}",
        user_cache.get(&"cart:100:item1".to_string()).await?
    );
    println!(
        "     cart:200:item1 = {:?}",
        user_cache.get(&"cart:200:item1".to_string()).await?
    );
    println!();

    // 5. TTL 失效
    println!("5. TTL 自动失效");
    let ttl_cache: Cache<String, String> = Cache::builder().build().await?;

    println!("   添加 2 秒过期的数据");
    ttl_cache
        .set_with_ttl(
            &"temp:data".to_string(),
            &"临时数据".to_string(),
            Some(Duration::from_secs(2)),
        )
        .await?;

    println!("   立即获取: {:?}", ttl_cache.get(&"temp:data".to_string()).await?);

    println!("   等待 3 秒...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    println!(
        "   3 秒后获取: {:?}
",
        ttl_cache.get(&"temp:data".to_string()).await?
    );

    // 6. 更新时失效 (Write-Invalidation)
    println!("6. 更新时失效 (Write-Invalidation)");
    let cache: Cache<String, String> = Cache::builder().build().await?;

    cache.set(&"config:theme".to_string(), &"dark".to_string()).await?;
    println!("   初始主题: {:?}", cache.get(&"config:theme".to_string()).await?);

    // 更新配置时直接覆盖旧值
    cache.set(&"config:theme".to_string(), &"light".to_string()).await?;
    println!("   更新后主题: {:?}", cache.get(&"config:theme".to_string()).await?);

    println!();
    println!("=== 失效策略示例完成 ===");
    Ok(())
}
