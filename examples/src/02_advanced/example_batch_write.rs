// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 批量写入优化示例
//!
//! 本示例演示了 Oxcache 的批量写入优化功能：
//! - 批量添加商品到缓存
//! - 批量更新价格
//! - 批量删除过期商品
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_batch_write
//! ```

use oxcache::Cache;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Product {
    id: u64,
    name: String,
    price: f64,
    category: String,
    stock: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "=== Batch write optimization example ===
"
    );

    // 创建分层缓存
    let cache: Arc<Cache<String, Product>> = Arc::new(Cache::builder().build().await?);

    println!("1. Batch add products");
    let products = vec![
        Product {
            id: 1,
            name: "laptop".to_string(),
            price: 5999.99,
            category: "electronics".to_string(),
            stock: 100,
        },
        Product {
            id: 2,
            name: "smartphone".to_string(),
            price: 3999.99,
            category: "electronics".to_string(),
            stock: 200,
        },
        Product {
            id: 3,
            name: "tablet".to_string(),
            price: 2999.99,
            category: "electronics".to_string(),
            stock: 150,
        },
        Product {
            id: 4,
            name: "mechanical keyboard".to_string(),
            price: 499.99,
            category: "peripherals".to_string(),
            stock: 300,
        },
        Product {
            id: 5,
            name: "mouse".to_string(),
            price: 99.99,
            category: "peripherals".to_string(),
            stock: 500,
        },
    ];

    println!("  batch adding {} products...", products.len());
    let start = std::time::Instant::now();

    // 批量写入 - 使用并发提高性能
    let mut handles = Vec::new();
    for product in &products {
        let cache = cache.clone();
        let p = product.clone();
        let handle = tokio::spawn(async move {
            cache
                .set_with_ttl(
                    &format!("product:{}", p.id),
                    &p,
                    Some(Duration::from_secs(3600)),
                )
                .await
        });
        handles.push(handle);
    }

    // 等待所有写入完成
    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!("   ✓ batch add complete, took: {:?}", elapsed);
    println!();

    println!("2. Batch update prices (simulating a promotion)");
    let updates = vec![
        (1, 5499.99), // 笔记本电脑降价
        (2, 3499.99), // 手机降价
        (3, 2499.99), // 平板降价
    ];

    println!("  Updating product prices:");
    for (id, new_price) in &updates {
        if let Some(mut product) = cache.get(&format!("product:{}", id)).await? {
            product.price = *new_price;
            cache
                .set_with_ttl(
                    &format!("product:{}", id),
                    &product,
                    Some(Duration::from_secs(3600)),
                )
                .await?;
            println!(
                "     product {}: {} new price: ¥{:.2}",
                id, product.name, product.price
            );
        }
    }
    println!();

    println!("3. Batch read verification");
    println!("  Reading all product info:");
    for product in &products {
        if let Some(p) = cache.get(&format!("product:{}", product.id)).await? {
            println!(
                "     [{}] {} - ¥{:.2} (stock: {})",
                p.id, p.name, p.price, p.stock
            );
        }
    }
    println!();

    println!("4. Using set_many / get_many / delete_many");

    // 批量写入
    let new_products: Vec<(String, Product)> = (10..15)
        .map(|i| {
            (
                format!("product:{}", i),
                Product {
                    id: i,
                    name: format!("product {}", i),
                    price: (i as f64) * 10.0,
                    category: "batch".to_string(),
                    stock: 50,
                },
            )
        })
        .collect();
    let items: Vec<(&String, &Product)> = new_products.iter().map(|(k, v)| (k, v)).collect();
    cache.set_many(items).await?;
    println!("   ✓ set_many wrote {} entries", new_products.len());

    // 批量读取
    let keys: Vec<String> = (10..15).map(|i| format!("product:{}", i)).collect();
    let key_refs: Vec<&String> = keys.iter().collect();
    let fetched = cache.get_many(key_refs).await?;
    println!("   ✓ get_many read {} entries", fetched.len());
    for (key, product) in &fetched {
        println!("     {} -> {} (¥{:.2})", key, product.name, product.price);
    }

    // 批量删除
    let del_keys: Vec<String> = (10..13).map(|i| format!("product:{}", i)).collect();
    let del_refs: Vec<&String> = del_keys.iter().collect();
    cache.delete_many(del_refs).await?;
    println!("   ✓ delete_many deleted {} entries", del_keys.len());

    // 验证删除后剩余
    let remaining = cache.get_many(keys.iter().collect::<Vec<_>>()).await?;
    println!("  {} entries remain after delete", remaining.len());
    println!();

    println!("5. Batch delete (simulating delisted products)");
    let out_of_stock_ids = vec![6, 7, 8]; // 假设这些商品已下架

    println!("  Delisted product IDs: {:?}", out_of_stock_ids);
    for id in &out_of_stock_ids {
        // 删除不存在的 key 不会报错
        cache.delete(&format!("product:{}", id)).await?;
    }
    println!(
        "   ✓ products delisted
"
    );

    println!("6. Cache statistics");
    let stats = cache.stats().await?;
    println!(
        "  - cache type: {}",
        stats.get("type").unwrap_or(&"N/A".to_string())
    );
    println!(
        "  - capacity: {}",
        stats.get("capacity").unwrap_or(&"N/A".to_string())
    );
    println!(
        "  - entries: {}",
        stats.get("entry_count").unwrap_or(&"N/A".to_string())
    );
    println!();

    println!("=== Batch write optimization example complete ===");
    Ok(())
}
