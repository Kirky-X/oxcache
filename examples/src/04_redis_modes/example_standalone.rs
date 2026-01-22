// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis Standalone mode example
//
// This example demonstrates using Redis in standalone mode
// (single Redis server).

use oxcache::Cache;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct StandaloneData {
    id: u64,
    name: String,
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Redis
    let cache: Cache<String, StandaloneData> =
        Cache::redis("redis://127.0.0.1:6379").await?;

    println!("Redis Standalone Mode Example");
    println!("============================\n");
    println!("Connected to Redis at redis://127.0.0.1:6379\n");

    // Test basic operations
    let data = StandaloneData {
        id: 1,
        name: "Test Item".to_string(),
        description: "A test item for standalone mode".to_string(),
    };

    println!("Writing data...");
    cache.set(&"standalone:test".to_string(), &data).await?;
    println!("  Wrote: {} - {}", data.name, data.description);

    println!("\nReading data...");
    if let Some(cached) = cache.get(&"standalone:test".to_string()).await? {
        println!("  Read: {} - {}", cached.name, cached.description);
    }

    println!("\nDeleting data...");
    cache.delete(&"standalone:test".to_string()).await?;
    let result: Option<StandaloneData> = cache.get(&"standalone:test".to_string()).await?;
    assert!(result.is_none());
    println!("  Deleted successfully");

    println!("\n✓ Standalone mode example completed!");
    Ok(())
}
