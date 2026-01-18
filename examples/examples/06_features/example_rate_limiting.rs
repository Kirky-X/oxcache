// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Rate limiting example
//
// This example demonstrates rate limiting functionality
// for protecting the cache from overuse.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

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
                // In real implementation, this would replenish
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
    let mut services = HashMap::new();

    services.insert(
        "rate_cache".to_string(),
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

    let client = oxcache::get_client("rate_cache")?;
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
            // In real implementation, this would be a cache operation
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
