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
fn test_cache_error_config() {
    let err = CacheError::ConfigError("missing required field".to_string());
    assert!(err.to_string().contains("Configuration error"));
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
fn test_cache_error_invalid_input() {
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
