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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CACHE_BUILDER_1_BASIC_CONSTRUCTION, MSG_EXAMPLE_CACHE_BUILDER_2_SET_CAPACITY,
    MSG_EXAMPLE_CACHE_BUILDER_3_SET_TTL, MSG_EXAMPLE_CACHE_BUILDER_4_SET_TTI,
    MSG_EXAMPLE_CACHE_BUILDER_5_COMBINED_CONFIGURATION, MSG_EXAMPLE_CACHE_BUILDER_6_USE_A_CUSTOM,
    MSG_EXAMPLE_CACHE_BUILDER_7_REAL_USAGE, MSG_EXAMPLE_CACHE_BUILDER_8_SET_WITH_TTL,
    MSG_EXAMPLE_CACHE_BUILDER_9_STATISTICS, MSG_EXAMPLE_CACHE_BUILDER_10_HEALTH_CHECK,
    MSG_EXAMPLE_CACHE_BUILDER_BASIC_CACHE_CREATED, MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_1,
    MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_5, MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_A,
    MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_CAPACITY, MSG_EXAMPLE_CACHE_BUILDER_CACHE_STATS,
    MSG_EXAMPLE_CACHE_BUILDER_CACHEBUILDER_FULL_USAGE_EXAMPLE,
    MSG_EXAMPLE_CACHE_BUILDER_COMBINED_CACHE_CREATED_CAPACITY,
    MSG_EXAMPLE_CACHE_BUILDER_EXAMPLE_COMPLETE, MSG_EXAMPLE_CACHE_BUILDER_EXISTS_CHECK,
    MSG_EXAMPLE_CACHE_BUILDER_HEALTH_STATUS_OK, MSG_EXAMPLE_CACHE_BUILDER_READ,
    MSG_EXAMPLE_CACHE_BUILDER_STORING_A_PRODUCT_WITH, MSG_EXAMPLE_CACHE_BUILDER_WRITE, t,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Product {
    id: u64,
    name: String,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(
            MSG_EXAMPLE_CACHE_BUILDER_CACHEBUILDER_FULL_USAGE_EXAMPLE,
            &[]
        )
    );

    println!("{}", t(MSG_EXAMPLE_CACHE_BUILDER_1_BASIC_CONSTRUCTION, &[]));
    let _cache1: Cache<String, Product> = CacheBuilder::default().build().await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_BUILDER_BASIC_CACHE_CREATED, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_2_SET_CAPACITY, &[]));
    let _cache2: Cache<String, Product> = CacheBuilder::default().capacity(10000).build().await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_CAPACITY, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_3_SET_TTL, &[]));
    let _cache3: Cache<String, Product> = CacheBuilder::default()
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_1, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_4_SET_TTI, &[]));
    let _cache4: Cache<String, Product> = CacheBuilder::default()
        .tti(Duration::from_secs(300))
        .build()
        .await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_5, &[])
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CACHE_BUILDER_5_COMBINED_CONFIGURATION, &[])
    );
    let cache5: Cache<String, Product> = CacheBuilder::default()
        .capacity(5000)
        .ttl(Duration::from_secs(1800))
        .tti(Duration::from_secs(600))
        .build()
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_BUILDER_COMBINED_CACHE_CREATED_CAPACITY,
            &[]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_6_USE_A_CUSTOM, &[]));
    let backend = MokaMemoryBackend::builder().capacity(2000).build();
    let _cache6: Cache<String, Product> = CacheBuilder::default()
        .backend_arc(
            std::sync::Arc::new(backend) as std::sync::Arc<dyn oxcache::backend::CacheBackend>
        )
        .ttl(Duration::from_secs(120))
        .build()
        .await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_A, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_7_REAL_USAGE, &[]));
    let cache = cache5;

    let product = Product {
        id: 1,
        name: "Laptop".to_string(),
        price: 999.99,
    };

    cache.set(&"product:1".to_string(), &product).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_BUILDER_WRITE,
            &[("product", format!("{:?}", product))]
        )
    );

    let cached = cache.get(&"product:1".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_BUILDER_READ,
            &[("cached", format!("{:?}", cached))]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_8_SET_WITH_TTL, &[]));
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
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_BUILDER_STORING_A_PRODUCT_WITH, &[])
    );

    let exists = cache.exists(&"product:2".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_BUILDER_EXISTS_CHECK,
            &[("exists", exists.to_string())]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_9_STATISTICS, &[]));
    let stats = cache.stats().await?;
    println!("  {}", t(MSG_EXAMPLE_CACHE_BUILDER_CACHE_STATS, &[]));
    for (key, value) in &stats {
        println!("    {}: {}", key, value);
    }

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_10_HEALTH_CHECK, &[]));
    cache.health_check().await?;
    println!("  {}", t(MSG_EXAMPLE_CACHE_BUILDER_HEALTH_STATUS_OK, &[]));

    cache.clear().await?;

    println!("\n{}", t(MSG_EXAMPLE_CACHE_BUILDER_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
