// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Batch write operations example
//
// This example demonstrates how to use batch write operations
// for improved performance when writing multiple entries.

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Product {
    id: u64,
    name: String,
    price: f64,
    category: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only configuration for simplicity (no Redis required)
    let mut services = HashMap::new();

    services.insert(
        "product_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 10000,
                ..Default::default()
            }),
            cache_type: oxcache::config::CacheType::L1,
            ..Default::default()
        },
    );

    let config = oxcache::config::Config {
        services,
        ..Default::default()
    };
    if let Err(e) = oxcache::init(config).await {
        eprintln!("Init error: {:?}", e);
    }

    let client = oxcache::get_client("product_cache")?;

    // Batch write example
    let products: Vec<Product> = (1..=1000)
        .map(|i| Product {
            id: i,
            name: format!("Product {}", i),
            price: i as f64 * 10.99,
            category: format!("Category {}", (i % 10)),
        })
        .collect();

    println!("Writing {} products using batch write...", products.len());
    let start = std::time::Instant::now();

    for product in &products {
        client
            .set(&format!("product:{}", product.id), product, Some(7200))
            .await?;
    }

    // Flush any pending writes
    println!("Batch write completed in {:?}", start.elapsed());
    println!("Wrote {} products in {:?}", products.len(), start.elapsed());

    // Verify
    let first = client.get::<Product>("product:1").await?;
    assert!(first.is_some());
    println!(
        "Successfully retrieved first product: {:?}",
        first.unwrap().name
    );

    println!("\nBatch write example completed!");
    Ok(())
}
