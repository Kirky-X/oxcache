//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified cache interface that consolidates CacheOps, CacheExt, and CacheBackend
//! This provides a single, comprehensive interface for all cache operations

use crate::error::Result;

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::serialization::Serializer;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Core cache operations trait - unified interface for all cache backends
///
/// This trait combines the functionality of CacheOps, CacheExt, and CacheBackend
/// into a single, comprehensive interface. It provides both low-level byte operations
/// and high-level typed operations.
#[async_trait]
pub trait UnifiedCache: Send + Sync + 'static {
    // ============================================================================
    // Core byte-level operations (from CacheBackend)
    // ============================================================================

    /// Get raw bytes from cache
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set raw bytes in cache with optional TTL
    async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;

    /// Delete a key from cache
    async fn delete(&self, key: &str) -> Result<()>;

    /// Check if key exists in cache
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Clear all cache entries
    async fn clear(&self) -> Result<()>;

    /// Shutdown the cache and release resources
    async fn shutdown(&self);

    /// Get TTL for a key
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;

    /// Set TTL for an existing key
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;

    /// Health check for the cache backend
    async fn health_check(&self) -> Result<()>;

    /// Get cache statistics
    async fn stats(&self) -> Result<HashMap<String, String>>;

    // ============================================================================
    // Typed operations (from CacheExt)
    // ============================================================================

    /// Get typed value from cache
    async fn get_typed<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let bytes = self.get_bytes(key).await?;
        match bytes {
            Some(data) => {
                let val: T = serde_json::from_slice(&data)
                    .map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// Set typed value in cache
    async fn set_typed<T: Serialize + Send + Sync>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        self.set_bytes(key, bytes, ttl).await
    }

    // ============================================================================
    // Required methods for implementation
    // ============================================================================

    /// Get the serializer used by this cache
    fn serializer(&self) -> &dyn Serializer;

    /// Get the backend type for runtime identification
    fn backend_kind(&self) -> crate::backend::interface::BackendKind;
}

/// Blanket implementation for all CacheBackend implementations
#[async_trait]
impl<T: crate::backend::CacheBackend + Send + Sync> UnifiedCache for T {
    // Core operations
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get(key).await
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        self.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.exists(key).await
    }

    async fn clear(&self) -> Result<()> {
        self.clear().await
    }

    async fn shutdown(&self) {
        self.shutdown().await
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        self.ttl(key).await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        self.expire(key, ttl).await
    }

    async fn health_check(&self) -> Result<()> {
        self.health_check().await
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        self.stats().await
    }

    // Default serializer implementation
    #[cfg(any(feature = "serialization", feature = "full"))]
    fn serializer(&self) -> &dyn Serializer {
        use crate::infra::serialization::unified::{default_serializer, UnifiedSerializerAdapter};
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
