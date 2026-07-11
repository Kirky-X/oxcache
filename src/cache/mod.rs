// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified Cache interface for the modernized cache API

// Submodules
pub mod api;
pub mod builder;
pub mod chain;
pub mod interface;

// Re-exports
pub use api::Cache;
pub use builder::CacheBuilder;
pub use chain::{ChainCache, ChainCacheBuilder, ChainLink};
pub use interface::UnifiedCache;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct TestValue {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn test_cache_basic() {
        let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();

        let value = TestValue {
            id: 1,
            name: "test".to_string(),
        };

        cache.set(&"test:1".to_string(), &value).await.unwrap();
        let result: Option<TestValue> = cache.get(&"test:1".to_string()).await.unwrap();
        assert_eq!(result, Some(value));
    }

    #[tokio::test]
    async fn test_cache_get_or() {
        let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();

        // First call should use fallback
        async fn fallback1() -> crate::error::OxCacheResult<TestValue> {
            Ok(TestValue {
                id: 1,
                name: "fallback".to_string(),
            })
        }

        let result = cache.get_or(&"test:2".to_string(), fallback1).await.unwrap();
        assert_eq!(result.name, "fallback");

        // Second call should use cache
        async fn fallback2() -> crate::error::OxCacheResult<TestValue> {
            panic!("Should not be called");
        }

        let result = cache.get_or(&"test:2".to_string(), fallback2).await.unwrap();
        assert_eq!(result.name, "fallback");
    }

    #[tokio::test]
    async fn test_cache_batch_operations() {
        let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();

        let items = [
            (
                "batch:1".to_string(),
                TestValue {
                    id: 1,
                    name: "a".to_string(),
                },
            ),
            (
                "batch:2".to_string(),
                TestValue {
                    id: 2,
                    name: "b".to_string(),
                },
            ),
        ];

        cache.set_many(items.iter().map(|(k, v)| (k, v))).await.unwrap();

        let keys: Vec<&String> = items.iter().map(|(k, _)| k).collect();
        let results = cache.get_many(keys).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();

        cache
            .set(
                &"clear:1".to_string(),
                &TestValue {
                    id: 1,
                    name: "test".to_string(),
                },
            )
            .await
            .unwrap();

        cache.clear().await.unwrap();
        let result: Option<TestValue> = cache.get(&"clear:1".to_string()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache: Cache<String, TestValue> = Cache::builder().build().await.unwrap();
        let stats = cache.stats().await.unwrap();
        assert!(!stats.is_empty());
    }
}
