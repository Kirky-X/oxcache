// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 综合使用示例 - 展示 Oxcache 的完整功能
//!
//! 本示例演示了 Oxcache 的各种功能，包括：
//! - 创建不同类型的缓存
//! - 基本 CRUD 操作
//! - TTL 控制
//! - 统计信息获取
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_comprehensive_usage
//! ```

use oxcache::Cache;
use oxcache::error::OxCacheResult;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_1_CREATE_AN_IN,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_2_CREATE_A_REDIS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_3_CREATE_A_TIERED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_4_BASIC_CRUD_OPERATIONS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_5_BATCH_OPERATIONS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_6_TTL_CONTROL,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_7_PERFORMANCE_TEST,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_8_STATISTICS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_DATA_THAT_EXPIRES,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_PRODUCTS_IN_BATCH,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_USER,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_ADD_SUCCEEDED_PRODUCTS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_DELETE_SUCCEEDED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_FETCH_SUCCEEDED_PRODUCTS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_CACHE_STATS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_PRODUCTS_IN_BATCH,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_USER,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_EXAMPLE_RUN_COMPLETE,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_FAILED_TO_DELETE_THE,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_FETCHING_PRODUCTS_IN_BATCH,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_GET_AFTER_4_SECONDS,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_GETTING_USER, MSG_EXAMPLE_COMPREHENSIVE_USAGE_IMMEDIATE_GET,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_IN_MEMORY_CACHE_CREATED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_OXCACHE_COMPREHENSIVE_USAGE_EXAMPLE,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_READ_TOOK,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_READING_10000_ENTRIES_CONCURRENTLY,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_CACHE_CREATED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_UNAVAILABLE_DEGRADED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_TIERED_CACHE_CREATED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_UPDATING_USER, MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_ADDED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_AFTER_THE_UPDATE,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_DELETED, MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND_2,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_RETRIEVED, MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_UPDATED,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_WAITING_4_SECONDS_THEN,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITE_TOOK,
    MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITING_10000_ENTRIES_CONCURRENTLY, t,
};

// 简单的用户结构体
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    age: u32,
}

// 商品结构体
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Product {
    id: u64,
    name: String,
    price: f64,
    description: String,
}

#[tokio::main]
async fn main() -> OxCacheResult<()> {
    println!(
        "{}\n",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_OXCACHE_COMPREHENSIVE_USAGE_EXAMPLE,
            &[]
        )
    );

    println!("{}", t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_1_CREATE_AN_IN, &[]));
    let _memory_cache: Cache<String, User> = Cache::builder().build().await?;
    println!(
        "   {}\n",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_IN_MEMORY_CACHE_CREATED, &[])
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_2_CREATE_A_REDIS, &[])
    );
    match Cache::<String, User>::redis("redis://127.0.0.1:6379").await {
        Ok(_redis_cache) => println!(
            "   {}",
            t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_CACHE_CREATED, &[])
        ),
        Err(e) => println!(
            "   {}",
            t(
                MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_UNAVAILABLE_DEGRADED,
                &[("e", e.to_string())]
            )
        ),
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_3_CREATE_A_TIERED, &[])
    );
    let _tiered_cache: Cache<String, User> = Cache::builder().build().await?;
    println!(
        "   {}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_TIERED_CACHE_CREATED, &[])
    );
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_4_BASIC_CRUD_OPERATIONS, &[])
    );
    let cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "zhangsan@example.com".to_string(),
        age: 28,
    };

    println!("  {}", t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_USER, &[]));
    cache.set(&"user:1".to_string(), &user).await?;
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_ADDED,
            &[("name", format!("{:?}", user.name))]
        )
    );

    println!("  {}", t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_GETTING_USER, &[]));
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!(
            "   {}",
            t(
                MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_RETRIEVED,
                &[("name", format!("{:?}", u.name))]
            )
        ),
        None => println!(
            "   {}",
            t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND, &[])
        ),
    }

    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_UPDATING_USER, &[])
    );
    let updated_user = User {
        id: 1,
        name: "Bob".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
        age: 30,
    };
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!(
        "   {}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_UPDATED, &[])
    );

    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!(
            "   {}",
            t(
                MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_AFTER_THE_UPDATE,
                &[("name", format!("{:?}", u.name))]
            )
        ),
        None => println!(
            "   {}",
            t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND_2, &[])
        ),
    }

    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_USER, &[])
    );
    cache.delete(&"user:1".to_string()).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(_) => println!(
            "   {}",
            t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_FAILED_TO_DELETE_THE, &[])
        ),
        None => println!(
            "   {}",
            t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_DELETED, &[])
        ),
    }

    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_5_BATCH_OPERATIONS, &[])
    );
    let cache: Cache<String, Product> = Cache::builder().build().await?;

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_PRODUCTS_IN_BATCH,
            &[]
        )
    );
    let products = vec![
        Product {
            id: 1,
            name: "laptop".to_string(),
            price: 5999.99,
            description: "high-performance ultrabook".to_string(),
        },
        Product {
            id: 2,
            name: "smartphone".to_string(),
            price: 3999.99,
            description: "flagship phone".to_string(),
        },
        Product {
            id: 3,
            name: "tablet".to_string(),
            price: 2999.99,
            description: "large-screen tablet".to_string(),
        },
    ];

    for product in &products {
        cache
            .set(&format!("product:{}", product.id), product)
            .await?;
    }
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_ADD_SUCCEEDED_PRODUCTS,
            &[("products", products.len().to_string())]
        )
    );

    // 批量获取
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_FETCHING_PRODUCTS_IN_BATCH,
            &[]
        )
    );
    let mut fetched = Vec::new();
    for i in 1..=3 {
        if let Some(product) = cache.get(&format!("product:{}", i)).await? {
            fetched.push(product);
        }
    }
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_FETCH_SUCCEEDED_PRODUCTS,
            &[("fetched", fetched.len().to_string())]
        )
    );

    // 批量删除
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_PRODUCTS_IN_BATCH,
            &[]
        )
    );
    for i in 1..=3 {
        cache.delete(&format!("product:{}", i)).await?;
    }
    println!(
        "   {}\n",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_DELETE_SUCCEEDED, &[])
    );

    println!("{}", t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_6_TTL_CONTROL, &[]));
    let cache: Cache<String, String> = Cache::builder().build().await?;

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_DATA_THAT_EXPIRES,
            &[]
        )
    );
    cache
        .set_with_ttl(
            &"temp:1".to_string(),
            &"short-term data".to_string(),
            Some(Duration::from_secs(3)),
        )
        .await?;

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_IMMEDIATE_GET,
            &[(
                "await",
                format!("{:?}", cache.get(&"temp:1".to_string()).await?)
            )]
        )
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_WAITING_4_SECONDS_THEN, &[])
    );
    sleep(Duration::from_secs(4)).await;
    println!(
        "   {}\n",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_GET_AFTER_4_SECONDS,
            &[(
                "await",
                format!("{:?}", cache.get(&"temp:1".to_string()).await?)
            )]
        )
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_7_PERFORMANCE_TEST, &[])
    );
    let cache: Arc<Cache<String, i32>> = Arc::new(Cache::builder().build().await?);

    // 并发写入测试
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITING_10000_ENTRIES_CONCURRENTLY,
            &[]
        )
    );
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..100 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                cache
                    .set(&format!("key:{}:{}", i, j), &(i * 100 + j))
                    .await
                    .unwrap();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.unwrap();
    }
    let write_time = start.elapsed();
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITE_TOOK,
            &[("write_time", format!("{:?}", write_time))]
        )
    );

    // 并发读取测试
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_READING_10000_ENTRIES_CONCURRENTLY,
            &[]
        )
    );
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..100 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..100 {
                cache.get(&format!("key:{}:{}", i, j)).await.unwrap();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.unwrap();
    }
    let read_time = start.elapsed();
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_COMPREHENSIVE_USAGE_READ_TOOK,
            &[("read_time", format!("{:?}", read_time))]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_8_STATISTICS, &[]));
    if let Ok(stats) = cache.stats().await {
        println!("  {}", t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_CACHE_STATS, &[]));
        for (key, value) in stats {
            println!("     - {}: {}", key, value);
        }
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPREHENSIVE_USAGE_EXAMPLE_RUN_COMPLETE, &[])
    );
    Ok(())
}
