// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Cache API - impl blocks extracted from mod.rs

use super::*;
use crate::backend::{CacheBackend, SyncCacheBackend};
// UnifiedSerializer 仅在 serialization/full feature 下可用
#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::UnifiedSerializer;
use crate::traits::CacheKey;
use std::sync::Arc;

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
            serializer: Arc::new(crate::infra::JsonSerializer::new()),
            #[cfg(any(feature = "serialization", feature = "full"))]
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

#[cfg(feature = "redis")]
impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn redis(connection_string: &str) -> crate::error::OxCacheResult<Self> {
        let backend = crate::backend::memory::RedisBackend::new(connection_string).await?;
        Ok(Self {
            backend: Arc::new(backend),
            backend_sync: None,
            #[cfg(any(feature = "serialization", feature = "full"))]
            serializer: Arc::new(crate::infra::JsonSerializer::new()),
            #[cfg(any(feature = "serialization", feature = "full"))]
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
    pub async fn memory() -> crate::error::OxCacheResult<Self> {
        use crate::backend::MokaMemoryBackend as MemoryBackend;
        let backend = MemoryBackend::new();
        Ok(Self::new_with_backend(Arc::new(backend)))
    }
}
