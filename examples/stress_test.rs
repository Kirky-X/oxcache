//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该示例展示了oxcache的压力测试，用于评估缓存系统的性能。

use oxcache::{
    backend::{l1::L1Backend, l2::L2Backend},
    client::two_level::TwoLevelClient,
    config::{L2Config, TwoLevelConfig},
    serialization::SerializerEnum,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting stress test...");

    let args: Vec<String> = std::env::args().collect();
    let concurrency: usize = args
        .iter()
        .position(|arg| arg == "--concurrency")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let duration_secs: u64 = args
        .iter()
        .position(|arg| arg == "--duration")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    println!("Concurrency: {}, Duration: {}s", concurrency, duration_secs);

    let service_name = "stress_test_service".to_string();

    let l1 = Arc::new(L1Backend::new(10000));
    let l2_config = L2Config {
        connection_string: "redis://127.0.0.1:6379".to_string().into(),
        ..Default::default()
    };

    // Check if Redis is available
    let l2_result = L2Backend::new(&l2_config).await;
    if let Err(e) = l2_result {
        println!("Redis connection failed: {}. Running in L1-only mode.", e);
        // Fallback to L1 only logic if needed, but for stress test we usually want full stack
        // For now, let's exit if we can't connect to L2 as it's a stress test for the full system
        println!("Skipping stress test requiring Redis.");
        return Ok(());
    }
    let l2 = Arc::new(l2_result.unwrap());

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: true,
        batch_size: 100,
        batch_interval_ms: 10,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
    };

    let client = Arc::new(
        TwoLevelClient::new(
            service_name,
            config,
            l1.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer),
        )
        .await?,
    );

    // Pre-populate some data
    println!("Pre-populating data...");
    for i in 0..100 {
        client
            .set(&format!("key_{}", i), &format!("value_{}", i), Some(600))
            .await?;
    }

    let start_time = Instant::now();
    let barrier = Arc::new(Barrier::new(concurrency));
    let mut handles = vec![];

    for i in 0..concurrency {
        let c = client.clone();
        let b = barrier.clone();
        let duration = Duration::from_secs(duration_secs);

        handles.push(tokio::spawn(async move {
            b.wait().await;
            let mut ops = 0;
            let mut errors = 0;
            let start = Instant::now();

            while start.elapsed() < duration {
                let key_idx = i % 100; // Use subset of keys to increase contention
                let key = format!("key_{}", key_idx);

                // Mix of reads (80%) and writes (20%)
                if ops % 5 == 0 {
                    let val = format!("value_{}_{}", key_idx, ops);
                    if c.set(&key, &val, Some(600)).await.is_err() {
                        errors += 1;
                    }
                } else if c.get::<String>(&key).await.is_err() {
                    errors += 1;
                }
                ops += 1;
            }
            (ops, errors)
        }));
    }

    let mut total_ops = 0;
    let mut total_errors = 0;

    for handle in handles {
        let (ops, errors) = handle.await?;
        total_ops += ops;
        total_errors += errors;
    }

    let elapsed = start_time.elapsed();
    println!("Stress test completed in {:.2?}", elapsed);
    println!("Total operations: {}", total_ops);
    println!("Total errors: {}", total_errors);
    println!(
        "Throughput: {:.2} ops/s",
        total_ops as f64 / elapsed.as_secs_f64()
    );

    if total_errors > 0 {
        println!("WARNING: Encountered {} errors!", total_errors);
    }

    Ok(())
}
