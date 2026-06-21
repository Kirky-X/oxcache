//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified cache builder for single and multi-backend configurations

use crate::backend::interface::CacheBackend;
use crate::backend::memory::moka::MokaMemoryBackend;
use crate::cache::Cache;
use crate::core::traits::CacheKey;
use crate::error::Result;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// Unified builder for creating Cache instances
///
/// Supports both single backend and multi-backend (tiered cache) configurations.
pub struct CacheBuilder<K, V> {
    backends: Vec<Arc<dyn CacheBackend>>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
    capacity: Option<u64>,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            backends: Vec::new(),
            ttl: None,
            tti: None,
            capacity: None,
            _phantom: PhantomData,
        }
    }
}

impl<K, V> CacheBuilder<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    /// Add a pre-built backend
    pub fn backend_arc(mut self, backend: Arc<dyn CacheBackend>) -> Self {
        self.backends.push(backend);
        self
    }

    /// Set the default TTL for cache entries
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the default TTI (time-to-idle) for cache entries
    pub fn tti(mut self, tti: Duration) -> Self {
        self.tti = Some(tti);
        self
    }

    /// Set the capacity for memory-based backends
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// Configure tiered backend (Moka + Redis)
    ///
    /// # Deprecated
    ///
    /// Use `ChainCache::builder()` instead for more flexible multi-backend configuration.
    /// Build the cache instance
    pub async fn build(self) -> Result<Cache<K, V>> {
        let backend = if self.backends.is_empty() {
            let capacity = self.capacity.unwrap_or(10000);
            let mut builder = MokaMemoryBackend::builder().capacity(capacity);
            if let Some(ttl) = self.ttl {
                builder = builder.ttl(ttl);
            }
            if let Some(tti) = self.tti {
                builder = builder.time_to_idle(tti);
            }
            Arc::new(builder.build()) as Arc<dyn CacheBackend>
        } else {
            // Single backend (or prebuilt ChainCache)
            self.backends[0].clone()
        };

        Ok(Cache::new_with_backend(backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default();
        assert!(builder.backends.is_empty());
        assert!(builder.ttl.is_none());
    }

    #[tokio::test]
    async fn test_builder_empty() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_builder_single_backend() {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let cache: Cache<String, i32> = Cache::builder().backend_arc(Arc::new(backend)).build().await.unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }
}
