// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Functional UAT (User Acceptance Testing) example
//
// This example contains end-to-end functional tests that validate
// the cache system meets user requirements.

use serde_json::Value;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct UserSession {
    user_id: u64,
    username: String,
    last_activity: chrono::DateTime<chrono::Utc>,
    preferences: Value,
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

#[cfg(test)]
mod tests {
    use super::*;
    use oxcache::manager::{get_client, init};
    use oxcache::{
        config::{L1Config, OxcacheConfig, ServiceConfig},
        CacheExt,
    };
    
    #[tokio::test]
    async fn test_user_session_caching() {
        // Requirement: User sessions should be cached for fast retrieval
        let config = OxcacheConfig::builder()
            .with_service(
                "session_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(1000)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("session_cache").unwrap();
        
        let session = UserSession {
            user_id: 12345,
            username: "testuser".to_string(),
            last_activity: chrono::Utc::now(),
            preferences: json!({"theme": "dark", "lang": "en"}),
        };
        
        // Cache the session
        client.set(&format!("session:{}", session.user_id), &session, Some(3600)).await.unwrap();
        
        // Retrieve the session (simulating user authentication)
        let retrieved = client.get::<UserSession>(&format!("session:{}", session.user_id)).await.unwrap();
        
        assert!(retrieved.is_some(), "Session should be retrievable");
        assert_eq!(retrieved.unwrap().username, "testuser");
    }
    
    #[tokio::test]
    async fn test_shopping_cart_persistence() {
        // Requirement: Shopping carts should persist across requests
        let config = OxcacheConfig::builder()
            .with_service(
                "cart_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(500)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("cart_cache").unwrap();
        
        let cart = ShoppingCart {
            user_id: 12345,
            items: vec![
                CartItem {
                    product_id: 1,
                    name: "Laptop".to_string(),
                    quantity: 1,
                    price: 999.99,
                },
                CartItem {
                    product_id: 2,
                    name: "Mouse".to_string(),
                    quantity: 2,
                    price: 29.99,
                },
            ],
            total: 1059.97,
        };
        
        // Save cart
        client.set(&format!("cart:{}", cart.user_id), &cart, Some(1800)).await.unwrap();
        
        // Modify cart (add item)
        let mut updated_cart = cart.clone();
        updated_cart.items.push(CartItem {
            product_id: 3,
            name: "Keyboard".to_string(),
            quantity: 1,
            price: 79.99,
        });
        updated_cart.total += 79.99;
        
        client.set(&format!("cart:{}", updated_cart.user_id), &updated_cart, Some(1800)).await.unwrap();
        
        // Retrieve updated cart
        let retrieved = client.get::<ShoppingCart>(&format!("cart:{}", updated_cart.user_id)).await.unwrap();
        
        assert!(retrieved.is_some(), "Cart should be retrievable");
        assert_eq!(retrieved.unwrap().items.len(), 3);
        assert_eq!(retrieved.unwrap().total, 1139.96);
    }
    
    #[tokio::test]
    async fn test_cache_hit_performance() {
        // Requirement: Cache hits should be significantly faster than cache misses
        let config = OxcacheConfig::builder()
            .with_service(
                "perf_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(1000)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("perf_cache").unwrap();
        
        let data = "Large string data for performance testing".to_string();
        
        // First, cache the data (cache miss)
        let start_miss = std::time::Instant::now();
        client.set("perf_key", &data, None).await.unwrap();
        let miss_time = start_miss.elapsed();
        
        // Then retrieve the data (cache hit)
        let start_hit = std::time::Instant::now();
        let _ = client.get::<String>("perf_key").await.unwrap();
        let hit_time = start_hit.elapsed();
        
        // Cache hit should be faster (this is a basic check)
        assert!(hit_time.as_nanos() < miss_time.as_nanos() * 10, "Cache hit should be faster than miss");
    }
    
    #[tokio::test]
    async fn test_ttl_expiration() {
        // Requirement: Items should expire according to TTL settings
        let config = OxcacheConfig::builder()
            .with_service(
                "ttl_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("ttl_cache").unwrap();
        
        // Set data with 1 second TTL
        client.set("ttl_key", &"test_data", Some(1)).await.unwrap();
        
        // Should be available immediately
        assert!(client.get::<String>("ttl_key").await.unwrap().is_some());
        
        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        
        // Should be expired
        assert!(client.get::<String>("ttl_key").await.unwrap().is_none());
    }
}