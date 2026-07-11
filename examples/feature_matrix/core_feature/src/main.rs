//! Example: oxcache with `core` feature only.
//!
//! Simulates a user who wants L1 memory + L2 Redis without the full feature
//! set (no macros, no compression, no cli, no lua-script, no testing utils).
//!
//! This example is part of the feature matrix test suite — it ensures that
//! the `core` feature combination compiles and runs without requiring
//! `features = ["full"]`.

use oxcache::Cache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build with default L1 memory backend (no Redis URL provided → falls back to L1 only).
    let cache: Cache<String, String> = Cache::builder().build().await?;

    cache.set(&"key".to_string(), &"value".to_string()).await?;
    let val = cache.get(&"key".to_string()).await?;
    println!("got key = {:?}", val);

    println!("core feature example OK");
    Ok(())
}
