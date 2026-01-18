// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis TLS mode example
//
// This example demonstrates using Redis with TLS encryption
// for secure communication.
//
// Note: This example uses L1-only mode for demonstration.
// To use with TLS, configure with:
// - cache_type: TwoLevel
// - l2.connection_string: rediss://host:6380 (note: rediss://)
// - l2.enable_tls: true

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct SecureData {
    id: u64,
    sensitive_info: String,
    encrypted_field: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real TLS usage, configure with TwoLevel + enable_tls
    let mut services = HashMap::new();

    services.insert(
        "tls_cache".to_string(),
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

    let client = oxcache::get_client("tls_cache")?;

    println!("Redis TLS Mode Example");
    println!("======================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real TLS, configure:");
    println!("  - cache_type: TwoLevel");
    println!("  - l2.connection_string: rediss://host:6380");
    println!("  - l2.enable_tls: true\n");

    // Test basic operations
    let data = SecureData {
        id: 1,
        sensitive_info: "Secret data".to_string(),
        encrypted_field: vec![0u8; 32],
    };

    println!("Writing encrypted data...");
    client.set("tls:test", &data, None).await?;
    println!("  Wrote: ID={}, info={}", data.id, data.sensitive_info);

    println!("\nReading data...");
    if let Some(cached) = client.get::<SecureData>("tls:test").await? {
        println!("  Read: ID={}, info={}", cached.id, cached.sensitive_info);
    }

    println!("\nTLS benefits:");
    println!("  - Encrypted communication");
    println!("  - Data in transit protection");
    println!("  - Compliance with security standards");

    println!("\n✓ TLS mode example completed!");
    Ok(())
}
