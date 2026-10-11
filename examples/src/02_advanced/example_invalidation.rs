// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
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
        "=== Cache invalidation strategy example ===
"
    );

    // 创建分层缓存用于演示
    let cache: Arc<Cache<String, Order>> = Arc::new(Cache::builder().build().await?);

    println!("1. Prepare test data");
    let orders = vec![
        Order {
            id: 1,
            user_id: 100,
            product: "laptop".to_string(),
            quantity: 1,
            total: 5999.99,
        },
        Order {
            id: 2,
            user_id: 100,
            product: "mouse".to_string(),
            quantity: 2,
            total: 199.98,
        },
        Order {
            id: 3,
            user_id: 200,
            product: "keyboard".to_string(),
            quantity: 1,
            total: 299.99,
        },
    ];

    for order in &orders {
        cache.set(&format!("order:{}", order.id), order).await?;
    }
    println!(
        "   ✓ added {} orders
",
        orders.len()
    );

    println!("2. Single-key invalidation");
    println!("  Deleting order:1");
    cache.delete(&"order:1".to_string()).await?;
    let result = cache.get(&"order:1".to_string()).await?;
    match result {
        Some(_) => println!("   ✗ the order still exists"),
        None => println!("   ✓ the order was deleted"),
    }
    println!();

    println!("3. Bulk invalidation (by clearing the cache)");
    // 清空整个缓存
    cache.clear().await?;
    let remaining = cache.get(&"order:2".to_string()).await?;
    match remaining {
        Some(_) => println!("   ✗ order:2 still exists"),
        None => println!("   ✓ all orders cleared"),
    }
    println!();

    println!("4. User-ID-based invalidation patterns");
    let user_cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await?);

    // 添加用户 100 的多个购物车项目
    user_cache
        .set(&"cart:100:item1".to_string(), &"laptop".to_string())
        .await?;
    user_cache
        .set(&"cart:100:item2".to_string(), &"mouse".to_string())
        .await?;
    user_cache
        .set(&"cart:200:item1".to_string(), &"keyboard".to_string())
        .await?;

    println!("  Original cart:");
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
   user 100 checks out, clearing their cart..."
    );

    // 实际应用中应该维护 key 的索引列表
    // 这里演示概念，实际需要应用层维护关联
    user_cache.delete(&"cart:100:item1".to_string()).await?;
    user_cache.delete(&"cart:100:item2".to_string()).await?;

    println!("  After clear:");
    println!(
        "     cart:100:item1 = {:?}",
        user_cache.get(&"cart:100:item1".to_string()).await?
    );
    println!(
        "     cart:200:item1 = {:?}",
        user_cache.get(&"cart:200:item1".to_string()).await?
    );
    println!();

    println!("5. TTL-based automatic invalidation");
    let ttl_cache: Cache<String, String> = Cache::builder().build().await?;

    println!("  Adding data that expires in 2 seconds");
    ttl_cache
        .set_with_ttl(
            &"temp:data".to_string(),
            &"temporary data".to_string(),
            Some(Duration::from_secs(2)),
        )
        .await?;

    println!(
        "  immediate get: {:?}",
        ttl_cache.get(&"temp:data".to_string()).await?
    );

    println!("  Waiting 3 seconds...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    println!(
        "   get after 3 seconds: {:?}
",
        ttl_cache.get(&"temp:data".to_string()).await?
    );

    println!("6. Write-time invalidation (write-invalidation)");
    let cache: Cache<String, String> = Cache::builder().build().await?;

    cache
        .set(&"config:theme".to_string(), &"dark".to_string())
        .await?;
    println!(
        "  Initial theme: {:?}",
        cache.get(&"config:theme".to_string()).await?
    );

    // 更新配置时直接覆盖旧值
    cache
        .set(&"config:theme".to_string(), &"light".to_string())
        .await?;
    println!(
        "  Updated theme: {:?}",
        cache.get(&"config:theme".to_string()).await?
    );

    println!();
    println!("=== Invalidation strategy example complete ===");
    Ok(())
}
