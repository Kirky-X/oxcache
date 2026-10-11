// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! CacheBuilder 完整用法示例
//!
//! 本示例演示 CacheBuilder 的所有配置选项，
//! 包括 TTL、TTI、容量、自定义后端等。

use oxcache::backend::MokaMemoryBackend;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Product {
    id: u64,
    name: String,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CacheBuilder full usage example ===\n");

    println!("--- 1. Basic construction ---");
    let _cache1: Cache<String, Product> = CacheBuilder::default().build().await?;
    println!("  ✓ basic cache created");

    println!("\n--- 2. Set capacity ---");
    let _cache2: Cache<String, Product> = CacheBuilder::default().capacity(10000).build().await?;
    println!("  ✓ cache created with capacity 10000");

    println!("\n--- 3. Set TTL ---");
    let _cache3: Cache<String, Product> = CacheBuilder::default()
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;
    println!("  ✓ cache created with 1 hour TTL");

    println!("\n--- 4. Set TTI ---");
    let _cache4: Cache<String, Product> = CacheBuilder::default()
        .tti(Duration::from_secs(300))
        .build()
        .await?;
    println!("  ✓ cache created with 5 minute TTI");

    println!("\n--- 5. Combined configuration ---");
    let cache5: Cache<String, Product> = CacheBuilder::default()
        .capacity(5000)
        .ttl(Duration::from_secs(1800))
        .tti(Duration::from_secs(600))
        .build()
        .await?;
    println!("  ✓ combined cache created (capacity=5000, TTL=30min, TTI=10min)");

    println!("\n--- 6. Use a custom backend ---");
    let backend = MokaMemoryBackend::builder().capacity(2000).build();
    let _cache6: Cache<String, Product> = CacheBuilder::default()
        .backend_arc(
            std::sync::Arc::new(backend) as std::sync::Arc<dyn oxcache::backend::CacheBackend>
        )
        .ttl(Duration::from_secs(120))
        .build()
        .await?;
    println!("  ✓ cache created with a custom Moka backend");

    println!("\n--- 7. Real usage ---");
    let cache = cache5;

    let product = Product {
        id: 1,
        name: "Laptop".to_string(),
        price: 999.99,
    };

    cache.set(&"product:1".to_string(), &product).await?;
    println!("  write: {:?}", product);

    let cached = cache.get(&"product:1".to_string()).await?;
    println!("  read: {:?}", cached);

    println!("\n--- 8. Set with TTL ---");
    cache
        .set_with_ttl(
            &"product:2".to_string(),
            &Product {
                id: 2,
                name: "Phone".to_string(),
                price: 599.99,
            },
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("  storing a product with 60s TTL");

    let exists = cache.exists(&"product:2".to_string()).await?;
    println!("  exists check: {}", exists);

    println!("\n--- 9. Statistics ---");
    let stats = cache.stats().await?;
    println!("  cache stats:");
    for (key, value) in &stats {
        println!("    {}: {}", key, value);
    }

    println!("\n--- 10. Health check ---");
    cache.health_check().await?;
    println!("  health status: ✓ OK");

    // 清理
    cache.clear().await?;

    println!("\n✓ Example complete");
    Ok(())
}
