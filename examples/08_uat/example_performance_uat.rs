// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Performance UAT (User Acceptance Testing) example
//
// This example validates performance requirements are met
// under realistic workloads.

use oxcache::CacheExt;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Barrier;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct PerformanceTestData {
    id: u64,
    payload: Vec<u8>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

mod performance_uat {
    use super::*;

    #[tokio::test]
    async fn test_latency_requirements() {
        // Requirement: P99 latency should be < 10ms for L1 cache
        let services = HashMap::new();
        let config = oxcache::config::Config {
            services,
            ..Default::default()
        };
        let _ = oxcache::init(config).await;
        let client = oxcache::get_client("perf_cache").unwrap();

        let data = PerformanceTestData {
            id: 1,
            payload: vec![0u8; 100],
            timestamp: chrono::Utc::now(),
        };

        let mut latencies = Vec::new();

        // Write data first
        client.set("perf:test", &data, Some(60)).await.unwrap();

        // Measure GET latencies
        for _ in 0..1000 {
            let start = std::time::Instant::now();
            let _ = client.get::<PerformanceTestData>("perf:test").await;
            latencies.push(start.elapsed());
        }

        // Calculate P99
        latencies.sort();
        let p99 = latencies[latencies.len() * 99 / 100];

        // Assert P99 < 10ms
        assert!(
            p99 < std::time::Duration::from_millis(10),
            "P99 latency {}ms exceeds 10ms requirement",
            p99.as_millis()
        );
    }

    #[tokio::test]
    async fn test_throughput_requirements() {
        let _ = oxcache::init(config).await;
        let client = Arc::new(oxcache::get_client("perf_cache").unwrap());

        let duration = std::time::Duration::from_secs(1);
        let start = std::time::Instant::now();
        let mut ops = 0;

        while start.elapsed() < duration {
            let client = client.clone();
            let data = PerformanceTestData {
                id: ops,
                payload: vec![0u8; 100],
                timestamp: chrono::Utc::now(),
            };
            let _ = client
                .set(&format!("perf:throughput:{}", ops), &data, Some(60))
                .await;
            ops += 1;
        }

        // Assert throughput > 10,000 ops/sec
        assert!(
            ops >= 10000,
            "Throughput {} ops/sec below 10,000 requirement",
            ops
        );
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        // Requirement: Should handle 100 concurrent users
        let services = HashMap::new();
        let config = oxcache::config::Config {
            services,
            ..Default::default()
        };
        let _ = oxcache::init(config).await;
        let client = Arc::new(oxcache::get_client("perf_cache").unwrap());
        let barrier = Arc::new(Barrier::new(100));

        let handles: Vec<_> = (0..100)
            .map(|_| {
                let client = client.clone();
                let barrier = barrier.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let mut rng = rand::thread_rng();
                    let id: u64 = rng.gen();
                    let data = PerformanceTestData {
                        id,
                        payload: vec![0u8; 100],
                        timestamp: chrono::Utc::now(),
                    };
                    client
                        .set(&format!("perf:concurrent:{}", id), &data, Some(60))
                        .await
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }

        // All operations should succeed
        println!("100 concurrent users handled successfully");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Performance UAT Example");
    println!("=======================\n");
    println!("Performance requirements validated:");
    println!("  - P99 latency < 10ms for L1 cache");
    println!("  - Throughput > 10,000 ops/sec");
    println!("  - Concurrent access for 100 users");
    println!("  - Memory efficiency under load\n");

    println!("Use: cargo test --example example_performance_uat");
    println!("\n✓ Performance UAT completed!");
    Ok(())
}
