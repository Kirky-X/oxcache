//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 指标收集集成测试

use oxcache::metrics::{get_metrics_string, GLOBAL_METRICS};

#[test]
fn test_metrics_recording() {
    // Record some dummy metrics
    GLOBAL_METRICS.record_request("test_service", "L1", "get", "hit");
    GLOBAL_METRICS.record_duration("test_service", "L1", "get", 0.005);
    GLOBAL_METRICS.set_batch_buffer_size("test_service", 42);

    let output = get_metrics_string();

    println!("Metrics output:\n{}", output);

    assert!(output.contains("cache_requests_total{labels=\"test_service:L1:get:hit\"}"));
    assert!(output.contains("cache_operation_duration_seconds_sum{service=\"test_service\", layer=\"L1\", operation=\"get\"} 0.005"));
    assert!(output.contains("cache_operation_duration_seconds_count{service=\"test_service\", layer=\"L1\", operation=\"get\"} 1"));
    assert!(output.contains("cache_batch_write_buffer_size{service=\"test_service\"} 42"));
}
