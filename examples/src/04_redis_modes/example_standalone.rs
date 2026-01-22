// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis Standalone mode example
//
// This example demonstrates using Redis in standalone mode
// (single Redis server).
//
// Note: This example uses L1-only mode for demonstration.
// To use with Redis, configure with:
// - cache_type: TwoLevel
// - l2.connection_string: "redis://127.0.0.1:6379"

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct StandaloneData {
    id: u64,
    name: String,
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real Redis usage, configure with TwoLevel + L2
    let config = OxcacheConfig::builder()
        .with_service(
            "standalone_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("standalone_cache")?;

    println!("Redis Standalone Mode Example");
    println!("============================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real Redis, configure:");
    println!("  - cache_type: TwoLevel");
    println!("  - l2.connection_string: redis://127.0.0.1:6379\n");

    // Test basic operations
    let data = StandaloneData {
        id: 1,
        name: "Test Item".to_string(),
        description: "A test item for standalone mode".to_string(),
    };

    println!("Writing data...");
    client.set("standalone:test", &data, None).await?;
    println!("  Wrote: {} - {}", data.name, data.description);

    println!("\nReading data...");
    if let Some(cached) = client.get::<StandaloneData>("standalone:test").await? {
        println!("  Read: {} - {}", cached.name, cached.description);
    }

    println!("\nDeleting data...");
    client.delete("standalone:test").await?;
    let result: Option<StandaloneData> = client.get("standalone:test").await?;
    assert!(result.is_none());
    println!("  Deleted successfully");

    println!("\n✓ Standalone mode example completed!");
    Ok(())
}
