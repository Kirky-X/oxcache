// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Error display tests extracted from error.rs

use oxcache::error::{CacheConfigError, CacheError};

// ========================================================================
// CacheConfigError tests
// ========================================================================

#[test]
fn test_config_error_missing_field() {
    let err = CacheConfigError::MissingField("capacity".to_string());
    let msg = err.to_string();
    assert!(msg.contains("Missing required field"));
    assert!(msg.contains("capacity"));
}

#[test]
fn test_config_error_invalid_value() {
    let err = CacheConfigError::InvalidValue {
        field: "capacity".to_string(),
        reason: "must be greater than 0".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Invalid value for field 'capacity'"));
    assert!(msg.contains("must be greater than 0"));
}

#[test]
fn test_config_error_unsupported_backend() {
    let err = CacheConfigError::UnsupportedBackend("unknown".to_string());
    let msg = err.to_string();
    assert!(msg.contains("Unsupported backend combination"));
    assert!(msg.contains("unknown"));
}

#[test]
fn test_config_error_connection_failed() {
    let err = CacheConfigError::ConnectionFailed("redis".to_string());
    let msg = err.to_string();
    assert!(msg.contains("Connection failed during initialization"));
    assert!(msg.contains("redis"));
}

// ========================================================================
// CacheError tests
// ========================================================================

#[test]
fn test_error_display_invalid_input() {
    let err = CacheError::InvalidInput("test message".to_string());
    assert!(err.to_string().contains("test message"));
}

#[test]
fn test_error_display_database_error() {
    let err = CacheError::DatabaseError("connection failed".to_string());
    assert!(err.to_string().contains("Database error"));
    assert!(err.to_string().contains("connection failed"));
}

#[test]
fn test_error_display_timeout() {
    let err = CacheError::Timeout("operation".to_string());
    assert!(err.to_string().contains("Timeout") || err.to_string().contains("timeout"));
}

#[test]
fn test_error_display_not_found() {
    let err = CacheError::NotFound("key".to_string());
    assert!(err.to_string().contains("Key not found"));
    assert!(err.to_string().contains("key"));
}

#[test]
fn test_error_display_degraded() {
    let err = CacheError::Degraded("redis unavailable".to_string());
    assert!(err.to_string().contains("Cache degraded"));
    assert!(err.to_string().contains("redis unavailable"));
}

#[test]
fn test_error_is_recoverable() {
    let err = CacheError::Connection("timeout".to_string());
    assert!(err.is_recoverable());
}

#[test]
fn test_error_is_not_found() {
    let err = CacheError::NotFound("key".to_string());
    assert!(err.is_not_found());
}

#[test]
fn test_error_is_degraded() {
    let err = CacheError::Degraded("backend down".to_string());
    assert!(err.is_degraded());
}
