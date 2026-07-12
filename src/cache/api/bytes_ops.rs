// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Cache 字节操作方法（用于宏兼容）

use super::Cache;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
use std::sync::Arc;
use std::time::Duration;

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::serialization::Serializer;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn get_bytes(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.backend.get(key).await
    }

    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> OxCacheResult<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    /// Synchronously get raw bytes from the cache (macro-compatible sync path).
    ///
    /// Returns `Err(NotSupported)` if `sync_mode(true)` was not set on the
    /// builder (i.e., `backend_sync` is `None`).
    pub fn get_bytes_sync(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let backend = self.backend_sync.as_ref().ok_or_else(|| {
            OxCacheError::NotSupported(
                "sync byte API requires CacheBuilder::sync_mode(true); backend_sync is None".to_string(),
            )
        })?;
        backend.get(key)
    }

    /// Synchronously set raw bytes in the cache (macro-compatible sync path).
    ///
    /// `ttl` is in seconds (matching the async `set_bytes` signature for
    /// macro symmetry). Returns `Err(NotSupported)` if `sync_mode(true)` was
    /// not set on the builder.
    pub fn set_bytes_sync(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> OxCacheResult<()> {
        let backend = self.backend_sync.as_ref().ok_or_else(|| {
            OxCacheError::NotSupported(
                "sync byte API requires CacheBuilder::sync_mode(true); backend_sync is None".to_string(),
            )
        })?;
        let ttl_duration = ttl.map(Duration::from_secs);
        backend.set(key, value, ttl_duration)
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn serializer(&self) -> Arc<dyn Serializer> {
        self.serializer.clone()
    }

    pub fn unified_serializer(&self) -> crate::infra::serialization::unified::UnifiedSerializer {
        self.unified_serializer.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::Cache;

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

    // ========================================================================
    // get_bytes_sync / set_bytes_sync tests
    // ========================================================================

    // NOTE: multi_thread flavor required — MokaMemoryBackend's sync_block_on
    // uses block_in_place, which panics on current_thread runtimes.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_bytes_sync_returns_none_for_missing_key() {
        let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
        let result = cache.get_bytes_sync("nonexistent_key").unwrap();
        assert!(result.is_none(), "missing key should return None");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_bytes_sync_then_get_bytes_sync_roundtrip() {
        let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
        let data = vec![1, 2, 3, 4, 5];
        cache.set_bytes_sync("test_key", data.clone(), None).unwrap();
        let result = cache.get_bytes_sync("test_key").unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_bytes_sync_with_ttl_roundtrip() {
        let cache: Cache<String, Vec<u8>> = Cache::builder().sync_mode(true).build().await.unwrap();
        let data = b"expiring".to_vec();
        cache.set_bytes_sync("temp", data.clone(), Some(3600)).unwrap();
        let result = cache.get_bytes_sync("temp").unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_bytes_sync_without_sync_mode_returns_not_supported() {
        // Default builder (sync_mode=false) should return Err(NotSupported)
        let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
        let result = cache.get_bytes_sync("any_key");
        assert!(
            matches!(result, Err(crate::error::OxCacheError::NotSupported(_))),
            "expected Err(NotSupported) when sync_mode is false, got {:?}",
            result
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_bytes_sync_without_sync_mode_returns_not_supported() {
        let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
        let result = cache.set_bytes_sync("any_key", vec![1], None);
        assert!(
            matches!(result, Err(crate::error::OxCacheError::NotSupported(_))),
            "expected Err(NotSupported) when sync_mode is false, got {:?}",
            result
        );
    }
}
