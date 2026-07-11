// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified cache builder for single and multi-backend configurations

use crate::backend::interface::CacheBackend;
use crate::backend::memory::moka::MokaMemoryBackend;
use crate::cache::Cache;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
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
    /// When true, `build()` wires up `Cache.backend_sync` so the sync API
    /// (`get_sync`/`set_sync`/...) is usable. Only supported with the default
    /// Moka backend; combining with `backend_arc()` returns
    /// `Err(NotSupported)` because `Arc<dyn CacheBackend>` cannot be upcast
    /// to `Arc<dyn SyncCacheBackend>` in stable Rust (no `trait_upcasting`).
    sync_mode: bool,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            backends: Vec::new(),
            ttl: None,
            tti: None,
            capacity: None,
            sync_mode: false,
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

    /// Enable or disable sync API support.
    ///
    /// When `true`, `build()` wires up `Cache.backend_sync` so that
    /// `get_sync`/`set_sync`/`get_or_sync`/etc. are usable.
    ///
    /// **Limitation**: only supported with the default Moka backend (i.e.,
    /// when `backend_arc()` is NOT called). Combining `sync_mode(true)` with
    /// `backend_arc()` returns `Err(NotSupported)` because
    /// `Arc<dyn CacheBackend>` cannot be upcast to `Arc<dyn SyncCacheBackend>`
    /// in stable Rust (the `trait_upcasting` feature is unstable).
    pub fn sync_mode(mut self, enabled: bool) -> Self {
        self.sync_mode = enabled;
        self
    }

    /// Build the cache instance
    pub async fn build(self) -> OxCacheResult<Cache<K, V>> {
        // sync_mode(true) + backend_arc() is unsupported: Arc<dyn CacheBackend>
        // cannot be upcast to Arc<dyn SyncCacheBackend> in stable Rust (no
        // `trait_upcasting` feature). Reject early with a clear message.
        if self.sync_mode && !self.backends.is_empty() {
            return Err(OxCacheError::NotSupported(
                "sync_mode(true) cannot be combined with backend_arc(); \
                 Arc<dyn CacheBackend> cannot be upcast to Arc<dyn SyncCacheBackend> \
                 in stable Rust (no trait_upcasting). Use the default Moka backend \
                 with sync_mode, or construct the Cache manually via \
                 Cache::new_with_backend + set_sync_backend."
                    .to_string(),
            ));
        }

        if self.backends.is_empty() {
            // Default Moka path — keep the concrete Arc<MokaMemoryBackend> so
            // we can coerce it to BOTH Arc<dyn CacheBackend> (for async API)
            // AND Arc<dyn SyncCacheBackend> (for sync API) when sync_mode is on.
            let capacity = self.capacity.unwrap_or(10000);
            let mut builder = MokaMemoryBackend::builder().capacity(capacity);
            if let Some(ttl) = self.ttl {
                builder = builder.ttl(ttl);
            }
            if let Some(tti) = self.tti {
                builder = builder.time_to_idle(tti);
            }
            let moka = Arc::new(builder.build());

            let mut cache = Cache::new_with_backend(moka.clone());
            if self.sync_mode {
                cache.set_sync_backend(moka);
            }
            return Ok(cache);
        }

        // User-provided backend (sync_mode is guaranteed false here)
        let backend = self.backends[0].clone();
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

    // ============================================================================
    // ttl() 方法测试 (lines 51-53)
    // ============================================================================

    #[test]
    fn test_builder_ttl() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().ttl(Duration::from_secs(60));
        assert_eq!(builder.ttl, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_builder_ttl_zero() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().ttl(Duration::from_secs(0));
        assert_eq!(builder.ttl, Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_builder_ttl_chained() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().ttl(Duration::from_secs(30)).capacity(100);
        assert_eq!(builder.ttl, Some(Duration::from_secs(30)));
        assert_eq!(builder.capacity, Some(100));
    }

    // ============================================================================
    // tti() 方法测试 (lines 57-59)
    // ============================================================================

    #[test]
    fn test_builder_tti() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().tti(Duration::from_secs(120));
        assert_eq!(builder.tti, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_builder_tti_zero() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().tti(Duration::from_secs(0));
        assert_eq!(builder.tti, Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_builder_tti_chained() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default()
            .tti(Duration::from_secs(45))
            .ttl(Duration::from_secs(300));
        assert_eq!(builder.tti, Some(Duration::from_secs(45)));
        assert_eq!(builder.ttl, Some(Duration::from_secs(300)));
    }

    // ============================================================================
    // capacity() 方法测试 (lines 63-65)
    // ============================================================================

    #[test]
    fn test_builder_capacity() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().capacity(10000);
        assert_eq!(builder.capacity, Some(10000));
    }

    #[test]
    fn test_builder_capacity_zero() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().capacity(0);
        assert_eq!(builder.capacity, Some(0));
    }

    #[test]
    fn test_builder_capacity_chained() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default()
            .capacity(500)
            .ttl(Duration::from_secs(60))
            .tti(Duration::from_secs(30));
        assert_eq!(builder.capacity, Some(500));
        assert_eq!(builder.ttl, Some(Duration::from_secs(60)));
        assert_eq!(builder.tti, Some(Duration::from_secs(30)));
    }

    // ============================================================================
    // backend_arc() 方法测试 (line 74, 77)
    // ============================================================================

    #[test]
    fn test_builder_backend_arc() {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let builder: CacheBuilder<String, String> = CacheBuilder::default().backend_arc(Arc::new(backend));
        assert_eq!(builder.backends.len(), 1);
    }

    #[test]
    fn test_builder_backend_arc_multiple() {
        let backend1 = MokaMemoryBackend::builder().capacity(100).build();
        let backend2 = MokaMemoryBackend::builder().capacity(200).build();
        let builder: CacheBuilder<String, String> = CacheBuilder::default()
            .backend_arc(Arc::new(backend1))
            .backend_arc(Arc::new(backend2));
        assert_eq!(builder.backends.len(), 2);
    }

    // ============================================================================
    // build() 方法测试 - 使用 ttl 和 tti (lines 74, 77)
    // ============================================================================

    #[tokio::test]
    async fn test_builder_build_with_ttl() {
        let cache: Cache<String, i32> = Cache::builder().ttl(Duration::from_secs(60)).build().await.unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_builder_build_with_tti() {
        let cache: Cache<String, i32> = Cache::builder().tti(Duration::from_secs(60)).build().await.unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_builder_build_with_capacity() {
        let cache: Cache<String, i32> = Cache::builder().capacity(100).build().await.unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_builder_build_with_ttl_and_tti() {
        let cache: Cache<String, i32> = Cache::builder()
            .ttl(Duration::from_secs(60))
            .tti(Duration::from_secs(30))
            .capacity(100)
            .build()
            .await
            .unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    // ============================================================================
    // Default 和 builder 链式调用测试
    // ============================================================================

    #[test]
    fn test_builder_default_capacity_none() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default();
        assert!(builder.capacity.is_none());
    }

    #[test]
    fn test_builder_default_tti_none() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default();
        assert!(builder.tti.is_none());
    }

    #[test]
    fn test_builder_default_backends_empty() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default();
        assert!(builder.backends.is_empty());
    }

    #[test]
    fn test_builder_full_chain() {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let builder: CacheBuilder<String, String> = CacheBuilder::default()
            .ttl(Duration::from_secs(60))
            .tti(Duration::from_secs(30))
            .capacity(1000)
            .backend_arc(Arc::new(backend));

        assert_eq!(builder.ttl, Some(Duration::from_secs(60)));
        assert_eq!(builder.tti, Some(Duration::from_secs(30)));
        assert_eq!(builder.capacity, Some(1000));
        assert_eq!(builder.backends.len(), 1);
    }

    // ============================================================================
    // sync_mode() method tests (lines 85-87)
    // ============================================================================

    #[test]
    fn test_builder_sync_mode_true() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().sync_mode(true);
        assert!(builder.sync_mode, "sync_mode(true) should set field to true");
    }

    #[test]
    fn test_builder_sync_mode_false_explicit() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().sync_mode(false);
        assert!(!builder.sync_mode, "sync_mode(false) should set field to false");
    }

    #[test]
    fn test_builder_default_sync_mode_false() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default();
        assert!(!builder.sync_mode, "default sync_mode should be false");
    }

    // ============================================================================
    // build() with sync_mode — end-to-end sync API tests
    // ============================================================================

    // NOTE: multi_thread flavor required — MokaMemoryBackend's sync_block_on
    // uses `block_in_place` to safely drive the async moka future from sync
    // context, but `block_in_place` panics on current_thread runtimes. The
    // sync API is intended for use from multi_thread tokio runtimes (or from
    // outside any async runtime); calling it from a current_thread runtime
    // is an unsupported configuration.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_builder_sync_mode_true_enables_backend_sync() {
        let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();
        // Sync API should work end-to-end
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), Some("v".to_string()));
    }

    #[tokio::test]
    async fn test_builder_default_sync_mode_false_backend_sync_none() {
        let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
        // Sync API should return Err(NotSupported) since sync_mode was not enabled
        let result = cache.get_sync(&"k".to_string());
        assert!(
            matches!(result, Err(crate::error::OxCacheError::NotSupported(_))),
            "expected Err(NotSupported) when sync_mode is false, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_builder_sync_mode_with_unsupported_backend_returns_err() {
        // backend_arc() provides Arc<dyn CacheBackend> which cannot be upcast
        // to Arc<dyn SyncCacheBackend> in stable Rust — even if the underlying
        // concrete type implements SyncCacheBackend. This is a builder-level
        // limitation, not a backend capability issue.
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let result: crate::error::OxCacheResult<Cache<String, String>> = Cache::builder()
            .backend_arc(Arc::new(backend))
            .sync_mode(true)
            .build()
            .await;

        assert!(result.is_err(), "sync_mode(true) + backend_arc() should return Err");
        match result {
            Err(crate::error::OxCacheError::NotSupported(msg)) => {
                assert!(
                    msg.contains("sync_mode") || msg.contains("backend_arc"),
                    "error message should explain the sync_mode+backend_arc limitation, got: {}",
                    msg
                );
            }
            Err(e) => panic!("expected NotSupported, got {:?}", e),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }
}
