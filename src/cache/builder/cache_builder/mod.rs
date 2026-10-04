// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Unified cache builder for single and multi-backend configurations

#[cfg(feature = "memory")]
use crate::backend::MokaMemoryBackend;
use crate::backend::SyncCacheBackend;
use crate::backend::{AsyncToSyncBridge, CacheBackend, SyncBackendAdapter};
use crate::cache::Cache;
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(all(feature = "adaptive-ttl", feature = "stale"))]
use crate::i18n::messages::MSG_DETAIL_BUILDER_ADAPTIVE_TTL_STALE_CONFLICT;
#[cfg(feature = "adaptive-ttl")]
use crate::i18n::messages::MSG_DETAIL_BUILDER_ADAPTIVE_TTL_SYNC_CONFLICT;
#[cfg(not(feature = "memory"))]
use crate::i18n::messages::MSG_DETAIL_BUILDER_NO_BACKEND_REQUIRES_MEMORY;
#[cfg(feature = "stale")]
use crate::i18n::messages::MSG_DETAIL_BUILDER_STALE_TTL_SYNC_CONFLICT;
#[cfg(any(not(feature = "memory"), feature = "stale", feature = "adaptive-ttl"))]
use crate::i18n::messages::t;
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
    /// （`UnifiedMetricsRecorder::global_tagged`）——JSON 导出出现
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
                return Err(OxCacheError::NotSupported(t(
                    MSG_DETAIL_BUILDER_NO_BACKEND_REQUIRES_MEMORY,
                    &[],
                )));
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
                        return Err(OxCacheError::NotSupported(t(
                            MSG_DETAIL_BUILDER_STALE_TTL_SYNC_CONFLICT,
                            &[],
                        )));
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
                        return Err(OxCacheError::NotSupported(t(
                            MSG_DETAIL_BUILDER_ADAPTIVE_TTL_SYNC_CONFLICT,
                            &[],
                        )));
                    }
                    #[cfg(feature = "stale")]
                    if self.stale_ttl.is_some() {
                        return Err(OxCacheError::NotSupported(t(
                            MSG_DETAIL_BUILDER_ADAPTIVE_TTL_STALE_CONFLICT,
                            &[],
                        )));
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
                return Err(OxCacheError::NotSupported(t(
                    MSG_DETAIL_BUILDER_STALE_TTL_SYNC_CONFLICT,
                    &[],
                )));
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
                return Err(OxCacheError::NotSupported(t(
                    MSG_DETAIL_BUILDER_ADAPTIVE_TTL_SYNC_CONFLICT,
                    &[],
                )));
            }
            #[cfg(feature = "stale")]
            if self.stale_ttl.is_some() {
                return Err(OxCacheError::NotSupported(t(
                    MSG_DETAIL_BUILDER_ADAPTIVE_TTL_STALE_CONFLICT,
                    &[],
                )));
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

#[cfg(all(test, feature = "memory"))]
mod tests;
