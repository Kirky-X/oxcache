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

use oxcache::CacheExt;
use std::collections::HashMap;

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
    let mut services = HashMap::new();

    services.insert(
        "sqlite_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 1000,
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

    let client = oxcache::get_client("sqlite_cache")?;

    println!("SQLite Cache Example");
    println!("====================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real SQLite caching:");
    println!("  - database: Some(DatabaseConfig {{ type: Sqlite, ... }})");
    println!("  - Use oxcache::DbLoader for DB-first caching patterns\n");

    // Simulate reading application settings
    let settings: Vec<(&str, &str, Option<&str>)> = vec![
        ("app.name", "My Application", Some("Application name")),
        ("app.version", "1.0.0", Some("Current version")),
        ("app.debug", "false", Some("Debug mode")),
        ("database.path", "/data/app.db", Some("Database file path")),
    ];

    println!("Loading application settings...");
    for item in &settings {
        let setting = Setting {
            key: item.0.to_string(),
            value: item.1.to_string(),
            description: item.2.map(|s| s.to_string()),
        };
        client
            .set(&format!("setting:{}", item.0), &setting, None)
            .await?;
        println!("  ✓ {} = {}", item.0, item.1);
    }

    println!("\nSimulating cache retrieval...");
    let cached = client.get::<Setting>("setting:app.name").await?;
    if let Some(setting) = cached {
        println!("  ✓ Retrieved: {} = {}", setting.key, setting.value);
    }

    println!("\nSQLite benefits:");
    println!("  - Embedded database (no server needed)");
    println!("  - Zero configuration");
    println!("  - Perfect for single-machine caching");

    println!("\n✓ SQLite cache example completed!");
    Ok(())
}
