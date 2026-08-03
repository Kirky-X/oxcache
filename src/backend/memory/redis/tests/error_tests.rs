// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unit tests for Redis error mapping.

use crate::backend::memory::redis::error::{is_connection_error, map_redis_error};
use crate::error::OxCacheError;
use redis::{ErrorKind, RedisError};

fn make_redis_error(kind: ErrorKind, msg: &str) -> RedisError {
    RedisError::from((kind, msg.to_string()))
}

// ============================================================================
// map_redis_error
// ============================================================================

#[test]
fn test_map_timeout_error() {
    let e = make_redis_error(ErrorKind::IoError, "Connection timed out");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Timeout(_)));
}

#[test]
fn test_map_connection_dropped_error() {
    let e = make_redis_error(ErrorKind::ConnectionDropped, "peer disconnected");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Connection(_)));
}

#[test]
fn test_map_io_error() {
    let e = make_redis_error(ErrorKind::IoError, "broken pipe");
    let mapped = map_redis_error(e);
    // IoError maps to Connection (via is_io_error check)
    assert!(matches!(mapped, OxCacheError::Connection(_)));
}

#[test]
fn test_map_operation_error() {
    let e = make_redis_error(ErrorKind::TypeError, "wrong type");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Operation(_)));
}

#[test]
fn test_map_exec_error() {
    let e = make_redis_error(ErrorKind::ExecAbortError, "script aborted");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Operation(_)));
}

#[test]
fn test_map_error_preserves_message() {
    let e = make_redis_error(ErrorKind::TypeError, "specific error text");
    let mapped = map_redis_error(e);
    if let OxCacheError::Operation(msg) = mapped {
        assert!(msg.contains("specific error text"));
    } else {
        panic!("Expected Operation error");
    }
}

// ============================================================================
// is_connection_error
// ============================================================================

#[test]
fn test_is_connection_error_timeout() {
    let e = make_redis_error(ErrorKind::IoError, "timeout");
    assert!(is_connection_error(&e));
}

#[test]
fn test_is_connection_error_dropped() {
    let e = make_redis_error(ErrorKind::ConnectionDropped, "dropped");
    assert!(is_connection_error(&e));
}

#[test]
fn test_is_connection_error_io() {
    let e = make_redis_error(ErrorKind::IoError, "broken pipe");
    assert!(is_connection_error(&e));
}

#[test]
fn test_is_not_connection_error_for_type_error() {
    let e = make_redis_error(ErrorKind::TypeError, "wrong type");
    assert!(!is_connection_error(&e));
}

#[test]
fn test_is_not_connection_error_for_abort() {
    let e = make_redis_error(ErrorKind::ExecAbortError, "aborted");
    assert!(!is_connection_error(&e));
}
