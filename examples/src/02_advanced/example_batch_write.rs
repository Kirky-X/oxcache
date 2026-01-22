// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Batch write operations example
//
// This example demonstrates how to use batch write operations
// for improved performance when writing multiple entries.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{CacheType, L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

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
    let config = OxcacheConfig::builder()
        .with_service(
            "product_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    if let Err(e) = init(config).await {
        eprintln!("Init error: {:?}", e);
    }

    let client = get_client("product_cache")?;

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
    let first: Option<Product> = client.get("product:1").await?;
    assert!(first.is_some());
    println!(
        "Successfully retrieved first product: {:?}",
        first.unwrap().name
    );

    println!("\nBatch write example completed!");
    Ok(())
}
