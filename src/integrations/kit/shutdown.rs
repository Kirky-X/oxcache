// Copyright (c) 2025-2026 Kirky.X
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
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// Minimal mock `CacheBackend` for shutdown testing.
    struct MockShutdownBackend {
        shutdown_called: Arc<AtomicBool>,
        health_check_count: Arc<AtomicUsize>,
    }

    impl MockShutdownBackend {
        fn new() -> (Self, Arc<AtomicBool>, Arc<AtomicUsize>) {
            let sd = Arc::new(AtomicBool::new(false));
            let hc = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    shutdown_called: Arc::clone(&sd),
                    health_check_count: Arc::clone(&hc),
                },
                sd,
                hc,
            )
        }
    }

    use crate::backend::{CacheConnector, CacheReader, CacheSetItem, CacheWriter};

    #[async_trait::async_trait]
    impl CacheReader for MockShutdownBackend {
        async fn get(&self, _key: &str) -> crate::error::OxCacheResult<Option<Vec<u8>>> {
            Ok(None)
        }
        async fn exists(&self, _key: &str) -> crate::error::OxCacheResult<bool> {
            Ok(false)
        }
        async fn ttl(&self, _key: &str) -> crate::error::OxCacheResult<Option<std::time::Duration>> {
            Ok(None)
        }
        async fn len(&self) -> crate::error::OxCacheResult<u64> {
            Ok(0)
        }
        async fn capacity(&self) -> crate::error::OxCacheResult<u64> {
            Ok(0)
        }
        async fn stats(&self) -> crate::error::OxCacheResult<HashMap<String, String>> {
            Ok(HashMap::new())
        }
        async fn get_many(&self, _keys: &[String]) -> crate::error::OxCacheResult<Vec<Option<Vec<u8>>>> {
            Ok(vec![])
        }
    }

    #[async_trait::async_trait]
    impl CacheWriter for MockShutdownBackend {
        async fn set(
            &self,
            _key: Arc<str>,
            _value: Arc<Vec<u8>>,
            _ttl: Option<std::time::Duration>,
        ) -> crate::error::OxCacheResult<()> {
            Ok(())
        }
        async fn delete(&self, _key: &str) -> crate::error::OxCacheResult<()> {
            Ok(())
        }
        async fn clear(&self) -> crate::error::OxCacheResult<()> {
            Ok(())
        }
        async fn expire(&self, _key: &str, _ttl: std::time::Duration) -> crate::error::OxCacheResult<bool> {
            Ok(false)
        }
        async fn set_many(&self, _items: &[CacheSetItem]) -> crate::error::OxCacheResult<()> {
            Ok(())
        }
        async fn delete_many(&self, _keys: &[String]) -> crate::error::OxCacheResult<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl CacheConnector for MockShutdownBackend {
        async fn health_check(&self) -> crate::error::OxCacheResult<()> {
            self.health_check_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn shutdown(&self) {
            self.shutdown_called.store(true, Ordering::SeqCst);
        }
        fn backend_kind(&self) -> crate::backend::BackendKind {
            crate::backend::BackendKind::Mock
        }
    }

    /// All three phases execute in order; `shutdown()` is called in
    /// `CloseConnections`.
    #[tokio::test]
    async fn register_cache_shutdown_executes_all_phases() {
        let (mock, shutdown_flag, hc_count) = MockShutdownBackend::new();
        let backend: Arc<dyn CacheBackend + Send + Sync> = Arc::new(mock);
        let coord = AsyncShutdownCoordinator::new();

        register_cache_shutdown(&coord, backend).expect("register succeeds");

        let result = coord.shutdown().await;
        assert!(result.is_ok(), "all phases complete without timeout");
        assert!(shutdown_flag.load(Ordering::SeqCst), "shutdown() was called");
        assert_eq!(
            hc_count.load(Ordering::SeqCst),
            1,
            "health_check() called once in DrainQueue"
        );
    }

    /// `register_cache_shutdown` returns `Ok(())` on success.
    #[tokio::test]
    async fn register_cache_shutdown_returns_ok() {
        let (mock, _, _) = MockShutdownBackend::new();
        let backend: Arc<dyn CacheBackend + Send + Sync> = Arc::new(mock);
        let coord = AsyncShutdownCoordinator::new();
        assert!(register_cache_shutdown(&coord, backend).is_ok());
    }
}
