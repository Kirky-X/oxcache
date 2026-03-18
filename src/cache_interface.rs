//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified cache interface that consolidates CacheOps, CacheExt, and CacheBackend
//! This provides a single, comprehensive interface for all cache operations

use crate::error::Result;

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::serialization::SerializerEnum;
use async_trait::async_trait;
#[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
use futures::future::join_all;
use serde::{de::DeserializeOwned, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

/// Core cache operations trait - unified interface for all cache backends
///
/// This trait combines the functionality of CacheOps, CacheExt, and CacheBackend
/// into a single, comprehensive interface. It provides both low-level byte operations
/// and high-level typed operations.
#[async_trait]
pub trait UnifiedCache: Send + Sync + Any {
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

    /// Close the cache and release resources
    async fn close(&self) -> Result<()>;

    /// Get TTL for a key
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;

    /// Set TTL for an existing key
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;

    /// Health check for the cache backend
    async fn health_check(&self) -> Result<bool>;

    /// Get cache statistics
    async fn stats(&self) -> Result<HashMap<String, String>>;

    // ============================================================================
    // Layer-specific operations (from CacheOps)
    // ============================================================================

    /// Get from L1 cache only
    async fn get_l1_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Err(crate::error::CacheError::NotSupported("get_l1_bytes".to_string()))
    }

    /// Get from L2 cache only
    async fn get_l2_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Err(crate::error::CacheError::NotSupported("get_l2_bytes".to_string()))
    }

    /// Set in L1 cache only
    async fn set_l1_bytes(&self, _key: &str, _value: Vec<u8>, _ttl: Option<Duration>) -> Result<()> {
        Err(crate::error::CacheError::NotSupported("set_l1_bytes".to_string()))
    }

    /// Set in L2 cache only
    async fn set_l2_bytes(&self, _key: &str, _value: Vec<u8>, _ttl: Option<Duration>) -> Result<()> {
        Err(crate::error::CacheError::NotSupported("set_l2_bytes".to_string()))
    }

    /// Clear L1 cache only
    async fn clear_l1(&self) -> Result<()> {
        Err(crate::error::CacheError::NotSupported("clear_l1".to_string()))
    }

    /// Clear L2 cache only
    async fn clear_l2(&self) -> Result<()> {
        Err(crate::error::CacheError::NotSupported("clear_l2".to_string()))
    }

    // ============================================================================
    // Distributed operations (from CacheOps)
    // ============================================================================

    /// Acquire distributed lock
    async fn lock(&self, _key: &str, _ttl: u64) -> Result<Option<String>> {
        Ok(None)
    }

    /// Release distributed lock
    async fn unlock(&self, _key: &str, _value: &str) -> Result<bool> {
        Ok(false)
    }

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

    /// Set typed value in L1 only
    async fn set_l1_typed<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        self.set_l1_bytes(key, bytes, ttl).await
    }

    /// Set typed value in L2 only
    async fn set_l2_typed<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        self.set_l2_bytes(key, bytes, ttl).await
    }

    /// Get typed value or fetch with fallback
    async fn get_or_fetch<T, F, Fut>(&self, key: &str, ttl: Option<Duration>, fetch: F) -> Result<T>
    where
        T: DeserializeOwned + Serialize + Send + Sync + Clone,
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        if let Some(cached) = self.get_typed::<T>(key).await? {
            return Ok(cached);
        }

        let value = fetch().await?;
        self.set_typed(key, &value, ttl).await?;
        Ok(value)
    }

    /// Try get without triggering fetch
    async fn try_get_typed<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        self.get_typed(key).await
    }

    /// Remove and return old value
    async fn remove_typed<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let old_value = self.get_typed::<T>(key).await?;
        self.delete(key).await?;
        Ok(old_value)
    }

    /// Check if key exists (typed version)
    async fn contains(&self, key: &str) -> Result<bool> {
        Ok(self.get_bytes(key).await?.is_some())
    }

    // ============================================================================
    // Batch operations (refactored with helper methods)
    // ============================================================================

    /// Check if this is an L2 (Redis) cache backend
    fn is_l2_cache(&self) -> bool {
        #[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
        {
            self.as_any()
                .downcast_ref::<crate::backend::client::RedisBackend>()
                .is_some()
                || self
                    .as_any()
                    .downcast_ref::<crate::backend::client::redis::RedisBackend>()
                    .is_some()
        }
        #[cfg(not(any(feature = "redis", feature = "futures", feature = "core", feature = "full")))]
        {
            false
        }
    }

    /// Determine if parallel execution should be used
    fn should_parallelize(&self, item_count: usize) -> bool {
        self.is_l2_cache() && item_count > 1
    }

    /// Set multiple values with parallel execution for L2 cache
    async fn set_many_bytes<'a, I>(&self, items: I) -> Result<()>
    where
        I: IntoIterator<Item = (&'a str, Vec<u8>)> + Send,
        I::IntoIter: Send,
    {
        let items: Vec<_> = items.into_iter().collect();

        if self.should_parallelize(items.len()) {
            #[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
            {
                let futures: Vec<_> = items.iter().map(|(k, v)| self.set_bytes(k, v.clone(), None)).collect();
                let results: Vec<Result<()>> = join_all(futures).await;
                for result in results {
                    result?;
                }
            }
            #[cfg(not(any(feature = "redis", feature = "futures", feature = "core", feature = "full")))]
            {
                for (key, value) in &items {
                    self.set_bytes(key, value.clone(), None).await?;
                }
            }
        } else {
            for (key, value) in &items {
                self.set_bytes(key, value.clone(), None).await?;
            }
        }
        Ok(())
    }

    /// Get multiple values with parallel execution for L2 cache
    async fn get_many_bytes<'a, I>(&self, keys: I) -> Result<HashMap<String, Vec<u8>>>
    where
        I: IntoIterator<Item = &'a str> + Send,
        I::IntoIter: Send,
    {
        let keys: Vec<_> = keys.into_iter().collect();
        let mut result = HashMap::new();

        if self.should_parallelize(keys.len()) {
            #[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
            {
                let futures: Vec<_> = keys.iter().map(|k| self.get_bytes(k)).collect();
                let results: Vec<Result<Option<Vec<u8>>>> = join_all(futures).await;
                for (key, value_result) in keys.iter().zip(results) {
                    if let Ok(Some(value)) = value_result {
                        result.insert(key.to_string(), value);
                    }
                }
            }
            #[cfg(not(any(feature = "redis", feature = "futures", feature = "core", feature = "full")))]
            {
                for key in &keys {
                    if let Some(value) = self.get_bytes(key).await? {
                        result.insert(key.to_string(), value);
                    }
                }
            }
        } else {
            for key in &keys {
                if let Some(value) = self.get_bytes(key).await? {
                    result.insert(key.to_string(), value);
                }
            }
        }
        Ok(result)
    }

    /// Delete multiple keys with parallel execution for L2 cache
    async fn delete_many<'a, I>(&self, keys: I) -> Result<()>
    where
        I: IntoIterator<Item = &'a str> + Send,
        I::IntoIter: Send,
    {
        let keys: Vec<_> = keys.into_iter().collect();

        if self.should_parallelize(keys.len()) {
            #[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
            {
                let futures: Vec<_> = keys.iter().map(|k| self.delete(k)).collect();
                let results: Vec<Result<()>> = join_all(futures).await;
                for result in results {
                    result?;
                }
            }
            #[cfg(not(any(feature = "redis", feature = "futures", feature = "core", feature = "full")))]
            {
                for key in &keys {
                    self.delete(key).await?;
                }
            }
        } else {
            for key in &keys {
                self.delete(key).await?;
            }
        }
        Ok(())
    }

    /// Set multiple typed values with parallel execution for L2 cache
    async fn set_many_typed<'a, I, T>(&self, items: I) -> Result<()>
    where
        T: Serialize + Send + Sync + 'a,
        I: IntoIterator<Item = (&'a str, &'a T)> + Send,
        I::IntoIter: Send,
    {
        let items: Vec<_> = items.into_iter().collect();

        if self.should_parallelize(items.len()) {
            #[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
            {
                let futures: Vec<_> = items.iter().map(|(k, v)| self.set_typed(k, v, None)).collect();
                let results: Vec<Result<()>> = join_all(futures).await;
                for result in results {
                    result?;
                }
            }
            #[cfg(not(any(feature = "redis", feature = "futures", feature = "core", feature = "full")))]
            {
                for (key, value) in &items {
                    self.set_typed(key, value, None).await?;
                }
            }
        } else {
            for (key, value) in &items {
                self.set_typed(key, value, None).await?;
            }
        }
        Ok(())
    }

    /// Get multiple typed values with parallel execution for L2 cache
    async fn get_many_typed<'a, I, T>(&self, keys: I) -> Result<HashMap<String, T>>
    where
        T: DeserializeOwned + Send + 'a,
        I: IntoIterator<Item = &'a str> + Send,
        I::IntoIter: Send,
    {
        let keys: Vec<_> = keys.into_iter().collect();
        let mut result = HashMap::new();

        if self.should_parallelize(keys.len()) {
            #[cfg(any(feature = "redis", feature = "futures", feature = "core", feature = "full"))]
            {
                let futures: Vec<_> = keys.iter().map(|k| self.get_typed::<T>(k)).collect();
                let results: Vec<Result<Option<T>>> = join_all(futures).await;
                for (key, value_result) in keys.iter().zip(results) {
                    if let Ok(Some(value)) = value_result {
                        result.insert(key.to_string(), value);
                    }
                }
            }
            #[cfg(not(any(feature = "redis", feature = "futures", feature = "core", feature = "full")))]
            {
                for key in &keys {
                    if let Some(value) = self.get_typed::<T>(key).await? {
                        result.insert(key.to_string(), value);
                    }
                }
            }
        } else {
            for key in &keys {
                if let Some(value) = self.get_typed::<T>(key).await? {
                    result.insert(key.to_string(), value);
                }
            }
        }
        Ok(result)
    }

    // ============================================================================
    // Required methods for implementation
    // ============================================================================

    /// Get the serializer used by this cache
    fn serializer(&self) -> &SerializerEnum;

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Convert Arc<Self> to Arc<dyn Any>
    fn into_any_arc(self: std::sync::Arc<Self>) -> std::sync::Arc<dyn Any + Send + Sync>;
}

/// Blanket implementation for all CacheBackend implementations
#[async_trait]
impl<T: crate::backend::CacheBackend + Send + Sync + Any> UnifiedCache for T {
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

    async fn close(&self) -> Result<()> {
        self.close().await
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        self.ttl(key).await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        self.expire(key, ttl).await
    }

    async fn health_check(&self) -> Result<bool> {
        self.health_check().await
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        self.stats().await
    }

    // Default serializer implementation
    #[cfg(any(feature = "serialization", feature = "full"))]
    fn serializer(&self) -> &SerializerEnum {
        use crate::serialization::unified::default_serializer;
        use once_cell::sync::Lazy;

        static DEFAULT_SERIALIZER: Lazy<SerializerEnum> = Lazy::new(|| {
            let unified = default_serializer();
            match unified.format() {
                #[cfg(feature = "bincode")]
                crate::serialization::SerializationFormat::Bincode => {
                    SerializerEnum::Bincode(crate::serialization::bincode::BincodeSerializer)
                }
                _ => SerializerEnum::Json(crate::serialization::json::JsonSerializer::new()),
            }
        });

        &DEFAULT_SERIALIZER
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any_arc(self: std::sync::Arc<Self>) -> std::sync::Arc<dyn Any + Send + Sync> {
        self as std::sync::Arc<dyn Any + Send + Sync>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::client::MokaMemoryBackend as MemoryBackend;
    use crate::backend::CacheBackend;
    use serde::Deserialize;

    #[tokio::test]
    async fn test_unified_cache_backend() {
        let backend = MemoryBackend::new();

        backend.set("test_key", b"test_value".to_vec(), None).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let value = backend.get("test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));

        <MemoryBackend as crate::backend::CacheBackend>::exists(&backend, "test_key")
            .await
            .unwrap();
        <MemoryBackend as crate::backend::CacheBackend>::delete(&backend, "test_key")
            .await
            .unwrap();
        assert!(
            !<MemoryBackend as crate::backend::CacheBackend>::exists(&backend, "test_key")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_unified_cache_typed() {
        let backend = MemoryBackend::new();

        #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
        struct TestStruct {
            name: String,
            value: i32,
        }

        let test_val: TestStruct = TestStruct {
            name: "test".to_string(),
            value: 42,
        };

        backend.set_typed("typed_key", &test_val, None).await.unwrap();
        let retrieved: Option<TestStruct> = backend.get_typed("typed_key").await.unwrap();
        assert_eq!(retrieved, Some(test_val));

        let fetched: TestStruct = backend
            .get_or_fetch::<TestStruct, _, _>("fetch_key", None, || async {
                Ok(TestStruct {
                    name: "fetched".to_string(),
                    value: 100,
                })
            })
            .await
            .unwrap();

        assert_eq!(fetched.name, "fetched");
        assert_eq!(fetched.value, 100);

        let cached: Option<TestStruct> = backend.get_typed("fetch_key").await.unwrap();
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let backend = MemoryBackend::new();

        let items: Vec<(&str, Vec<u8>)> = vec![("key1", b"value1".to_vec()), ("key2", b"value2".to_vec())];
        backend
            .set_many_bytes(items.iter().map(|(k, v)| (*k, v.clone())))
            .await
            .unwrap();

        let keys: Vec<&str> = vec!["key1", "key2", "key3"];
        let results: std::collections::HashMap<String, Vec<u8>> =
            backend.get_many_bytes(keys.iter().cloned()).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains_key("key1"));
        assert!(results.contains_key("key2"));
        assert!(!results.contains_key("key3"));

        CacheBackend::delete_many(&backend, &["key1".to_string(), "key2".to_string()])
            .await
            .unwrap();
        assert!(!CacheBackend::exists(&backend, "key1").await.unwrap());
        assert!(!CacheBackend::exists(&backend, "key2").await.unwrap());
    }
}
