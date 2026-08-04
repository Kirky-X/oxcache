// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unit tests for Redis error mapping.

use crate::backend::memory::redis::error::{is_connection_error, map_redis_error};
use crate::error::OxCacheError;
use redis::{ErrorKind, RedisError, ServerErrorKind};
use std::io;

/// Helper: build a `RedisError` from `io::Error` (Internal repr).
fn make_io_redis_error(kind: io::ErrorKind, msg: &str) -> RedisError {
    RedisError::from(io::Error::new(kind, msg))
}

/// Helper: build a `RedisError` from `ErrorKind` + static desc (General repr).
fn make_general_error(kind: ErrorKind, msg: &'static str) -> RedisError {
    RedisError::from((kind, msg))
}

// ============================================================================
// map_redis_error
// ============================================================================

#[test]
fn test_map_timeout_error() {
    let e = make_io_redis_error(io::ErrorKind::TimedOut, "Connection timed out");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Timeout(_)));
}

#[test]
fn test_map_timeout_preserves_message() {
    let e = make_io_redis_error(io::ErrorKind::TimedOut, "timed out after 30s");
    let mapped = map_redis_error(e);
    if let OxCacheError::Timeout(msg) = mapped {
        assert!(msg.contains("timed out"));
    } else {
        panic!("Expected Timeout error");
    }
}

#[test]
fn test_map_connection_dropped_error() {
    // General(ErrorKind::Io) → is_io_error() == true → mapped to Connection
    let e = make_general_error(ErrorKind::Io, "peer disconnected");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Connection(_)));
}

#[test]
fn test_map_io_error_preserves_message() {
    // io::Error → Internal(ErrorKind::Io) → is_io_error() == true
    let e = make_io_redis_error(io::ErrorKind::BrokenPipe, "broken pipe detail");
    let mapped = map_redis_error(e);
    if let OxCacheError::Connection(msg) = mapped {
        assert!(msg.contains("broken pipe"));
    } else {
        panic!("Expected Connection error");
    }
}

#[test]
fn test_map_unexpected_return_type() {
    let e = make_general_error(ErrorKind::UnexpectedReturnType, "wrong type");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Operation(_)));
}

#[test]
fn test_map_server_exec_abort() {
    let e = make_general_error(ErrorKind::Server(ServerErrorKind::ExecAbort), "script aborted");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Operation(_)));
}

#[test]
fn test_map_error_preserves_message() {
    let e = make_general_error(ErrorKind::UnexpectedReturnType, "specific error text");
    let mapped = map_redis_error(e);
    if let OxCacheError::Operation(msg) = mapped {
        assert!(msg.contains("specific error text"));
    } else {
        panic!("Expected Operation error");
    }
}

#[test]
fn test_map_client_error() {
    let e = make_general_error(ErrorKind::Client, "client config wrong");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Operation(_)));
}

#[test]
fn test_map_parse_error() {
    let e = make_general_error(ErrorKind::Parse, "parse failure");
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Operation(_)));
}

// ============================================================================
// is_connection_error
// ============================================================================

#[test]
fn test_is_connection_error_timeout() {
    let e = make_io_redis_error(io::ErrorKind::TimedOut, "timeout");
    assert!(is_connection_error(&e));
}

#[test]
fn test_is_connection_error_dropped() {
    // General(ErrorKind::Io) → is_io_error() == true → is_connection_error()
    let e = make_general_error(ErrorKind::Io, "dropped");
    assert!(is_connection_error(&e));
}

#[test]
fn test_is_connection_error_io() {
    // io::Error → is_io_error() == true
    let e = make_io_redis_error(io::ErrorKind::BrokenPipe, "broken pipe");
    assert!(is_connection_error(&e));
}

#[test]
fn test_is_not_connection_error_for_type_mismatch() {
    let e = make_general_error(ErrorKind::UnexpectedReturnType, "wrong type");
    assert!(!is_connection_error(&e));
}

#[test]
fn test_is_not_connection_error_for_abort() {
    let e = make_general_error(ErrorKind::Server(ServerErrorKind::ExecAbort), "aborted");
    assert!(!is_connection_error(&e));
}

#[test]
fn test_is_not_connection_error_for_auth_failure() {
    let e = make_general_error(ErrorKind::AuthenticationFailed, "bad password");
    assert!(!is_connection_error(&e));
}

// ============================================================================
// Priority: timeout > connection_dropped > io_error > operation
// ============================================================================

#[test]
fn test_timeout_takes_priority_over_io_error() {
    // An io::Error with TimedOut is both timeout and io_error.
    // map_redis_error should pick Timeout first.
    let e = make_io_redis_error(io::ErrorKind::TimedOut, "timeout");
    assert!(e.is_timeout());
    assert!(e.is_io_error());
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Timeout(_)));
}

#[test]
fn test_would_block_also_timeout() {
    // WouldBlock is also treated as timeout by redis crate
    let e = make_io_redis_error(io::ErrorKind::WouldBlock, "would block");
    assert!(e.is_timeout());
    let mapped = map_redis_error(e);
    assert!(matches!(mapped, OxCacheError::Timeout(_)));
}
