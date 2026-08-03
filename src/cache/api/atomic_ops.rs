// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Cache 原子操作方法 — 通过 `as_atomic_writer()` 运行时发现后端原子能力。

use super::Cache;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
use std::time::Duration;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    /// 原子递增。委托后端的 `AtomicCacheWriter::incr`。
    ///
    /// 若后端未实现 `AtomicCacheWriter`（`as_atomic_writer()` 返回 `None`），
    /// 返回 `Err(NotSupported)`。
    pub async fn incr(&self, key: &K, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        let writer = self.backend.as_atomic_writer().ok_or_else(|| {
            OxCacheError::NotSupported(
                "incr: backend does not implement AtomicCacheWriter".to_string(),
            )
        })?;
        let key_str = key.to_key_string();
        writer.incr(&key_str, delta, ttl).await
    }

    /// 原子 CAS（compare-and-swap）。
    ///
    /// `expected=None` 表示 SETNX 语义（key 不存在时写入）。
    /// `expected=Some(old_bytes)` 表示当 key 当前值等于 `old_bytes` 时替换为 `new_bytes`。
    pub async fn compare_and_swap(
        &self,
        key: &K,
        expected: Option<&[u8]>,
        new_bytes: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let writer = self.backend.as_atomic_writer().ok_or_else(|| {
            OxCacheError::NotSupported(
                "compare_and_swap: backend does not implement AtomicCacheWriter".to_string(),
            )
        })?;
        let key_str = key.to_key_string();
        writer.compare_and_swap(&key_str, expected, new_bytes, ttl).await
    }

    /// 原子 SETNX（set if absent）。仅在 key 不存在时写入，返回 `true` 表示成功写入。
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub async fn set_if_absent(
        &self,
        key: &K,
        value: &V,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let writer = self.backend.as_atomic_writer().ok_or_else(|| {
            OxCacheError::NotSupported(
                "set_if_absent: backend does not implement AtomicCacheWriter".to_string(),
            )
        })?;
        let key_str = key.to_key_string();
        let bytes = serde_json::to_vec(value).map_err(|e| {
            OxCacheError::Serialization(e.to_string())
        })?;
        writer.set_if_absent(&key_str, bytes, ttl).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MokaMemoryBackend;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_incr_via_moka_backend() {
        let backend = Arc::new(MokaMemoryBackend::new());
        let cache: Cache<String, i64> = Cache::new_with_backend(backend);

        // incr on non-existing key → 0 + 1 = 1
        let val = cache.incr(&"counter".to_string(), 1, None).await.unwrap();
        assert_eq!(val, 1);

        // incr again → 1 + 5 = 6
        let val = cache.incr(&"counter".to_string(), 5, None).await.unwrap();
        assert_eq!(val, 6);

        // negative delta
        let val = cache.incr(&"counter".to_string(), -2, None).await.unwrap();
        assert_eq!(val, 4);
    }

    #[tokio::test]
    #[cfg(any(feature = "serialization", feature = "full"))]
    async fn test_set_if_absent_via_moka_backend() {
        let backend = Arc::new(MokaMemoryBackend::new());
        let cache: Cache<String, String> = Cache::new_with_backend(backend);

        // First set_if_absent should succeed
        let ok = cache
            .set_if_absent(&"nx_key".to_string(), &"first".to_string(), None)
            .await
            .unwrap();
        assert!(ok);

        // Second set_if_absent should fail (key already exists)
        let ok = cache
            .set_if_absent(&"nx_key".to_string(), &"second".to_string(), None)
            .await
            .unwrap();
        assert!(!ok);

        // Value should still be "first"
        let val = cache.get(&"nx_key".to_string()).await.unwrap();
        assert_eq!(val, Some("first".to_string()));
    }

    #[tokio::test]
    async fn test_compare_and_swap_via_moka_backend() {
        let backend = Arc::new(MokaMemoryBackend::new());
        let cache: Cache<String, Vec<u8>> = Cache::new_with_backend(backend);

        // CAS with expected=None → SETNX semantics
        let ok = cache
            .compare_and_swap(&"cas_key".to_string(), None, b"initial".to_vec(), None)
            .await
            .unwrap();
        assert!(ok);

        // CAS with correct expected value
        let ok = cache
            .compare_and_swap(
                &"cas_key".to_string(),
                Some(b"initial"),
                b"updated".to_vec(),
                None,
            )
            .await
            .unwrap();
        assert!(ok);

        // CAS with wrong expected value → should fail
        let ok = cache
            .compare_and_swap(
                &"cas_key".to_string(),
                Some(b"initial"),
                b"again".to_vec(),
                None,
            )
            .await
            .unwrap();
        assert!(!ok);
    }

    // ========================================================================
    // NotSupported error path tests
    // ========================================================================

    #[tokio::test]
    async fn test_incr_not_supported_for_dashmap_backend() {
        use crate::backend::DashMapMemoryBackend;
        use crate::error::OxCacheError;

        let backend = Arc::new(DashMapMemoryBackend::new());
        let cache: Cache<String, i64> = Cache::new_with_backend(backend);

        // DashMap does not implement AtomicCacheWriter
        let result = cache.incr(&"counter".to_string(), 1, None).await;
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "incr should return NotSupported for DashMap backend, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_compare_and_swap_not_supported_for_dashmap_backend() {
        use crate::backend::DashMapMemoryBackend;
        use crate::error::OxCacheError;

        let backend = Arc::new(DashMapMemoryBackend::new());
        let cache: Cache<String, Vec<u8>> = Cache::new_with_backend(backend);

        let result = cache
            .compare_and_swap(&"k".to_string(), None, b"v".to_vec(), None)
            .await;
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "compare_and_swap should return NotSupported for DashMap backend, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[cfg(any(feature = "serialization", feature = "full"))]
    async fn test_set_if_absent_not_supported_for_dashmap_backend() {
        use crate::backend::DashMapMemoryBackend;
        use crate::error::OxCacheError;

        let backend = Arc::new(DashMapMemoryBackend::new());
        let cache: Cache<String, String> = Cache::new_with_backend(backend);

        let result = cache
            .set_if_absent(&"k".to_string(), &"v".to_string(), None)
            .await;
        assert!(
            matches!(result, Err(OxCacheError::NotSupported(_))),
            "set_if_absent should return NotSupported for DashMap backend, got {:?}",
            result
        );
    }
}
