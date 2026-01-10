// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// MySQL cache example
//
// This example demonstrates using oxcache with MySQL database
// for the cache-aside pattern.
//
// Note: This example uses L1-only mode for demonstration.
// To use with real MySQL, configure with:
// - database: Some(DatabaseConfig { type: Mysql, connection_string: ... })
// - Use oxcache::DbLoader for DB-first caching patterns

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct User {
    id: u64,
    name: String,
    email: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real DB caching, configure with database field
    let mut services = HashMap::new();

    services.insert(
        "mysql_cache".to_string(),
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

    let client = oxcache::get_client("mysql_cache")?;

    println!("MySQL Cache-Aside Pattern Example");
    println!("==================================");
    println!("Note: Using L1-only mode for demo");
    println!("For real MySQL caching:");
    println!("  - database: Some(DatabaseConfig {{ type: Mysql, ... }})");
    println!("  - Use oxcache::DbLoader for DB-first caching patterns");

    // Simulate user data caching
    let users = vec![
        User {
            id: 1,
            name: "Alice".into(),
            email: "alice@example.com".into(),
            created_at: chrono::Utc::now(),
        },
        User {
            id: 2,
            name: "Bob".into(),
            email: "bob@example.com".into(),
            created_at: chrono::Utc::now(),
        },
        User {
            id: 3,
            name: "Charlie".into(),
            email: "charlie@example.com".into(),
            created_at: chrono::Utc::now(),
        },
    ];

    println!("Caching user data...");
    for user in &users {
        client.set(&format!("user:{}", user.id), user, None).await?;
        println!("  ✓ Cached user: {} ({})", user.name, user.email);
    }

    println!("");
    println!("Simulating cache retrieval...");
    let cached = client.get::<User>("user:1").await?;
    if let Some(user) = cached {
        println!("  ✓ Retrieved: {} ({})", user.name, user.email);
    }

    println!("");
    println!("Cache-Aside Pattern:");
    println!("  1. Read from cache first");
    println!("  2. On cache miss, read from DB");
    println!("  3. Populate cache with DB result");
    println!("  4. On write, invalidate/update cache");

    println!("");
    println!("✓ MySQL cache example completed!");
    Ok(())
}
