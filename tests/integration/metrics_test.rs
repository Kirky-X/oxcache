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

    // Check global counter
    assert!(output.contains("cache_l1_get_hits_total"));
    // Check per-service duration metric which is recorded via DashMap
    assert!(output.contains("cache_operation_duration_seconds{service=\"test_service\",layer=\"L1\",op=\"get\"}"));
    assert!(output.contains("cache_batch_buffer_size{service=\"test_service\"} 42"));
}
