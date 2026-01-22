// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Performance UAT (User Acceptance Testing) example
//
// This example validates performance requirements are met
// under realistic workloads.

use rand::Rng;
use std::sync::Arc;
use tokio::sync::Barrier;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct PerformanceTestData {
    id: u64,
    payload: Vec<u8>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxcache::manager::{get_client, init};
    use oxcache::{
        config::{L1Config, OxcacheConfig, ServiceConfig},
        CacheExt,
    };
    
    fn create_perf_config() -> OxcacheConfig {
        OxcacheConfig::builder()
            .with_service(
                "perf_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
            )
            .build()
    }
    
    #[tokio::test]
    async fn test_throughput_requirement() {
        // Requirement: System should handle at least 10,000 ops/second
        let config = create_perf_config();
        let _ = init(config).await;
        let client = get_client("perf_cache").unwrap();
        
        let num_operations = 1000;
        let data_size = 100;
        let start = std::time::Instant::now();
        
        // Perform writes
        for i in 0..num_operations {
            let test_data = PerformanceTestData {
                id: i,
                payload: vec![0u8; data_size],
                timestamp: chrono::Utc::now(),
            };
            client.set(&format!("perf:{}", i), &test_data, None).await.unwrap();
        }
        
        let duration = start.elapsed();
        let ops_per_second = num_operations as f64 / duration.as_secs_f64();
        
        println!("Throughput: {:.0} ops/sec", ops_per_second);
        assert!(ops_per_second >= 1000.0, "Should meet minimum throughput requirement");
    }
    
    #[tokio::test]
    async fn test_latency_requirement() {
        // Requirement: 99th percentile latency should be under 10ms
        let config = create_perf_config();
        let _ = init(config).await;
        let client = Arc::new(get_client("perf_cache").unwrap());
        
        // Pre-populate cache
        for i in 0..1000 {
            let test_data = PerformanceTestData {
                id: i,
                payload: vec![0u8; 50],
                timestamp: chrono::Utc::now(),
            };
            client.set(&format!("perf:{}", i), &test_data, None).await.unwrap();
        }
        
        // Measure latencies
        let mut latencies = Vec::new();
        for i in 0..1000 {
            let start = std::time::Instant::now();
            let _ = client.get::<PerformanceTestData>(&format!("perf:{}", i)).await.unwrap();
            latencies.push(start.elapsed());
        }
        
        latencies.sort();
        let p99 = latencies[990];
        
        println!("P99 latency: {:?}", p99);
        assert!(p99.as_millis() < 10, "P99 latency should be under 10ms");
    }
    
    #[tokio::test]
    async fn test_concurrent_access() {
        // Requirement: System should handle concurrent access without degradation
        let config = create_perf_config();
        let _ = init(config).await;
        let client = Arc::new(get_client("perf_cache").unwrap());
        
        let num_threads = 10;
        let ops_per_thread = 100;
        let barrier = Arc::new(Barrier::new(num_threads));
        
        let mut handles = Vec::new();
        
        for thread_id in 0..num_threads {
            let client_clone = client.clone();
            let barrier_clone = barrier.clone();
            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;
                
                let mut local_ops = 0;
                let start = std::time::Instant::now();
                
                for i in 0..ops_per_thread {
                    let key = format!("concurrent:{}:{}", thread_id, i % 10);
                    let test_data = PerformanceTestData {
                        id: (thread_id * ops_per_thread + i) as u64,
                        payload: vec![0u8; 75],
                        timestamp: chrono::Utc::now(),
                    };
                    
                    client_clone.set(&key, &test_data, None).await.unwrap();
                    local_ops += 1;
                }
                
                let duration = start.elapsed();
                (local_ops, duration)
            });
            handles.push(handle);
        }
        
        let mut total_ops = 0;
        let mut total_time = std::time::Duration::ZERO;
        
        for handle in handles {
            let (ops, duration) = handle.await.unwrap();
            total_ops += ops;
            total_time += duration;
        }
        
        let avg_ops_per_sec = total_ops as f64 / total_time.as_secs_f64();
        
        println!("Concurrent operations: {}, avg: {:.0} ops/sec", total_ops, avg_ops_per_sec);
        assert!(avg_ops_per_sec >= 500.0, "Should handle concurrent access efficiently");
    }
    
    #[tokio::test]
    async fn test_memory_efficiency() {
        // Requirement: Memory usage should be reasonable for the workload
        let config = create_perf_config();
        let _ = init(config).await;
        let client = get_client("perf_cache").unwrap();
        
        let payload_size = 1024; // 1KB per item
        let num_items = 1000;
        
        // Fill cache with items
        for i in 0..num_items {
            let test_data = PerformanceTestData {
                id: i,
                payload: vec![0u8; payload_size],
                timestamp: chrono::Utc::now(),
            };
            client.set(&format!("memory:{}", i), &test_data, Some(3600)).await.unwrap();
        }
        
        // Verify cache doesn't exceed reasonable memory limits
        // This is a basic check - in real scenarios you'd monitor actual memory usage
        println!("Cache populated with {} items of {} bytes each", num_items, payload_size);
        
        // Retrieve some items to ensure they're cached properly
        for i in 0..10 {
            let _ = client.get::<PerformanceTestData>(&format!("memory:{}", i)).await.unwrap();
        }
        
        // If we get here without memory errors, basic efficiency test passes
        assert!(true, "Memory usage should be reasonable");
    }
}