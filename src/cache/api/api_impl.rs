// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Cache API - impl blocks extracted from mod.rs

use super::*;
use crate::backend::{CacheBackend, SyncCacheBackend};
#[cfg(not(feature = "memory"))]
use crate::i18n::messages::{MSG_DETAIL_CACHE_MEMORY_REQUIRES_FEATURE, t};
// UnifiedSerializer 仅在 serialization/full feature 下可用
#[cfg(any(feature = "serialization", feature = "full"))]
use crate::infra::UnifiedSerializer;

/// BackendKind → CacheLayer 映射（纯函数）：内存 → L1，分布式 → L2。
#[cfg(feature = "metrics")]
pub(crate) fn layer_for(kind: crate::backend::BackendKind) -> crate::core::CacheLayer {
    if kind.is_distributed() {
        crate::core::CacheLayer::L2
    } else {
        crate::core::CacheLayer::L1
    }
}

#[cfg(all(test, feature = "metrics"))]
mod layer_tests {
    use super::layer_for;
    use crate::backend::BackendKind;
    use crate::core::CacheLayer;

    #[test]
    fn memory_kinds_map_to_l1() {
        assert_eq!(layer_for(BackendKind::Moka), CacheLayer::L1);
        assert_eq!(layer_for(BackendKind::DashMap), CacheLayer::L1);
        assert_eq!(layer_for(BackendKind::Mock), CacheLayer::L1);
        assert_eq!(layer_for(BackendKind::Unknown), CacheLayer::L1);
    }

    #[test]
    fn distributed_kinds_map_to_l2() {
        assert_eq!(layer_for(BackendKind::Redis), CacheLayer::L2);
        assert_eq!(layer_for(BackendKind::Valkey), CacheLayer::L2);
        assert_eq!(layer_for(BackendKind::Dragonfly), CacheLayer::L2);
        assert_eq!(layer_for(BackendKind::Aerospike), CacheLayer::L2);
    }
}
use crate::traits::CacheKey;
use std::sync::Arc;
use std::time::Duration;

impl<K, V> std::fmt::Debug for Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("backend", &format_args!("{}", std::any::type_name::<K>()))
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
            null_cache_ttl: None,
            ttl_jitter_factor: crate::core::constants::DEFAULT_TTL_JITTER_FACTOR,
            // 默认接入全局 unified 指标：metrics feature 随 minimal 预设
            // 默认开启，所有构造路径（new/memory/builder）不再默认零计数。
            // 显式注入 NoOpMetricsRecorder 可恢复静默。
            #[cfg(feature = "metrics")]
            metrics: Arc::new(crate::infra::UnifiedMetricsRecorder::global()),
            #[cfg(feature = "audit")]
            audit: None,
            #[cfg(feature = "stale")]
            stale_backend: None,
            #[cfg(feature = "stale")]
            stale_policy: crate::features::stale::StalePolicy::default(),
            #[cfg(feature = "stale")]
            offload: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// 注入 SWR 装饰器句柄（builder 专用）。
    #[cfg(feature = "stale")]
    pub(crate) fn set_stale_backend(
        &mut self,
        backend: Arc<crate::features::stale::StaleWhileRevalidateBackend>,
    ) {
        self.stale_backend = Some(backend);
    }

    /// 设置 SWR 命中策略（builder 专用）。
    #[cfg(feature = "stale")]
    pub(crate) fn set_stale_policy(&mut self, policy: crate::features::stale::StalePolicy) {
        self.stale_policy = policy;
    }

    /// 注入 Offload 管理器（builder 专用）。
    #[cfg(feature = "stale")]
    pub(crate) fn set_offload_manager(
        &mut self,
        manager: Arc<crate::features::offload::OffloadManager>,
    ) {
        self.offload = Some(manager);
    }

    /// Offload 管理器访问器（`stale` feature，OffloadRevalidate 策略时为
    /// `Some`）。可用于优雅关闭前 `wait_all` 等待后台刷新完成。
    #[cfg(feature = "stale")]
    pub fn offload_manager(&self) -> Option<Arc<crate::features::offload::OffloadManager>> {
        self.offload.clone()
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

    /// Set the null cache TTL for penetration guard.
    pub(crate) fn set_null_cache_ttl(&mut self, ttl: Option<Duration>) {
        self.null_cache_ttl = ttl;
    }

    /// Set the TTL jitter factor for stampede prevention.
    pub(crate) fn set_ttl_jitter_factor(&mut self, factor: f64) {
        self.ttl_jitter_factor = factor;
    }

    /// Metrics layer for the current backend: in-memory backends report as
    /// L1, distributed backends as L2 (replaces the former hardcoded L1).
    #[cfg(feature = "metrics")]
    pub(crate) fn metrics_layer(&self) -> crate::core::CacheLayer {
        layer_for(self.backend.backend_kind())
    }

    /// Backend-labeled operation counter (global unified metrics).
    #[cfg(feature = "metrics")]
    pub(crate) fn record_backend_op(&self) {
        crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS
            .record_backend_operation(self.backend.backend_kind().name());
    }

    /// Inject a metrics recorder.
    ///
    /// When set (non-NoOp), the pure L1 path (`get`/`set`/`delete`) records
    /// hit/miss/set/delete counts and latency samples through the recorder.
    #[cfg(feature = "metrics")]
    pub(crate) fn set_metrics_recorder(
        &mut self,
        recorder: Arc<dyn crate::infra::MetricsRecorder>,
    ) {
        self.metrics = recorder;
    }

    /// Inject an audit event publisher.
    ///
    /// After injection, `get`/`set`/`delete` publish structured audit events
    /// (hit/miss/set/delete) with redacted keys.
    #[cfg(feature = "audit")]
    pub(crate) fn set_audit_publisher(
        &mut self,
        publisher: Arc<dyn crate::features::audit::AuditEventPublisher>,
    ) {
        self.audit = Some(publisher);
    }

    /// 设置同步后端（供 CacheBuilder::sync_mode 在 build() 中调用）。
    /// 当 backend 已实现 SyncCacheBackend 时，将其 Arc 升级为 trait 对象。
    pub(crate) fn set_sync_backend(&mut self, backend: Arc<dyn SyncCacheBackend>) {
        self.backend_sync = Some(backend);
    }

    /// Construct the `NotSupported` error for sync API calls when
    /// `backend_sync` is `None` (i.e. `sync_mode(true)` was not set).
    pub(crate) fn sync_mode_error() -> crate::error::OxCacheError {
        crate::error::OxCacheError::NotSupported(
            "sync API requires CacheBuilder::sync_mode(true); backend_sync is None".to_string(),
        )
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
            null_cache_ttl: None,
            ttl_jitter_factor: crate::core::constants::DEFAULT_TTL_JITTER_FACTOR,
            #[cfg(feature = "metrics")]
            metrics: Arc::new(crate::infra::UnifiedMetricsRecorder::global()),
            #[cfg(feature = "audit")]
            audit: None,
            #[cfg(feature = "stale")]
            stale_backend: None,
            #[cfg(feature = "stale")]
            stale_policy: crate::features::stale::StalePolicy::default(),
            #[cfg(feature = "stale")]
            offload: None,
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
        #[cfg(feature = "memory")]
        {
            use crate::backend::MokaMemoryBackend as MemoryBackend;
            let backend = MemoryBackend::new();
            Ok(Self::new_with_backend(Arc::new(backend)))
        }
        #[cfg(not(feature = "memory"))]
        Err(crate::error::OxCacheError::NotSupported(t(
            MSG_DETAIL_CACHE_MEMORY_REQUIRES_FEATURE,
            &[],
        )))
    }
}
