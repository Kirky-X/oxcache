// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Latency benchmark example
//
// This example benchmarks cache latency for different operations.

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct LatencyTest {
    id: u64,
    data: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only configuration for simplicity (no Redis required)
    let mut services = HashMap::new();

    services.insert(
        "benchmark_cache".to_string(),
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

    let client = oxcache::get_client("benchmark_cache")?;

    let test_data = LatencyTest {
        id: 1,
        data: vec![0u8; 100],
    };

    // Warmup
    println!("Warming up...");
    for i in 0..100 {
        client
            .set(&format!("bench:{}", i), &test_data, None)
            .await?;
    }

    // Benchmark GET latency
    println!("\nBenchmarking GET latency...");
    let iterations = 1000;
    let mut latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = std::time::Instant::now();
        let _ = client
            .get::<LatencyTest>(&format!("bench:{}", i % 100))
            .await;
        latencies.push(start.elapsed());
    }

    // Calculate statistics
    latencies.sort();
    let avg_us =
        latencies.iter().sum::<std::time::Duration>().as_micros() as f64 / iterations as f64;
    let p50_us = latencies[iterations / 2].as_micros() as f64;
    let p99_us = latencies[(iterations * 99) / 100].as_micros() as f64;
    let p999_us = latencies[(iterations * 999) / 1000].as_micros() as f64;

    println!("\nLatency Results (GET):");
    println!("  Average: {:.2}µs", avg_us);
    println!("  P50: {:.2}µs", p50_us);
    println!("  P99: {:.2}µs", p99_us);
    println!("  P99.9: {:.2}µs", p999_us);

    // Benchmark SET latency
    println!("\nBenchmarking SET latency...");
    let mut latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let test_data = LatencyTest {
            id: i as u64,
            data: vec![0u8; 100],
        };
        let start = std::time::Instant::now();
        let _ = client
            .set(&format!("bench:write:{}", i), &test_data, None)
            .await;
        latencies.push(start.elapsed());
    }

    let avg_us =
        latencies.iter().sum::<std::time::Duration>().as_micros() as f64 / iterations as f64;
    println!("\nLatency Results (SET):");
    println!("  Average: {:.2}µs", avg_us);

    // Calculate QPS
    let get_qps = 1_000_000.0 / avg_us;
    println!("\nEstimated QPS: {:.0}", get_qps);

    println!("\n✓ Latency benchmark completed!");
    Ok(())
}
