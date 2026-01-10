// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Stress test example
//
// This example stress tests the cache under high load.
// Uses L1-only mode for demonstration without Redis.

use oxcache::CacheExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct StressTestData {
    id: u64,
    value: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for stress test demo (no Redis required)
    let mut services = HashMap::new();

    services.insert(
        "stress_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 100000,
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

    let client = Arc::new(oxcache::get_client("stress_cache")?);
    let semaphore = Arc::new(Semaphore::new(100)); // Limit concurrent operations
    let counter = Arc::new(AtomicU64::new(0)); // Simple ID counter
    let duration = std::time::Duration::from_secs(5);

    println!("Stress Test Example (L1-only mode)");
    println!("===================================\n");
    println!("Starting stress test for {} seconds...", duration.as_secs());
    println!("Concurrency level: 100");
    println!("\nMetrics will be collected during the test...\n");

    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    while start.elapsed() < duration {
        let client = client.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let id = counter.fetch_add(1, Ordering::SeqCst);

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let data = StressTestData {
                id,
                value: format!("stress_test_value_{}", id),
                timestamp: chrono::Utc::now(),
            };

            // Random operation: 60% read, 40% write
            let read = id % 10 < 6;
            if read {
                let _ = client
                    .get::<StressTestData>(&format!("stress:{}", id % 10000))
                    .await;
            } else {
                let _ = client
                    .set(&format!("stress:{}", id % 10000), &data, Some(3600))
                    .await;
            }
        }));
    }

    // Wait for all ongoing tasks to complete
    let handle_count = handles.len();
    for handle in handles {
        let _ = handle.await;
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (handle_count as f64) / elapsed.as_secs_f64();

    println!("=== Stress Test Results ===");
    println!("Duration: {:.2}s", elapsed.as_secs_f64());
    println!("Total operations: {}", handle_count);
    println!("Throughput: {:.0} ops/sec", ops_per_sec);
    println!("\nNote: L1-only mode for demo. For real stress testing:");
    println!("  - Enable TwoLevel with Redis for distributed testing");
    println!("  - Use enable_batch_write for higher write throughput");
    println!("\nStress test completed!");
    Ok(())
}
