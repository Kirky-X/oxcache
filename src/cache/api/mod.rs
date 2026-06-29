//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache API - 核心缓存结构和方法

mod basic_ops;
mod batch_ops;
mod bytes_ops;
mod macros;

use crate::backend::{CacheBackend, SyncCacheBackend};
use crate::infra::serialization::unified::UnifiedSerializer;
use crate::traits::CacheKey;
use std::sync::Arc;

/// 核心 Cache 类型
pub struct Cache<K, V> {
    pub(crate) backend: Arc<dyn CacheBackend>,
    /// 同步后端（可选）。当 builder 启用 `sync_mode` 且后端支持 SyncCacheBackend 时填充。
    /// sync API（get_sync/set_sync 等）通过此字段派发；为 None 时返回 Err(NotSupported)。
    pub(crate) backend_sync: Option<Arc<dyn SyncCacheBackend>>,
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub(crate) serializer: Arc<crate::infra::serialization::json::JsonSerializer>,
    pub(crate) unified_serializer: UnifiedSerializer,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> std::fmt::Debug for Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("backend", &"<CacheBackend>")
            .field("backend_sync", &self.backend_sync.is_some())
            .finish()
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub(crate) fn new_with_backend(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            backend,
            backend_sync: None,
            #[cfg(any(feature = "serialization", feature = "full"))]
            serializer: Arc::new(crate::infra::serialization::json::JsonSerializer::new()),
            unified_serializer: UnifiedSerializer::json(),
            _phantom: std::marker::PhantomData,
        }
    }

    #[cfg(feature = "memory")]
    pub fn new() -> Self {
        use crate::backend::MokaMemoryBackend;
        Self::new_with_backend(Arc::new(MokaMemoryBackend::new()))
    }

    pub fn builder() -> crate::cache::builder::CacheBuilder<K, V> {
        crate::cache::builder::CacheBuilder::default()
    }

    pub fn with_dependencies(backend: Arc<dyn CacheBackend>) -> Self {
        Self::new_with_backend(backend)
    }

    /// 设置同步后端（供 CacheBuilder::sync_mode 在 build() 中调用）。
    /// 当 backend 已实现 SyncCacheBackend 时，将其 Arc 升级为 trait 对象。
    pub(crate) fn set_sync_backend(&mut self, backend: Arc<dyn SyncCacheBackend>) {
        self.backend_sync = Some(backend);
    }
}

#[cfg(feature = "memory")]
impl<K, V> Default for Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "dashmap-backend", not(feature = "memory"), not(feature = "memory")))]
impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub fn new() -> Self {
        use crate::backend::DashMapMemoryBackend;
        Self::new_with_backend(Arc::new(DashMapMemoryBackend::new()))
    }
}

#[cfg(feature = "redis")]
impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn redis(connection_string: &str) -> crate::error::Result<Self> {
        let backend = crate::backend::memory::RedisBackend::new(connection_string).await?;
        Ok(Self {
            backend: Arc::new(backend),
            backend_sync: None,
            #[cfg(any(feature = "serialization", feature = "full"))]
            serializer: Arc::new(crate::infra::serialization::json::JsonSerializer::new()),
            unified_serializer: UnifiedSerializer::json(),
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn memory() -> crate::error::Result<Self> {
        use crate::backend::memory::MokaMemoryBackend as MemoryBackend;
        let backend = MemoryBackend::new();
        Ok(Self::new_with_backend(Arc::new(backend)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_memory() {
        let cache: Cache<String, String> = Cache::memory().await.unwrap();
        assert!(cache.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_new_with_backend() {
        use crate::backend::memory::MokaMemoryBackend;
        let backend = Arc::new(MokaMemoryBackend::new());
        let cache: Cache<String, String> = Cache::new_with_backend(backend);
        assert!(cache.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_cache_builder_default() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        let val = cache.get(&"key".to_string()).await.unwrap().unwrap();
        assert_eq!(val, 42);
    }

    #[tokio::test]
    async fn test_cache_serializer_pool() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        cache.set(&"test".to_string(), &"value".to_string()).await.unwrap();
        assert!(cache.get(&"test".to_string()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cache_unified_serializer() {
        let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
        let data = b"binary data".to_vec();
        cache.set(&"bin".to_string(), &data.clone()).await.unwrap();
        let retrieved = cache.get(&"bin".to_string()).await.unwrap().unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_cache_new() {
        // Cache::new() uses MokaMemoryBackend under the memory feature
        let cache: Cache<String, String> = Cache::new();
        assert!(cache.health_check().await.is_ok());

        cache.set(&"key".to_string(), &"value".to_string()).await.unwrap();
        let val = cache.get(&"key".to_string()).await.unwrap().unwrap();
        assert_eq!(val, "value");
    }

    #[tokio::test]
    async fn test_cache_with_dependencies() {
        use crate::backend::memory::MokaMemoryBackend;
        let backend = Arc::new(MokaMemoryBackend::new());
        let cache: Cache<String, i32> = Cache::with_dependencies(backend);

        assert!(cache.health_check().await.is_ok());
        cache.set(&"k".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_cache_default() {
        // Default impl delegates to Cache::new()
        let cache: Cache<String, String> = Cache::default();
        assert!(cache.health_check().await.is_ok());

        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), "v".to_string());
    }

    #[test]
    fn test_cache_debug() {
        let cache: Cache<String, String> = Cache::new();
        let debug_str = format!("{:?}", cache);
        assert!(debug_str.contains("Cache"));
    }

    #[tokio::test]
    async fn test_cache_new_with_backend_custom() {
        // Verify new_with_backend works with a builder-configured Moka backend
        use crate::backend::memory::MokaMemoryBackend;
        let backend = Arc::new(MokaMemoryBackend::builder().capacity(50).build());
        let cache: Cache<String, Vec<u8>> = Cache::new_with_backend(backend);

        let data = b"hello".to_vec();
        cache.set(&"k".to_string(), &data.clone()).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), data);
    }

    #[tokio::test]
    async fn test_cache_builder_with_backend_arc() {
        // Verify builder() works with a pre-built backend
        use crate::backend::memory::MokaMemoryBackend;
        let backend = Arc::new(MokaMemoryBackend::new());
        let cache: Cache<String, i32> = Cache::builder().backend_arc(backend).build().await.unwrap();

        cache.set(&"n".to_string(), &7).await.unwrap();
        assert_eq!(cache.get(&"n".to_string()).await.unwrap().unwrap(), 7);
    }
}
