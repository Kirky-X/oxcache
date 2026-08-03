// Copyright (c) 2025-2026 Kirky.X
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
        "=== 批量写入优化示例 ===
"
    );

    // 创建分层缓存
    let cache: Arc<Cache<String, Product>> = Arc::new(Cache::builder().build().await?);

    // 1. 批量添加商品
    println!("1. 批量添加商品");
    let products = vec![
        Product {
            id: 1,
            name: "笔记本电脑".to_string(),
            price: 5999.99,
            category: "电子产品".to_string(),
            stock: 100,
        },
        Product {
            id: 2,
            name: "智能手机".to_string(),
            price: 3999.99,
            category: "电子产品".to_string(),
            stock: 200,
        },
        Product {
            id: 3,
            name: "平板电脑".to_string(),
            price: 2999.99,
            category: "电子产品".to_string(),
            stock: 150,
        },
        Product {
            id: 4,
            name: "机械键盘".to_string(),
            price: 499.99,
            category: "外设".to_string(),
            stock: 300,
        },
        Product {
            id: 5,
            name: "鼠标".to_string(),
            price: 99.99,
            category: "外设".to_string(),
            stock: 500,
        },
    ];

    println!("   开始批量添加 {} 个商品...", products.len());
    let start = std::time::Instant::now();

    // 批量写入 - 使用并发提高性能
    let mut handles = Vec::new();
    for product in &products {
        let cache = cache.clone();
        let p = product.clone();
        let handle = tokio::spawn(async move {
            cache
                .set_with_ttl(&format!("product:{}", p.id), &p, Some(Duration::from_secs(3600)))
                .await
        });
        handles.push(handle);
    }

    // 等待所有写入完成
    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!("   ✓ 批量添加完成，耗时: {:?}", elapsed);
    println!();

    // 2. 批量更新价格
    println!("2. 批量更新价格 (模拟促销活动)");
    let updates = vec![
        (1, 5499.99), // 笔记本电脑降价
        (2, 3499.99), // 手机降价
        (3, 2499.99), // 平板降价
    ];

    println!("   更新商品价格:");
    for (id, new_price) in &updates {
        if let Some(mut product) = cache.get(&format!("product:{}", id)).await? {
            product.price = *new_price;
            cache
                .set_with_ttl(&format!("product:{}", id), &product, Some(Duration::from_secs(3600)))
                .await?;
            println!("     产品 {}: {} 新价格: ¥{:.2}", id, product.name, product.price);
        }
    }
    println!();

    // 3. 批量读取验证
    println!("3. 批量读取验证");
    println!("   读取所有商品信息:");
    for product in &products {
        if let Some(p) = cache.get(&format!("product:{}", product.id)).await? {
            println!("     [{}] {} - ¥{:.2} (库存: {})", p.id, p.name, p.price, p.stock);
        }
    }
    println!();

    // 4. 使用 set_many / get_many / delete_many 批量操作
    println!("4. 使用 set_many / get_many / delete_many");

    // 批量写入
    let new_products: Vec<(String, Product)> = (10..15)
        .map(|i| {
            (
                format!("product:{}", i),
                Product {
                    id: i,
                    name: format!("商品{}", i),
                    price: (i as f64) * 10.0,
                    category: "批量".to_string(),
                    stock: 50,
                },
            )
        })
        .collect();
    let items: Vec<(&String, &Product)> = new_products.iter().map(|(k, v)| (k, v)).collect();
    cache.set_many(items).await?;
    println!("   ✓ set_many 写入 {} 条", new_products.len());

    // 批量读取
    let keys: Vec<String> = (10..15).map(|i| format!("product:{}", i)).collect();
    let key_refs: Vec<&String> = keys.iter().collect();
    let fetched = cache.get_many(key_refs).await?;
    println!("   ✓ get_many 读取 {} 条", fetched.len());
    for (key, product) in &fetched {
        println!("     {} -> {} (¥{:.2})", key, product.name, product.price);
    }

    // 批量删除
    let del_keys: Vec<String> = (10..13).map(|i| format!("product:{}", i)).collect();
    let del_refs: Vec<&String> = del_keys.iter().collect();
    cache.delete_many(del_refs).await?;
    println!("   ✓ delete_many 删除 {} 条", del_keys.len());

    // 验证删除后剩余
    let remaining = cache.get_many(keys.iter().collect::<Vec<_>>()).await?;
    println!("   删除后剩余 {} 条", remaining.len());
    println!();

    // 5. 批量删除过期商品
    println!("5. 批量删除 (模拟下架商品)");
    let out_of_stock_ids = vec![6, 7, 8]; // 假设这些商品已下架

    println!("   下架商品 ID: {:?}", out_of_stock_ids);
    for id in &out_of_stock_ids {
        // 删除不存在的 key 不会报错
        cache.delete(&format!("product:{}", id)).await?;
    }
    println!(
        "   ✓ 商品下架完成
"
    );

    // 6. 统计信息
    println!("6. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 缓存类型: {}", stats.get("type").unwrap_or(&"N/A".to_string()));
    println!("   - 容量: {}", stats.get("capacity").unwrap_or(&"N/A".to_string()));
    println!(
        "   - 条目数: {}",
        stats.get("entry_count").unwrap_or(&"N/A".to_string())
    );
    println!();

    println!("=== 批量写入优化示例完成 ===");
    Ok(())
}
