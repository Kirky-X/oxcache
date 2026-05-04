//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache API - 核心缓存结构和方法

mod basic_ops;
mod batch_ops;
mod bytes_ops;
mod macros;

use crate::backend::CacheBackend;
use crate::core::traits::{CacheKey, Cacheable};
use crate::infra::serialization::unified::UnifiedSerializer;
use std::sync::Arc;

/// 序列化器实例复用管理器
#[cfg(any(feature = "serialization", feature = "full"))]
pub(crate) struct SerializerPool {
    json: Arc<crate::serialization::json::JsonSerializer>,
}

#[cfg(any(feature = "serialization", feature = "full"))]
impl SerializerPool {
    pub(crate) fn new() -> Self {
        Self {
            json: Arc::new(crate::serialization::json::JsonSerializer::new()),
        }
    }

    pub(crate) fn json(&self) -> Arc<crate::serialization::json::JsonSerializer> {
        self.json.clone()
    }
}

#[cfg(any(feature = "serialization", feature = "full"))]
impl Default for SerializerPool {
    fn default() -> Self {
        Self::new()
    }
}

/// 核心 Cache 类型
pub struct Cache<K, V> {
    pub(crate) backend: Arc<dyn CacheBackend>,
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub(crate) serializer_pool: Arc<SerializerPool>,
    pub(crate) unified_serializer: UnifiedSerializer,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> std::fmt::Debug for Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache").field("backend", &"<CacheBackend>").finish()
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    pub(crate) fn new_with_backend(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            backend,
            #[cfg(any(feature = "serialization", feature = "full"))]
            serializer_pool: Arc::new(SerializerPool::new()),
            unified_serializer: UnifiedSerializer::json(),
            _phantom: std::marker::PhantomData,
        }
    }

    #[cfg(feature = "moka")]
    pub fn new() -> Self {
        use crate::backend::MokaMemoryBackend;
        Self::new_with_backend(Arc::new(MokaMemoryBackend::new()))
    }

    pub fn builder() -> crate::builder::CacheBuilder<K, V> {
        crate::builder::CacheBuilder::default()
    }

    pub fn with_dependencies(backend: Arc<dyn CacheBackend>) -> Self {
        Self::new_with_backend(backend)
    }
}

#[cfg(feature = "moka")]
impl<K, V> Default for Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "dashmap-backend", not(feature = "moka")))]
impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
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
    V: Cacheable,
{
    pub async fn redis(connection_string: &str) -> crate::error::Result<Self> {
        let backend = crate::backend::memory::RedisBackend::new(connection_string).await?;
        Ok(Self {
            backend: Arc::new(backend),
            #[cfg(any(feature = "serialization", feature = "full"))]
            serializer_pool: Arc::new(SerializerPool::new()),
            unified_serializer: UnifiedSerializer::json(),
            _phantom: std::marker::PhantomData,
        })
    }
}

#[cfg(feature = "confers")]
impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    #[tracing::instrument(skip(config), level = "info")]
    pub async fn with_confers(config: &serde_json::Value) -> crate::error::Result<Self> {
        use crate::backend::memory::RedisBackend;

        let oxcache_config = config.get("oxcache").unwrap_or(&serde_json::json!({}));
        let backend_type = oxcache_config
            .get("backend")
            .and_then(|v| v.as_str())
            .unwrap_or("memory");

        match backend_type {
            "redis" => {
                let redis_url = oxcache_config
                    .get("redis")
                    .and_then(|r| r.get("url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("redis://localhost:6379");
                let backend = RedisBackend::new(redis_url).await?;
                Ok(Self::new_with_backend(Arc::new(backend)))
            }
            _ => {
                use crate::backend::MokaMemoryBackend;
                Ok(Self::new_with_backend(Arc::new(MokaMemoryBackend::new())))
            }
        }
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    pub async fn memory() -> crate::error::Result<Self> {
        use crate::backend::memory::MokaMemoryBackend as MemoryBackend;
        let backend = MemoryBackend::new();
        Ok(Self::new_with_backend(Arc::new(backend)))
    }
}
