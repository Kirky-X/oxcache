//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified Cache interface for the modernized cache API

use crate::backend::{CacheBackend, MemoryBackend};
use crate::error::Result;
use crate::serialization::json::JsonSerializer;
use crate::serialization::Serializer;
use crate::traits::{CacheKey, Cacheable};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Unified cache interface with type-safe key and value types
///
/// This is the main cache type that users interact with. It provides a
/// type-safe interface over the pluggable backend architecture.
///
/// # Type Parameters
///
/// * `K` - Key type, must implement `CacheKey` trait
/// * `V` - Value type, must implement `Cacheable` trait (Serialize + Deserialize)
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::Cache;
///
/// // Create a simple memory cache
/// let cache: Cache<String, User> = Cache::new().await?;
///
/// // Set a value
/// let user = User { id: 1, name: "Alice".to_string() };
/// cache.set("user:1", &user).await?;
///
/// // Get a value
/// let user: Option<User> = cache.get("user:1").await?;
///
/// // Get with fallback
/// let user: User = cache.get_or("user:1", || async {
///     fetch_user_from_db(1).await
/// }).await?;
/// ```
pub struct Cache<K, V> {
    backend: Arc<dyn CacheBackend>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    /// Create a new cache with default memory backend
    ///
    /// # Returns
    ///
    /// Configured cache instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::new().await?;
    /// ```
    pub async fn new() -> Result<Self> {
        let backend = MemoryBackend::new();
        Ok(Self {
            backend: Arc::new(backend),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Internal constructor for builder
    pub(crate) fn new_with_backend(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            backend,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a cache with a memory backend
    ///
    /// # Returns
    ///
    /// Configured cache instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::memory().await?;
    /// ```
    pub async fn memory() -> Result<Self> {
        Self::new().await
    }

    /// Create a cache with a Redis backend
    ///
    /// # Arguments
    ///
    /// * `connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Configured cache instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::redis("redis://localhost:6379").await?;
    /// ```
    pub async fn redis(connection_string: &str) -> Result<Self> {
        let backend = crate::backend::RedisBackend::new(connection_string).await?;
        Ok(Self {
            backend: Arc::new(backend),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a cache with a tiered backend (L1 memory + L2 Redis)
    ///
    /// # Arguments
    ///
    /// * `l1_capacity` - Maximum entries in L1 cache
    /// * `l2_connection_string` - Redis connection URL
    ///
    /// # Returns
    ///
    /// Configured cache instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::tiered(
    ///     10000,
    ///     "redis://localhost:6379"
    /// ).await?;
    /// ```
    pub async fn tiered(l1_capacity: u64, l2_connection_string: &str) -> Result<Self> {
        let l1 = MemoryBackend::builder().capacity(l1_capacity).build();
        let l2 = crate::backend::RedisBackend::new(l2_connection_string).await?;
        let backend = crate::backend::TieredBackend::new(l1, l2);
        Ok(Self {
            backend: Arc::new(backend),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a cache builder for advanced configuration
    ///
    /// # Returns
    ///
    /// CacheBuilder instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::builder()
    ///     .ttl(Duration::from_secs(3600))
    ///     .capacity(10000)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn builder() -> crate::builder::CacheBuilder<K, V> {
        crate::builder::CacheBuilder::default()
    }

    /// Get a value from the cache
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - Value found
    /// * `Ok(None)` - Key not found
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let user: Option<User> = cache.get("user:1").await?;
    /// ```
    pub async fn get(&self, key: &K) -> Result<Option<V>> {
        let key_str = key.to_key_string();
        let bytes = self.backend.get(&key_str).await?;

        match bytes {
            Some(data) => {
                let serializer = JsonSerializer::new();
                let value: V = serializer.deserialize(&data)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Set a value in the cache
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    /// * `value` - Value to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// cache.set("user:1", &user).await?;
    /// ```
    pub async fn set(&self, key: &K, value: &V) -> Result<()> {
        self.set_with_ttl(key, value, None).await
    }

    /// Set a value in the cache with TTL
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    /// * `value` - Value to store
    /// * `ttl` - Time-to-live duration
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// cache.set_with_ttl("user:1", &user, Some(Duration::from_secs(3600))).await?;
    /// ```
    pub async fn set_with_ttl(&self, key: &K, value: &V, ttl: Option<Duration>) -> Result<()> {
        let key_str = key.to_key_string();
        let serializer = JsonSerializer::new();
        let bytes = serializer.serialize(value)?;
        self.backend.set(&key_str, bytes, ttl).await
    }

    /// Delete a value from the cache
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Key deleted successfully
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// cache.delete("user:1").await?;
    /// ```
    pub async fn delete(&self, key: &K) -> Result<()> {
        let key_str = key.to_key_string();
        self.backend.delete(&key_str).await
    }

    /// Check if a key exists in the cache
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key to check
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Key exists
    /// * `Ok(false)` - Key does not exist
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if cache.exists("user:1").await? {
    ///     println!("User is cached");
    /// }
    /// ```
    pub async fn exists(&self, key: &K) -> Result<bool> {
        let key_str = key.to_key_string();
        self.backend.exists(&key_str).await
    }

    /// Get a value or compute it using a fallback function
    ///
    /// This method provides a convenient way to implement the cache-aside pattern.
    /// If the key exists in the cache, it returns the cached value. Otherwise,
    /// it calls the provided function to compute the value, stores it in the cache,
    /// and returns it.
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key
    /// * `fallback` - Async function to compute the value if not in cache
    ///
    /// # Returns
    ///
    /// * `Ok(value)` - Value from cache or fallback
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let user: User = cache.get_or("user:1", || async {
    ///     fetch_user_from_db(1).await
    /// }).await?;
    /// ```
    pub async fn get_or<F, Fut>(&self, key: &K, fallback: F) -> Result<V>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V>>,
    {
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let value = fallback().await?;
        self.set(key, &value).await?;
        Ok(value)
    }

    /// Set multiple values in the cache
    ///
    /// # Arguments
    ///
    /// * `items` - Iterator of (key, value) pairs
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All values stored successfully
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let users = vec![
    ///     ("user:1", user1),
    ///     ("user:2", user2),
    /// ];
    /// cache.set_many(users.iter().map(|(k, v)| (*k, v))).await?;
    /// ```
    pub async fn set_many<'a, I>(&self, items: I) -> Result<()>
    where
        K: 'a,
        V: 'a,
        I: IntoIterator<Item = (&'a K, &'a V)>,
    {
        for (key, value) in items {
            self.set(key, value).await?;
        }
        Ok(())
    }

    /// Get multiple values from the cache
    ///
    /// # Arguments
    ///
    /// * `keys` - Iterator of keys to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(map)` - Map of keys to values (only found keys)
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let keys = vec!["user:1", "user:2", "user:3"];
    /// let users: HashMap<String, User> = cache.get_many(keys.iter()).await?;
    /// ```
    pub async fn get_many<'a, I>(&self, keys: I) -> Result<HashMap<String, V>>
    where
        K: 'a,
        I: IntoIterator<Item = &'a K>,
    {
        let mut result = HashMap::new();
        for key in keys {
            if let Some(value) = self.get(key).await? {
                result.insert(key.to_key_string(), value);
            }
        }
        Ok(result)
    }

    /// Delete multiple keys from the cache
    ///
    /// # Arguments
    ///
    /// * `keys` - Iterator of keys to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All keys deleted successfully
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let keys = vec!["user:1", "user:2"];
    /// cache.delete_many(keys.iter()).await?;
    /// ```
    pub async fn delete_many<'a, I>(&self, keys: I) -> Result<()>
    where
        K: 'a,
        I: IntoIterator<Item = &'a K>,
    {
        for key in keys {
            self.delete(key).await?;
        }
        Ok(())
    }

    /// Clear all values from the cache
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Cache cleared successfully
    /// * `Err(CacheError)` - Operation failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// cache.clear().await?;
    /// ```
    pub async fn clear(&self) -> Result<()> {
        self.backend.clear().await
    }

    /// Get cache statistics
    ///
    /// # Returns
    ///
    /// * `Ok(stats)` - Map of statistics
    /// * `Err(CacheError)` - Failed to retrieve statistics
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let stats = cache.stats().await?;
    /// println!("Cache type: {}", stats.get("type").unwrap());
    /// ```
    pub async fn stats(&self) -> Result<HashMap<String, String>> {
        self.backend.stats().await
    }

    /// Check if the cache backend is healthy
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Backend is healthy
    /// * `Ok(false)` - Backend is unhealthy
    /// * `Err(CacheError)` - Health check failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if cache.health_check().await? {
    ///     println!("Cache is healthy");
    /// }
    /// ```
    pub async fn health_check(&self) -> Result<bool> {
        self.backend.health_check().await
    }
}

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
        let cache: Cache<String, TestValue> = Cache::new().await.unwrap();

        let value = TestValue {
            id: 1,
            name: "test".to_string(),
        };

        // Test set and get
        cache.set(&"key1".to_string(), &value).await.unwrap();
        let result = cache.get(&"key1".to_string()).await.unwrap();
        assert_eq!(result, Some(value));

        // Test exists
        assert!(cache.exists(&"key1".to_string()).await.unwrap());
        assert!(!cache.exists(&"key2".to_string()).await.unwrap());

        // Test delete
        cache.delete(&"key1".to_string()).await.unwrap();
        assert!(!cache.exists(&"key1".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_get_or() {
        use crate::error::CacheError;
        let cache: Cache<String, TestValue> = Cache::new().await.unwrap();

        let value = TestValue {
            id: 1,
            name: "test".to_string(),
        };

        // First call should use fallback
        async fn fallback1() -> Result<TestValue> {
            Ok(TestValue {
                id: 1,
                name: "test".to_string(),
            })
        }
        let result1 = cache.get_or(&"key1".to_string(), fallback1).await.unwrap();
        assert_eq!(result1, value);

        // Second call should use cache
        async fn fallback2() -> Result<TestValue> {
            Err(CacheError::NotFound("should not be called".to_string()))
        }
        let result2 = cache.get_or(&"key1".to_string(), fallback2).await.unwrap();
        assert_eq!(result2, value);
    }

    #[tokio::test]
    async fn test_cache_batch_operations() {
        let cache: Cache<String, TestValue> = Cache::new().await.unwrap();

        let value1 = TestValue {
            id: 1,
            name: "test1".to_string(),
        };
        let value2 = TestValue {
            id: 2,
            name: "test2".to_string(),
        };

        // Test set_many
        cache
            .set_many(vec![
                (&"key1".to_string(), &value1),
                (&"key2".to_string(), &value2),
            ])
            .await
            .unwrap();

        // Test get_many
        let results = cache
            .get_many(vec![
                &"key1".to_string(),
                &"key2".to_string(),
                &"key3".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results.get("key1"), Some(&value1));
        assert_eq!(results.get("key2"), Some(&value2));

        // Test delete_many
        cache
            .delete_many(vec![&"key1".to_string(), &"key2".to_string()])
            .await
            .unwrap();
        assert!(!cache.exists(&"key1".to_string()).await.unwrap());
        assert!(!cache.exists(&"key2".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache: Cache<String, TestValue> = Cache::new().await.unwrap();

        cache
            .set(
                &"key1".to_string(),
                &TestValue {
                    id: 1,
                    name: "test".to_string(),
                },
            )
            .await
            .unwrap();

        cache.clear().await.unwrap();

        assert!(!cache.exists(&"key1".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache: Cache<String, TestValue> = Cache::new().await.unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"memory".to_string()));
    }
}
