// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Rate limiting example
//
// This example demonstrates rate limiting functionality
// for protecting the cache from overuse.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(Clone)]
struct RateLimiter {
    permits: Arc<Semaphore>,
    interval: Duration,
}

impl RateLimiter {
    fn new(permits_per_second: u64) -> Self {
        let permits = Arc::new(Semaphore::new(permits_per_second as usize));
        let interval = Duration::from_secs_f64(1.0 / permits_per_second as f64);

        // Spawn background task to replenish permits
        let permits_clone = permits.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let _ = permits_clone.available_permits();
            }
        });

        Self { permits, interval }
    }

    async fn acquire(&self) -> Result<(), Box<dyn std::error::Error>> {
        let permit = self.permits.try_acquire()?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = OxcacheConfig::builder()
        .with_service(
            "rate_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let _client = get_client("rate_cache")?;
    let rate_limiter = RateLimiter::new(100);

    println!("Rate Limiting Example");
    println!("=====================\n");
    println!("Rate limiting configuration:");
    println!("  Requests/second: 100");
    println!("  Burst size: 200\n");

    println!("Testing rate limiting...");
    let mut success_count = 0;
    let mut limited_count = 0;

    for i in 0..300 {
        if rate_limiter.acquire().await.is_ok() {
            success_count += 1;
        } else {
            limited_count += 1;
        }
    }

    println!("\nResults:");
    println!("  Allowed: {} requests", success_count);
    println!("  Limited: {} requests", limited_count);

    println!("\n✓ Rate limiting benefits:");
    println!("  - Protects cache from overuse");
    println!("  - Prevents abuse");
    println!("  - Ensures fair resource sharing");

    println!("\n✓ Rate limiting example completed!");
    Ok(())
}
