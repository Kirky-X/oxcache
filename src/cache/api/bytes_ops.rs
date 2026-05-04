//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 字节操作方法（用于宏兼容）

use super::Cache;
use crate::core::traits::{CacheKey, Cacheable};
use crate::error::Result;
use std::sync::Arc;
use std::time::Duration;

#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::serialization::Serializer;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
{
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.backend.get(key).await
    }

    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    pub async fn set_l1_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    pub async fn set_l2_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let ttl_duration = ttl.map(Duration::from_secs);
        self.backend.set(key, value, ttl_duration).await
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn serializer(&self) -> Arc<dyn Serializer> {
        self.serializer_pool.json()
    }

    pub fn unified_serializer(&self) -> crate::serialization::unified::UnifiedSerializer {
        self.unified_serializer.clone()
    }

    pub fn supports_l1_only(&self) -> bool {
        true
    }

    pub fn supports_l2_only(&self) -> bool {
        false
    }
}
