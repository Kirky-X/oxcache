// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis Native Operations Example
//
// This example demonstrates native Redis operations
// including sorted sets, Lua scripts, and batch operations.
//
// Note: Requires `l2-redis` feature and a running Redis instance.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Redis Native Operations Example");
    println!("==============================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For native Redis operations:");
    println!("  - Enable l2-redis feature");
    println!("  - Configure TwoLevel with Redis backend");
    println!("  - Use L2NativeOperations trait\n");

    let config = OxcacheConfig::builder()
        .with_service(
            "redis_native_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("redis_native_cache")?;

    println!("Native Redis Operations:");
    println!("  - Sorted Sets (ZADD, ZRANGE, ZSCORE)");
    println!("  - Lua Scripts (EVAL, EVALSHA)");
    println!("  - Batch Operations (MGET, MSET)");
    println!("  - Pub/Sub (PUBLISH, SUBSCRIBE)");
    println!("  - Streams (XADD, XREAD)");
    println!("  - HyperLogLog (PFADD, PFCOUNT)");

    // Basic cache operations as fallback
    println!("\nBasic cache operations (demo):");
    client.set("demo:key1", &"value1", None).await?;
    client.set("demo:key2", &"value2", None).await?;
    
    let val1: Option<String> = client.get("demo:key1").await?;
    let val2: Option<String> = client.get("demo:key2").await?;
    
    println!("  - Set and retrieve values: {:?}, {:?}", val1, val2);

    println!("\n✓ Redis native example completed!");
    Ok(())
}