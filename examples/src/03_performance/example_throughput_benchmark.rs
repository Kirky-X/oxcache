// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Throughput benchmark example
//
// This example measures throughput (operations per second) for the cache.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};
use std::sync::Arc;
use tokio::sync::Barrier;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct ThroughputTest {
    id: u64,
    data: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only configuration for simplicity (no Redis required)
    let config = OxcacheConfig::builder()
        .with_service(
            "throughput_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(50000)),
        )
        .build();

    let _ = init(config).await;

    let num_threads = 4;
    let ops_per_thread = 1000;
    let barrier = Arc::new(Barrier::new(num_threads));
    let client = Arc::new(get_client("throughput_cache")?);

    println!(
        "Throughput benchmark with {} threads, {} ops each...",
        num_threads, ops_per_thread
    );
    println!("Total operations: {}\n", num_threads * ops_per_thread);

    let mut handles = Vec::new();

    for thread_id in 0..num_threads {
        let barrier = barrier.clone();
        let client = client.clone();
        let handle = tokio::spawn(async move {
            barrier.wait().await;

            let mut ops = 0;
            let start = std::time::Instant::now();

            for i in 0..ops_per_thread {
                let key = format!("perf:{}:{}", thread_id, i);
                let test_data = ThroughputTest {
                    id: (thread_id * ops_per_thread + i) as u64,
                    data: vec![0u8; 100],
                };

                // Alternate between write and read
                if i % 2 == 0 {
                    // Write
                    let _ = client.set(&key, &test_data, None).await;
                } else {
                    // Read
                    let _ = client.get::<ThroughputTest>(&key).await;
                }
                ops += 1;
            }

            let elapsed = start.elapsed();
            (ops, elapsed)
        });
        handles.push(handle);
    }

    let mut total_ops = 0;
    let mut total_elapsed = std::time::Duration::ZERO;

    for handle in handles {
        let (ops, elapsed) = handle.await.unwrap();
        total_ops += ops;
        total_elapsed += elapsed;
    }

    let total_seconds = total_elapsed.as_secs_f64();
    let ops_per_sec = total_ops as f64 / total_seconds;

    println!("\nThroughput Results:");
    println!("  Total operations: {}", total_ops);
    println!("  Total time: {:?}", total_elapsed);
    println!("  Throughput: {:.0} ops/sec", ops_per_sec);

    println!("\n✓ Throughput benchmark completed!");
    Ok(())
}
