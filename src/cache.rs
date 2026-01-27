//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified Cache interface for the modernized cache API

use crate::backend::client::MokaMemoryBackend as MemoryBackend;
use crate::backend::CacheBackend;
use crate::error::{CacheError, Result};

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::serialization::json::JsonSerializer;
#[cfg(any(feature = "serialization", feature = "full"))]
use crate::serialization::Serializer;
use crate::traits::{CacheKey, Cacheable};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 序列化器实例复用管理器
///
/// 优化点：
/// 1. 复用 JsonSerializer 实例，避免每次创建新的实例
/// 2. 使用 Arc 共享序列化器，减少内存开销
#[cfg(any(feature = "serialization", feature = "full"))]
pub struct SerializerPool {
    json_serializer: Arc<JsonSerializer>,
}

#[cfg(any(feature = "serialization", feature = "full"))]
impl SerializerPool {
    /// 创建新的序列化器池
    pub fn new() -> Self {
        Self {
            json_serializer: Arc::new(JsonSerializer::new()),
        }
    }

    /// 获取 JSON 序列化器
    pub fn json(&self) -> Arc<JsonSerializer> {
        self.json_serializer.clone()
    }
}

#[cfg(any(feature = "serialization", feature = "full"))]
impl Default for SerializerPool {
    fn default() -> Self {
        Self::new()
    }
}
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
    serializer_pool: Arc<SerializerPool>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> std::fmt::Debug for Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("backend", &"<CacheBackend>")
            .finish()
    }
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
            serializer_pool: Arc::new(SerializerPool::new()),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Internal constructor for builder
    pub(crate) fn new_with_backend(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            backend,
            serializer_pool: Arc::new(SerializerPool::new()),
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
    #[cfg(feature = "redis")]
    pub async fn redis(connection_string: &str) -> Result<Self> {
        let backend = crate::backend::client::RedisBackend::new(connection_string).await?;
        Ok(Self {
            backend: Arc::new(backend),
            serializer_pool: Arc::new(SerializerPool::new()),
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

        #[cfg(any(feature = "serialization", feature = "full"))]
        match bytes {
            Some(data) => {
                let value: V = serde_json::from_slice(&data)
                    .map_err(|e| CacheError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            // Without serialization, we can only work with bytes directly
            // This is a limitation - the get method requires deserialization
            let _ = bytes;
            Err(CacheError::Serialization(
                "Serialization feature is required for typed get operations".to_string(),
            ))
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

        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            let bytes =
                serde_json::to_vec(value).map_err(|e| CacheError::Serialization(e.to_string()))?;
            self.backend.set(&key_str, bytes, ttl).await
        }

        #[cfg(not(any(feature = "serialization", feature = "full")))]
        {
            // Without serialization, we cannot serialize the value
            let _ = (key_str, value);
            Err(CacheError::Serialization(
                "Serialization feature is required for typed set operations".to_string(),
            ))
        }
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

    // ============================================================================
    // Low-level byte operations (for #[cached] macro compatibility)
    // ============================================================================

    /// Get raw bytes from cache (for macro compatibility)
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key as string
    ///
    /// # Returns
    ///
    /// * `Ok(Some(bytes))` - Value found
    /// * `Ok(None)` - Key not found
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.backend.get(key).await
    }

    /// Set raw bytes in cache (for macro compatibility)
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key as string
    /// * `value` - Raw bytes to store
    /// * `ttl` - Optional TTL in seconds
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    /// Set raw bytes in L1 cache only (for macro compatibility)
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key as string
    /// * `value` - Raw bytes to store
    /// * `ttl` - Optional TTL in seconds
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    /// * `Err(CacheError::NotSupported)` - If backend doesn't support L1-only operations
    pub async fn set_l1_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        // Try to use L1-specific method if available
        if let Some(l1_backend) = self
            .backend
            .as_any()
            .downcast_ref::<crate::backend::client::MokaMemoryBackend>()
        {
            l1_backend.set(key, value, ttl_duration).await?;
            return Ok(());
        }
        // Fallback to generic set
        self.backend.set(key, value, ttl_duration).await
    }

    /// Set raw bytes in L2 cache only (for macro compatibility)
    ///
    /// # Arguments
    ///
    /// * `key` - Cache key as string
    /// * `value` - Raw bytes to store
    /// * `ttl` - Optional TTL in seconds
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    /// * `Err(CacheError::NotSupported)` - If backend doesn't support L2-only operations
    pub async fn set_l2_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        // For non-tiered backends, fall back to regular set
        self.backend.set(key, value, ttl_duration).await
    }

    /// Get the serializer for this cache (for macro compatibility)
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn serializer(&self) -> Arc<dyn Serializer> {
        self.serializer_pool.json()
    }

    /// Check if the cache supports L1-only operations
    pub fn supports_l1_only(&self) -> bool {
        self.backend
            .as_any()
            .is::<crate::backend::client::MokaMemoryBackend>()
    }

    /// Check if the cache supports L2-only operations
    pub fn supports_l2_only(&self) -> bool {
        false // Only tiered backends support this
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
    /// Perform a health check on the cache backend.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Cache is healthy
    /// * `Ok(false)` - Cache is unhealthy
    /// * `Err(CacheError)` - Health check failed
    pub async fn health_check(&self) -> Result<bool> {
        self.backend.health_check().await
    }

    /// Shutdown the cache and release all resources.
    ///
    /// This method should be called during application shutdown to properly
    /// close connections and release resources.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache: Cache<String, User> = Cache::new().await?;
    /// // ... use cache ...
    /// cache.shutdown().await?;
    /// ```
    pub async fn shutdown(&self) -> Result<()> {
        self.backend.close().await
    }

    /// Register this cache instance for use with the #[cached] macro.
    ///
    /// This method requires the cache to use `String` keys and `Vec<u8>` values
    /// (the default for macro usage).
    ///
    /// # Arguments
    ///
    /// * `service_name` - A unique name to identify this cache instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::Cache;
    ///
    /// // Create a byte cache for macro usage
    /// let cache = Cache::<String, Vec<u8>>::new().await?;
    /// cache.register_for_macro("my_service").await;
    ///
    /// // Now you can use:
    /// // #[cached(service = "my_service", ttl = 300)]
    /// // async fn get_user(id: u64) -> User { ... }
    /// ```
    pub async fn register_for_macro(&self, service_name: &str)
    where
        K: 'static,
        V: 'static,
    {
        use crate::internal::__internal_register_cache;
        use std::any::TypeId;

        // Only allow registration for Cache<String, Vec<u8>>
        // This is the expected type for #[cached] macro
        if TypeId::of::<K>() == TypeId::of::<String>()
            && TypeId::of::<V>() == TypeId::of::<Vec<u8>>()
        {
            // Safe to cast since we checked the types
            let cache: Cache<String, Vec<u8>> = unsafe {
                std::ptr::read(self as *const Cache<K, V> as *const Cache<String, Vec<u8>>)
            };
            __internal_register_cache(service_name, Arc::new(cache));
        }
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
        // Cache::new() uses MokaMemoryBackend which reports type as "moka"
        assert_eq!(stats.get("type"), Some(&"moka".to_string()));
    }
}
