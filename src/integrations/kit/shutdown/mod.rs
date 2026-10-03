// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `register_cache_shutdown` — maps `CacheBackend` lifecycle onto
//! trait-kit's `AsyncShutdownCoordinator` three-phase shutdown.
//!
//! Phases:
//! 1. `StopRequests` — no-op (CacheBackend has no accept/reject model)
//! 2. `DrainQueue` — health-check probe to confirm connectivity
//! 3. `CloseConnections` — calls `backend.shutdown()` to release resources

use std::sync::Arc;

use trait_kit::prelude::*;

use crate::backend::CacheBackend;
use crate::error::OxCacheError;

/// Register cache backend shutdown hooks on an `AsyncShutdownCoordinator`.
///
/// Maps the three `ShutdownPhase` stages to cache operations:
/// - `StopRequests` → no-op
/// - `DrainQueue` → `health_check()` probe
/// - `CloseConnections` → `shutdown()`
///
/// # Errors
///
/// Returns `OxCacheError::Internal` if hook registration fails
/// (e.g. internal lock poisoned in `AsyncShutdownCoordinator`).
pub fn register_cache_shutdown(
    coord: &AsyncShutdownCoordinator,
    backend: Arc<dyn CacheBackend + Send + Sync>,
) -> Result<(), OxCacheError> {
    // Phase 1: StopRequests — no-op for cache backends.
    coord
        .register_hook(ShutdownPhase::StopRequests, || Box::pin(async {}))
        .map_err(|e| OxCacheError::Internal(format!("shutdown register StopRequests: {e}")))?;

    // Phase 2: DrainQueue — health-check probe.
    let drain_backend = Arc::clone(&backend);
    coord
        .register_hook(ShutdownPhase::DrainQueue, || {
            Box::pin(async move {
                let _ = drain_backend.health_check().await;
            })
        })
        .map_err(|e| OxCacheError::Internal(format!("shutdown register DrainQueue: {e}")))?;

    // Phase 3: CloseConnections — actual shutdown.
    let close_backend = Arc::clone(&backend);
    coord
        .register_hook(ShutdownPhase::CloseConnections, || {
            Box::pin(async move {
                close_backend.shutdown().await;
            })
        })
        .map_err(|e| OxCacheError::Internal(format!("shutdown register CloseConnections: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests;
