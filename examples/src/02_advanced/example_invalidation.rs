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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_INVALIDATION_1_PREPARE_TEST_DATA,
    MSG_EXAMPLE_INVALIDATION_2_SINGLE_KEY_INVALIDATION,
    MSG_EXAMPLE_INVALIDATION_3_BULK_INVALIDATION_BY, MSG_EXAMPLE_INVALIDATION_4_USER_ID_BASED,
    MSG_EXAMPLE_INVALIDATION_5_TTL_BASED_AUTOMATIC,
    MSG_EXAMPLE_INVALIDATION_6_WRITE_TIME_INVALIDATION, MSG_EXAMPLE_INVALIDATION_ADDED_ORDERS,
    MSG_EXAMPLE_INVALIDATION_ADDING_DATA_THAT_EXPIRES, MSG_EXAMPLE_INVALIDATION_AFTER_CLEAR,
    MSG_EXAMPLE_INVALIDATION_ALL_ORDERS_CLEARED,
    MSG_EXAMPLE_INVALIDATION_CACHE_INVALIDATION_STRATEGY_EXAMPLE,
    MSG_EXAMPLE_INVALIDATION_DELETING_ORDER_1, MSG_EXAMPLE_INVALIDATION_GET_AFTER_3_SECONDS,
    MSG_EXAMPLE_INVALIDATION_IMMEDIATE_GET, MSG_EXAMPLE_INVALIDATION_INITIAL_THEME,
    MSG_EXAMPLE_INVALIDATION_ORDER_2_STILL_EXISTS, MSG_EXAMPLE_INVALIDATION_ORIGINAL_CART,
    MSG_EXAMPLE_INVALIDATION_STRATEGY_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_INVALIDATION_THE_ORDER_STILL_EXISTS,
    MSG_EXAMPLE_INVALIDATION_THE_ORDER_WAS_DELETED, MSG_EXAMPLE_INVALIDATION_UPDATED_THEME,
    MSG_EXAMPLE_INVALIDATION_USER_100_CHECKS_OUT, MSG_EXAMPLE_INVALIDATION_WAITING_3_SECONDS, t,
};

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
        "{}\n",
        t(
            MSG_EXAMPLE_INVALIDATION_CACHE_INVALIDATION_STRATEGY_EXAMPLE,
            &[]
        )
    );

    // 创建分层缓存用于演示
    let cache: Arc<Cache<String, Order>> = Arc::new(Cache::builder().build().await?);

    println!("{}", t(MSG_EXAMPLE_INVALIDATION_1_PREPARE_TEST_DATA, &[]));
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
        "   {}\n",
        t(
            MSG_EXAMPLE_INVALIDATION_ADDED_ORDERS,
            &[("orders", orders.len().to_string())]
        )
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_INVALIDATION_2_SINGLE_KEY_INVALIDATION, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_INVALIDATION_DELETING_ORDER_1, &[]));
    cache.delete(&"order:1".to_string()).await?;
    let result = cache.get(&"order:1".to_string()).await?;
    match result {
        Some(_) => println!(
            "   {}",
            t(MSG_EXAMPLE_INVALIDATION_THE_ORDER_STILL_EXISTS, &[])
        ),
        None => println!(
            "   {}",
            t(MSG_EXAMPLE_INVALIDATION_THE_ORDER_WAS_DELETED, &[])
        ),
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_INVALIDATION_3_BULK_INVALIDATION_BY, &[])
    );
    // 清空整个缓存
    cache.clear().await?;
    let remaining = cache.get(&"order:2".to_string()).await?;
    match remaining {
        Some(_) => println!(
            "   {}",
            t(MSG_EXAMPLE_INVALIDATION_ORDER_2_STILL_EXISTS, &[])
        ),
        None => println!("   {}", t(MSG_EXAMPLE_INVALIDATION_ALL_ORDERS_CLEARED, &[])),
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_INVALIDATION_4_USER_ID_BASED, &[]));
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

    println!("  {}", t(MSG_EXAMPLE_INVALIDATION_ORIGINAL_CART, &[]));
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
        "\n   {}",
        t(MSG_EXAMPLE_INVALIDATION_USER_100_CHECKS_OUT, &[])
    );

    // 实际应用中应该维护 key 的索引列表
    // 这里演示概念，实际需要应用层维护关联
    user_cache.delete(&"cart:100:item1".to_string()).await?;
    user_cache.delete(&"cart:100:item2".to_string()).await?;

    println!("  {}", t(MSG_EXAMPLE_INVALIDATION_AFTER_CLEAR, &[]));
    println!(
        "     cart:100:item1 = {:?}",
        user_cache.get(&"cart:100:item1".to_string()).await?
    );
    println!(
        "     cart:200:item1 = {:?}",
        user_cache.get(&"cart:200:item1".to_string()).await?
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_INVALIDATION_5_TTL_BASED_AUTOMATIC, &[]));
    let ttl_cache: Cache<String, String> = Cache::builder().build().await?;

    println!(
        "  {}",
        t(MSG_EXAMPLE_INVALIDATION_ADDING_DATA_THAT_EXPIRES, &[])
    );
    ttl_cache
        .set_with_ttl(
            &"temp:data".to_string(),
            &"temporary data".to_string(),
            Some(Duration::from_secs(2)),
        )
        .await?;

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_INVALIDATION_IMMEDIATE_GET,
            &[(
                "await",
                format!("{:?}", ttl_cache.get(&"temp:data".to_string()).await?)
            )]
        )
    );

    println!("  {}", t(MSG_EXAMPLE_INVALIDATION_WAITING_3_SECONDS, &[]));
    tokio::time::sleep(Duration::from_secs(3)).await;

    println!(
        "   {}\n",
        t(
            MSG_EXAMPLE_INVALIDATION_GET_AFTER_3_SECONDS,
            &[(
                "await",
                format!("{:?}", ttl_cache.get(&"temp:data".to_string()).await?)
            )]
        )
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_INVALIDATION_6_WRITE_TIME_INVALIDATION, &[])
    );
    let cache: Cache<String, String> = Cache::builder().build().await?;

    cache
        .set(&"config:theme".to_string(), &"dark".to_string())
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_INVALIDATION_INITIAL_THEME,
            &[(
                "await",
                format!("{:?}", cache.get(&"config:theme".to_string()).await?)
            )]
        )
    );

    cache
        .set(&"config:theme".to_string(), &"light".to_string())
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_INVALIDATION_UPDATED_THEME,
            &[(
                "await",
                format!("{:?}", cache.get(&"config:theme".to_string()).await?)
            )]
        )
    );

    println!();
    println!(
        "{}",
        t(MSG_EXAMPLE_INVALIDATION_STRATEGY_EXAMPLE_COMPLETE, &[])
    );
    Ok(())
}
