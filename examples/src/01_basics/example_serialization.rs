// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Serialization options example
//
// This example demonstrates different serialization options:
// - JSON serialization (human-readable)
// - Bincode serialization (binary, more efficient)

use oxcache::Cache;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    profile: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, User> = Cache::new().await?;

    println!("Serialization Options Example");
    println!("============================\n");

    // Create test data with JSON
    let user = User {
        id: 1,
        name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        profile: serde_json::json!({
            "age": 30,
            "city": "New York",
            "skills": ["Rust", "Redis", "Cache"]
        }),
    };

    println!("1. JSON Serialization (default):");
    println!("   - Human-readable format");
    println!("   - Good for debugging");
    println!("   - Slightly larger size\n");

    // Store with JSON
    cache.set(&"user:json:1".to_string(), &user).await?;
    println!("   Stored user: {} - {}", user.name, user.email);

    // Retrieve
    if let Some(cached) = cache.get(&"user:json:1".to_string()).await? {
        println!("   Retrieved: {} - {}", cached.name, cached.email);
        println!("   Profile: {:?}\n", cached.profile);
    }

    println!("2. Serialization Features:");
    println!("   - JSON: Standard format, readable");
    println!("   - Bincode: Binary format, efficient (when enabled)");
    println!("   - Custom serializers supported\n");

    println!("✓ Serialization example completed!");
    Ok(())
}
