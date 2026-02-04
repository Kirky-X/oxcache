// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 遥测模块测试 - 验证OpenTelemetry遥测功能

use oxcache::telemetry;

#[test]
fn test_init_tracing_basic() {
    // 测试基本的遥测初始化功能
    // 注意：在测试环境中调用可能导致与全局tracing subscriber的冲突
    // 所以我们只测试函数能够被正常调用

    let result = std::panic::catch_unwind(|| {
        telemetry::init_tracing("test-service", Some("http://localhost:4317"));
    });

    // 函数应该能够正常执行而不会panic
    assert!(result.is_ok());
}

#[test]
fn test_init_tracing_without_endpoint() {
    let result = std::panic::catch_unwind(|| {
        telemetry::init_tracing("test-service-no-endpoint", None);
    });

    assert!(result.is_ok());
}

#[test]
fn test_init_tracing_different_service_names() {
    let result = std::panic::catch_unwind(|| {
        telemetry::init_tracing("service-1", None);
        telemetry::init_tracing("service-2", None);
    });

    assert!(result.is_ok());
}
