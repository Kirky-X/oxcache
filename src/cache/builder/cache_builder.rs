// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Unified cache builder for single and multi-backend configurations

#[cfg(feature = "memory")]
use crate::backend::MokaMemoryBackend;
use crate::backend::SyncCacheBackend;
use crate::backend::{CacheBackend, SyncBackendAdapter};
use crate::cache::Cache;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// 注入 builder 的后端槽位：async 一等入口或 sync 一等入口
///
/// async/sync 两条 trait 层次无 supertrait 关系，`Arc<dyn SyncCacheBackend>`
/// 无法呈现为 `Arc<dyn CacheBackend>`（`trait_upcasting` 不适用），故以 enum
/// 分槽保存；sync 槽位经 [`SyncBackendAdapter`] 门面同时呈现两个面。
pub(crate) enum BackendSlot {
    /// `backend_arc()` 注入的 async 面（无 sync 面，类型已擦除）
    Async(Arc<dyn CacheBackend>),
    /// `sync_backend_arc()` 注入的原生同步后端（双面经门面呈现）
    Sync(Arc<dyn SyncCacheBackend>),
}

/// Unified builder for creating Cache instances
///
/// Supports a **single** backend (or the default Moka backend when none is
/// set). For tiered / multi-backend behavior use
/// [`ChainCacheBuilder`](crate::cache::ChainCacheBuilder) instead — building
/// with more than one `backend_arc()` returns `Err(NotSupported)`.
pub struct CacheBuilder<K, V> {
    backends: Vec<BackendSlot>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
    capacity: Option<u64>,
    /// When true, `build()` wires up `Cache.backend_sync` so the sync API
    /// (`get_sync`/`set_sync`/...) is usable. Supported with the default Moka
    /// backend and with a native sync backend injected via `sync_backend_arc()`;
    /// `backend_arc()` only carries the async surface (the concrete sync impl
    /// is erased behind `Arc<dyn CacheBackend>`), so that combination still
    /// returns `Err(NotSupported)` by necessity.
    sync_mode: bool,
    /// Null cache TTL for penetration guard.
    null_cache_ttl: Option<Duration>,
    /// TTL jitter factor for stampede prevention.
    ttl_jitter_factor: f64,
    /// Injected metrics recorder (metrics feature only).
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<dyn crate::infra::MetricsRecorder>>,
    /// Serialization transport format (serialization feature only).
    #[cfg(any(feature = "serialization", feature = "full"))]
    serialization_format: Option<crate::infra::serialization::SerializationFormat>,
    /// Injected audit event publisher (audit feature only).
    #[cfg(feature = "audit")]
    audit: Option<Arc<dyn crate::features::audit::AuditEventPublisher>>,
    _phantom: PhantomData<(K, V)>,
    /// SWR stale 窗口（`stale` feature）。Some = 启用三态过期装饰器。
    #[cfg(feature = "stale")]
    stale_ttl: Option<Duration>,
    /// SWR 命中策略（`stale` feature）。默认 Return。
    #[cfg(feature = "stale")]
    stale_policy: crate::features::stale::StalePolicy,
    /// SWR stale 事件发布器（`stale` feature）。
    #[cfg(feature = "stale")]
    event_publisher: Option<Arc<dyn crate::core::events::EventPublisher>>,
}

impl<K, V> std::fmt::Debug for CacheBuilder<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // metrics 下披露 recorder 是否已注入：dyn trait 无类型名可打印，
        // 仅判存在——足以锁定「显式 metrics 配置必须真的走到注入分支」
        #[cfg(feature = "metrics")]
        let metrics_injected = self.metrics.is_some();
        #[cfg(not(feature = "metrics"))]
        let metrics_injected = false;
        f.debug_struct("CacheBuilder")
            .field("backends_count", &self.backends.len())
            .field("ttl", &self.ttl)
            .field("tti", &self.tti)
            .field("capacity", &self.capacity)
            .field("sync_mode", &self.sync_mode)
            .field("null_cache_ttl", &self.null_cache_ttl)
            .field("ttl_jitter_factor", &self.ttl_jitter_factor)
            .field("metrics_injected", &metrics_injected)
            .finish()
    }
}

impl<K, V> Default for CacheBuilder<K, V> {
    fn default() -> Self {
        Self {
            #[cfg(feature = "stale")]
            stale_ttl: None,
            #[cfg(feature = "stale")]
            stale_policy: crate::features::stale::StalePolicy::default(),
            #[cfg(feature = "stale")]
            event_publisher: None,
            backends: Vec::new(),
            ttl: None,
            tti: None,
            capacity: None,
            sync_mode: false,
            null_cache_ttl: None,
            ttl_jitter_factor: crate::core::constants::DEFAULT_TTL_JITTER_FACTOR,
            #[cfg(feature = "metrics")]
            metrics: None,
            #[cfg(any(feature = "serialization", feature = "full"))]
            serialization_format: None,
            #[cfg(feature = "audit")]
            audit: None,
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
    ///
    /// The handle carries only the async surface — combining this with
    /// `sync_mode(true)` cannot yield a sync API (the concrete sync impl is
    /// erased); use [`Self::sync_backend_arc`] for that.
    pub fn backend_arc(mut self, backend: Arc<dyn CacheBackend>) -> Self {
        self.backends.push(BackendSlot::Async(backend));
        self
    }

    /// Add a pre-built **native sync** backend (sync-first injection)
    ///
    /// The backend is stored as `Arc<dyn SyncCacheBackend>`: with
    /// `sync_mode(true)` the sync API (`get_sync`/`set_sync`/...) calls it
    /// directly, while the async API reaches it through a
    /// [`SyncBackendAdapter`] facade whose async methods complete
    /// synchronously (no runtime required, blocking semantics of the
    /// underlying sync call). Without `sync_mode(true)` only the async API
    /// is wired — same contract as the default Moka path.
    ///
    /// # Blocking hazard
    ///
    /// Because the facade's async methods complete synchronously, wrapping a
    /// **network-backed** sync backend (e.g. `RedisBackend`, whose sync
    /// surface bridges each call through `block_in_place`) makes every async
    /// API call block an executor thread — under sustained load this starves
    /// the runtime. Network backends must be injected via
    /// [`Self::backend_arc`] and driven through the async API instead;
    /// in-memory backends (Moka / DashMap) are unaffected.
    ///
    /// Only one backend may be added (see [`Self::backend_arc`]).
    pub fn sync_backend_arc(mut self, backend: Arc<dyn SyncCacheBackend>) -> Self {
        self.backends.push(BackendSlot::Sync(backend));
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
    /// Range: `0.0..=1.0`. Values outside this range are clamped; NaN is
    /// treated as `0.0` (no jitter) since NaN would poison the TTL math.
    pub fn ttl_jitter(mut self, factor: f64) -> Self {
        self.ttl_jitter_factor = if factor.is_nan() {
            0.0
        } else {
            factor.clamp(0.0, 1.0)
        };
        self
    }

    /// Inject a metrics recorder.
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

    /// 启用 SWR 三态过期：过期条目在 `stale_ttl` 窗口内仍可返回旧值
    /// （absorb-hitbox-features）。与 `sync_mode(true)` 互斥。
    #[cfg(feature = "stale")]
    pub fn stale_ttl(mut self, stale_ttl: Duration) -> Self {
        self.stale_ttl = Some(stale_ttl);
        self
    }

    /// 设置 SWR 命中策略（默认 Return；OffloadRevalidate 经
    /// `get_or_refresh` 触发后台刷新）。
    #[cfg(feature = "stale")]
    pub fn stale_policy(mut self, policy: crate::features::stale::StalePolicy) -> Self {
        self.stale_policy = policy;
        self
    }

    /// 注入 SWR stale 命中事件的发布器（`CacheEventType::Expire`）。
    #[cfg(feature = "stale")]
    pub fn event_publisher(
        mut self,
        publisher: Arc<dyn crate::core::events::EventPublisher>,
    ) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    /// Set the serialization transport format.
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

    /// Inject an audit event publisher.
    ///
    /// After injection, `get`/`set`/`delete` publish structured audit events
    /// (hit/miss/set/delete) with redacted keys through this publisher.
    /// See [`NoOpAuditPublisher`](crate::features::audit::NoOpAuditPublisher)
    /// and [`InMemoryAuditPublisher`](crate::features::audit::InMemoryAuditPublisher).
    #[cfg(feature = "audit")]
    pub fn audit_publisher(
        mut self,
        publisher: Arc<dyn crate::features::audit::AuditEventPublisher>,
    ) -> Self {
        self.audit = Some(publisher);
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
        if self.backends.is_empty() {
            // 默认 Moka 路径依赖 `memory` 特性；关闭时必须显式提供 backend
            #[cfg(not(feature = "memory"))]
            {
                return Err(OxCacheError::NotSupported(
                    "CacheBuilder with no backend requires the `memory` feature \
                     (default Moka); pass .backend_arc() explicitly otherwise."
                        .to_string(),
                ));
            }

            #[cfg(feature = "memory")]
            {
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
                // stale 装饰器包装（构建路径共用）
                #[cfg(feature = "stale")]
                if let Some(stale_ttl) = self.stale_ttl {
                    if self.sync_mode {
                        return Err(OxCacheError::NotSupported(
                            "stale_ttl cannot be combined with sync_mode(true); the sync API \
                     bypasses the decorator and would see incomplete stale semantics"
                                .to_string(),
                        ));
                    }
                    let mut decorator = crate::features::stale::StaleWhileRevalidateBackend::new(
                        cache.backend.clone(),
                        stale_ttl,
                    )
                    .with_policy(self.stale_policy);
                    if let Some(publisher) = self.event_publisher.clone() {
                        decorator = decorator.with_event_publisher(publisher);
                    }
                    let decorator = Arc::new(decorator);
                    cache.backend = decorator.clone();
                    cache.set_stale_backend(decorator);
                    cache.set_stale_policy(self.stale_policy);
                    if self.stale_policy == crate::features::stale::StalePolicy::OffloadRevalidate {
                        cache.set_offload_manager(Arc::new(
                            crate::features::offload::OffloadManager::new(8),
                        ));
                    }
                }

                #[cfg(any(feature = "serialization", feature = "full"))]
                if let Some(format) = self.serialization_format {
                    cache.unified_serializer = crate::infra::UnifiedSerializer::with_format(format);
                }
                #[cfg(feature = "audit")]
                if let Some(publisher) = self.audit {
                    cache.set_audit_publisher(publisher);
                }
                return Ok(cache);
            }
        }

        // User-provided backend. Fail fast on misconfiguration: only a single
        // backend is supported. Silently dropping the extras would serve
        // traffic from an unintended backend; use ChainCache for
        // tiered/multi-backend behavior.
        if self.backends.len() > 1 {
            return Err(OxCacheError::NotSupported(format!(
                "CacheBuilder supports a single backend, but {} backends were \
                 added; only the first would be used. For tiered/multi-backend \
                 behavior use ChainCache (oxcache::cache::ChainCacheBuilder).",
                self.backends.len()
            )));
        }
        // 槽位归一：Async 仅 async 面（sync 面在 dyn 层已擦除，无法凭空恢复）；
        // Sync 经门面同时呈现双面，sync API 直连原生同步调用
        let (backend, sync_surface) = match &self.backends[0] {
            BackendSlot::Async(b) => {
                if self.sync_mode {
                    return Err(OxCacheError::NotSupported(
                        "backend_arc() only carries Arc<dyn CacheBackend> — the concrete \
                         sync impl is erased behind the async trait object, so the sync API \
                         has nothing to call. Inject a native sync backend via \
                         sync_backend_arc(Arc<dyn SyncCacheBackend>) (e.g. MokaMemoryBackend \
                         or DashMapMemoryBackend), or use the default Moka backend with \
                         sync_mode(true)."
                            .to_string(),
                    ));
                }
                (b.clone(), None)
            }
            BackendSlot::Sync(s) => {
                let adapter: Arc<dyn CacheBackend> = Arc::new(SyncBackendAdapter::new(s.clone()));
                let sync_surface = if self.sync_mode {
                    Some(s.clone())
                } else {
                    None
                };
                (adapter, sync_surface)
            }
        };
        let mut cache = Cache::new_with_backend(backend);
        if let Some(sync) = sync_surface {
            cache.set_sync_backend(sync);
        }
        cache.set_null_cache_ttl(self.null_cache_ttl);
        cache.set_ttl_jitter_factor(self.ttl_jitter_factor);
        #[cfg(feature = "metrics")]
        if let Some(recorder) = self.metrics {
            cache.set_metrics_recorder(recorder);
        }
        // stale 装饰器包装（构建路径共用）
        #[cfg(feature = "stale")]
        if let Some(stale_ttl) = self.stale_ttl {
            if self.sync_mode {
                return Err(OxCacheError::NotSupported(
                    "stale_ttl cannot be combined with sync_mode(true); the sync API \
                     bypasses the decorator and would see incomplete stale semantics"
                        .to_string(),
                ));
            }
            let mut decorator = crate::features::stale::StaleWhileRevalidateBackend::new(
                cache.backend.clone(),
                stale_ttl,
            )
            .with_policy(self.stale_policy);
            if let Some(publisher) = self.event_publisher.clone() {
                decorator = decorator.with_event_publisher(publisher);
            }
            let decorator = Arc::new(decorator);
            cache.backend = decorator.clone();
            cache.set_stale_backend(decorator);
            cache.set_stale_policy(self.stale_policy);
            if self.stale_policy == crate::features::stale::StalePolicy::OffloadRevalidate {
                cache.set_offload_manager(Arc::new(crate::features::offload::OffloadManager::new(
                    8,
                )));
            }
        }

        #[cfg(any(feature = "serialization", feature = "full"))]
        if let Some(format) = self.serialization_format {
            cache.unified_serializer = crate::infra::UnifiedSerializer::with_format(format);
        }
        #[cfg(feature = "audit")]
        if let Some(publisher) = self.audit {
            cache.set_audit_publisher(publisher);
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

    // ========================================================================
    // 默认 unified 指标
    // ========================================================================

    /// 默认构建（未注入 recorder）即产生 unified 指标：set/get/delete 后
    /// 全局计数器递增。全局静态为跨测试共享，用单调 delta 断言保证并行安全；
    /// 与 metrics 重置类测试互斥执行（serial 组），避免 reset 竞态。
    #[cfg(feature = "metrics")]
    #[tokio::test]
    #[serial_test::serial]
    async fn default_cache_records_unified_metrics() {
        let before = crate::infra::GLOBAL_UNIFIED_METRICS.get_counters();

        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache.set(&"metrics-key".to_string(), &7).await.unwrap();
        let _ = cache.get(&"metrics-key".to_string()).await.unwrap(); // hit
        let _ = cache.get(&"metrics-miss".to_string()).await.unwrap(); // miss
        cache.delete(&"metrics-key".to_string()).await.unwrap();

        let after = crate::infra::GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(
            after.l1_sets > before.l1_sets,
            "default cache must record sets into unified metrics"
        );
        assert!(
            after.l1_hits > before.l1_hits,
            "default cache must record hits into unified metrics"
        );
        assert!(
            after.l1_misses > before.l1_misses,
            "default cache must record misses into unified metrics"
        );
        assert!(
            after.l1_deletes > before.l1_deletes,
            "default cache must record deletes into unified metrics"
        );
    }

    /// 显式注入 NoOpMetricsRecorder 仍可恢复静默（覆盖默认 unified）。
    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn explicit_noop_recorder_still_supported() {
        let cache: Cache<String, i32> = Cache::builder()
            .metrics(Arc::new(crate::infra::NoOpMetricsRecorder))
            .build()
            .await
            .unwrap();
        // 不 panic、不落 unified 计数即可构建使用
        cache.set(&"noop-key".to_string(), &1).await.unwrap();
        assert_eq!(
            cache.get(&"noop-key".to_string()).await.unwrap().unwrap(),
            1
        );
    }

    /// backend 维度计数经 export_prometheus_standard 以 backend label 导出。
    #[cfg(feature = "metrics")]
    #[tokio::test]
    async fn backend_label_exported_in_prometheus_standard() {
        let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
        cache
            .set(&"backend-label-key".to_string(), &1)
            .await
            .unwrap();

        let exported = crate::infra::export_prometheus_standard();
        assert!(
            exported.contains("# TYPE oxcache_backend_moka_operations_total counter"),
            "prometheus standard export must declare backend counter type"
        );
        assert!(
            exported.contains("oxcache_backend_moka_operations_total{backend=\"moka\"} "),
            "prometheus standard export must carry the backend label"
        );
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
    async fn test_builder_sync_mode_with_async_backend_still_rejected() {
        // backend_arc() 擦除具体类型后仅剩 async 面：sync API 无可调用对象，
        // 该组合在类型系统上无解（与 trait_upcasting 无关——两套 trait 层次
        // 无 supertrait 关系）。拒绝信息必须指向 sync_backend_arc 一等入口。
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
                    msg.contains("sync_backend_arc"),
                    "error message should point to sync_backend_arc, got: {}",
                    msg
                );
            }
            Err(e) => panic!("expected NotSupported, got {:?}", e),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn test_builder_sync_backend_arc_enables_sync_api() {
        use crate::backend::SyncCacheBackend;

        // sync 一等入口：原生同步后端 + sync_mode(true) → sync API 直连，
        // async API 经 SyncBackendAdapter 门面。Moka 的 sync 面内部桥接
        // （sync_block_on）：sync 直连须在 runtime 外，async 经门面须
        // multi_thread（block_in_place）——与 bytes_ops/cache_builder 既有 NOTE 一致
        let moka = MokaMemoryBackend::builder().capacity(100).build();
        let sync_backend: Arc<dyn SyncCacheBackend> = Arc::new(moka);
        let cache: Cache<String, i32> = Cache::builder()
            .sync_backend_arc(Arc::clone(&sync_backend))
            .sync_mode(true)
            .build_sync()
            .expect("sync backend + sync_mode should build");

        // sync API 直连（无 runtime 上下文）
        cache.set_sync(&"k".to_string(), &7).unwrap();
        assert_eq!(cache.get_sync(&"k".to_string()).unwrap(), Some(7));

        // async API 经门面（multi_thread runtime：block_in_place 安全桥接）
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            cache.set(&"a".to_string(), &1).await.unwrap();
            assert_eq!(cache.get(&"a".to_string()).await.unwrap().unwrap(), 1);
            assert_eq!(
                cache.get(&"k".to_string()).await.unwrap().unwrap(),
                7,
                "sync 写入必须经同一后端对 async 面可见"
            );
        });

        // async 写入对 sync 面同样可见（单一后端，两面共享数据）
        assert_eq!(cache.get_sync(&"a".to_string()).unwrap(), Some(1));
    }

    #[test]
    fn test_builder_sync_backend_arc_without_sync_mode_wires_async_only() {
        use crate::backend::SyncCacheBackend;

        // 不开 sync_mode：仅接线 async 面（与默认 Moka 路径契约一致），
        // sync API 显性报错而非静默可用
        let moka = MokaMemoryBackend::builder().capacity(100).build();
        let sync_backend: Arc<dyn SyncCacheBackend> = Arc::new(moka);
        let cache: Cache<String, i32> = Cache::builder()
            .sync_backend_arc(sync_backend)
            .build_sync()
            .expect("sync backend without sync_mode should still build");

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            cache.set(&"k".to_string(), &3).await.unwrap();
            assert_eq!(cache.get(&"k".to_string()).await.unwrap().unwrap(), 3);
        });

        let err = cache.get_sync(&"k".to_string()).unwrap_err();
        assert!(
            matches!(err, crate::error::OxCacheError::NotSupported(ref m) if m.contains("sync_mode")),
            "sync API without sync_mode must stay rejected, got: {err:?}"
        );
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

    // ============================================================================
    // 审计事件流 —— 注入 publisher 后 get/set/delete 发布结构化事件
    // ============================================================================

    #[cfg(feature = "audit")]
    mod audit_injection {
        use super::*;
        use crate::features::audit::{AuditAction, InMemoryAuditPublisher};

        #[tokio::test]
        async fn injected_publisher_observes_get_set_delete() {
            let publisher = Arc::new(InMemoryAuditPublisher::new(64));
            let cache: Cache<String, i32> = Cache::builder()
                .audit_publisher(publisher.clone())
                .build()
                .await
                .unwrap();

            let _ = cache.get(&"missing".to_string()).await.unwrap(); // miss
            cache.set(&"user:1".to_string(), &42).await.unwrap(); // set
            let _ = cache.get(&"user:1".to_string()).await.unwrap(); // hit
            cache.delete(&"user:1".to_string()).await.unwrap(); // delete

            let events = publisher.snapshot();
            assert_eq!(events.len(), 4, "应发布 4 条审计事件");
            assert_eq!(events[0].action, AuditAction::Miss);
            assert_eq!(events[1].action, AuditAction::Set);
            assert_eq!(events[2].action, AuditAction::Hit);
            assert_eq!(events[3].action, AuditAction::Delete);
            // 键已脱敏透传（普通键原样）
            assert_eq!(events[1].key.as_deref(), Some("user:1"));
            // 事件带时间戳
            assert!(events.iter().all(|e| e.timestamp_ms > 0));
        }

        #[tokio::test]
        async fn sensitive_keys_are_masked_in_audit_events() {
            let publisher = Arc::new(InMemoryAuditPublisher::new(8));
            let cache: Cache<String, String> = Cache::builder()
                .audit_publisher(publisher.clone())
                .build()
                .await
                .unwrap();

            cache
                .set(&"user:password".to_string(), &"hunter2".to_string())
                .await
                .unwrap();

            let events = publisher.snapshot();
            assert_eq!(events.len(), 1);
            let key = events[0].key.as_deref().unwrap_or("");
            assert!(key.starts_with("<sensitive>"), "敏感键应被掩码，got {key}");
            assert!(!key.contains("hunter2"));
        }

        #[tokio::test]
        async fn no_publisher_produces_no_events() {
            let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
            cache.set(&"k".to_string(), &1).await.unwrap();
            let _ = cache.get(&"k".to_string()).await.unwrap();
            // 未注入 publisher：无审计字段，操作照常成功
            assert_eq!(cache.get(&"k".to_string()).await.unwrap(), Some(1));
        }
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
    // 指标注入 —— 注入指标后端可观察到 L1 get/set/evict 计数与延迟样本
    // ============================================================================

    #[cfg(feature = "metrics")]
    mod metrics_injection {
        use super::*;
        use crate::infra::MetricsRecorder;
        use crate::infra::UnifiedMetricsRecorder;

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
            assert!(counters.total_operations >= 4, "总操作数应覆盖纯 L1 路径");

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
    // 二进制序列化格式 —— Cache 级格式切换
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
