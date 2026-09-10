// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified cache interface that consolidates CacheOps, CacheExt, and CacheBackend
//! This provides a single, comprehensive interface for all cache operations

use crate::error::OxCacheResult;

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::Serializer;
use async_trait::async_trait;
#[cfg(any(feature = "serialization", feature = "full"))]
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Core cache operations trait - unified interface for all cache backends
///
/// This trait combines the functionality of CacheOps, CacheExt, and CacheBackend
/// into a single, comprehensive interface. It provides both low-level byte operations
/// and high-level typed operations.
///
/// # Object safety (T313)
///
/// 本 trait 是 **object-safe 核心**（`Arc<dyn UnifiedCache>` 可用）：泛型
/// 的 typed 读写已拆分到 [`TypedCacheExt`]（blanket impl 保持所有既有调用
/// 点兼容），关闭 kit 模块记录的 dyn-safety 设计分歧（H1）。
#[async_trait]
pub trait UnifiedCache: Send + Sync + 'static {
    // ============================================================================
    // Core byte-level operations (from CacheBackend)
    // ============================================================================

    /// Get raw bytes from cache
    async fn get_bytes(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>>;

    /// Set raw bytes in cache with optional TTL
    async fn set_bytes(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()>;

    /// Delete a key from cache
    async fn delete(&self, key: &str) -> OxCacheResult<()>;

    /// Check if key exists in cache
    async fn exists(&self, key: &str) -> OxCacheResult<bool>;

    /// Clear all cache entries
    async fn clear(&self) -> OxCacheResult<()>;

    /// Shutdown the cache and release resources
    async fn shutdown(&self);

    /// Get TTL for a key
    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>>;

    /// Set TTL for an existing key
    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool>;

    /// Health check for the cache backend
    async fn health_check(&self) -> OxCacheResult<()>;

    /// Get cache statistics
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>>;

    // ============================================================================
    // Required methods for implementation
    // ============================================================================

    /// Get the serializer used by this cache
    // Serializer trait 仅在 serialization/full feature 下可用
    #[cfg(any(feature = "serialization", feature = "full"))]
    fn serializer(&self) -> &dyn Serializer;

    /// Get the backend type for runtime identification
    fn backend_kind(&self) -> crate::backend::interface::BackendKind;
}

/// Typed read/write extension (T313, generic — dyn-incompatible by nature)
///
/// 通过 blanket impl 自动为所有 `UnifiedCache`（含 `dyn UnifiedCache`）
/// 提供 `get_typed` / `set_typed`；既有调用点在 trait 可见时无需改动。
///
/// ```rust,ignore
/// use oxcache::cache::{UnifiedCache, TypedCacheExt};
/// let cache: Arc<dyn UnifiedCache> = Arc::new(moka_backend);
/// let v: Option<User> = cache.get_typed("user:1").await?;
/// ```
#[cfg(any(feature = "serialization", feature = "full"))]
#[async_trait]
pub trait TypedCacheExt: Send + Sync {
    /// Get typed value from cache
    async fn get_typed<T: DeserializeOwned + Send>(&self, key: &str) -> OxCacheResult<Option<T>>;

    /// Set typed value in cache
    async fn set_typed<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()>;
}

#[cfg(any(feature = "serialization", feature = "full"))]
#[async_trait]
impl<T: UnifiedCache + ?Sized> TypedCacheExt for T {
    async fn get_typed<T2: DeserializeOwned + Send>(&self, key: &str) -> OxCacheResult<Option<T2>> {
        let bytes = self.get_bytes(key).await?;
        match bytes {
            Some(data) => {
                let val: T2 = crate::infra::serialization::depth_limited::deserialize_safe(
                    &data,
                    crate::core::MAX_JSON_DEPTH,
                )
                .map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    async fn set_typed<T2: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T2,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))?;
        self.set_bytes(key, bytes, ttl).await
    }
}

/// 兼容别名（T313）：`dyn UnifiedCache` 的可读别名
///
/// kit 集成等下游可改回 `Arc<dyn UnifiedCache>`（设计分歧关闭）。
pub type DynUnifiedCache = dyn UnifiedCache;

/// Blanket implementation for all CacheBackend implementations
#[async_trait]
impl<T: crate::backend::CacheBackend + Send + Sync> UnifiedCache for T {
    // Core operations
    async fn get_bytes(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.get(key).await
    }

    async fn set_bytes(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        self.set(Arc::from(key), Arc::new(value), ttl).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.delete(key).await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.exists(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.clear().await
    }

    async fn shutdown(&self) {
        self.shutdown().await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.ttl(key).await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.expire(key, ttl).await
    }

    async fn health_check(&self) -> OxCacheResult<()> {
        self.health_check().await
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        self.stats().await
    }

    // Default serializer implementation
    #[cfg(any(feature = "serialization", feature = "full"))]
    fn serializer(&self) -> &dyn Serializer {
        use crate::infra::{UnifiedSerializerAdapter, default_serializer};
        use once_cell::sync::Lazy;
        use std::sync::Arc;

        static DEFAULT_SERIALIZER: Lazy<Arc<UnifiedSerializerAdapter>> =
            Lazy::new(|| Arc::new(UnifiedSerializerAdapter::new(default_serializer())));

        DEFAULT_SERIALIZER.as_ref() as &dyn Serializer
    }

    fn backend_kind(&self) -> crate::backend::interface::BackendKind {
        self.backend_kind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;
    #[cfg(any(feature = "serialization", feature = "full"))]
    use serde::{Deserialize, Serialize};

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
    }

    fn make_backend() -> MokaMemoryBackend {
        MokaMemoryBackend::builder().capacity(100).build()
    }

    #[tokio::test]
    async fn test_unified_cache_get_bytes_set_bytes() {
        let backend = make_backend();
        backend
            .set_bytes("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        let result = backend.get_bytes("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_unified_cache_get_bytes_missing() {
        let backend = make_backend();
        let result = backend.get_bytes("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_unified_cache_set_bytes_with_ttl() {
        let backend = make_backend();
        backend
            .set_bytes("key1", b"value1".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let result = backend.get_bytes("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_unified_cache_delete() {
        let backend = make_backend();
        backend
            .set_bytes("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        assert!(backend.exists("key1").await.unwrap());
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_unified_cache_exists() {
        let backend = make_backend();
        assert!(!backend.exists("missing").await.unwrap());
        backend
            .set_bytes("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        assert!(backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_unified_cache_clear() {
        let backend = make_backend();
        backend
            .set_bytes("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        backend
            .set_bytes("key2", b"value2".to_vec(), None)
            .await
            .unwrap();
        backend.clear().await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_unified_cache_health_check() {
        let backend = make_backend();
        assert!(backend.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_unified_cache_stats() {
        let backend = make_backend();
        backend
            .set_bytes("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        let stats = backend.stats().await.unwrap();
        assert!(!stats.is_empty());
    }

    #[tokio::test]
    async fn test_unified_cache_shutdown() {
        let backend = make_backend();
        // Should not panic
        backend.shutdown().await;
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[tokio::test]
    async fn test_unified_cache_get_typed() {
        let backend = make_backend();
        let data = TestData {
            id: 42,
            name: "test".to_string(),
        };
        backend.set_typed("key1", &data, None).await.unwrap();
        let result: Option<TestData> = backend.get_typed("key1").await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[tokio::test]
    async fn test_unified_cache_get_typed_missing() {
        let backend = make_backend();
        let result: Option<TestData> = backend.get_typed("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[tokio::test]
    async fn test_unified_cache_set_typed_with_ttl() {
        let backend = make_backend();
        let data = TestData {
            id: 1,
            name: "hello".to_string(),
        };
        backend
            .set_typed("key1", &data, Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let result: Option<TestData> = backend.get_typed("key1").await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[tokio::test]
    async fn test_unified_cache_get_typed_deserialization_error() {
        let backend = make_backend();
        // Store invalid JSON bytes
        backend
            .set_bytes("key1", b"not valid json".to_vec(), None)
            .await
            .unwrap();
        let result: OxCacheResult<Option<TestData>> = backend.get_typed("key1").await;
        assert!(result.is_err());
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[test]
    fn test_unified_cache_serializer() {
        let backend = make_backend();
        let _serializer = backend.serializer();
    }

    #[test]
    fn test_unified_cache_backend_kind() {
        let backend = make_backend();
        let kind = backend.backend_kind();
        // MokaMemoryBackend should return Moka variant
        assert_eq!(kind, crate::backend::interface::BackendKind::Moka);
    }

    // ============================================================================
    // T313: 对象安全拆分 —— Arc<dyn UnifiedCache> 可用，typed 走 TypedCacheExt
    // ============================================================================

    #[cfg(any(feature = "serialization", feature = "full"))]
    mod object_safety {
        use super::*;
        use crate::cache::TypedCacheExt;

        #[tokio::test]
        async fn dyn_unified_cache_is_usable() {
            // 编译期断言：dyn UnifiedCache 成立（此前 get_typed/set_typed 泛型方法使其 dyn 不安全）
            let cache: Arc<dyn UnifiedCache> = Arc::new(make_backend());

            cache
                .set_bytes("user:1", serde_json::to_vec(&TestData { id: 1, name: "a".into() }).unwrap(), None)
                .await
                .unwrap();
            let raw = cache.get_bytes("user:1").await.unwrap().unwrap();
            assert!(raw.starts_with(b"{"), "JSON 序列化的对象字节");

            let dyn_ref: &DynUnifiedCache = cache.as_ref();
            dyn_ref.health_check().await.unwrap();
            assert_eq!(dyn_ref.backend_kind(), crate::backend::interface::BackendKind::Moka);
        }

        #[tokio::test]
        async fn typed_ops_via_dyn_object_with_extension_trait() {
            let cache: Arc<dyn UnifiedCache> = Arc::new(make_backend());
            let data = TestData { id: 42, name: "dyn".into() };

            cache.set_typed("k", &data, None).await.unwrap();
            let back: Option<TestData> = cache.get_typed("k").await.unwrap();
            assert_eq!(back, Some(data));
        }

        #[tokio::test]
        async fn typed_ops_on_concrete_backend_still_compile() {
            // 兼容性：具体后端上 get_typed/set_typed 经 blanket impl 照常可用
            let backend = make_backend();
            let data = TestData { id: 7, name: "concrete".into() };
            backend.set_typed("k", &data, None).await.unwrap();
            let back: Option<TestData> = backend.get_typed("k").await.unwrap();
            assert_eq!(back, Some(data));
        }
    }
}
