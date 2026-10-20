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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_BATCH_WRITE_1_BATCH_ADD_PRODUCTS, MSG_EXAMPLE_BATCH_WRITE_2_BATCH_UPDATE_PRICES,
    MSG_EXAMPLE_BATCH_WRITE_3_BATCH_READ_VERIFICATION, MSG_EXAMPLE_BATCH_WRITE_4_USING_SET_MANY,
    MSG_EXAMPLE_BATCH_WRITE_5_BATCH_DELETE_SIMULATING, MSG_EXAMPLE_BATCH_WRITE_6_CACHE_STATISTICS,
    MSG_EXAMPLE_BATCH_WRITE_BATCH_ADD_COMPLETE_TOOK, MSG_EXAMPLE_BATCH_WRITE_BATCH_ADDING_PRODUCTS,
    MSG_EXAMPLE_BATCH_WRITE_CACHE_TYPE, MSG_EXAMPLE_BATCH_WRITE_CAPACITY,
    MSG_EXAMPLE_BATCH_WRITE_DELETE_MANY_DELETED_ENTRIES,
    MSG_EXAMPLE_BATCH_WRITE_DELISTED_PRODUCT_IDS, MSG_EXAMPLE_BATCH_WRITE_ENTRIES,
    MSG_EXAMPLE_BATCH_WRITE_ENTRIES_REMAIN_AFTER_DELETE,
    MSG_EXAMPLE_BATCH_WRITE_GET_MANY_READ_ENTRIES, MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE,
    MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE_2, MSG_EXAMPLE_BATCH_WRITE_PRODUCT_NEW_PRICE,
    MSG_EXAMPLE_BATCH_WRITE_PRODUCTS_DELISTED, MSG_EXAMPLE_BATCH_WRITE_READING_ALL_PRODUCT_INFO,
    MSG_EXAMPLE_BATCH_WRITE_SET_MANY_WROTE_ENTRIES, MSG_EXAMPLE_BATCH_WRITE_STOCK,
    MSG_EXAMPLE_BATCH_WRITE_UPDATING_PRODUCT_PRICES, t,
};

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
    println!("{}\n", t(MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE, &[]));

    let cache: Arc<Cache<String, Product>> = Arc::new(Cache::builder().build().await?);

    println!("{}", t(MSG_EXAMPLE_BATCH_WRITE_1_BATCH_ADD_PRODUCTS, &[]));
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

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_BATCH_ADDING_PRODUCTS,
            &[("products", products.len().to_string())]
        )
    );
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

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_BATCH_ADD_COMPLETE_TOOK,
            &[("elapsed", format!("{:?}", elapsed))]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_BATCH_WRITE_2_BATCH_UPDATE_PRICES, &[]));
    let updates = vec![
        (1, 5499.99), // 笔记本电脑降价
        (2, 3499.99), // 手机降价
        (3, 2499.99), // 平板降价
    ];

    println!(
        "  {}",
        t(MSG_EXAMPLE_BATCH_WRITE_UPDATING_PRODUCT_PRICES, &[])
    );
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
                "     {}",
                t(
                    MSG_EXAMPLE_BATCH_WRITE_PRODUCT_NEW_PRICE,
                    &[
                        ("id", id.to_string()),
                        ("name", product.name.to_string()),
                        ("price", format!("{:.2}", product.price))
                    ]
                )
            );
        }
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_BATCH_WRITE_3_BATCH_READ_VERIFICATION, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_BATCH_WRITE_READING_ALL_PRODUCT_INFO, &[])
    );
    for product in &products {
        if let Some(p) = cache.get(&format!("product:{}", product.id)).await? {
            println!(
                "     {}",
                t(
                    MSG_EXAMPLE_BATCH_WRITE_STOCK,
                    &[
                        ("id", p.id.to_string()),
                        ("name", p.name.to_string()),
                        ("price", format!("{:.2}", p.price)),
                        ("stock", p.stock.to_string())
                    ]
                )
            );
        }
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_BATCH_WRITE_4_USING_SET_MANY, &[]));

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
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_SET_MANY_WROTE_ENTRIES,
            &[("new_products", new_products.len().to_string())]
        )
    );

    // 批量读取
    let keys: Vec<String> = (10..15).map(|i| format!("product:{}", i)).collect();
    let key_refs: Vec<&String> = keys.iter().collect();
    let fetched = cache.get_many(key_refs).await?;
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_GET_MANY_READ_ENTRIES,
            &[("fetched", fetched.len().to_string())]
        )
    );
    for (key, product) in &fetched {
        println!("     {} -> {} (¥{:.2})", key, product.name, product.price);
    }

    // 批量删除
    let del_keys: Vec<String> = (10..13).map(|i| format!("product:{}", i)).collect();
    let del_refs: Vec<&String> = del_keys.iter().collect();
    cache.delete_many(del_refs).await?;
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_DELETE_MANY_DELETED_ENTRIES,
            &[("del_keys", del_keys.len().to_string())]
        )
    );

    let remaining = cache.get_many(keys.iter().collect::<Vec<_>>()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_ENTRIES_REMAIN_AFTER_DELETE,
            &[("remaining", remaining.len().to_string())]
        )
    );
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_BATCH_WRITE_5_BATCH_DELETE_SIMULATING, &[])
    );
    let out_of_stock_ids = vec![6, 7, 8]; // 假设这些商品已下架

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_DELISTED_PRODUCT_IDS,
            &[("out_of_stock_ids", format!("{:?}", out_of_stock_ids))]
        )
    );
    for id in &out_of_stock_ids {
        // 删除不存在的 key 不会报错
        cache.delete(&format!("product:{}", id)).await?;
    }
    println!("   {}\n", t(MSG_EXAMPLE_BATCH_WRITE_PRODUCTS_DELISTED, &[]));

    println!("{}", t(MSG_EXAMPLE_BATCH_WRITE_6_CACHE_STATISTICS, &[]));
    let stats = cache.stats().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_CACHE_TYPE,
            &[(
                "stats",
                (stats.get("type").unwrap_or(&"N/A".to_string())).to_string()
            )]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_CAPACITY,
            &[(
                "stats",
                (stats.get("capacity").unwrap_or(&"N/A".to_string())).to_string()
            )]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BATCH_WRITE_ENTRIES,
            &[(
                "stats",
                (stats.get("entry_count").unwrap_or(&"N/A".to_string())).to_string()
            )]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE_2, &[]));
    Ok(())
}
