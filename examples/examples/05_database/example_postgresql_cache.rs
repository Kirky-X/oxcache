// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// PostgreSQL cache example
//
// This example demonstrates using oxcache with PostgreSQL database
// for the cache-aside pattern.
//
// Note: This example uses L1-only mode for demonstration.
// To use with real PostgreSQL, configure with:
// - database: Some(DatabaseConfig { type: Postgresql, connection_string: ... })
// - Use oxcache::DbLoader for DB-first caching patterns

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Product {
    id: u64,
    name: String,
    price: f64,
    category: String,
    inventory: u32,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real DB caching, configure with database field
    let mut services = HashMap::new();

    services.insert(
        "postgres_cache".to_string(),
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
    let _ = oxcache::init(config).await;

    let client = oxcache::get_client("postgres_cache")?;

    println!("PostgreSQL Cache-Aside Pattern Example");
    println!("======================================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real PostgreSQL caching:");
    println!("  - database: Some(DatabaseConfig {{ type: Postgresql, ... }})");
    println!("  - Use oxcache::DbLoader for DB-first caching patterns\n");

    // Simulate product data caching
    let products = vec![
        Product {
            id: 1,
            name: "Laptop".into(),
            price: 999.99,
            category: "Electronics".into(),
            inventory: 50,
            created_at: chrono::Utc::now(),
        },
        Product {
            id: 2,
            name: "Book".into(),
            price: 29.99,
            category: "Books".into(),
            inventory: 200,
            created_at: chrono::Utc::now(),
        },
        Product {
            id: 3,
            name: "Headphones".into(),
            price: 149.99,
            category: "Electronics".into(),
            inventory: 75,
            created_at: chrono::Utc::now(),
        },
    ];

    println!("Caching product data...");
    for product in &products {
        client
            .set(&format!("product:{}", product.id), product, None)
            .await?;
        println!("  ✓ Cached: {} (${})", product.name, product.price);
    }

    println!("\nSimulating cache retrieval...");
    let cached = client.get::<Product>("product:1").await?;
    if let Some(product) = cached {
        println!(
            "  ✓ Retrieved: {} (${}, {} in stock)",
            product.name, product.price, product.inventory
        );
    }

    println!("\nPostgreSQL benefits:");
    println!("  - Advanced features (JSON, full-text search)");
    println!("  - Excellent performance");
    println!("  - Strong consistency guarantees");

    println!("\n✓ PostgreSQL cache example completed!");
    Ok(())
}
