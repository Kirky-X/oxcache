//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! CacheBackend trait for the modernized cache API
//!
//! This module provides ISP-compliant trait hierarchy:
//! - `CacheReader` - Read-only operations
//! - `CacheWriter` - Write operations
//! - `CacheConnector` - Lifecycle management
//! - `CacheBackend` - Combines all traits

use crate::error::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Backend kind enumeration for runtime type identification
///
/// This replaces `as_any()` for type checking, following the Brick Architecture
/// principle that concrete implementations should be invisible to consumers.
/// Unlike `core::types::BackendType` (used for configuration), this enum is
/// used for runtime identification without feature gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Moka in-memory cache
    Moka,
    /// DashMap in-memory cache
    DashMap,
    /// Redis distributed cache
    Redis,
    /// Chain cache (multi-tier)
    Chain,
    /// Mock backend for testing
    Mock,
    /// Unknown or custom backend
    Unknown,
}

impl BackendKind {
    /// Returns true if this is an in-memory cache (L1)
    pub fn is_memory(&self) -> bool {
        matches!(self, BackendKind::Moka | BackendKind::DashMap | BackendKind::Mock)
    }

    /// Returns true if this is a distributed cache (L2)
    pub fn is_distributed(&self) -> bool {
        matches!(self, BackendKind::Redis)
    }
}

// ============================================================================
// ISP-Compliant Trait Hierarchy
// ============================================================================

/// Read-only cache operations.
///
/// This trait provides methods for reading data from the cache.
/// It can be used by consumers that only need read access.
///
/// # Example
///
/// ```rust,ignore
/// fn get_value(cache: &dyn CacheReader, key: &str) -> Result<Option<Vec<u8>>> {
///     cache.get(key)
/// }
/// ```
#[async_trait]
pub trait CacheReader: Send + Sync + 'static {
    /// Get a value from the cache.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Check if a key exists in the cache.
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Get the time-to-live for a key.
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;

    /// Get the number of entries in the cache.
    async fn len(&self) -> Result<u64>;

    /// Check if the cache is empty.
    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await?.eq(&0))
    }

    /// Get the capacity of the cache.
    async fn capacity(&self) -> Result<u64>;

    /// Get backend statistics.
    async fn stats(&self) -> Result<std::collections::HashMap<String, String>>;

    /// Get multiple values in a single operation.
    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }
        Ok(results)
    }
}

/// Write operations for the cache.
///
/// This trait provides methods for modifying data in the cache.
/// It can be used by consumers that only need write access.
///
/// # Example
///
/// ```rust,ignore
/// fn set_value(cache: &mut dyn CacheWriter, key: &str, value: Vec<u8>) -> Result<()> {
///     cache.set(key, value, None)
/// }
/// ```
#[async_trait]
pub trait CacheWriter: Send + Sync + 'static {
    /// Set a value in the cache.
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;

    /// Delete a value from the cache.
    async fn delete(&self, key: &str) -> Result<()>;

    /// Clear all values from the cache.
    async fn clear(&self) -> Result<()>;

    /// Set the time-to-live for an existing key.
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;

    /// Set multiple key-value pairs in a single operation.
    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> Result<()> {
        for (key, value, ttl) in items {
            self.set(key, value.clone(), *ttl).await?;
        }
        Ok(())
    }

    /// Delete multiple keys in a single operation.
    async fn delete_many(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            self.delete(key).await?;
        }
        Ok(())
    }
}

/// Lifecycle management for cache backends.
///
/// This trait provides methods for connection management and health monitoring.
/// It can be used by infrastructure code that manages backend lifecycle.
///
/// # Example
///
/// ```rust,ignore
/// fn check_and_shutdown(backend: &dyn CacheConnector) {
///     if backend.health_check().await.is_err() {
///         backend.shutdown().await;
///     }
/// }
/// ```
#[async_trait]
pub trait CacheConnector: Send + Sync + 'static {
    /// Check if the backend is healthy.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Backend is healthy
    /// * `Err(CacheError)` - Health check failed (backend is unhealthy)
    async fn health_check(&self) -> Result<()>;

    /// Shutdown the backend and release resources.
    ///
    /// Internal errors are logged but not propagated.
    async fn shutdown(&self);

    /// Get the backend kind for runtime identification.
    fn backend_kind(&self) -> BackendKind;

    /// Get Lua script executor if this backend supports it.
    #[cfg(feature = "lua-script")]
    fn as_lua_executor(&self) -> Option<&dyn LuaExecutor> {
        None
    }
}

// ============================================================================
// Lua Executor Trait (Optional, Redis-only)
// ============================================================================

#[cfg(feature = "lua-script")]
#[async_trait]
pub trait LuaExecutor: Send + Sync {
    async fn eval_lua(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value>;
    async fn eval_sha(&self, sha: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value>;
    async fn script_load(&self, script: &str) -> Result<String>;
}

// ============================================================================
// Combined CacheBackend Trait
// ============================================================================

/// Full cache backend interface combining all ISP traits.
///
/// Combines `CacheReader`, `CacheWriter`, and `CacheConnector` for consumers
/// that need full cache functionality. Single trait object type for backends.
///
/// # Design Pattern
///
/// Strategy pattern: allows different backend implementations to be swapped
/// without changing the cache interface.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::{CacheReader, CacheWriter, CacheConnector};
/// use async_trait::async_trait;
///
/// struct MyCustomBackend;
///
/// #[async_trait]
/// impl CacheReader for MyCustomBackend {
///     async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> { Ok(None) }
///     async fn exists(&self, key: &str) -> Result<bool> { Ok(false) }
///     async fn ttl(&self, key: &str) -> Result<Option<std::time::Duration>> { Ok(None) }
///     async fn len(&self) -> Result<u64> { Ok(0) }
///     async fn capacity(&self) -> Result<u64> { Ok(0) }
///     async fn stats(&self) -> Result<std::collections::HashMap<String, String>> { Ok(HashMap::new()) }
/// }
///
/// #[async_trait]
/// impl CacheWriter for MyCustomBackend { /* ... */ }
///
/// #[async_trait]
/// impl CacheConnector for MyCustomBackend { /* ... */ }
/// // CacheBackend is automatically provided via blanket impl
/// ```
#[async_trait]
pub trait CacheBackend: CacheReader + CacheWriter + CacheConnector + 'static {}

#[async_trait]
impl<T: CacheReader + CacheWriter + CacheConnector + 'static> CacheBackend for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock::MockBackend;

    #[tokio::test]
    async fn test_mock_backend() {
        let backend = MockBackend::new("mock", 50, false);

        // Test set and get
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());

        // Test health check
        backend.health_check().await.unwrap();

        // Test stats
        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"mock".to_string()));
    }

    #[tokio::test]
    async fn test_isp_traits() {
        let backend = MockBackend::new("mock", 50, false);

        // Test CacheReader trait object
        let reader: &dyn CacheReader = &backend;
        assert!(reader.get("nonexistent").await.unwrap().is_none());

        // Test CacheWriter trait object
        let writer: &dyn CacheWriter = &backend;
        writer.set("key", b"value".to_vec(), None).await.unwrap();

        // Test CacheConnector trait object
        let connector: &dyn CacheConnector = &backend;
        connector.health_check().await.unwrap();
        assert_eq!(connector.backend_kind(), BackendKind::Mock);
    }

    // ============================================================================
    // BackendKind 方法测试 (lines 41-42, 46-47)
    // ============================================================================

    #[test]
    fn test_backend_kind_is_memory_moka() {
        assert!(BackendKind::Moka.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_dashmap() {
        assert!(BackendKind::DashMap.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_mock() {
        assert!(BackendKind::Mock.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_redis_false() {
        assert!(!BackendKind::Redis.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_chain_false() {
        assert!(!BackendKind::Chain.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_unknown_false() {
        assert!(!BackendKind::Unknown.is_memory());
    }

    #[test]
    fn test_backend_kind_is_distributed_redis() {
        assert!(BackendKind::Redis.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_moka_false() {
        assert!(!BackendKind::Moka.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_dashmap_false() {
        assert!(!BackendKind::DashMap.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_chain_false() {
        assert!(!BackendKind::Chain.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_mock_false() {
        assert!(!BackendKind::Mock.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_unknown_false() {
        assert!(!BackendKind::Unknown.is_distributed());
    }

    // ============================================================================
    // BackendKind Debug, Clone, PartialEq 测试
    // ============================================================================

    #[test]
    fn test_backend_kind_debug() {
        let kind = BackendKind::Moka;
        let debug_str = format!("{:?}", kind);
        assert!(debug_str.contains("Moka"));
    }

    #[test]
    fn test_backend_kind_clone() {
        let kind = BackendKind::Redis;
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_backend_kind_equality() {
        assert_eq!(BackendKind::Moka, BackendKind::Moka);
        assert_ne!(BackendKind::Moka, BackendKind::Redis);
    }

    // ============================================================================
    // CacheReader is_empty 默认方法测试 (lines 82-83)
    // ============================================================================

    #[tokio::test]
    async fn test_cache_reader_is_empty_default() {
        let backend = MockBackend::new("mock", 50, false);
        let reader: &dyn CacheReader = &backend;
        // 空缓存应该返回 true
        assert!(reader.is_empty().await.unwrap());

        // 添加数据后应该返回 false
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        assert!(!reader.is_empty().await.unwrap());
    }

    // ============================================================================
    // CacheReader get_many 默认方法测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_reader_get_many_default() {
        let backend = MockBackend::new("mock", 50, false);
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        backend.set("key2", b"value2".to_vec(), None).await.unwrap();

        let reader: &dyn CacheReader = &backend;
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let results = reader.get_many(&keys).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
        assert_eq!(results[2], None);
    }

    #[tokio::test]
    async fn test_cache_reader_get_many_empty() {
        let backend = MockBackend::new("mock", 50, false);
        let reader: &dyn CacheReader = &backend;
        let keys: Vec<String> = vec![];
        let results = reader.get_many(&keys).await.unwrap();
        assert!(results.is_empty());
    }

    // ============================================================================
    // CacheWriter set_many 和 delete_many 默认方法测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_writer_set_many_default() {
        let backend = MockBackend::new("mock", 50, false);
        let writer: &dyn CacheWriter = &backend;
        let items = vec![
            ("key1".to_string(), b"value1".to_vec(), None),
            ("key2".to_string(), b"value2".to_vec(), None),
        ];
        writer.set_many(&items).await.unwrap();

        assert!(backend.exists("key1").await.unwrap());
        assert!(backend.exists("key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_writer_delete_many_default() {
        let backend = MockBackend::new("mock", 50, false);
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        backend.set("key2", b"value2".to_vec(), None).await.unwrap();

        let writer: &dyn CacheWriter = &backend;
        let keys = vec!["key1".to_string(), "key2".to_string()];
        writer.delete_many(&keys).await.unwrap();

        assert!(!backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());
    }

    // ============================================================================
    // CacheConnector backend_kind 测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_connector_backend_kind_mock() {
        let backend = MockBackend::new("mock", 50, false);
        let connector: &dyn CacheConnector = &backend;
        assert_eq!(connector.backend_kind(), BackendKind::Mock);
    }

    #[tokio::test]
    async fn test_cache_connector_shutdown() {
        let backend = MockBackend::new("mock", 50, false);
        let connector: &dyn CacheConnector = &backend;
        // shutdown 不应 panic
        connector.shutdown().await;
    }

    // ============================================================================
    // CacheBackend blanket impl 测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_backend_trait_object() {
        let backend = MockBackend::new("mock", 50, false);
        let backend_dyn: &dyn CacheBackend = &backend;
        // 测试 CacheBackend 可以作为 trait 对象使用
        backend_dyn.set("key", b"value".to_vec(), None).await.unwrap();
        let value = backend_dyn.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }
}
