// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified cache builder for single and multi-backend configurations

use crate::backend::CacheBackend;
use crate::backend::MokaMemoryBackend;
use crate::cache::Cache;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// Unified builder for creating Cache instances
///
/// Supports a **single** backend (or the default Moka backend when none is
/// set). For tiered / multi-backend behavior use
/// [`ChainCacheBuilder`](crate::cache::ChainCacheBuilder) instead — building
/// with more than one `backend_arc()` returns `Err(NotSupported)`.
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
    /// Null cache TTL for penetration guard.
    null_cache_ttl: Option<Duration>,
    /// TTL jitter factor for stampede prevention.
    ttl_jitter_factor: f64,
    /// Injected metrics recorder (T302; metrics feature only).
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<dyn crate::infra::MetricsRecorder>>,
    /// Serialization transport format (T305; serialization feature only).
    #[cfg(any(feature = "serialization", feature = "full"))]
    serialization_format: Option<crate::infra::serialization::SerializationFormat>,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> std::fmt::Debug for CacheBuilder<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheBuilder")
            .field("backends_count", &self.backends.len())
            .field("ttl", &self.ttl)
            .field("tti", &self.tti)
            .field("capacity", &self.capacity)
            .field("sync_mode", &self.sync_mode)
            .field("null_cache_ttl", &self.null_cache_ttl)
            .field("ttl_jitter_factor", &self.ttl_jitter_factor)
            .finish()
    }
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            backends: Vec::new(),
            ttl: None,
            tti: None,
            capacity: None,
            sync_mode: false,
            null_cache_ttl: None,
            ttl_jitter_factor: 0.0,
            #[cfg(feature = "metrics")]
            metrics: None,
            #[cfg(any(feature = "serialization", feature = "full"))]
            serialization_format: None,
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
    ///
    /// Only one backend may be added: calling [`Self::build_sync`] with two or
    /// more backends returns `Err(NotSupported)`. For tiered caching use
    /// [`ChainCacheBuilder`](crate::cache::ChainCacheBuilder).
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

    /// Set the null cache TTL for cache penetration guard.
    ///
    /// When configured, `get_or_option` will cache a null sentinel value
    /// when the fallback returns `None`, preventing repeated lookups for
    /// non-existent keys (cache penetration).
    pub fn null_cache_ttl(mut self, ttl: Duration) -> Self {
        self.null_cache_ttl = Some(ttl);
        self
    }

    /// Set the TTL jitter factor for cache stampede prevention.
    ///
    /// When > 0.0, the actual TTL for each entry is randomized within
    /// `base_ttl * (1.0 ± factor)`. For example, with `factor = 0.1`
    /// and `base_ttl = 60s`, the actual TTL will be between 54s and 66s.
    ///
    /// Range: `0.0..=1.0`. Values outside this range are clamped.
    pub fn ttl_jitter(mut self, factor: f64) -> Self {
        self.ttl_jitter_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Inject a metrics recorder (T302).
    ///
    /// After injection, the pure L1 path (`get`/`set`/`delete`) records
    /// hits/misses/latency samples through this recorder — previously the
    /// default Moka path produced no metrics at all. Use
    /// [`UnifiedMetricsRecorder`](crate::infra::UnifiedMetricsRecorder)
    /// for counters with standard Prometheus naming, or implement the
    /// [`MetricsRecorder`](crate::infra::MetricsRecorder) port to bridge
    /// to a custom metrics stack.
    #[cfg(feature = "metrics")]
    pub fn metrics(mut self, recorder: Arc<dyn crate::infra::MetricsRecorder>) -> Self {
        self.metrics = Some(recorder);
        self
    }

    /// Set the serialization transport format (T305).
    ///
    /// Default is JSON. Enable `serde-bincode` / `postcard` features and
    /// select a binary format for compact L2 transport. **Do not mix formats
    /// under the same key prefix** — values are not self-describing.
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn serialization_format(
        mut self,
        format: crate::infra::serialization::SerializationFormat,
    ) -> Self {
        self.serialization_format = Some(format);
        self
    }

    /// Build the cache instance (async variant).
    ///
    /// Equivalent to [`Self::build_sync`]; kept as `async` for API stability.
    /// Internally no `.await` is used, so it completes without yielding.
    pub async fn build(self) -> OxCacheResult<Cache<K, V>> {
        self.build_sync()
    }

    /// Build the cache instance (sync variant).
    ///
    /// Constructs a [`Cache`] without awaiting. The default Moka path and the
    /// user-provided backend path are both fully synchronous, so this is
    /// equivalent to [`Self::build`] without an `async` boundary.
    pub fn build_sync(self) -> OxCacheResult<Cache<K, V>> {
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
            cache.set_null_cache_ttl(self.null_cache_ttl);
            cache.set_ttl_jitter_factor(self.ttl_jitter_factor);
            #[cfg(feature = "metrics")]
            if let Some(recorder) = self.metrics {
                cache.set_metrics_recorder(recorder);
            }
            #[cfg(any(feature = "serialization", feature = "full"))]
            if let Some(format) = self.serialization_format {
                cache.unified_serializer = crate::infra::UnifiedSerializer::with_format(format);
            }
            return Ok(cache);
        }

        // User-provided backend (sync_mode is guaranteed false here)
        // Fail fast on misconfiguration: only a single backend is supported.
        // Silently dropping the extras would serve traffic from an unintended
        // backend; use ChainCache for tiered/multi-backend behavior.
        if self.backends.len() > 1 {
            return Err(OxCacheError::NotSupported(format!(
                "CacheBuilder supports a single backend, but {} backends were \
                 added; only the first would be used. For tiered/multi-backend \
                 behavior use ChainCache (oxcache::cache::ChainCacheBuilder).",
                self.backends.len()
            )));
        }
        let backend = self.backends[0].clone();
        let mut cache = Cache::new_with_backend(backend);
        cache.set_null_cache_ttl(self.null_cache_ttl);
        cache.set_ttl_jitter_factor(self.ttl_jitter_factor);
        #[cfg(feature = "metrics")]
        if let Some(recorder) = self.metrics {
            cache.set_metrics_recorder(recorder);
        }
        #[cfg(any(feature = "serialization", feature = "full"))]
        if let Some(format) = self.serialization_format {
            cache.unified_serializer = crate::infra::UnifiedSerializer::with_format(format);
        }
        Ok(cache)
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
        let cache: Cache<String, i32> = Cache::builder()
            .backend_arc(Arc::new(backend))
            .build()
            .await
            .unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    // ============================================================================
    // ttl() 方法测试 (lines 51-53)
    // ============================================================================

    #[test]
    fn test_builder_ttl() {
        let builder: CacheBuilder<String, String> =
            CacheBuilder::default().ttl(Duration::from_secs(60));
        assert_eq!(builder.ttl, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_builder_ttl_zero() {
        let builder: CacheBuilder<String, String> =
            CacheBuilder::default().ttl(Duration::from_secs(0));
        assert_eq!(builder.ttl, Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_builder_ttl_chained() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default()
            .ttl(Duration::from_secs(30))
            .capacity(100);
        assert_eq!(builder.ttl, Some(Duration::from_secs(30)));
        assert_eq!(builder.capacity, Some(100));
    }

    // ============================================================================
    // tti() 方法测试 (lines 57-59)
    // ============================================================================

    #[test]
    fn test_builder_tti() {
        let builder: CacheBuilder<String, String> =
            CacheBuilder::default().tti(Duration::from_secs(120));
        assert_eq!(builder.tti, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_builder_tti_zero() {
        let builder: CacheBuilder<String, String> =
            CacheBuilder::default().tti(Duration::from_secs(0));
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
        let builder: CacheBuilder<String, String> =
            CacheBuilder::default().backend_arc(Arc::new(backend));
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

    #[test]
    fn test_builder_multiple_backends_rejected_at_build() {
        // Building with more than one backend must fail fast instead of
        // silently serving traffic from the first backend only.
        let backend1 = MokaMemoryBackend::builder().capacity(100).build();
        let backend2 = MokaMemoryBackend::builder().capacity(200).build();
        let result: OxCacheResult<Cache<String, String>> = CacheBuilder::default()
            .backend_arc(Arc::new(backend1))
            .backend_arc(Arc::new(backend2))
            .build_sync();
        let err = result.expect_err("multi-backend build must be rejected");
        match &err {
            OxCacheError::NotSupported(msg) => {
                assert!(msg.contains("single backend"), "unexpected message: {msg}");
                assert!(msg.contains("ChainCache"), "unexpected message: {msg}");
            }
            other => panic!("expected NotSupported, got {other:?}"),
        }
    }

    // ============================================================================
    // build() 方法测试 - 使用 ttl 和 tti (lines 74, 77)
    // ============================================================================

    #[tokio::test]
    async fn test_builder_build_with_ttl() {
        let cache: Cache<String, i32> = Cache::builder()
            .ttl(Duration::from_secs(60))
            .build()
            .await
            .unwrap();
        cache.set(&"key".to_string(), &42).await.unwrap();
        assert_eq!(cache.get(&"key".to_string()).await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_builder_build_with_tti() {
        let cache: Cache<String, i32> = Cache::builder()
            .tti(Duration::from_secs(60))
            .build()
            .await
            .unwrap();
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
        assert!(
            builder.sync_mode,
            "sync_mode(true) should set field to true"
        );
    }

    #[test]
    fn test_builder_sync_mode_false_explicit() {
        let builder: CacheBuilder<String, String> = CacheBuilder::default().sync_mode(false);
        assert!(
            !builder.sync_mode,
            "sync_mode(false) should set field to false"
        );
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
        assert_eq!(
            cache.get_sync(&"k".to_string()).unwrap(),
            Some("v".to_string())
        );
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

        assert!(
            result.is_err(),
            "sync_mode(true) + backend_arc() should return Err"
        );
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

    #[tokio::test]
    async fn test_builder_build_sync_default_moka_path() {
        let cache: Cache<String, i32> = Cache::builder()
            .capacity(100)
            .build_sync()
            .expect("build_sync should succeed for default Moka path");
        // async set/get roundtrip works on the cache produced by build_sync
        cache.set(&"k".to_string(), &7).await.unwrap();
        assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), 7);
    }

    #[tokio::test]
    async fn test_builder_build_sync_equivalent_to_build() {
        let via_build: Cache<String, i32> = Cache::builder().capacity(100).build().await.unwrap();
        let via_sync: Cache<String, i32> = Cache::builder().capacity(100).build_sync().unwrap();

        via_build.set(&"a".to_string(), &1).await.unwrap();
        via_sync.set(&"a".to_string(), &1).await.unwrap();
        assert_eq!(
            via_build.get(&"a".to_string()).await.unwrap(),
            via_sync.get(&"a".to_string()).await.unwrap()
        );
    }

    #[test]
    fn test_builder_build_sync_rejects_sync_mode_plus_backend_arc() {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let result: crate::error::OxCacheResult<Cache<String, String>> = Cache::builder()
            .backend_arc(Arc::new(backend))
            .sync_mode(true)
            .build_sync();

        assert!(
            result.is_err(),
            "build_sync with sync_mode+backend_arc should return Err"
        );
    }

    // ============================================================================
    // T302: 指标注入 —— 注入指标后端可观察到 L1 get/set/evict 计数与延迟样本
    // ============================================================================

    #[cfg(feature = "metrics")]
    mod metrics_injection {
        use super::*;
        use crate::infra::UnifiedMetricsRecorder;
        use crate::infra::MetricsRecorder;

        #[tokio::test]
        async fn injected_recorder_observes_l1_get_set_delete_counts() {
            let recorder = Arc::new(UnifiedMetricsRecorder::new());
            let cache: Cache<String, i32> = Cache::builder()
                .metrics(recorder.clone())
                .build()
                .await
                .unwrap();

            // miss（键不存在）
            assert_eq!(cache.get(&"m".to_string()).await.unwrap(), None);
            // set + hit
            cache.set(&"k".to_string(), &42).await.unwrap();
            assert_eq!(cache.get(&"k".to_string()).await.unwrap(), Some(42));
            // delete
            cache.delete(&"k".to_string()).await.unwrap();

            let counters = recorder.metrics().get_counters();
            assert_eq!(counters.l1_misses, 1, "未命中应计数 1");
            assert_eq!(counters.l1_hits, 1, "命中应计数 1");
            assert_eq!(counters.l1_sets, 1, "写入应计数 1");
            assert_eq!(counters.l1_deletes, 1, "删除应计数 1");
            assert!(
                counters.total_operations >= 4,
                "总操作数应覆盖纯 L1 路径"
            );

            // 延迟直方图样本已记录（标准 Prometheus 命名）
            let prom = recorder.metrics().export_prometheus_standard();
            assert!(prom.contains("# TYPE oxcache_hits_total counter"));
            assert!(prom.contains("# TYPE oxcache_misses_total counter"));
            assert!(prom.contains("# TYPE oxcache_operation_duration_seconds histogram"));
            assert!(prom.contains("oxcache_operation_duration_seconds_count 4"));
        }

        #[test]
        fn standard_export_includes_evictions_with_help_type() {
            let recorder = UnifiedMetricsRecorder::new();
            recorder.record_eviction(7);
            let prom = recorder.metrics().export_prometheus_standard();
            assert!(prom.contains("# HELP oxcache_evictions_total "));
            assert!(prom.contains("# TYPE oxcache_evictions_total counter\n"));
            assert!(prom.contains("oxcache_evictions_total 7"));
        }

        #[tokio::test]
        #[serial_test::serial]
        async fn dashmap_capacity_evictions_reach_global_metrics() {
            use crate::backend::DashMapMemoryBackend;
            use crate::infra::GLOBAL_UNIFIED_METRICS;

            convenience_reset();
            let backend = DashMapMemoryBackend::builder().capacity(4).build();
            let cache: Cache<String, i32> = Cache::builder()
                .backend_arc(Arc::new(backend))
                .build()
                .await
                .unwrap();

            // 写入超过容量：触发 FIFO 淘汰
            for i in 0..20 {
                cache.set(&format!("evict-key-{i}"), &i).await.unwrap();
            }

            assert!(
                GLOBAL_UNIFIED_METRICS.get_counters().evictions > 0,
                "DashMap 容量淘汰应计入 evictions 指标"
            );
        }

        fn convenience_reset() {
            crate::infra::convenience::reset();
        }
    }

    // ============================================================================
    // T305: 二进制序列化格式 —— Cache 级格式切换
    // ============================================================================

    #[cfg(all(feature = "serde-bincode", feature = "postcard"))]
    mod binary_serialization {
        use super::*;
        use crate::infra::serialization::SerializationFormat;

        #[tokio::test]
        async fn bincode_format_roundtrips_through_cache() {
            let cache: Cache<String, i32> = Cache::builder()
                .serialization_format(SerializationFormat::Bincode)
                .build()
                .await
                .unwrap();

            cache.set(&"k".to_string(), &12345).await.unwrap();
            assert_eq!(cache.get(&"k".to_string()).await.unwrap(), Some(12345));

            // 原始字节应为 bincode 二进制而非 JSON 文本
            // （bincode 1.x 默认 varint 编码，整数变长）
            let raw = cache.backend.get("k").await.unwrap().unwrap();
            assert_ne!(raw, b"12345".to_vec(), "不应存 JSON 文本");
            assert!(raw.len() <= 8, "bincode i64 应不超过 8 字节，got {raw:?}");
        }

        #[tokio::test]
        async fn postcard_format_roundtrips_through_cache() {
            let cache: Cache<String, String> = Cache::builder()
                .serialization_format(SerializationFormat::Postcard)
                .build()
                .await
                .unwrap();

            cache
                .set(&"k".to_string(), &"postcard-value".to_string())
                .await
                .unwrap();
            assert_eq!(
                cache.get(&"k".to_string()).await.unwrap(),
                Some("postcard-value".to_string())
            );

            // postcard 字符串不以引号开头（JSON 特征）
            let raw = cache.backend.get("k").await.unwrap().unwrap();
            assert_ne!(raw[0], b'"');
        }

        #[tokio::test]
        async fn json_format_remains_default() {
            let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
            cache.set(&"k".to_string(), &7).await.unwrap();
            // JSON 文本特征：数字以 ASCII 数字存储
            let raw = cache.backend.get("k").await.unwrap().unwrap();
            assert_eq!(raw, b"7".to_vec());
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn sync_api_honors_binary_format() {
            let cache: Cache<String, i32> = Cache::builder()
                .sync_mode(true)
                .serialization_format(SerializationFormat::Bincode)
                .build()
                .await
                .unwrap();
            cache.set_sync(&"k".to_string(), &99).unwrap();
            assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), Some(99));
        }
    }
}
