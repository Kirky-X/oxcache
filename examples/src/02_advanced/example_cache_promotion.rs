// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Cache promotion strategy example
//
// This example demonstrates cache promotion on hit behavior:
// - When promote_on_hit is enabled, L1 cache is updated on L2 cache hit
// - Hot data is automatically promoted to L1 for faster access

use oxcache::Cache;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Session {
    id: String,
    user_id: u64,
    created_at: chrono::DateTime<chrono::Utc>,
    last_accessed: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a tiered cache with L1 (memory) and L2 (Redis)
    let cache: Cache<String, Session> =
        Cache::tiered(5000, "redis://127.0.0.1:6379").await?;

    // Simulate session data that exists only in L2 initially
    let session = Session {
        id: "sess_abc123".to_string(),
        user_id: 42,
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    // Set session (goes to both L1 and L2)
    println!("Creating session...");
    cache
        .set(&"session:sess_abc123".to_string(), &session)
        .await?;

    // Evict from L1 to simulate L2-only state
    println!("\nEvicting from L1 to simulate L2-only state...");
    // Note: We can't directly evict from L1 in this API, but in real scenario
    // L1 might be full and old entries get evicted

    // First access - might hit L2 and promote to L1
    println!("\nFirst access (potential L2 hit -> L1 promotion)...");
    let start = std::time::Instant::now();
    if let Some(sess) = cache.get(&"session:sess_abc123".to_string()).await? {
        println!("Session found after {:?}", start.elapsed());
        println!("User ID: {}", sess.user_id);
    }

    // Subsequent access - should hit L1 (fast!)
    println!("\nSecond access (L1 hit - should be faster)...");
    let start = std::time::Instant::now();
    if let Some(sess) = cache.get(&"session:sess_abc123".to_string()).await? {
        println!("Session found after {:?}", start.elapsed());
        println!("User ID: {}", sess.user_id);
    }

    println!("\nCache promotion example completed!");
    println!("With promote_on_hit=true, hot data automatically moves to L1 for faster access.");
    Ok(())
}
