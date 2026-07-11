// Copyright (c) 2025-2026 Kirky.X
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
    async fn test_cache_set_and_get() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let user = User {
            id: 1,
            name: "Alice".to_string(),
        };

        // Set a value
        cache.set("user:1", &user).await?;

        // Get the value
        let retrieved: Option<User> = cache.get("user:1").await?;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Alice");

        Ok(())
    }

    /// Test delete operation
    #[tokio::test]
    async fn test_cache_delete() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let user = User {
            id: 1,
            name: "Alice".to_string(),
        };

        // Set a value
        cache.set("user:1", &user).await?;

        // Verify it exists
        let exists = cache.exists("user:1").await?;
        assert!(exists);

        // Delete the value
        cache.delete("user:1").await?;

        // Verify it's gone
        let retrieved: Option<User> = cache.get("user:1").await?;
        assert!(retrieved.is_none());

        Ok(())
    }

    /// Test expiration (TTL)
    #[tokio::test]
    async fn test_cache_expiration() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let user = User {
            id: 1,
            name: "Alice".to_string(),
        };

        // Set with TTL of 1 second
        cache
            .set_with_ttl("user:1", &user, Some(Duration::from_secs(1)))
            .await?;

        // Verify it exists immediately
        let exists = cache.exists("user:1").await?;
        assert!(exists);

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Verify it's gone
        let retrieved: Option<User> = cache.get("user:1").await?;
        assert!(retrieved.is_none());

        Ok(())
    }

    /// Test get_or fallback
    #[tokio::test]
    async fn test_cache_get_or_fallback() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        // Get non-existent key with fallback
        let user: User = cache
            .get_or("user:999", || async {
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
        cache.set("user:999", &user).await?;

        // Get again - should return cached value
        let cached: User = cache
            .get_or("user:999", || async {
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
    async fn test_cache批量_operations() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        let users = vec![
            ("user:1", User { id: 1, name: "Alice".to_string() }),
            ("user:2", User { id: 2, name: "Bob".to_string() }),
            ("user:3", User { id: 3, name: "Charlie".to_string() }),
        ];

        // Set many
        cache.set_many(users).await?;

        // Get many
        let keys = vec!["user:1", "user:2", "user:3"];
        let results: std::collections::HashMap<String, User> =
            cache.get_many(keys).await?;

        assert_eq!(results.len(), 3);
        assert_eq!(results.get("user:1").unwrap().name, "Alice");
        assert_eq!(results.get("user:2").unwrap().name, "Bob");
        assert_eq!(results.get("user:3").unwrap().name, "Charlie");

        Ok(())
    }

    /// Test clear all
    #[tokio::test]
    async fn test_cache_clear() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        // Add some values
        cache
            .set_many(vec![
                ("user:1", User { id: 1, name: "Alice".to_string() }),
                ("user:2", User { id: 2, name: "Bob".to_string() }),
            ])
            .await?;

        // Clear all
        cache.clear().await?;

        // Verify empty
        let len = cache.len().await?;
        assert_eq!(len, 0);

        Ok(())
    }

    /// Test stats
    #[tokio::test]
    async fn test_cache_stats() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        // Add some values
        cache
            .set_many(vec![
                ("user:1", User { id: 1, name: "Alice".to_string() }),
                ("user:2", User { id: 2, name: "Bob".to_string() }),
            ])
            .await?;

        // Get stats
        let stats = cache.stats().await?;

        // Stats should contain some information
        assert!(!stats.is_empty());

        Ok(())
    }
}
