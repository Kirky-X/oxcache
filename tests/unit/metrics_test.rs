// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// UnifiedMetrics tests

#[cfg(any(feature = "metrics", feature = "memory"))]
mod tests {
    use oxcache::infra::metrics::{
        export_json_format, export_prometheus_format, get_enhanced_stats, AtomicCounters, CacheLayer, CacheOpResult,
        CacheOpType, CacheOperation, CacheStats, UnifiedMetrics, GLOBAL_UNIFIED_METRICS,
    };
    use std::sync::atomic::Ordering;

    // ========================================
    // Basic operation tests
    // ========================================

    #[test]
    fn test_record_l1_get_hit() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l1_hits > 0);
    }

    #[test]
    fn test_record_l1_get_miss() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l1_misses > 0);
    }

    #[test]
    fn test_record_l2_get_hit() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l2_hits > 0);
    }

    #[test]
    fn test_record_l2_get_miss() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l2_misses > 0);
    }

    #[test]
    fn test_record_l1_set() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Set,
            result: CacheOpResult::Success,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l1_sets > 0);
    }

    #[test]
    fn test_record_l2_set() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Set,
            result: CacheOpResult::Success,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l2_sets > 0);
    }

    #[test]
    fn test_record_l1_delete() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Delete,
            result: CacheOpResult::Success,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l1_deletes > 0);
    }

    #[test]
    fn test_record_l2_delete() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Delete,
            result: CacheOpResult::Success,
        });
        let counters = GLOBAL_UNIFIED_METRICS.get_counters();
        assert!(counters.l2_deletes > 0);
    }

    #[test]
    fn test_total_operations() {
        let initial = GLOBAL_UNIFIED_METRICS.get_counters().total_operations;
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        let after = GLOBAL_UNIFIED_METRICS.get_counters().total_operations;
        assert!(after > initial);
    }

    // ========================================
    // UnifiedMetrics instance tests
    // ========================================

    #[test]
    fn test_metrics_default() {
        let metrics = UnifiedMetrics::new();
        let counters = metrics.get_counters();
        assert_eq!(counters.l1_hits, 0);
        assert_eq!(counters.l1_misses, 0);
        assert_eq!(counters.l2_hits, 0);
        assert_eq!(counters.l2_misses, 0);
    }

    #[test]
    fn test_snapshot_basic() {
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });

        let snapshot = metrics.snapshot();
        assert!(snapshot.counters.l1_hits > 0);
        assert!(snapshot.counters.l2_misses > 0);
    }

    #[test]
    fn test_reset_clears_all_counters() {
        let metrics = UnifiedMetrics::new();

        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });

        let before_reset = metrics.snapshot();
        assert!(before_reset.counters.total_operations > 0);

        metrics.reset();

        let after_reset = metrics.snapshot();
        assert_eq!(after_reset.counters.l1_hits, 0);
        assert_eq!(after_reset.counters.l1_misses, 0);
        assert_eq!(after_reset.counters.total_operations, 0);
    }

    // ========================================
    // Hit rate tests
    // ========================================

    #[test]
    fn test_hit_rate_empty() {
        let metrics = UnifiedMetrics::new();
        metrics.reset();
        let rates = metrics.hit_rates();
        assert_eq!(rates.l1_hit_rate, 0.0);
    }

    #[test]
    fn test_hit_rate_all_hits() {
        let metrics = UnifiedMetrics::new();
        metrics.reset();

        for _ in 0..3 {
            metrics.record_operation(CacheOperation {
                layer: CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Hit,
            });
        }

        let rates = metrics.hit_rates();
        assert_eq!(rates.l1_hit_rate, 1.0);
    }

    #[test]
    fn test_hit_rate_all_misses() {
        let metrics = UnifiedMetrics::new();
        metrics.reset();

        for _ in 0..2 {
            metrics.record_operation(CacheOperation {
                layer: CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Miss,
            });
        }

        let rates = metrics.hit_rates();
        assert_eq!(rates.l1_hit_rate, 0.0);
    }

    #[test]
    fn test_hit_rate_mixed() {
        let metrics = UnifiedMetrics::new();
        metrics.reset();

        for _ in 0..7 {
            metrics.record_operation(CacheOperation {
                layer: CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Hit,
            });
        }
        for _ in 0..3 {
            metrics.record_operation(CacheOperation {
                layer: CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Miss,
            });
        }

        let rates = metrics.hit_rates();
        assert!((rates.l1_hit_rate - 0.7).abs() < 0.001);
    }

    // ========================================
    // CacheStats tests
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
    // Prometheus export tests
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
        assert!(prometheus.contains("cache_l1_sets_total 30"));
        assert!(prometheus.contains("cache_operations_total 133"));
        assert!(prometheus.contains("cache_l1_hit_rate"));
        assert!(prometheus.contains("# Generated at:"));
    }

    #[test]
    fn test_metrics_export_prometheus() {
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L2,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Miss,
        });

        let prometheus = metrics.export_prometheus();
        assert!(prometheus.contains("cache_l1_hits_total"));
        assert!(prometheus.contains("cache_l2_misses_total"));
    }

    #[test]
    fn test_export_prometheus_format_global() {
        GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });
        let output = export_prometheus_format();
        assert!(output.contains("cache_"));
    }

    // ========================================
    // JSON export tests
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
        assert!(json.contains("\"total_operations\": 88"));
        assert!(json.contains("\"timestamp\":"));
    }

    #[test]
    fn test_metrics_export_json() {
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });

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
    // get_enhanced_stats test
    // ========================================

    #[test]
    fn test_get_enhanced_stats() {
        let stats = get_enhanced_stats();
        assert!(stats.timestamp <= chrono::Utc::now());
    }

    // ========================================
    // AtomicCounters Default test
    // ========================================

    #[test]
    fn test_atomic_counters_default() {
        let counters = AtomicCounters::default();

        assert_eq!(counters.l1_hits.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_misses.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_hits.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_misses.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_sets.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_sets.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l1_deletes.load(Ordering::Relaxed), 0);
        assert_eq!(counters.l2_deletes.load(Ordering::Relaxed), 0);
        assert_eq!(counters.total_operations.load(Ordering::Relaxed), 0);
        assert_eq!(counters.prefetch_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.compression_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.compression_bytes_saved.load(Ordering::Relaxed), 0);
    }

    // ========================================
    // Clone and thread safety tests
    // ========================================

    #[test]
    fn test_metrics_clone() {
        let metrics = UnifiedMetrics::new();
        metrics.record_operation(CacheOperation {
            layer: CacheLayer::L1,
            op_type: CacheOpType::Get,
            result: CacheOpResult::Hit,
        });

        let cloned = metrics.clone();

        let original_counters = metrics.get_counters();
        let cloned_counters = cloned.get_counters();

        assert_eq!(original_counters.l1_hits, cloned_counters.l1_hits);
    }

    #[test]
    fn test_metrics_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(UnifiedMetrics::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                m.record_operation(CacheOperation {
                    layer: CacheLayer::L1,
                    op_type: CacheOpType::Get,
                    result: CacheOpResult::Hit,
                });
                m.record_operation(CacheOperation {
                    layer: CacheLayer::L1,
                    op_type: CacheOpType::Get,
                    result: CacheOpResult::Miss,
                });
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let counters = metrics.get_counters();
        assert_eq!(counters.l1_hits, 10);
        assert_eq!(counters.l1_misses, 10);
        assert_eq!(counters.total_operations, 20);
    }
}
