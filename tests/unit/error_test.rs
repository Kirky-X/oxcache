// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 错误类型单元测试

use oxcache::error::CacheError;

#[test]
fn test_cache_error_serialization() {
    let err = CacheError::Serialization("test error".to_string());
    assert!(err.to_string().contains("Serialization error"));
}

#[test]
fn test_cache_error_operation() {
    let err = CacheError::Operation("operation failed".to_string());
    assert!(err.to_string().contains("Operation failed"));
}

#[test]
fn test_cache_error_connection() {
    let err = CacheError::Connection("connection refused".to_string());
    assert!(err.to_string().contains("Connection error"));
}

#[test]
fn test_cache_error_not_found() {
    let err = CacheError::NotFound("key not found".to_string());
    assert!(err.to_string().contains("Key not found"));
    assert!(err.is_not_found());
}

#[test]
fn test_cache_error_degraded() {
    let err = CacheError::Degraded("L2 unavailable".to_string());
    assert!(err.to_string().contains("Cache degraded"));
    assert!(err.is_degraded());
}

#[test]
fn test_cache_error_l1() {
    let err = CacheError::L1Error("memory pressure".to_string());
    assert!(err.to_string().contains("L1 cache operation failed"));
}

#[test]
fn test_cache_error_l2() {
    let err = CacheError::L2Error("redis connection failed".to_string());
    assert!(err.to_string().contains("L2 cache operation failed"));
}

#[test]
fn test_cache_error_invalid_input_missing_field() {
    let err = CacheError::InvalidInput("missing required field".to_string());
    assert!(err.to_string().contains("Invalid input"));
}

#[test]
fn test_cache_error_not_supported() {
    let err = CacheError::NotSupported("feature not available".to_string());
    assert!(err.to_string().contains("Operation not supported"));
}

#[test]
fn test_cache_error_wal() {
    let err = CacheError::WalError("disk full".to_string());
    assert!(err.to_string().contains("WAL"));
}

#[test]
fn test_cache_error_database() {
    let err = CacheError::DatabaseError("query failed".to_string());
    assert!(err.to_string().contains("Database error"));
}

#[test]
fn test_cache_error_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = CacheError::IoError(io_err);
    assert!(err.to_string().contains("I/O error"));
}

#[test]
fn test_cache_error_backend() {
    let err = CacheError::BackendError("backend unavailable".to_string());
    assert!(err.to_string().contains("Backend error"));
}

#[test]
fn test_cache_error_timeout() {
    let err = CacheError::Timeout("operation timed out".to_string());
    assert!(err.to_string().contains("Operation timed out"));
}

#[test]
fn test_cache_error_shutdown() {
    let err = CacheError::ShutdownError("cleanup failed".to_string());
    assert!(err.to_string().contains("Shutdown error"));
}

#[test]
fn test_cache_error_key_too_long() {
    let err = CacheError::KeyTooLong(300, 256);
    assert!(err.to_string().contains("Key too long"));
    assert!(err.to_string().contains("300"));
    assert!(err.to_string().contains("256"));
}

#[test]
fn test_cache_error_value_too_large() {
    let err = CacheError::ValueTooLarge(1024 * 1024, 512 * 1024);
    assert!(err.to_string().contains("Value too large"));
}

#[test]
fn test_cache_error_buffer_full() {
    let err = CacheError::BufferFull("batch buffer".to_string());
    assert!(err.to_string().contains("Buffer full"));
}

#[test]
fn test_cache_error_invalid_input_bad_format() {
    let err = CacheError::InvalidInput("bad format".to_string());
    assert!(err.to_string().contains("Invalid input"));
}

#[test]
fn test_cache_error_invalid_key() {
    let err = CacheError::InvalidKey("contains forbidden chars".to_string());
    assert!(err.to_string().contains("Invalid key"));
}

#[test]
fn test_cache_error_lock() {
    let err = CacheError::LockError("mutex poisoned".to_string());
    assert!(err.to_string().contains("Lock error"));
}

#[test]
fn test_cache_error_service_not_found() {
    let err = CacheError::ServiceNotFound("cache-service".to_string());
    assert!(err.to_string().contains("Service not found"));
}

#[test]
fn test_is_not_found_method() {
    let err = CacheError::NotFound("key".to_string());
    assert!(err.is_not_found());

    let other_err = CacheError::Connection("failed".to_string());
    assert!(!other_err.is_not_found());
}

#[test]
fn test_is_connection_error_method() {
    let conn_err = CacheError::Connection("failed".to_string());
    assert!(conn_err.is_connection_error());

    let l2_err = CacheError::L2Error("redis failed".to_string());
    assert!(l2_err.is_connection_error());

    let other_err = CacheError::NotFound("key".to_string());
    assert!(!other_err.is_connection_error());
}

#[test]
fn test_is_degraded_method() {
    let err = CacheError::Degraded("L2 unavailable".to_string());
    assert!(err.is_degraded());

    let other_err = CacheError::Connection("failed".to_string());
    assert!(!other_err.is_degraded());
}

#[test]
fn test_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let cache_err: CacheError = io_err.into();
    assert!(matches!(cache_err, CacheError::IoError(_)));
}

#[test]
fn test_connection_error_display() {
    let err = CacheError::Connection("redis://localhost:6379".to_string());
    let display = err.to_string();
    assert!(display.contains("Connection error"));
    assert!(display.contains("redis://localhost:6379"));
}

// ========================================================================
// code() 方法测试 - 所有错误码
// ========================================================================

#[test]
fn test_error_code_not_found() {
    let err = CacheError::NotFound("key".to_string());
    assert_eq!(err.code(), "CACHE_001");
}

#[test]
fn test_error_code_connection() {
    let err = CacheError::Connection("failed".to_string());
    assert_eq!(err.code(), "CACHE_002");
}

#[test]
fn test_error_code_serialization() {
    let err = CacheError::Serialization("error".to_string());
    assert_eq!(err.code(), "CACHE_003");
}

#[test]
fn test_error_code_operation() {
    let err = CacheError::Operation("failed".to_string());
    assert_eq!(err.code(), "CACHE_004");
}

#[test]
fn test_error_code_degraded() {
    let err = CacheError::Degraded("L2 down".to_string());
    assert_eq!(err.code(), "CACHE_005");
}

#[test]
fn test_error_code_l1() {
    let err = CacheError::L1Error("memory".to_string());
    assert_eq!(err.code(), "CACHE_006");
}

#[test]
fn test_error_code_l2() {
    let err = CacheError::L2Error("redis".to_string());
    assert_eq!(err.code(), "CACHE_007");
}

#[test]
fn test_error_code_invalid_input_missing() {
    let err = CacheError::InvalidInput("missing".to_string());
    assert_eq!(err.code(), "CACHE_020");
}

#[test]
fn test_error_code_not_supported() {
    let err = CacheError::NotSupported("feature".to_string());
    assert_eq!(err.code(), "CACHE_009");
}

#[test]
fn test_error_code_wal() {
    let err = CacheError::WalError("disk".to_string());
    assert_eq!(err.code(), "CACHE_010");
}

#[test]
fn test_error_code_database() {
    let err = CacheError::DatabaseError("query".to_string());
    assert_eq!(err.code(), "CACHE_011");
}

#[test]
#[cfg(not(feature = "redis"))]
fn test_error_code_redis() {
    let err = CacheError::RedisError("connection".to_string());
    assert_eq!(err.code(), "CACHE_012");
}

#[test]
fn test_error_code_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file");
    let err = CacheError::IoError(io_err);
    assert_eq!(err.code(), "CACHE_013");
}

#[test]
fn test_error_code_backend() {
    let err = CacheError::BackendError("unavailable".to_string());
    assert_eq!(err.code(), "CACHE_014");
}

#[test]
fn test_error_code_timeout() {
    let err = CacheError::Timeout("30s".to_string());
    assert_eq!(err.code(), "CACHE_015");
}

#[test]
fn test_error_code_shutdown() {
    let err = CacheError::ShutdownError("cleanup".to_string());
    assert_eq!(err.code(), "CACHE_016");
}

#[test]
fn test_error_code_key_too_long() {
    let err = CacheError::KeyTooLong(300, 256);
    assert_eq!(err.code(), "CACHE_017");
}

#[test]
fn test_error_code_value_too_large() {
    let err = CacheError::ValueTooLarge(1024, 512);
    assert_eq!(err.code(), "CACHE_018");
}

#[test]
fn test_error_code_buffer_full() {
    let err = CacheError::BufferFull("batch".to_string());
    assert_eq!(err.code(), "CACHE_019");
}

#[test]
fn test_error_code_invalid_input_bad() {
    let err = CacheError::InvalidInput("bad".to_string());
    assert_eq!(err.code(), "CACHE_020");
}

#[test]
fn test_error_code_invalid_key() {
    let err = CacheError::InvalidKey("chars".to_string());
    assert_eq!(err.code(), "CACHE_021");
}

#[test]
fn test_error_code_lock() {
    let err = CacheError::LockError("poisoned".to_string());
    assert_eq!(err.code(), "CACHE_022");
}

#[test]
fn test_error_code_service_not_found() {
    let err = CacheError::ServiceNotFound("cache".to_string());
    assert_eq!(err.code(), "CACHE_023");
}

// ========================================================================
// is_recoverable() 方法测试
// ========================================================================

#[test]
fn test_is_recoverable_connection() {
    let err = CacheError::Connection("failed".to_string());
    assert!(err.is_recoverable());
}

#[test]
fn test_is_recoverable_timeout() {
    let err = CacheError::Timeout("timed out".to_string());
    assert!(err.is_recoverable());
}

#[test]
fn test_is_recoverable_l2() {
    let err = CacheError::L2Error("redis down".to_string());
    assert!(err.is_recoverable());
}

#[test]
fn test_is_recoverable_backend() {
    let err = CacheError::BackendError("transient".to_string());
    assert!(err.is_recoverable());
}

#[test]
fn test_is_recoverable_buffer_full() {
    let err = CacheError::BufferFull("at capacity".to_string());
    assert!(err.is_recoverable());
}

#[test]
fn test_is_not_recoverable_not_found() {
    let err = CacheError::NotFound("key".to_string());
    assert!(!err.is_recoverable());
}

#[test]
fn test_is_not_recoverable_serialization() {
    let err = CacheError::Serialization("invalid".to_string());
    assert!(!err.is_recoverable());
}

#[test]
fn test_is_not_recoverable_invalid_input_first() {
    let err = CacheError::InvalidInput("bad".to_string());
    assert!(!err.is_recoverable());
}

#[test]
fn test_is_not_recoverable_invalid_input_second() {
    let err = CacheError::InvalidInput("bad".to_string());
    assert!(!err.is_recoverable());
}

// ========================================================================
// Debug trait 测试
// ========================================================================

#[test]
fn test_error_debug_output() {
    let err = CacheError::NotFound("test_key".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("NotFound"));
    assert!(debug.contains("test_key"));
}

#[test]
fn test_error_debug_tuple_variants() {
    let err = CacheError::KeyTooLong(300, 256);
    let debug = format!("{:?}", err);
    assert!(debug.contains("KeyTooLong"));
    assert!(debug.contains("300"));
    assert!(debug.contains("256"));
}

// ========================================================================
// 错误消息完整性测试
// ========================================================================

#[test]
fn test_all_error_messages_are_helpful() {
    // 验证所有错误消息都包含有用的指导信息
    let err = CacheError::Serialization("data".to_string());
    assert!(err.to_string().contains("check the data format"));

    let err = CacheError::Connection("net".to_string());
    assert!(err.to_string().contains("check network"));

    let err = CacheError::L1Error("mem".to_string());
    assert!(err.to_string().contains("memory pressure"));

    let err = CacheError::L2Error("redis".to_string());
    assert!(err.to_string().contains("Redis connection"));

    let err = CacheError::WalError("disk".to_string());
    assert!(err.to_string().contains("disk space"));

    let err = CacheError::Timeout("op".to_string());
    assert!(err.to_string().contains("increasing the timeout"));
}
