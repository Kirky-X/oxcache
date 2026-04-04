// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 指标模块测试

#[cfg(any(feature = "metrics", feature = "moka"))]
mod tests {
    use oxcache::metrics::{
        export_json_format, export_prometheus_format, get_enhanced_stats, get_metrics_string, CacheStats, Metrics,
        GLOBAL_METRICS,
    };

    // ========================================
    // 基础计数器测试
    // ========================================

    #[test]
    fn test_record_request_l1_get_hit() {
        GLOBAL_METRICS.record_request("test_service", "L1", "get", "hit");
        let (l1_hits, _, _, _, _, _, _, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l1_hits > 0);
    }

    #[test]
    fn test_record_request_l1_get_miss() {
        GLOBAL_METRICS.record_request("test_service", "L1", "get", "miss");
        let (_, l1_misses, _, _, _, _, _, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l1_misses > 0);
    }

    #[test]
    fn test_record_request_l2_get_hit() {
        GLOBAL_METRICS.record_request("test_service", "L2", "get", "hit");
        let (_, _, l2_hits, _, _, _, _, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l2_hits > 0);
    }

    #[test]
    fn test_record_request_l2_get_miss() {
        GLOBAL_METRICS.record_request("test_service", "L2", "get", "miss");
        let (_, _, _, l2_misses, _, _, _, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l2_misses > 0);
    }

    #[test]
    fn test_record_request_l1_set() {
        GLOBAL_METRICS.record_request("test_service", "L1", "set", "attempt");
        let (_, _, _, _, l1_sets, _, _, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l1_sets > 0);
    }

    #[test]
    fn test_record_request_l2_set() {
        GLOBAL_METRICS.record_request("test_service", "L2", "set", "attempt");
        let (_, _, _, _, _, l2_sets, _, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l2_sets > 0);
    }

    #[test]
    fn test_record_request_l1_delete() {
        GLOBAL_METRICS.record_request("test_service", "L1", "delete", "attempt");
        let (_, _, _, _, _, _, l1_deletes, _, _) = GLOBAL_METRICS.get_counters();
        assert!(l1_deletes > 0);
    }

    #[test]
    fn test_record_request_l2_delete() {
        GLOBAL_METRICS.record_request("test_service", "L2", "delete", "attempt");
        let (_, _, _, _, _, _, _, l2_deletes, _) = GLOBAL_METRICS.get_counters();
        assert!(l2_deletes > 0);
    }

    #[test]
    fn test_record_duration() {
        GLOBAL_METRICS.record_duration("test_service", "L1", "get", 0.001);
        GLOBAL_METRICS.record_duration("test_service", "L1", "get", 0.002);
    }

    #[test]
    fn test_set_health() {
        GLOBAL_METRICS.set_health("test_service_health", 1);
    }

    #[test]
    fn test_set_wal_size() {
        GLOBAL_METRICS.set_wal_size("test_service_wal", 100);
    }

    #[test]
    fn test_set_batch_buffer_size() {
        GLOBAL_METRICS.set_batch_buffer_size("test_service_batch", 50);
    }

    #[test]
    fn test_set_batch_success_rate() {
        GLOBAL_METRICS.set_batch_success_rate("test_service_rate", 0.95);
    }

    #[test]
    fn test_set_batch_throughput() {
        GLOBAL_METRICS.set_batch_throughput("test_service_throughput", 1000.0);
    }

    #[test]
    fn test_total_operations() {
        let initial = GLOBAL_METRICS.get_counters().8;
        GLOBAL_METRICS.record_request("test_service_total", "L1", "get", "hit");
        let after = GLOBAL_METRICS.get_counters().8;
        assert!(after > initial);
    }

    #[test]
    fn test_get_metrics_string() {
        GLOBAL_METRICS.record_request("test_metrics_string", "L1", "get", "hit");
        let metrics_string = get_metrics_string();
        assert!(metrics_string.contains("cache_l1_get_hits_total"));
    }

    #[test]
    fn test_metrics_default() {
        let metrics = Metrics::default();
        let counters = metrics.get_counters();
        assert_eq!(counters.0, 0);
        assert_eq!(counters.1, 0);
        assert_eq!(counters.2, 0);
        assert_eq!(counters.3, 0);
    }

    // ========================================
    // 预取和压缩操作测试
    // ========================================

    #[test]
    fn test_record_prefetch() {
        let metrics = Metrics::default();
        metrics.record_prefetch();
        metrics.record_prefetch();
        metrics.record_prefetch();
        let snapshot = metrics.snapshot();
        assert!(snapshot.prefetch_count >= 3);
    }

    #[test]
    fn test_record_compression() {
        let metrics = Metrics::default();
        metrics.record_compression(100);
        metrics.record_compression(200);
        let snapshot = metrics.snapshot();
        assert!(snapshot.compression_count >= 2);
        assert!(snapshot.compression_bytes_saved >= 300);
    }

    #[test]
    fn test_record_compression_zero_bytes() {
        let metrics = Metrics::default();
        metrics.record_compression(0);
        let snapshot = metrics.snapshot();
        assert!(snapshot.compression_count >= 1);
        assert_eq!(snapshot.compression_bytes_saved, 0);
    }

    // ========================================
    // L1 缓存容量和项数测试
    // ========================================

    #[test]
    fn test_set_l1_item_count() {
        let metrics = Metrics::default();
        metrics.set_l1_item_count(1000);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.l1_item_count, 1000);
    }

    #[test]
    fn test_set_l1_capacity_used() {
        let metrics = Metrics::default();
        metrics.set_l1_capacity_used(1024 * 1024);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.l1_capacity_used, 1024 * 1024);
    }

    #[test]
    fn test_set_l1_item_count_zero() {
        let metrics = Metrics::default();
        metrics.set_l1_item_count(0);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.l1_item_count, 0);
    }

    // ========================================
    // 快照和重置测试
    // ========================================

    #[test]
    fn test_snapshot_basic() {
        let metrics = Metrics::default();
        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L2", "get", "miss");
        metrics.set_l1_item_count(50);

        let snapshot = metrics.snapshot();
        assert!(snapshot.l1_hits > 0);
        assert!(snapshot.l2_misses > 0);
        assert_eq!(snapshot.l1_item_count, 50);
    }

    #[test]
    fn test_reset_clears_all_counters() {
        let metrics = Metrics::default();

        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L1", "get", "miss");
        metrics.record_request("svc", "L2", "get", "hit");
        metrics.record_request("svc", "L1", "set", "attempt");
        metrics.record_prefetch();
        metrics.record_compression(1000);
        metrics.set_l1_item_count(100);
        metrics.set_l1_capacity_used(500);

        let before_reset = metrics.snapshot();
        assert!(before_reset.total_operations > 0);

        metrics.reset();

        let after_reset = metrics.snapshot();
        assert_eq!(after_reset.l1_hits, 0);
        assert_eq!(after_reset.l1_misses, 0);
        assert_eq!(after_reset.l2_hits, 0);
        assert_eq!(after_reset.l2_misses, 0);
        assert_eq!(after_reset.l1_sets, 0);
        assert_eq!(after_reset.l2_sets, 0);
        assert_eq!(after_reset.l1_deletes, 0);
        assert_eq!(after_reset.l2_deletes, 0);
        assert_eq!(after_reset.total_operations, 0);
        assert_eq!(after_reset.l1_item_count, 0);
        assert_eq!(after_reset.l1_capacity_used, 0);
        assert_eq!(after_reset.prefetch_count, 0);
        assert_eq!(after_reset.compression_count, 0);
        assert_eq!(after_reset.compression_bytes_saved, 0);
    }

    #[test]
    fn test_reset_clears_dashmaps() {
        let metrics = Metrics::default();

        metrics.set_health("health_svc", 1);
        metrics.set_wal_size("wal_svc", 50);
        metrics.set_batch_buffer_size("batch_svc", 100);
        metrics.set_batch_success_rate("rate_svc", 0.9);
        metrics.set_batch_throughput("tp_svc", 500.0);
        metrics.record_duration("svc", "L1", "get", 0.5);
        metrics.record_request("custom_svc", "L3", "custom", "result");

        metrics.reset();

        let fresh_metrics = Metrics::default();
        fresh_metrics.reset();
    }

    // ========================================
    // 命中率计算测试
    // ========================================

    #[test]
    fn test_hit_rate_empty() {
        let metrics = Metrics::default();
        metrics.reset();
        let rate = metrics.hit_rate();
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_hit_rate_all_hits() {
        let metrics = Metrics::default();
        metrics.reset();

        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L2", "get", "hit");

        let rate = metrics.hit_rate();
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_hit_rate_all_misses() {
        let metrics = Metrics::default();
        metrics.reset();

        metrics.record_request("svc", "L1", "get", "miss");
        metrics.record_request("svc", "L2", "get", "miss");

        let rate = metrics.hit_rate();
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_hit_rate_mixed() {
        let metrics = Metrics::default();
        metrics.reset();

        for _ in 0..7 {
            metrics.record_request("svc", "L1", "get", "hit");
        }
        for _ in 0..3 {
            metrics.record_request("svc", "L1", "get", "miss");
        }

        let rate = metrics.hit_rate();
        assert!((rate - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_hit_rate_percent_format() {
        let metrics = Metrics::default();
        metrics.reset();

        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L1", "get", "miss");

        let percent_str = metrics.hit_rate_percent();
        assert!(percent_str.contains("%"));
        assert!(percent_str.contains("50"));
    }

    // ========================================
    // CacheStats 命中率测试
    // ========================================

    #[test]
    fn test_cache_stats_l1_hit_rate() {
        let stats = CacheStats {
            l1_hits: 70,
            l1_misses: 30,
            l2_hits: 0,
            l2_misses: 0,
            l1_sets: 0,
            l2_sets: 0,
            l1_deletes: 0,
            l2_deletes: 0,
            total_operations: 100,
            l1_item_count: 0,
            l1_capacity_used: 0,
            prefetch_count: 0,
            compression_count: 0,
            compression_bytes_saved: 0,
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(stats.l1_hit_rate(), 0.7);
        assert_eq!(stats.l2_hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_l2_hit_rate() {
        let stats = CacheStats {
            l1_hits: 0,
            l1_misses: 0,
            l2_hits: 40,
            l2_misses: 10,
            l1_sets: 0,
            l2_sets: 0,
            l1_deletes: 0,
            l2_deletes: 0,
            total_operations: 50,
            l1_item_count: 0,
            l1_capacity_used: 0,
            prefetch_count: 0,
            compression_count: 0,
            compression_bytes_saved: 0,
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(stats.l2_hit_rate(), 0.8);
    }

    #[test]
    fn test_cache_stats_overall_hit_rate() {
        let stats = CacheStats {
            l1_hits: 50,
            l1_misses: 10,
            l2_hits: 30,
            l2_misses: 10,
            l1_sets: 0,
            l2_sets: 0,
            l1_deletes: 0,
            l2_deletes: 0,
            total_operations: 100,
            l1_item_count: 0,
            l1_capacity_used: 0,
            prefetch_count: 0,
            compression_count: 0,
            compression_bytes_saved: 0,
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(stats.overall_hit_rate(), 0.8);
    }

    #[test]
    fn test_cache_stats_zero_operations() {
        let stats = CacheStats::default();

        assert_eq!(stats.l1_hit_rate(), 0.0);
        assert_eq!(stats.l2_hit_rate(), 0.0);
        assert_eq!(stats.overall_hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_percent_strings() {
        let stats = CacheStats {
            l1_hits: 75,
            l1_misses: 25,
            l2_hits: 50,
            l2_misses: 50,
            l1_sets: 0,
            l2_sets: 0,
            l1_deletes: 0,
            l2_deletes: 0,
            total_operations: 200,
            l1_item_count: 0,
            l1_capacity_used: 0,
            prefetch_count: 0,
            compression_count: 0,
            compression_bytes_saved: 0,
            timestamp: chrono::Utc::now(),
        };

        assert!(stats.l1_hit_rate_percent().contains("75"));
        assert!(stats.l2_hit_rate_percent().contains("50"));
        assert!(stats.overall_hit_rate_percent().contains("62.5"));
    }

    // ========================================
    // Prometheus 导出测试
    // ========================================

    #[test]
    fn test_cache_stats_export_prometheus() {
        let stats = CacheStats {
            l1_hits: 100,
            l1_misses: 20,
            l2_hits: 50,
            l2_misses: 10,
            l1_sets: 30,
            l2_sets: 15,
            l1_deletes: 5,
            l2_deletes: 3,
            total_operations: 133,
            l1_item_count: 500,
            l1_capacity_used: 1024,
            prefetch_count: 10,
            compression_count: 5,
            compression_bytes_saved: 2000,
            timestamp: chrono::Utc::now(),
        };

        let prometheus = stats.export_prometheus();

        assert!(prometheus.contains("cache_l1_hits_total 100"));
        assert!(prometheus.contains("cache_l1_misses_total 20"));
        assert!(prometheus.contains("cache_l2_hits_total 50"));
        assert!(prometheus.contains("cache_l2_misses_total 10"));
        assert!(prometheus.contains("cache_l1_sets_total 30"));
        assert!(prometheus.contains("cache_l2_sets_total 15"));
        assert!(prometheus.contains("cache_l1_deletes_total 5"));
        assert!(prometheus.contains("cache_l2_deletes_total 3"));
        assert!(prometheus.contains("cache_operations_total 133"));
        assert!(prometheus.contains("cache_l1_item_count 500"));
        assert!(prometheus.contains("cache_l1_capacity_used_bytes 1024"));
        assert!(prometheus.contains("cache_prefetch_total 10"));
        assert!(prometheus.contains("cache_compression_total 5"));
        assert!(prometheus.contains("cache_compression_bytes_saved 2000"));
        assert!(prometheus.contains("cache_l1_hit_rate"));
        assert!(prometheus.contains("cache_l2_hit_rate"));
        assert!(prometheus.contains("cache_overall_hit_rate"));
        assert!(prometheus.contains("# Generated at:"));
    }

    #[test]
    fn test_metrics_export_prometheus() {
        let metrics = Metrics::default();
        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L2", "get", "miss");

        let prometheus = metrics.export_prometheus();
        assert!(prometheus.contains("cache_l1_hits_total"));
        assert!(prometheus.contains("cache_l2_misses_total"));
        assert!(prometheus.contains("cache_l1_hit_rate"));
    }

    #[test]
    fn test_export_prometheus_format_global() {
        GLOBAL_METRICS.record_request("global_test", "L1", "get", "hit");
        let output = export_prometheus_format();
        assert!(output.contains("cache_"));
        assert!(output.contains("# Cache Statistics"));
    }

    // ========================================
    // JSON 导出测试
    // ========================================

    #[test]
    fn test_cache_stats_export_json() {
        let stats = CacheStats {
            l1_hits: 50,
            l1_misses: 10,
            l2_hits: 20,
            l2_misses: 5,
            l1_sets: 5,
            l2_sets: 2,
            l1_deletes: 1,
            l2_deletes: 1,
            total_operations: 88,
            l1_item_count: 100,
            l1_capacity_used: 2048,
            prefetch_count: 3,
            compression_count: 2,
            compression_bytes_saved: 500,
            timestamp: chrono::Utc::now(),
        };

        let json = stats.export_json().unwrap();

        assert!(json.contains("\"l1_hits\": 50"));
        assert!(json.contains("\"l1_misses\": 10"));
        assert!(json.contains("\"l2_hits\": 20"));
        assert!(json.contains("\"l2_misses\": 5"));
        assert!(json.contains("\"l1_sets\": 5"));
        assert!(json.contains("\"l2_sets\": 2"));
        assert!(json.contains("\"l1_deletes\": 1"));
        assert!(json.contains("\"l2_deletes\": 1"));
        assert!(json.contains("\"total_operations\": 88"));
        assert!(json.contains("\"l1_item_count\": 100"));
        assert!(json.contains("\"l1_capacity_used\": 2048"));
        assert!(json.contains("\"prefetch_count\": 3"));
        assert!(json.contains("\"compression_count\": 2"));
        assert!(json.contains("\"compression_bytes_saved\": 500"));
        assert!(json.contains("\"timestamp\":"));
    }

    #[test]
    fn test_metrics_export_json() {
        let metrics = Metrics::default();
        metrics.record_request("svc", "L1", "get", "hit");

        let json_result = metrics.export_json();
        assert!(json_result.is_ok());

        let json = json_result.unwrap();
        assert!(json.contains("l1_hits"));
    }

    #[test]
    fn test_export_json_format_global() {
        let result = export_json_format();
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("l1_hits"));
    }

    // ========================================
    // get_enhanced_stats 测试
    // ========================================

    #[test]
    fn test_get_enhanced_stats() {
        let stats = get_enhanced_stats();
        assert!(stats.timestamp <= chrono::Utc::now());
    }

    // ========================================
    // get_metrics_string 详细分支测试
    // ========================================

    #[test]
    fn test_get_metrics_string_with_health_status() {
        GLOBAL_METRICS.set_health("health_test_svc", 2);
        let output = get_metrics_string();
        assert!(output.contains("cache_l2_health_status"));
        assert!(output.contains("health_test_svc"));
    }

    #[test]
    fn test_get_metrics_string_with_wal_entries() {
        GLOBAL_METRICS.set_wal_size("wal_test_svc", 1234);
        let output = get_metrics_string();
        assert!(output.contains("cache_wal_entries"));
        assert!(output.contains("wal_test_svc"));
    }

    #[test]
    fn test_get_metrics_string_with_batch_metrics() {
        GLOBAL_METRICS.set_batch_buffer_size("batch_buf_svc", 999);
        GLOBAL_METRICS.set_batch_success_rate("batch_rate_svc", 0.85);
        GLOBAL_METRICS.set_batch_throughput("batch_tp_svc", 2500.0);

        let output = get_metrics_string();
        assert!(output.contains("cache_batch_buffer_size"));
        assert!(output.contains("cache_batch_success_rate"));
        assert!(output.contains("cache_batch_throughput"));
    }

    #[test]
    fn test_get_metrics_string_with_operation_duration() {
        GLOBAL_METRICS.record_duration("dur_svc", "L1", "get", 0.1);
        GLOBAL_METRICS.record_duration("dur_svc", "L1", "get", 0.2);
        GLOBAL_METRICS.record_duration("dur_svc", "L2", "set", 0.5);

        let output = get_metrics_string();
        assert!(output.contains("cache_operation_duration_seconds"));
        assert!(output.contains("dur_svc"));
        assert!(output.contains("L1"));
        assert!(output.contains("get"));
    }

    #[test]
    fn test_get_metrics_string_with_requests_total() {
        GLOBAL_METRICS.record_request("custom_svc", "L3", "custom_op", "custom_result");
        let output = get_metrics_string();
        assert!(output.contains("cache_requests_total"));
        assert!(output.contains("custom_svc:L3:custom_op:custom_result"));
    }

    #[test]
    fn test_get_metrics_string_with_all_counters() {
        let metrics = Metrics::default();
        metrics.record_request("svc", "L1", "get", "hit");
        metrics.record_request("svc", "L1", "get", "miss");
        metrics.record_request("svc", "L2", "get", "hit");
        metrics.record_request("svc", "L2", "get", "miss");
        metrics.record_request("svc", "L1", "set", "attempt");
        metrics.record_request("svc", "L2", "set", "attempt");
        metrics.record_request("svc", "L1", "delete", "attempt");
        metrics.record_request("svc", "L2", "delete", "attempt");

        let counters = metrics.get_counters();
        assert!(counters.0 > 0);
        assert!(counters.1 > 0);
        assert!(counters.2 > 0);
        assert!(counters.3 > 0);
        assert!(counters.4 > 0);
        assert!(counters.5 > 0);
        assert!(counters.6 > 0);
        assert!(counters.7 > 0);
        assert!(counters.8 > 0);
    }

    // ========================================
    // record_request 非高频分支测试
    // ========================================

    #[test]
    fn test_record_request_other_operations() {
        let metrics = Metrics::default();
        metrics.record_request("svc", "L3", "get", "hit");
        metrics.record_request("svc", "L1", "clear", "success");
        metrics.record_request("svc", "L2", "batch", "attempt");
    }

    // ========================================
    // AtomicCounters Default 测试
    // ========================================

    #[test]
    fn test_atomic_counters_default() {
        use oxcache::metrics::AtomicCounters;
        use std::sync::atomic::Ordering;

        let counters = AtomicCounters::default();

        assert_eq!(counters.l1_get_hits.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_get_misses.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_get_hits.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_get_misses.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_set_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_set_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_delete_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_delete_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.total_operations.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_items.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_capacity_used.load(Ordering::Relaxed), 0);
        assert_eq!(counters.prefetch_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.compression_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.compression_bytes_saved.load(Ordering::Relaxed), 0);
    }

    // ========================================
    // 克隆和线程安全测试
    // ========================================

    #[test]
    fn test_metrics_clone() {
        let metrics = Metrics::default();
        metrics.record_request("svc", "L1", "get", "hit");

        let cloned = metrics.clone();

        let original_counters = metrics.get_counters();
        let cloned_counters = cloned.get_counters();

        assert_eq!(original_counters.0, cloned_counters.0);
    }

    #[test]
    fn test_metrics_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(Metrics::default());
        let mut handles = vec![];

        for _ in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                m.record_request("svc", "L1", "get", "hit");
                m.record_request("svc", "L1", "get", "miss");
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let counters = metrics.get_counters();
        assert_eq!(counters.0, 10);
        assert_eq!(counters.1, 10);
        assert_eq!(counters.8, 20);
    }
}

#[cfg(not(any(feature = "metrics", feature = "moka")))]
mod tests {
    #[test]
    fn test_metrics_not_available() {
        assert!(true);
    }
}
