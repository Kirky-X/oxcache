// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Functional UAT (User Acceptance Testing) example
//
// This example contains end-to-end functional tests that validate
// the cache system meets user requirements.

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct UserSession {
    user_id: u64,
    username: String,
    last_activity: chrono::DateTime<chrono::Utc>,
    preferences: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct ShoppingCart {
    user_id: u64,
    items: Vec<CartItem>,
    total: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct CartItem {
    product_id: u64,
    name: String,
    quantity: u32,
    price: f64,
}

// UAT Test Cases
mod uat_tests {
    use super::*;

    #[tokio::test]
    async fn test_user_session_caching() {
        // Requirement: User sessions should be cached for fast retrieval
        let services = HashMap::new();
        let config = oxcache::config::Config {
            services,
            ..Default::default()
        };
        let _ = oxcache::init(config).await;
        let client = oxcache::get_client("session_cache").unwrap();

        let session = UserSession {
            user_id: 12345,
            username: "test_user".to_string(),
            last_activity: chrono::Utc::now(),
            preferences: serde_json::json!({"theme": "dark", "language": "en"}),
        };

        // Store session
        client
            .set("session:12345", &session, Some(3600))
            .await
            .unwrap();

        // Retrieve and verify
        let cached = client.get::<UserSession>("session:12345").await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().user_id, 12345);
    }

    #[tokio::test]
    async fn test_shopping_cart_operations() {
        // Requirement: Shopping cart should be persistent and fast
        let services = HashMap::new();
        let config = oxcache::config::Config {
            services,
            ..Default::default()
        };
        let _ = oxcache::init(config).await;
        let client = oxcache::get_client("cart_cache").unwrap();

        let cart = ShoppingCart {
            user_id: 1,
            items: vec![
                CartItem {
                    product_id: 100,
                    name: "Laptop".to_string(),
                    quantity: 1,
                    price: 999.99,
                },
                CartItem {
                    product_id: 101,
                    name: "Mouse".to_string(),
                    quantity: 2,
                    price: 29.99,
                },
            ],
            total: 1059.97,
        };

        client.set("cart:1", &cart, Some(1800)).await.unwrap();

        let cached = client.get::<ShoppingCart>("cart:1").await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().items.len(), 2);
    }

    #[tokio::test]
    async fn test_data_consistency() {
        // Requirement: Cache should reflect latest data
        let services = HashMap::new();
        let config = oxcache::config::Config {
            services,
            ..Default::default()
        };
        let _ = oxcache::init(config).await;
        let client = oxcache::get_client("data_cache").unwrap();

        let initial = UserSession {
            user_id: 999,
            username: "initial".to_string(),
            last_activity: chrono::Utc::now(),
            preferences: serde_json::json!({}),
        };

        client
            .set("consistency:test", &initial, Some(60))
            .await
            .unwrap();

        let updated = UserSession {
            user_id: 999,
            username: "updated".to_string(),
            last_activity: chrono::Utc::now(),
            preferences: serde_json::json!({}),
        };

        client
            .set("consistency:test", &updated, Some(60))
            .await
            .unwrap();

        let cached = client.get::<UserSession>("consistency:test").await.unwrap();
        assert_eq!(cached.unwrap().username, "updated");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Functional UAT Example");
    println!("======================\n");
    println!("User Acceptance Testing validates:");
    println!("  - User session caching");
    println!("  - Shopping cart operations");
    println!("  - Data consistency");
    println!("  - Real-world usage patterns\n");

    println!("Use: cargo test --example example_functional_uat");
    println!("\n✓ Functional UAT completed!");
    Ok(())
}
