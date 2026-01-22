// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// HTTP Cache Example
//
// This example demonstrates HTTP caching functionality
// including ETags, conditional requests, and middleware.
//
// Note: Requires `http-cache` feature.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct HttpResponse {
    status: u16,
    headers: std::collections::HashMap<String, String>,
    body: String,
    etag: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("HTTP Cache Example");
    println!("==================\n");
    println!("Note: Using L1-only mode for demo");
    println!("HTTP Cache features:");
    println!("  - ETag support for conditional requests");
    println!("  - Cache-Control header parsing");
    println!("  - Last-Modified and If-Modified-Since");
    println!("  - Vary header handling");
    println!("  - Axum middleware integration\n");

    let config = OxcacheConfig::builder()
        .with_service(
            "http_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("http_cache")?;

    // Simulate HTTP response caching
    let response = HttpResponse {
        status: 200,
        headers: {
            let mut h = std::collections::HashMap::new();
            h.insert("Content-Type".to_string(), "application/json".to_string());
            h.insert("Cache-Control".to_string(), "max-age=3600".to_string());
            h
        },
        body: r#"{"message": "Hello from cached response"}"#.to_string(),
        etag: Some("\"abc123\"".to_string()),
    };

    println!("Caching HTTP response...");
    client.set("http:/api/data", &response, Some(3600)).await?;
    println!("  Cached response with ETag: {}", response.etag.as_deref().unwrap_or("none"));

    println!("\nRetrieving cached response...");
    if let Some(cached) = client.get::<HttpResponse>("http:/api/data").await? {
        println!("  Status: {}", cached.status);
        println!("  Content-Type: {}", cached.headers.get("Content-Type").unwrap_or(&"unknown".to_string()));
        println!("  Cache-Control: {}", cached.headers.get("Cache-Control").unwrap_or(&"none".to_string()));
        println!("  Body: {}", cached.body);
    }

    println!("\nHTTP Cache Benefits:");
    println!("  - Reduces server load");
    println!("  - Improves response times");
    println!("  - Bandwidth savings");
    println!("  - Better user experience");

    println!("\n✓ HTTP cache example completed!");
    Ok(())
}