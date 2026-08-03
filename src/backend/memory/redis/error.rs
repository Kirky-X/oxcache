// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis error mapping utilities.
//!
//! Provides fine-grained mapping from `RedisError` to `OxCacheError` variants,
//! replacing the coarse `conn_err` / `op_err` binary mapping.

use crate::error::{OxCacheError, OxCacheResult};
use redis::RedisError;

/// Map a `RedisError` to the most specific `OxCacheError` variant.
///
/// Priority: timeout → connection_dropped → connection_error → io_error → operation.
pub(crate) fn map_redis_error(e: RedisError) -> OxCacheError {
    if e.is_timeout() {
        OxCacheError::Timeout(e.to_string())
    } else if e.is_connection_dropped() || e.is_io_error() {
        OxCacheError::Connection(e.to_string())
    } else {
        OxCacheError::Operation(e.to_string())
    }
}

/// Check if a `RedisError` is a connection-level error.
///
/// Used in contexts where we need to distinguish connection errors from
/// operation errors (e.g. SCAN loop in `clear()`).
pub(crate) fn is_connection_error(e: &RedisError) -> bool {
    e.is_timeout() || e.is_io_error() || e.is_connection_dropped()
}

/// Map a Redis error in a connection context (alias for `map_redis_error`).
///
/// Kept for semantic clarity at call sites that are clearly connection-related.
#[allow(dead_code)]
pub(crate) fn conn_err(e: RedisError) -> OxCacheError {
    map_redis_error(e)
}

/// Map a Redis error in an operation context (alias for `map_redis_error`).
#[allow(dead_code)]
pub(crate) fn op_err(e: RedisError) -> OxCacheError {
    map_redis_error(e)
}

/// Convenience result type for Redis operations.
#[allow(dead_code)]
pub(crate) type RedisResult<T> = OxCacheResult<T>;
