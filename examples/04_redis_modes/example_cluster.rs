// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis Cluster mode example
//
// This example demonstrates using Redis Cluster for
// horizontal scaling and automatic sharding.
//
// Note: This example uses L1-only mode for demonstration.
// To use with Redis Cluster, configure with:
// - cache_type: TwoLevel
// - l2.mode: Cluster
// - l2.cluster.nodes: [node1:6379, node2:6379, ...]

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct ClusterData {
    id: u64,
    partition_key: String,
    data: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real Cluster usage, configure with TwoLevel + Cluster
    let mut services = HashMap::new();

    services.insert(
        "cluster_cache".to_string(),
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

    let client = oxcache::get_client("cluster_cache")?;

    println!("Redis Cluster Mode Example");
    println!("==========================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real Cluster, configure:");
    println!("  - cache_type: TwoLevel");
    println!("  - l2.mode: Cluster");
    println!("  - l2.cluster.nodes: [node1:6379, node2:6379, ...]\n");

    // Test basic operations with different keys (would be sharded in real cluster)
    for i in 0..6 {
        let data = ClusterData {
            id: i,
            partition_key: format!("partition_{}", i % 3),
            data: vec![0u8; 100],
        };
        client
            .set(&format!("cluster:key:{}", i), &data, None)
            .await?;
        println!("  Written to partition {}: key {}", data.partition_key, i);
    }

    println!("\nCluster benefits:");
    println!("  - Horizontal scaling");
    println!("  - Automatic sharding");
    println!("  - High availability with replica");

    println!("\n✓ Cluster mode example completed!");
    Ok(())
}
