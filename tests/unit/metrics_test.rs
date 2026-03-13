// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 指标模块测试

#[cfg(any(feature = "metrics", feature = "moka"))]
mod tests {
    use oxcache::metrics::{Metrics, GLOBAL_METRICS, get_metrics_string};

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
}

#[cfg(not(any(feature = "metrics", feature = "moka")))]
mod tests {
    #[test]
    fn test_metrics_not_available() {
        assert!(true);
    }
}
