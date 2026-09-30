// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Unified cache builder for single and multi-backend configurations

#[cfg(feature = "memory")]
use crate::backend::MokaMemoryBackend;
use crate::backend::SyncCacheBackend;
use crate::backend::{AsyncToSyncBridge, CacheBackend, SyncBackendAdapter};
use crate::cache::Cache;
use crate::error::{OxCacheError, OxCacheResult};
use crate::traits::CacheKey;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// 注入 builder 的后端槽位：async 一等入口、sync 一等入口或双面原生入口
///
/// async/sync 两条 trait 层次无 supertrait 关系，两个方向的 dyn 上转都不可达，
/// 故以 enum 分槽保存并在构建时补齐缺失面：sync 槽位经 [`SyncBackendAdapter`]
/// 门面呈现 async 面；async 槽位在 `sync_mode(true)` 下经
/// [`AsyncToSyncBridge`] 桥出 sync 面（运行时要求见该类型文档）；双面槽位
/// （同一具体后端的两次 coerce，如配置通路的 Moka/DashMap）两面皆原生——
/// async 面运行时无关，sync 面直连，均不走桥接。
pub(crate) enum BackendSlot {
    /// `backend_arc()` 注入的 async 面（原生同步实现已擦除，sync 面经桥接呈现）
    Async(Arc<dyn CacheBackend>),
    /// `sync_backend_arc()` 注入的原生同步后端（async 面经门面呈现）
    Sync(Arc<dyn SyncCacheBackend>),
    /// 同一具体后端的双面原生 coerce（配置通路对 Moka/DashMap 的零损耗注入）
    Dual {
        async_face: Arc<dyn CacheBackend>,
        sync_face: Arc<dyn SyncCacheBackend>,
    },
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
    /// backend, with a native sync backend injected via `sync_backend_arc()`
    /// (direct native calls), and with `backend_arc()` (bridged via
    /// [`AsyncToSyncBridge`], which requires a multi-thread Tokio runtime at
    /// sync-call time).
    sync_mode: bool,
    /// Null cache TTL for penetration guard.
    null_cache_ttl: Option<Duration>,
    /// TTL jitter factor for stampede prevention.
    ttl_jitter_factor: f64,
    /// Injected metrics recorder (metrics feature only).
    #[cfg(feature = "metrics")]
    metrics: Option<Arc<dyn crate::infra::MetricsRecorder>>,
    /// R9 service 维度标签（metrics feature only）：设置后默认全局
    /// recorder 换为 service 标签视图，op 计数带 `service` 维度；
    /// 默认 None = 维度关闭。显式 `.metrics()` 注入优先于本字段。
    #[cfg(feature = "metrics")]
    service_name: Option<Arc<str>>,
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
    /// 自适应 TTL 配置（R5，`adaptive-ttl` feature）。Some = 装饰器启用。
    #[cfg(feature = "adaptive-ttl")]
    adaptive_ttl: Option<crate::features::adaptive_ttl::AdaptiveTtlConfig>,
}

impl<K, V> std::fmt::Debug for CacheBuilder<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // metrics 下披露 recorder 是否已注入：dyn trait 无类型名可打印，
        // 仅判存在——足以锁定「显式 metrics 配置必须真的走到注入分支」
        #[cfg(feature = "metrics")]
        let metrics_injected = self.metrics.is_some() || self.service_name.is_some();
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
            #[cfg(feature = "adaptive-ttl")]
            adaptive_ttl: None,
            backends: Vec::new(),
            ttl: None,
            tti: None,
            capacity: None,
            sync_mode: false,
            null_cache_ttl: None,
            ttl_jitter_factor: crate::core::constants::DEFAULT_TTL_JITTER_FACTOR,
            #[cfg(feature = "metrics")]
            metrics: None,
            #[cfg(feature = "metrics")]
            service_name: None,
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
    /// The handle carries only the async surface — the concrete sync impl is
    /// erased at the `dyn` boundary. With `sync_mode(true)` the sync API is
    /// still available, bridged through [`AsyncToSyncBridge`] (multi-thread
    /// runtime required at sync-call time); prefer [`Self::sync_backend_arc`]
    /// when the backend has a native sync face usable without a runtime.
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

    /// 注入完整槽位（crate 内部通路：配置中枢按后端能力选择槽型）
    ///
    /// 与三个一等入口等价，但保留槽位信息——`Dual` 槽（同一具体后端的
    /// 双面原生 coerce）只能经此注入，保证 async 面不被门面降级。
    pub(crate) fn backend_slot(mut self, slot: BackendSlot) -> Self {
        self.backends.push(slot);
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
    /// Backend-dependent semantics:
    /// - default Moka path: native sync, no runtime required;
    /// - `sync_backend_arc()`: native sync of the wrapped backend (runtime
    ///   requirements are those of the backend);
    /// - dual-face slot (`backend_slot`, config path for Moka/DashMap): both
    ///   surfaces native — async runtime-independent, sync direct;
    /// - `backend_arc()`: bridged sync via [`AsyncToSyncBridge`] — every sync
    ///   call blocks on the async surface and requires a multi-thread Tokio
    ///   runtime (`Err(NotSupported)` outside any runtime or on a
    ///   current_thread runtime).
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

    /// R9 service 维度：为该缓存的操作计数附加 `service` 标签（显式启用）
    ///
    /// 设置后默认全局 recorder 换为 service 标签视图
    /// （[`UnifiedMetricsRecorder::global_tagged`]）——JSON 导出出现
    /// `service_operations` 段，`export_prometheus_standard` 出现
    /// `oxcache_service_operations_total{service="..."}` 行；标签基数上限
    /// 与溢出计数见 [`UnifiedMetrics`](crate::infra::metrics::unified::UnifiedMetrics)。
    /// 未设置（默认）时维度关闭，导出与既有格式逐字节兼容。
    /// 显式 `.metrics()` 注入优先于本字段（自定义 recorder 无法自动打标签）。
    #[cfg(feature = "metrics")]
    pub fn service_name(mut self, service: impl Into<Arc<str>>) -> Self {
        self.service_name = Some(service.into());
        self
    }

    /// R5 自适应 TTL：按访问模式调整条目 TTL（hot 延长 / cold 缩短，
    /// 全部阈值显式见 [`AdaptiveTtlConfig`](crate::features::adaptive_ttl::AdaptiveTtlConfig)）。
    /// 与 `sync_mode(true)`、`stale_ttl` 互斥（构建期显性拒绝）。
    #[cfg(feature = "adaptive-ttl")]
    pub fn adaptive_ttl(
        mut self,
        config: crate::features::adaptive_ttl::AdaptiveTtlConfig,
    ) -> Self {
        self.adaptive_ttl = Some(config);
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
        // R9 service 维度显性化：空串会静默吞掉维度（record 层早退），
        // 构建期显性拒绝而非默认无维度
        #[cfg(feature = "metrics")]
        if self.service_name.as_deref() == Some("") {
            return Err(OxCacheError::InvalidInput(
                "service_name must not be empty; drop the setter to keep the service \
                 dimension disabled"
                    .to_string(),
            ));
        }
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
                {
                    // 显式 .metrics() 优先；service_name 装配默认全局 recorder
                    // 的 service 标签视图（R9）
                    if let Some(recorder) = self.metrics {
                        cache.set_metrics_recorder(recorder);
                    } else if let Some(service) = self.service_name.clone() {
                        cache.set_metrics_recorder(Arc::new(
                            crate::infra::UnifiedMetricsRecorder::global_tagged(service),
                        ));
                    }
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

                #[cfg(feature = "adaptive-ttl")]
                if let Some(config) = self.adaptive_ttl {
                    if self.sync_mode {
                        return Err(OxCacheError::NotSupported(
                            "adaptive_ttl cannot be combined with sync_mode(true); the sync \
                             API bypasses the decorator and would see unadjusted TTLs"
                                .to_string(),
                        ));
                    }
                    #[cfg(feature = "stale")]
                    if self.stale_ttl.is_some() {
                        return Err(OxCacheError::NotSupported(
                            "adaptive_ttl cannot be combined with stale_ttl; stacking two TTL \
                             rewriters has undefined semantics"
                                .to_string(),
                        ));
                    }
                    let decorator =
                        Arc::new(crate::features::adaptive_ttl::AdaptiveTtlBackend::new(
                            cache.backend.clone(),
                            config,
                        )?);
                    cache.backend = decorator;
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
        // 槽位归一：Async 的原生同步实现在 dyn 层已擦除无法恢复，sync 面经
        // AsyncToSyncBridge 桥出（多线程 runtime 要求与拒绝语义见该类型文档）；
        // Sync 经门面同时呈现双面，sync API 直连原生同步调用；Dual 两面皆
        // 原生 coerce（async 面运行时无关，sync 面零桥接税）
        let (backend, sync_surface) = match &self.backends[0] {
            BackendSlot::Async(b) => {
                let sync_surface = if self.sync_mode {
                    Some(Arc::new(AsyncToSyncBridge::new(b.clone())) as Arc<dyn SyncCacheBackend>)
                } else {
                    None
                };
                (b.clone(), sync_surface)
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
            BackendSlot::Dual {
                async_face,
                sync_face,
            } => {
                let sync_surface = if self.sync_mode {
                    Some(sync_face.clone())
                } else {
                    None
                };
                (async_face.clone(), sync_surface)
            }
        };
        let mut cache = Cache::new_with_backend(backend);
        if let Some(sync) = sync_surface {
            cache.set_sync_backend(sync);
        }
        cache.set_null_cache_ttl(self.null_cache_ttl);
        cache.set_ttl_jitter_factor(self.ttl_jitter_factor);
        #[cfg(feature = "metrics")]
        {
            // 显式 .metrics() 优先；service_name 装配默认全局 recorder
            // 的 service 标签视图（R9）
            if let Some(recorder) = self.metrics {
                cache.set_metrics_recorder(recorder);
            } else if let Some(service) = self.service_name.clone() {
                cache.set_metrics_recorder(Arc::new(
                    crate::infra::UnifiedMetricsRecorder::global_tagged(service),
                ));
            }
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

        #[cfg(feature = "adaptive-ttl")]
        if let Some(config) = self.adaptive_ttl {
            if self.sync_mode {
                return Err(OxCacheError::NotSupported(
                    "adaptive_ttl cannot be combined with sync_mode(true); the sync API \
                     bypasses the decorator and would see unadjusted TTLs"
                        .to_string(),
                ));
            }
            #[cfg(feature = "stale")]
            if self.stale_ttl.is_some() {
                return Err(OxCacheError::NotSupported(
                    "adaptive_ttl cannot be combined with stale_ttl; stacking two TTL \
                     rewriters has undefined semantics"
                        .to_string(),
                ));
            }
            let decorator = Arc::new(crate::features::adaptive_ttl::AdaptiveTtlBackend::new(
                cache.backend.clone(),
                config,
            )?);
            cache.backend = decorator;
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

    // backend_arc() + sync_mode(true)：原生同步实现已擦除，sync API 经
    // AsyncToSyncBridge 桥出——阻塞语义，要求调用时处于多线程 runtime。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_builder_sync_mode_with_async_backend_bridges_sync_api() {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let cache: Cache<String, String> = Cache::builder()
            .backend_arc(Arc::new(backend))
            .sync_mode(true)
            .build()
            .await
            .unwrap();

        // 桥接 sync 面读写一致（多线程 runtime 内 block_in_place 合法）
        cache
            .set_sync(&"bridge-key".to_string(), &"v1".to_string())
            .unwrap();
        assert_eq!(
            cache.get_sync(&"bridge-key".to_string()).unwrap(),
            Some("v1".to_string())
        );
        // async 面保持直连后端本体（非门面转接），读写互通
        cache
            .set(&"bridge-key".to_string(), &"v2".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get_sync(&"bridge-key".to_string()).unwrap(),
            Some("v2".to_string())
        );
    }

    #[test]
    fn test_builder_sync_mode_with_async_backend_requires_runtime() {
        // runtime 之外桥接无从等待 async 操作：构建成功，sync 调用显性
        // NotSupported（async 面不受影响；运行时要求属调用期契约）
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let cache: Cache<String, String> = Cache::builder()
            .backend_arc(Arc::new(backend))
            .sync_mode(true)
            .build_sync()
            .unwrap();

        let result = cache.get_sync(&"k".to_string());
        assert!(
            matches!(result, Err(crate::error::OxCacheError::NotSupported(_))),
            "bridged sync outside a runtime must fail loudly, got {:?}",
            result
        );
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

    // current_thread runtime 上 block_in_place 不可用：桥接 sync 调用显性
    // NotSupported 而非 panic（守卫的第二拒绝分支）；async API 照常工作
    #[tokio::test]
    async fn test_builder_bridged_sync_rejected_on_current_thread_runtime() {
        let backend = MokaMemoryBackend::builder().capacity(100).build();
        let cache: Cache<String, String> = Cache::builder()
            .backend_arc(Arc::new(backend))
            .sync_mode(true)
            .build_sync()
            .unwrap();

        let sync_result = cache.set_sync(&"k".to_string(), &"v".to_string());
        assert!(
            matches!(
                sync_result,
                Err(crate::error::OxCacheError::NotSupported(_))
            ),
            "bridged sync on current_thread runtime must fail loudly, got {:?}",
            sync_result
        );

        cache.set(&"k".to_string(), &"v".to_string()).await.unwrap();
        assert_eq!(
            cache.get(&"k".to_string()).await.unwrap(),
            Some("v".to_string())
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

        // ====================================================================
        // R9 service 维度：service_name 装配标签视图 / 默认关闭 / 显式优先
        // ====================================================================

        #[tokio::test]
        #[serial_test::serial]
        async fn service_name_wires_tagged_global_recorder() {
            use crate::infra::GLOBAL_UNIFIED_METRICS;

            let cache: Cache<String, i32> = Cache::builder()
                .service_name("r9-builder-orders")
                .build()
                .await
                .unwrap();
            cache.set(&"svc".to_string(), &1).await.unwrap();
            assert_eq!(cache.get(&"svc".to_string()).await.unwrap(), Some(1));

            let export = GLOBAL_UNIFIED_METRICS.export_prometheus_standard();
            assert!(
                export.contains("oxcache_service_operations_total{service=\"r9-builder-orders\"}"),
                "service label must appear in prometheus export: {}",
                export
            );
        }

        #[tokio::test]
        #[serial_test::serial]
        async fn service_dimension_absent_by_default() {
            use crate::infra::{GLOBAL_UNIFIED_METRICS, convenience};

            convenience::reset();
            let cache: Cache<String, i32> = Cache::builder().build().await.unwrap();
            cache.set(&"plain".to_string(), &1).await.unwrap();
            assert_eq!(cache.get(&"plain".to_string()).await.unwrap(), Some(1));

            let export = GLOBAL_UNIFIED_METRICS.export_prometheus_standard();
            assert!(
                !export.contains("oxcache_service_operations_total"),
                "service lines must be absent without service_name: {export}"
            );
            // JSON 面的「逐字节兼容」承诺：默认（无 service 归因）时
            // service_operations 键经 skip_serializing_if 整体缺席
            #[cfg(feature = "serialization")]
            {
                let json = GLOBAL_UNIFIED_METRICS.export_json().unwrap();
                assert!(
                    !json.contains("\"service_operations\""),
                    "default snapshot JSON must omit service_operations: {json}"
                );
            }
        }

        #[tokio::test]
        #[serial_test::serial]
        async fn explicit_metrics_recorder_overrides_service_name() {
            use crate::infra::{GLOBAL_UNIFIED_METRICS, NoOpMetricsRecorder};

            let cache: Cache<String, i32> = Cache::builder()
                .metrics(Arc::new(NoOpMetricsRecorder))
                .service_name("r9-must-not-appear")
                .build()
                .await
                .unwrap();
            cache.set(&"noop-svc".to_string(), &1).await.unwrap();
            assert_eq!(cache.get(&"noop-svc".to_string()).await.unwrap(), Some(1));

            let export = GLOBAL_UNIFIED_METRICS.export_prometheus_standard();
            assert!(
                !export.contains("r9-must-not-appear"),
                "explicit .metrics() must override service_name: {export}"
            );
        }

        #[tokio::test]
        async fn empty_service_name_fails_at_build() {
            // 空串会静默吞掉维度（record 层早退），构建期显性拒绝
            let result: OxCacheResult<Cache<String, i32>> =
                Cache::builder().service_name("").build().await;
            assert!(
                matches!(
                    &result,
                    Err(OxCacheError::InvalidInput(m)) if m.contains("service_name")
                ),
                "empty service_name must fail loudly: {result:?}"
            );
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

    // ========================================================================
    // R5 自适应 TTL：builder 接线端到端与互斥
    // ========================================================================

    #[cfg(all(feature = "adaptive-ttl", feature = "memory"))]
    mod adaptive_ttl_builder {
        use super::*;
        use crate::features::adaptive_ttl::AdaptiveTtlConfig;
        use std::time::Duration;

        fn config() -> AdaptiveTtlConfig {
            AdaptiveTtlConfig {
                hot_threshold: 2,
                hot_ttl_multiplier: 10.0,
                min_ttl: Duration::from_secs(1),
                max_ttl: Duration::from_secs(30),
                ..AdaptiveTtlConfig::default()
            }
        }

        /// 端到端：builder 装配装饰器后 hot 键在 get 时被延长到上界
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn adaptive_ttl_extends_hot_keys_end_to_end() {
            let cache: Cache<String, i32> = Cache::builder()
                .adaptive_ttl(config())
                .build()
                .await
                .unwrap();
            cache
                .set_with_ttl(&"hot".to_string(), &1, Some(Duration::from_secs(60)))
                .await
                .unwrap();
            let before = cache.ttl(&"hot".to_string()).await.unwrap();
            assert!(
                before > Some(Duration::from_secs(50)),
                "plain base ttl: {before:?}"
            );

            // set 已计 freq=1：首次 get 达 hot_threshold=2 → get 主动延长，
            // 目标 clamp(剩余×10, 1s, 30s)=30s 上界（读数含亚毫秒衰减，按区间断言）
            let _ = cache.get(&"hot".to_string()).await.unwrap();
            let after_first = cache.ttl(&"hot".to_string()).await.unwrap();

            // adjust_interval（默认 60s）限速：同周期第二次 get 不再延长
            //（重置会跳回满额 30s，只会因限速缺席而单调衰减）
            let _ = cache.get(&"hot".to_string()).await.unwrap();
            let after_second = cache.ttl(&"hot".to_string()).await.unwrap();

            let first = after_first.expect("ttl after first get");
            assert!(
                first > Duration::from_secs(29) && first <= Duration::from_secs(30),
                "first get must land on the 30s cap, got {first:?}"
            );
            let second = after_second.expect("ttl after second get");
            assert!(
                second <= first,
                "second get within adjust_interval must not re-extend: {second:?} vs {first:?}"
            );
        }

        /// 与 sync_mode 互斥：构建期显性拒绝
        #[tokio::test]
        async fn adaptive_ttl_rejects_sync_mode() {
            let result: OxCacheResult<Cache<String, i32>> = Cache::builder()
                .adaptive_ttl(config())
                .sync_mode(true)
                .build()
                .await;
            assert!(
                matches!(
                    &result,
                    Err(OxCacheError::NotSupported(m)) if m.contains("adaptive_ttl")
                ),
                "{result:?}"
            );
        }

        /// 与 stale_ttl 互斥：构建期显性拒绝（sync 构建路径）
        #[cfg(feature = "stale")]
        #[test]
        fn adaptive_ttl_rejects_stale_ttl() {
            let result: OxCacheResult<Cache<String, i32>> = Cache::builder()
                .adaptive_ttl(config())
                .stale_ttl(Duration::from_secs(10))
                .build_sync();
            assert!(
                matches!(
                    &result,
                    Err(OxCacheError::NotSupported(m)) if m.contains("adaptive_ttl")
                ),
                "{result:?}"
            );
        }

        /// min_ttl > max_ttl：非法配置必须在构建期显性拒绝（Err 而非
        /// 请求路径上 Duration::clamp panic）
        #[tokio::test]
        async fn adaptive_ttl_rejects_min_ttl_above_max_ttl() {
            let bad = AdaptiveTtlConfig {
                min_ttl: Duration::from_secs(30),
                max_ttl: Duration::from_secs(1),
                ..config()
            };
            let result: OxCacheResult<Cache<String, i32>> =
                Cache::builder().adaptive_ttl(bad).build().await;
            assert!(
                matches!(
                    &result,
                    Err(OxCacheError::InvalidInput(m)) if m.contains("min_ttl") && m.contains("max_ttl")
                ),
                "min_ttl > max_ttl must be rejected at build: {result:?}"
            );
        }

        /// 同上，走 sync 构建入口
        #[test]
        fn adaptive_ttl_sync_build_rejects_min_ttl_above_max_ttl() {
            let bad = AdaptiveTtlConfig {
                min_ttl: Duration::from_secs(30),
                max_ttl: Duration::from_secs(1),
                ..config()
            };
            let result: OxCacheResult<Cache<String, i32>> =
                Cache::builder().adaptive_ttl(bad).build_sync();
            assert!(
                matches!(
                    &result,
                    Err(OxCacheError::InvalidInput(m)) if m.contains("min_ttl") && m.contains("max_ttl")
                ),
                "min_ttl > max_ttl must be rejected at build_sync: {result:?}"
            );
        }
    }
}
