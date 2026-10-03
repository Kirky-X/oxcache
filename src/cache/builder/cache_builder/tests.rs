#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

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
