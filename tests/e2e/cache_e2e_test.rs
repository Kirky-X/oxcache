// SPDX-License-Identifier: MIT
//! E2E tests for oxcache Cache operations
//!
//! Tests cover: set, get, delete, expiration scenarios

#[cfg(test)]
mod tests {
    use oxcache::Cache;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct User {
        id: u64,
        name: String,
    }

    /// Test basic set and get operations
    #[tokio::test]
    async fn test_cache_set_and_get() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let user = User {
            id: 1,
            name: "Alice".to_string(),
        };

        let key = "user:1".to_string();

        // Set a value
        cache.set(&key, &user).await?;

        // Get the value
        let retrieved: Option<User> = cache.get(&key).await?;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Alice");

        Ok(())
    }

    /// Test delete operation
    #[tokio::test]
    async fn test_cache_delete() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let user = User {
            id: 1,
            name: "Alice".to_string(),
        };

        let key = "user:1".to_string();

        // Set a value
        cache.set(&key, &user).await?;

        // Verify it exists
        let exists = cache.exists(&key).await?;
        assert!(exists);

        // Delete the value
        cache.delete(&key).await?;

        // Verify it's gone
        let retrieved: Option<User> = cache.get(&key).await?;
        assert!(retrieved.is_none());

        Ok(())
    }

    /// Test expiration (TTL)
    #[tokio::test]
    async fn test_cache_expiration() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let user = User {
            id: 1,
            name: "Alice".to_string(),
        };

        let key = "user:1".to_string();

        // Set with TTL of 1 second
        cache
            .set_with_ttl(&key, &user, Some(Duration::from_secs(1)))
            .await?;

        // Verify it exists immediately
        let exists = cache.exists(&key).await?;
        assert!(exists);

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Verify it's gone
        let retrieved: Option<User> = cache.get(&key).await?;
        assert!(retrieved.is_none());

        Ok(())
    }

    /// Test get_or fallback
    #[tokio::test]
    async fn test_cache_get_or_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let key = "user:999".to_string();

        // Get non-existent key with fallback
        let user: User = cache
            .get_or(&key, || async {
                Ok(User {
                    id: 999,
                    name: "Fallback".to_string(),
                })
            })
            .await?;

        assert_eq!(user.name, "Fallback");

        // Now set the value
        let user = User {
            id: 999,
            name: "Updated".to_string(),
        };
        cache.set(&key, &user).await?;

        // Get again - should return cached value
        let cached: User = cache
            .get_or(&key, || async {
                Ok(User {
                    id: 999,
                    name: "Fallback".to_string(),
                })
            })
            .await?;

        assert_eq!(cached.name, "Updated");

        Ok(())
    }

    /// Test set_many and get_many
    #[tokio::test]
    async fn test_cache批量_operations() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let users = vec![
            ("user:1".to_string(), User { id: 1, name: "Alice".to_string() }),
            ("user:2".to_string(), User { id: 2, name: "Bob".to_string() }),
            ("user:3".to_string(), User { id: 3, name: "Charlie".to_string() }),
        ];

        // Set many
        cache.set_many(users.iter().map(|(k, v)| (k, v))).await?;

        // Get many
        let keys: Vec<String> = vec!["user:1", "user:2", "user:3"]
            .into_iter()
            .map(String::from)
            .collect();
        let results: std::collections::HashMap<String, User> =
            cache.get_many(keys.iter()).await?;

        assert_eq!(results.len(), 3);
        assert_eq!(results.get("user:1").unwrap().name, "Alice");
        assert_eq!(results.get("user:2").unwrap().name, "Bob");
        assert_eq!(results.get("user:3").unwrap().name, "Charlie");

        Ok(())
    }

    /// Test clear all
    #[tokio::test]
    async fn test_cache_clear() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        // Add some values
        let users = vec![
            ("user:1".to_string(), User { id: 1, name: "Alice".to_string() }),
            ("user:2".to_string(), User { id: 2, name: "Bob".to_string() }),
        ];
        cache.set_many(users.iter().map(|(k, v)| (k, v))).await?;

        // Clear all
        cache.clear().await?;

        // Verify empty
        let len = cache.len().await?;
        assert_eq!(len, 0);

        Ok(())
    }

    /// Test stats
    #[tokio::test]
    async fn test_cache_stats() -> Result<(), Box<dyn std::error::Error>> {
        let cache: Cache<String, User> = Cache::memory().await?;

        // Add some values
        let users = vec![
            ("user:1".to_string(), User { id: 1, name: "Alice".to_string() }),
            ("user:2".to_string(), User { id: 2, name: "Bob".to_string() }),
        ];
        cache.set_many(users.iter().map(|(k, v)| (k, v))).await?;

        // Get stats
        let stats = cache.stats().await?;

        // Stats should contain some information
        assert!(!stats.is_empty());

        Ok(())
    }
}
