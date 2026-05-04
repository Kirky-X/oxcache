//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 字节操作方法（用于宏兼容）

use super::Cache;
use crate::core::traits::{CacheKey, Cacheable};
use crate::error::Result;
use std::sync::Arc;
use std::time::Duration;

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::serialization::Serializer;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.backend.get(key).await
    }

    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    pub async fn set_l1_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    pub async fn set_l2_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn serializer(&self) -> Arc<dyn Serializer> {
        self.serializer_pool.json()
    }

    pub fn unified_serializer(&self) -> crate::infra::serialization::unified::UnifiedSerializer {
        self.unified_serializer.clone()
    }

    pub fn supports_l1_only(&self) -> bool {
        true
    }

    pub fn supports_l2_only(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::api::Cache;

    // ========================================================================
    // get_bytes tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_bytes_returns_none_for_missing_key() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let result = cache.get_bytes("nonexistent_key").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_bytes_returns_stored_value() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let data = vec![1, 2, 3, 4, 5];
        cache.set_bytes("test_key", data.clone(), None).await.unwrap();
        let result = cache.get_bytes("test_key").await.unwrap();
        assert_eq!(result, Some(data));
    }

    // ========================================================================
    // set_bytes tests
    // ========================================================================

    #[tokio::test]
    async fn test_set_bytes_without_ttl() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let data = b"hello world".to_vec();
        let result = cache.set_bytes("hello", data.clone(), None).await;
        assert!(result.is_ok());
        let stored = cache.get_bytes("hello").await.unwrap();
        assert_eq!(stored, Some(data));
    }

    #[tokio::test]
    async fn test_set_bytes_with_ttl() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let data = b"expiring".to_vec();
        let result = cache.set_bytes("temp", data.clone(), Some(3600)).await;
        assert!(result.is_ok());
        let stored = cache.get_bytes("temp").await.unwrap();
        assert_eq!(stored, Some(data));
    }

    #[tokio::test]
    async fn test_set_bytes_overwrites_existing() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        cache.set_bytes("counter", vec![1], None).await.unwrap();
        cache.set_bytes("counter", vec![2], None).await.unwrap();
        let result = cache.get_bytes("counter").await.unwrap();
        assert_eq!(result, Some(vec![2]));
    }

    #[tokio::test]
    async fn test_set_bytes_empty_value() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let empty_data: Vec<u8> = vec![];
        cache.set_bytes("empty", empty_data.clone(), None).await.unwrap();
        let result = cache.get_bytes("empty").await.unwrap();
        assert_eq!(result, Some(empty_data));
    }

    // ========================================================================
    // set_l1_bytes tests
    // ========================================================================

    #[tokio::test]
    async fn test_set_l1_bytes_stores_value() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let data = b"l1_data".to_vec();
        cache.set_l1_bytes("l1_key", data.clone(), None).await.unwrap();
        let result = cache.get_bytes("l1_key").await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test]
    async fn test_set_l1_bytes_with_ttl() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let data = b"l1_ttl".to_vec();
        cache
            .set_l1_bytes("l1_ttl_key", data.clone(), Some(1800))
            .await
            .unwrap();
        let result = cache.get_bytes("l1_ttl_key").await.unwrap();
        assert_eq!(result, Some(data));
    }

    // ========================================================================
    // set_l2_bytes tests
    // ========================================================================

    #[tokio::test]
    async fn test_set_l2_bytes_stores_value() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let data = b"l2_data".to_vec();
        cache.set_l2_bytes("l2_key", data.clone(), None).await.unwrap();
        let result = cache.get_bytes("l2_key").await.unwrap();
        assert_eq!(result, Some(data));
    }

    // ========================================================================
    // Feature flag methods tests
    // ========================================================================

    #[tokio::test]
    async fn test_supports_l1_only_returns_true() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        assert!(cache.supports_l1_only());
    }

    #[tokio::test]
    async fn test_supports_l2_only_returns_false() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        assert!(!cache.supports_l2_only());
    }

    #[tokio::test]
    async fn test_unified_serializer_accessible() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let serializer = cache.unified_serializer();
        // Verify it's a valid UnifiedSerializer by serializing and deserializing
        let original = "hello";
        let serialized = serializer.serialize(&original).unwrap();
        let deserialized: String = serializer.deserialize(&serialized).unwrap();
        assert_eq!(deserialized, original);
    }

    // ========================================================================
    // serializer() feature-gated test
    // ========================================================================

    #[tokio::test]
    #[cfg(any(feature = "serialization", feature = "full"))]
    async fn test_serializer_returns_json() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let serializer = cache.serializer();
        // JsonSerializer should be returned - serialize then deserialize to verify
        let original = b"hello world";
        let serialized = serializer.serialize("bytes", original).unwrap();
        let deserialized = serializer.deserialize("bytes", &serialized).unwrap();
        assert_eq!(deserialized, original);
    }

    // ========================================================================
    // Large data tests
    // ========================================================================

    #[tokio::test]
    async fn test_set_bytes_large_data() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let large_data = vec![0xAB; 1024 * 100]; // 100KB
        cache.set_bytes("large", large_data.clone(), None).await.unwrap();
        let result = cache.get_bytes("large").await.unwrap();
        assert_eq!(result, Some(large_data));
    }
}
