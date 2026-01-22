// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// SQLite cache example
//
// This example demonstrates using oxcache with SQLite database
// for lightweight caching scenarios.
//
// Note: This example uses L1-only mode for demonstration.
// To use with real SQLite, configure with:
// - database: Some(DatabaseConfig { type: Sqlite, connection_string: ... })
// Or use oxcache's database loader for DB-first caching patterns.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Setting {
    key: String,
    value: String,
    description: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real DB caching, configure with database field
    let config = OxcacheConfig::builder()
        .with_service(
            "sqlite_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(1000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("sqlite_cache")?;

    println!("SQLite Cache Example");
    println!("====================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real SQLite caching:");
    println!("  - database: Some(DatabaseConfig {{ type: Sqlite, ... }})");
    println!("  - Use oxcache::DbLoader for DB-first caching patterns\n");

    // Simulate reading application settings
    let settings: Vec<(&str, &str, Option<&str>)> = vec![
        ("app.name", "My Application", Some("Application name")),
        ("app.version", "1.0.0", Some("Application version")),
        ("debug.mode", "false", Some("Enable debug mode")),
    ];

    println!("Caching settings...");
    for (key, value, desc) in &settings {
        let setting = Setting {
            key: key.to_string(),
            value: value.to_string(),
            description: desc.map(|s| s.to_string()),
        };
        client.set(&format!("setting:{}", key), &setting, None).await?;
        println!("  Cached: {} = {}", key, value);
    }

    println!("\nReading settings from cache...");
    for (key, _, _) in &settings {
        if let Some(cached) = client.get::<Setting>(&format!("setting:{}", key)).await? {
            println!("  {} = {}", cached.key, cached.value);
        }
    }

    println!("\n✓ SQLite cache example completed!");
    Ok(())
}
