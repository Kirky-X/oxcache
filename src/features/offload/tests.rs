#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn manager() -> OffloadManager {
        OffloadManager::new(8)
    }

    #[tokio::test]
    async fn spawn_deduplicates_same_key_while_in_flight() {
        static EXECUTED: AtomicUsize = AtomicUsize::new(0);
        let mgr = manager();
        let first = mgr.spawn("k", async {
            EXECUTED.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(80)).await;
        });
        assert!(first, "first spawn must run");
        // leader 任务体在其它 worker 上执行，轮询等待其到达计数点
        let deadline = Instant::now() + Duration::from_secs(1);
        while EXECUTED.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let second = mgr.spawn("k", async {
            EXECUTED.fetch_add(1, Ordering::SeqCst);
        });
        assert!(
            !second,
            "same-key spawn while in flight must be deduplicated"
        );
        assert_eq!(
            EXECUTED.load(Ordering::SeqCst),
            1,
            "deduped task must not run"
        );
        assert!(mgr.is_in_flight("k"));
        let completed = mgr.wait_all(Duration::from_secs(2)).await;
        assert_eq!(completed, 1);
        assert!(!mgr.is_in_flight("k"), "completion must clear in_flight");
        // 完成后可重新调度
        let third = mgr.spawn("k", async {});
        assert!(third, "spawn after completion must run");
    }

    #[tokio::test]
    async fn distinct_keys_spawn_concurrently() {
        let mgr = manager();
        assert!(mgr.spawn("a", async {
            tokio::time::sleep(Duration::from_millis(50)).await
        }));
        assert!(mgr.spawn("b", async {
            tokio::time::sleep(Duration::from_millis(50)).await
        }));
        assert_eq!(mgr.in_flight_count(), 2);
        assert!(mgr.wait_all(Duration::from_secs(2)).await >= 2);
    }

    #[tokio::test]
    async fn concurrency_limit_rejects_overflow() {
        let mgr = OffloadManager::new(1);
        assert!(mgr.spawn("slot", async {
            tokio::time::sleep(Duration::from_millis(80)).await
        }));
        let rejected = mgr.spawn("other", async {});
        assert!(
            !rejected,
            "spawn beyond max_concurrent_tasks must be rejected"
        );
        assert!(mgr.wait_all(Duration::from_secs(2)).await >= 1);
        // 许可释放后可再次调度
        assert!(mgr.spawn("other", async {}));
        mgr.wait_all(Duration::from_secs(2)).await;
    }

    #[tokio::test]
    async fn cancel_all_clears_registry_only() {
        let mgr = manager();
        assert!(mgr.spawn("c1", async {
            tokio::time::sleep(Duration::from_millis(60)).await
        }));
        assert_eq!(mgr.cancel_all(), 1);
        assert_eq!(mgr.in_flight_count(), 0);
        // 任务自然跑完后 guard 不二次计数（key 已被清走）
        mgr.wait_all(Duration::from_secs(2)).await;
    }

    #[tokio::test]
    async fn panic_in_task_does_not_leak_registry() {
        static AFTER_PANIC: AtomicUsize = AtomicUsize::new(0);
        let mgr = manager();
        // 吞掉 panic 输出以外的断言：guard 在 unwind 中必须清理注册表
        let _ = mgr.spawn("p", async {
            AFTER_PANIC.fetch_add(1, Ordering::SeqCst);
            panic!("simulated offload panic");
        });
        // 等待任务结束（panic 被 tokio 捕获）
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(
            !mgr.is_in_flight("p"),
            "panic must not leak the in_flight registry"
        );
        assert!(
            mgr.spawn("p", async {}),
            "spawn after panic must be accepted"
        );
        mgr.wait_all(Duration::from_secs(2)).await;
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    // delta 断言依赖全局计数器窗口期不被清零，须与重置全局指标的串行测试互斥
    #[serial_test::serial]
    async fn metrics_counters_record_lifecycle() {
        let mgr = manager();
        let metrics = &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
        let spawned_before = dynamic_counter(metrics, "oxcache_offload_spawned_total");
        let dedup_before = dynamic_counter(metrics, "oxcache_offload_deduplicated_total");

        assert!(mgr.spawn("m", async {
            tokio::time::sleep(Duration::from_millis(60)).await
        }));
        assert!(!mgr.spawn("m", async {}));
        mgr.wait_all(Duration::from_secs(2)).await;

        // 全局计数器与其它并行测试共享，用单调 delta 断言
        assert!(
            dynamic_counter(metrics, "oxcache_offload_spawned_total") > spawned_before,
            "spawn must increment its counter"
        );
        assert!(
            dynamic_counter(metrics, "oxcache_offload_deduplicated_total") > dedup_before,
            "dedup must increment its counter"
        );
    }

    #[cfg(feature = "metrics")]
    fn dynamic_counter(metrics: &crate::infra::metrics::unified::UnifiedMetrics, key: &str) -> u64 {
        metrics
            .get_dynamic_metrics()
            .get(key)
            .and_then(|v| match v {
                crate::infra::metrics::unified::MetricValue::Counter(c) => Some(*c),
                _ => None,
            })
            .unwrap_or(0)
    }

    #[tokio::test]
    // 与 metrics_counters_record_lifecycle 同规则：cfg(metrics) 下的 delta 断言
    // 依赖全局计数器窗口期不被清零，须与重置全局指标的串行测试互斥
    #[serial_test::serial]
    async fn cancel_policy_counts_timeout() {
        let mgr = OffloadManager::with_policy(2, TimeoutPolicy::Cancel(Duration::from_millis(30)));
        #[cfg(feature = "metrics")]
        let metrics = &crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS;
        #[cfg(feature = "metrics")]
        let timeout_before = dynamic_counter(metrics, "oxcache_offload_timeout_total");

        assert!(mgr.spawn("slow", async {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }));
        let completed = mgr.wait_all(Duration::from_secs(2)).await;
        assert_eq!(completed, 1, "cancelled task must release its slot");

        #[cfg(feature = "metrics")]
        assert!(
            dynamic_counter(metrics, "oxcache_offload_timeout_total") > timeout_before,
            "cancel-policy elapse must increment timeout counter"
        );
    }
}
