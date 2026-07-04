//! Example: oxcache with `minimal` feature only.
//!
//! Simulates the narrowest supported user scenario: L1 memory cache + metrics
//! + serialization + tracing + chrono, without redis/opentelemetry-otlp/cli/etc.
//!
//! This example exists to prevent regression of the 0.3.2 upstream bug where
//! `pub mod metrics;` and `pub mod serialization;` in `src/infra/mod.rs` were
//! not cfg-gated, causing compile failure when narrow feature combinations
//! were used. The `examples/Cargo.toml` workspace uses `features = ["full"]`,
//! which never exercises narrow combinations — this standalone sub-crate
//! fills that gap.

use oxcache::Cache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a minimal L1 memory cache (no Redis, no compression).
    let cache: Cache<String, String> = Cache::builder().build().await?;

    // Set + get
    cache
        .set(&"user:1".to_string(), &"Alice".to_string())
        .await?;
    let val = cache.get(&"user:1".to_string()).await?;
    println!("got user:1 = {:?}", val);

    // Verify metrics module is accessible under minimal feature
    let stats = oxcache::CacheStats::default();
    println!("metrics module accessible; stats.total_operations = {}", stats.total_operations);

    println!("minimal feature example OK");
    Ok(())
}
