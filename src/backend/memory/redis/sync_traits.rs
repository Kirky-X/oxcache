// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Synchronous trait implementations for RedisBackend (via block_in_place).

use super::client::RedisBackend;
use crate::backend::interface::{SyncAtomicCacheWriter, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
use crate::backend::{AtomicCacheWriter, BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
use std::collections::HashMap;
use std::time::Duration;

impl RedisBackend {
    /// Get the current Tokio runtime handle, requiring multi-thread flavor.
    ///
    /// # Invariants
    ///
    /// - Not in any Tokio runtime: returns `Err(NotSupported)`.
    /// - Current-thread runtime: returns `Err(NotSupported)` (block_in_place would panic).
    /// - Multi-thread runtime: returns the handle.
    ///
    /// # Deadlock Warning
    ///
    /// All sync trait methods (`SyncCacheReader`, `SyncCacheWriter`, etc.) use
    /// `block_in_place` + `handle.block_on()` to bridge async→sync. This will
    /// **deadlock** if called from a task already running on this backend's own
    /// multi-thread runtime when all worker threads are occupied. Callers MUST
    /// ensure they are NOT inside an async task on the same runtime when invoking
    /// sync methods. If in doubt, use the async API instead.
    pub(crate) fn multi_thread_handle() -> OxCacheResult<tokio::runtime::Handle> {
        let handle = tokio::runtime::Handle::try_current().map_err(|e| {
            OxCacheError::NotSupported(format!("sync API requires a Tokio runtime: {}", e))
        })?;
        if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
            return Err(OxCacheError::NotSupported(
                "sync API requires a multi-thread runtime; \
                 block_in_place is unavailable on current_thread runtime"
                    .to_string(),
            ));
        }
        Ok(handle)
    }
}

impl SyncCacheReader for RedisBackend {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::get(self, key)))
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::exists(self, key)))
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::ttl(self, key)))
    }

    fn len(&self) -> OxCacheResult<u64> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::len(self)))
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }

    fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::stats(self)))
    }
}

impl SyncCacheWriter for RedisBackend {
    fn set(
        &self,
        key: std::sync::Arc<str>,
        value: std::sync::Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::set(self, key, value, ttl)))
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::delete(self, key)))
    }

    fn clear(&self) -> OxCacheResult<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::clear(self)))
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::expire(self, key, ttl)))
    }
}

impl SyncCacheConnector for RedisBackend {
    fn health_check(&self) -> OxCacheResult<()> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheConnector::health_check(self)))
    }

    fn shutdown(&self) {
        // Consistent with async CacheConnector::shutdown (no-op).
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Redis
    }
}

impl SyncAtomicCacheWriter for RedisBackend {
    fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| handle.block_on(AtomicCacheWriter::incr(self, key, delta, ttl)))
    }

    fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(AtomicCacheWriter::compare_and_swap(self, key, expected, new, ttl))
        })
    }

    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let handle = Self::multi_thread_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(AtomicCacheWriter::set_if_absent(self, key, value, ttl))
        })
    }
}
