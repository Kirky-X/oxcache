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
        "=== Oxcache comprehensive usage example ===
"
    );

    println!("1. Create an in-memory cache (L1)");
    let _memory_cache: Cache<String, User> = Cache::builder().build().await?;
    println!(
        "   ✓ in-memory cache created
"
    );

    println!("2. Create a Redis cache (L2 only)");
    match Cache::<String, User>::redis("redis://127.0.0.1:6379").await {
        Ok(_redis_cache) => println!("   ✓ Redis cache created"),
        Err(e) => println!("   ⚠ Redis unavailable, degraded: {}", e),
    }
    println!();

    println!("3. Create a tiered cache (L1 + L2)");
    let _tiered_cache: Cache<String, User> = Cache::builder().build().await?;
    println!("   ✓ tiered cache created");
    println!();

    println!("4. Basic CRUD operations");
    let cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "zhangsan@example.com".to_string(),
        age: 28,
    };

    // Create
    println!("  adding user...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ user added: {:?}", user.name);

    // Read
    println!("  getting user...");
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!("   ✓ user retrieved: {:?}", u.name),
        None => println!("   ✗ user not found"),
    }

    // Update
    println!("  updating user...");
    let updated_user = User {
        id: 1,
        name: "Bob".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
        age: 30,
    };
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   ✓ user updated");

    // Read updated
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!("   ✓ user after the update: {:?}", u.name),
        None => println!("   ✗ user not found"),
    }

    // Delete
    println!("  deleting user...");
    cache.delete(&"user:1".to_string()).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(_) => println!("   ✗ failed to delete the user"),
        None => println!("   ✓ user deleted"),
    }

    println!();

    println!("5. Batch operations");
    let cache: Cache<String, Product> = Cache::builder().build().await?;

    println!("  adding products in batch...");
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
    println!("   ✓ batch add succeeded ({} products)", products.len());

    // 批量获取
    println!("  fetching products in batch...");
    let mut fetched = Vec::new();
    for i in 1..=3 {
        if let Some(product) = cache.get(&format!("product:{}", i)).await? {
            fetched.push(product);
        }
    }
    println!("   ✓ batch fetch succeeded ({} products)", fetched.len());

    // 批量删除
    println!("  deleting products in batch...");
    for i in 1..=3 {
        cache.delete(&format!("product:{}", i)).await?;
    }
    println!(
        "   ✓ batch delete succeeded
"
    );

    println!("6. TTL control");
    let cache: Cache<String, String> = Cache::builder().build().await?;

    println!("  adding data that expires in 3 seconds...");
    cache
        .set_with_ttl(
            &"temp:1".to_string(),
            &"short-term data".to_string(),
            Some(Duration::from_secs(3)),
        )
        .await?;

    println!(
        "  immediate get: {:?}",
        cache.get(&"temp:1".to_string()).await?
    );
    println!("  waiting 4 seconds, then reading...");
    sleep(Duration::from_secs(4)).await;
    println!(
        "   get after 4 seconds: {:?}
",
        cache.get(&"temp:1".to_string()).await?
    );

    println!("7. Performance test");
    let cache: Arc<Cache<String, i32>> = Arc::new(Cache::builder().build().await?);

    // 并发写入测试
    println!("  writing 10000 entries concurrently...");
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
    println!("   ✓ write took: {:?}", write_time);

    // 并发读取测试
    println!("  reading 10000 entries concurrently...");
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
    println!("   ✓ read took: {:?}", read_time);
    println!();

    println!("8. Statistics");
    if let Ok(stats) = cache.stats().await {
        println!("  cache stats:");
        for (key, value) in stats {
            println!("     - {}: {}", key, value);
        }
    }
    println!();

    println!("=== Example run complete ===");
    Ok(())
}
