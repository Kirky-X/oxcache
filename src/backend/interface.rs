//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! CacheBackend trait for the modernized cache API

use crate::error::Result;
use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::time::Duration;

/// Backend strategy trait for cache implementations
///
/// This trait defines the interface that all cache backends must implement.
/// It provides a pluggable architecture allowing different storage backends
/// (memory, Redis, tiered, etc.) to be used interchangeably.
///
/// # Design Pattern
///
/// This uses the Strategy pattern, allowing different backend implementations
/// to be swapped without changing the cache interface.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::CacheBackend;
/// use async_trait::async_trait;
///
/// struct MyCustomBackend;
///
/// #[async_trait]
/// impl CacheBackend for MyCustomBackend {
///     async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
///         // Custom implementation
///         Ok(None)
///     }
///
///     async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
///         // Custom implementation
///         Ok(())
///     }
///
///     // ... implement other methods
/// }
/// ```
#[async_trait]
pub trait CacheBackend: Send + Sync + 'static {
    /// Get a value from the cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(Some(bytes))` - Value found
    /// * `Ok(None)` - Key not found
    /// * `Err(CacheError)` - Operation failed
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a value in the cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to set
    /// * `value` - The value bytes to store
    /// * `ttl` - Optional time-to-live duration
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    /// * `Err(CacheError)` - Operation failed
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;

    /// Delete a value from the cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Key deleted successfully
    /// * `Err(CacheError)` - Operation failed
    async fn delete(&self, key: &str) -> Result<()>;

    /// Check if a key exists in the cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to check
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Key exists
    /// * `Ok(false)` - Key does not exist
    /// * `Err(CacheError)` - Operation failed
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Clear all values from the cache
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Cache cleared successfully
    /// * `Err(CacheError)` - Operation failed
    async fn clear(&self) -> Result<()>;

    /// Close the backend and release resources
    ///
    /// This method should be called when the cache is no longer needed.
    /// It should gracefully close connections and release any held resources.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Backend closed successfully
    /// * `Err(CacheError)` - Operation failed
    async fn close(&self) -> Result<()>;

    /// Get the time-to-live for a key
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to check
    ///
    /// # Returns
    ///
    /// * `Ok(Some(duration))` - TTL remaining
    /// * `Ok(None)` - Key exists but has no expiration
    /// * `Err(CacheError)` - Operation failed or key not found
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;

    /// Set the time-to-live for an existing key
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to update
    /// * `ttl` - The new TTL duration
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - TTL updated successfully
    /// * `Ok(false)` - Key does not exist
    /// * `Err(CacheError)` - Operation failed
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;

    /// Check if the backend is healthy
    ///
    /// This method can be used for health checks and monitoring.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Backend is healthy
    /// * `Ok(false)` - Backend is unhealthy or degraded
    /// * `Err(CacheError)` - Health check failed
    async fn health_check(&self) -> Result<bool>;

    /// Get backend statistics
    ///
    /// Returns a map of statistic names to values.
    /// The exact statistics depend on the backend implementation.
    ///
    /// # Returns
    ///
    /// * `Ok(stats)` - Map of statistics
    /// * `Err(CacheError)` - Failed to retrieve statistics
    async fn stats(&self) -> Result<std::collections::HashMap<String, String>>;

    /// Get as_any reference for type downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get the number of entries in the cache
    async fn len(&self) -> Result<u64>;

    /// Get the capacity of the cache
    async fn capacity(&self) -> Result<u64>;

    /// Check if backend is of specific type
    fn is<T: Any>(&self) -> bool
    where
        Self: Sized,
    {
        TypeId::of::<T>() == TypeId::of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock backend for testing
    struct MockBackend {
        data: std::sync::Arc<tokio::sync::RwLock<HashMap<String, Vec<u8>>>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                data: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl CacheBackend for MockBackend {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            let data = self.data.read().await;
            Ok(data.get(key).cloned())
        }

        async fn set(&self, key: &str, value: Vec<u8>, _ttl: Option<Duration>) -> Result<()> {
            let mut data = self.data.write().await;
            data.insert(key.to_string(), value);
            Ok(())
        }

        async fn delete(&self, key: &str) -> Result<()> {
            let mut data = self.data.write().await;
            data.remove(key);
            Ok(())
        }

        async fn exists(&self, key: &str) -> Result<bool> {
            let data = self.data.read().await;
            Ok(data.contains_key(key))
        }

        async fn clear(&self) -> Result<()> {
            let mut data = self.data.write().await;
            data.clear();
            Ok(())
        }

        async fn close(&self) -> Result<()> {
            Ok(())
        }

        async fn ttl(&self, _key: &str) -> Result<Option<Duration>> {
            Ok(None)
        }

        async fn expire(&self, _key: &str, _ttl: Duration) -> Result<bool> {
            Ok(false)
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(true)
        }

        async fn stats(&self) -> Result<HashMap<String, String>> {
            let mut stats = HashMap::new();
            stats.insert("type".to_string(), "mock".to_string());
            Ok(stats)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        async fn len(&self) -> Result<u64> {
            let data = self.data.read().await;
            Ok(data.len() as u64)
        }

        async fn capacity(&self) -> Result<u64> {
            // Mock backend has no fixed capacity
            Ok(0)
        }
    }

    #[tokio::test]
    async fn test_mock_backend() {
        let backend = MockBackend::new();

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
        assert!(backend.health_check().await.unwrap());

        // Test stats
        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"mock".to_string()));
    }
}
