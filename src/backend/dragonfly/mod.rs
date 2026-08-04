// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Dragonfly backend implementation.
//!
//! Dragonfly is a Redis-compatible in-memory data store with multi-threaded architecture.
//! This backend wraps `RedisBackend` and adds a restriction layer for commands that
//! Dragonfly does not fully support.
//!
//! # Feature Gate
//!
//! This module is gated behind the `dragonfly` feature, which implies `redis`.
//!
//! # Command Restrictions
//!
//! `DragonflyRestrictions.disabled_commands` is a **defensive documentation constraint**,
//! not a runtime interceptor on `CacheWriter` methods. The default disabled set
//! (FLUSHALL, FLUSHDB, DEBUG, MONITOR) contains commands that are NOT part of the
//! `CacheWriter` trait interface, so they can never be invoked through normal
//! `CacheWriter` operations. The restriction set serves as:
//! 1. Documentation of unsupported commands
//! 2. Future interception at `CacheConnector::execute_raw_command()` (if implemented)
//! 3. User-customizable extension via `with_disabled_commands()`

use crate::backend::interface::{
    AtomicCacheWriter, BackendKind, CacheConnector, CacheReader, CacheWriter,
};
use crate::backend::memory::redis::RedisBackend;
use crate::backend::score::{BackendScore, Scores};
use crate::error::OxCacheResult;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

/// Dragonfly backend wrapping RedisBackend with restriction awareness.
///
/// Internally delegates all operations to the wrapped `RedisBackend` instance.
/// The `DragonflyRestrictions` struct provides metadata about unsupported commands
/// but does NOT intercept `CacheWriter` method calls (see module docs).
pub struct DragonflyBackend {
    inner: RedisBackend,
    restrictions: DragonflyRestrictions,
}

/// Defensive constraint set for Dragonfly-specific limitations.
///
/// This struct documents commands that Dragonfly does not support or supports
/// incompletely. It does NOT perform runtime interception on `CacheWriter` methods.
#[derive(Debug, Clone)]
pub struct DragonflyRestrictions {
    /// Commands that Dragonfly does not support (documentation constraint).
    disabled_commands: HashSet<String>,
    /// Whether Redis Cluster mode checks should be disabled.
    /// Defaults to `true` since Dragonfly does not support Redis Cluster protocol.
    cluster_disabled: bool,
}

impl Default for DragonflyRestrictions {
    fn default() -> Self {
        Self {
            disabled_commands: ["FLUSHALL", "FLUSHDB", "DEBUG", "MONITOR"]
                .into_iter()
                .map(String::from)
                .collect(),
            cluster_disabled: true,
        }
    }
}

impl DragonflyRestrictions {
    /// Create restrictions with custom disabled command set.
    pub fn with_disabled_commands(mut self, commands: Vec<String>) -> Self {
        self.disabled_commands = commands.into_iter().collect();
        self
    }

    /// Set whether cluster mode checks are disabled.
    pub fn with_cluster_disabled(mut self, disabled: bool) -> Self {
        self.cluster_disabled = disabled;
        self
    }

    /// Check if a command is in the disabled set.
    pub fn is_command_disabled(&self, command: &str) -> bool {
        self.disabled_commands.contains(command)
    }

    /// Whether cluster mode checks are disabled.
    pub fn cluster_disabled(&self) -> bool {
        self.cluster_disabled
    }
}

impl DragonflyBackend {
    /// Create a new Dragonfly backend.
    ///
    /// TLS strategy: reuses RedisBackend's TLS configuration (`rediss://` URL scheme).
    /// DragonflyBackend does not enforce TLS independently.
    pub async fn new(url: &str, pool_size: usize) -> OxCacheResult<Self> {
        let inner = RedisBackend::builder()
            .connection_string(url)
            .pool_size(pool_size)
            .build()
            .await?;
        Ok(Self {
            inner,
            restrictions: DragonflyRestrictions::default(),
        })
    }

    /// Create with custom restrictions.
    pub fn with_restrictions(mut self, restrictions: DragonflyRestrictions) -> Self {
        self.restrictions = restrictions;
        self
    }

    /// Access the underlying RedisBackend.
    pub fn inner(&self) -> &RedisBackend {
        &self.inner
    }

    /// Access the restrictions.
    pub fn restrictions(&self) -> &DragonflyRestrictions {
        &self.restrictions
    }
}

// ============================================================================
// Trait Implementations — all delegate to inner RedisBackend
// ============================================================================

#[async_trait]
impl CacheReader for DragonflyBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.inner.get(key).await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key).await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        self.inner.stats().await
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait]
impl CacheWriter for DragonflyBackend {
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        self.inner.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[(Arc<str>, Arc<Vec<u8>>, Option<Duration>)]) -> OxCacheResult<()> {
        self.inner.set_many(items).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

impl BackendScore for DragonflyBackend {
    fn score(&self) -> u8 {
        Scores::REDIS
    }

    fn is_persistent(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        "dragonfly"
    }
}

#[async_trait]
impl CacheConnector for DragonflyBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Dragonfly
    }

    /// Dragonfly atomic operation compatibility is not yet verified.
    /// Returns `None` to disable atomic operations by default.
    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    #[test]
    fn test_dragonfly_restrictions_default() {
        let r = DragonflyRestrictions::default();
        assert!(r.is_command_disabled("FLUSHALL"));
        assert!(r.is_command_disabled("FLUSHDB"));
        assert!(r.is_command_disabled("DEBUG"));
        assert!(r.is_command_disabled("MONITOR"));
        assert!(!r.is_command_disabled("GET"));
        assert!(!r.is_command_disabled("SET"));
        assert!(r.cluster_disabled());
    }

    #[test]
    fn test_dragonfly_restrictions_custom() {
        // Note: with_disabled_commands REPLACES the default set, not extends it
        let r = DragonflyRestrictions::default()
            .with_disabled_commands(vec!["CUSTOM_CMD".to_string()])
            .with_cluster_disabled(false);
        assert!(!r.is_command_disabled("FLUSHALL")); // replaced, so FLUSHALL no longer disabled
        assert!(r.is_command_disabled("CUSTOM_CMD"));
        assert!(!r.cluster_disabled());
    }

    #[test]
    fn test_dragonfly_restrictions_clone_debug() {
        let r = DragonflyRestrictions::default();
        let r2 = r.clone();
        assert_eq!(r2.disabled_commands.len(), r.disabled_commands.len());
        // Debug
        let debug = format!("{:?}", r);
        assert!(debug.contains("DragonflyRestrictions"));
    }

    // ========================================================================
    // DragonflyBackend integration tests (require Dragonfly server on port 6380)
    // ========================================================================

    /// Dragonfly test URL — Dragonfly on port 6380 (Docker mapped from 6379)
    const DRAGONFLY_URL: &str = "redis://127.0.0.1:6380";

    use serial_test::serial;

    /// Set the insecure Redis env var for Dragonfly tests.
    fn set_insecure_env() {
        // SAFETY: Rust 2024 edition — set_var is unsafe; tests are serialized via #[serial]
        unsafe { std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS"); }
    }

    /// Remove the insecure Redis env var after Dragonfly tests.
    fn cleanup_insecure_env() {
        // SAFETY: Rust 2024 edition — remove_var is unsafe; tests are serialized via #[serial]
        unsafe { std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS"); }
    }

    async fn make_dragonfly() -> DragonflyBackend {
        set_insecure_env();
        DragonflyBackend::new(DRAGONFLY_URL, 4)
            .await
            .expect("Failed to connect to Dragonfly")
    }

    fn unique_key(prefix: &str) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}_{}", prefix, ts)
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_backend_kind() {
        let backend = make_dragonfly().await;
        assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);
        assert!(backend.backend_kind().is_distributed());
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_backend_score() {
        let backend = make_dragonfly().await;
        assert_eq!(backend.score(), Scores::REDIS);
        assert!(backend.is_persistent());
        assert_eq!(backend.backend_name(), "dragonfly");
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_set_get_delete() {
        use crate::backend::{CacheReader, CacheWriter};
        let backend = make_dragonfly().await;
        let key = unique_key("df_sg");

        // set
        backend.set(
            Arc::from(key.as_str()),
            Arc::new(b"dragonfly_value".to_vec()),
            None,
        ).await.expect("set failed");

        // get
        let val = backend.get(&key).await.expect("get failed");
        assert_eq!(val, Some(b"dragonfly_value".to_vec()));

        // exists
        assert!(backend.exists(&key).await.expect("exists failed"));

        // delete
        backend.delete(&key).await.expect("delete failed");
        assert!(!backend.exists(&key).await.expect("exists after delete failed"));
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_set_with_ttl() {
        use crate::backend::{CacheReader, CacheWriter};
        let backend = make_dragonfly().await;
        let key = unique_key("df_ttl");

        backend.set(
            Arc::from(key.as_str()),
            Arc::new(b"ttl_val".to_vec()),
            Some(Duration::from_secs(100)),
        ).await.expect("set with ttl failed");

        let ttl = backend.ttl(&key).await.expect("ttl failed");
        assert!(ttl.is_some());
        let secs = ttl.unwrap().as_secs();
        assert!(secs > 90 && secs <= 100, "ttl secs = {}", secs);

        backend.delete(&key).await.ok();
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_expire() {
        use crate::backend::CacheWriter;
        let backend = make_dragonfly().await;
        let key = unique_key("df_exp");

        backend.set(
            Arc::from(key.as_str()),
            Arc::new(b"v".to_vec()),
            None,
        ).await.expect("set failed");

        let ok = backend.expire(&key, Duration::from_secs(50)).await.expect("expire failed");
        assert!(ok);

        backend.delete(&key).await.ok();
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_health_check_and_stats() {
        use crate::backend::CacheConnector;
        let backend = make_dragonfly().await;
        backend.health_check().await.expect("health check failed");
        backend.shutdown().await;
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_set_many_get_many() {
        use crate::backend::{CacheReader, CacheWriter};
        let backend = make_dragonfly().await;
        let k1 = unique_key("df_m1");
        let k2 = unique_key("df_m2");

        let items = vec![
            (Arc::from(k1.clone()), Arc::new(b"v1".to_vec()), None),
            (Arc::from(k2.clone()), Arc::new(b"v2".to_vec()), None),
        ];
        backend.set_many(&items).await.expect("set_many failed");

        let keys = vec![k1.clone(), k2.clone()];
        let values = backend.get_many(&keys).await.expect("get_many failed");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], Some(b"v1".to_vec()));
        assert_eq!(values[1], Some(b"v2".to_vec()));

        backend.delete_many(&keys).await.expect("delete_many failed");
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_atomic_writer_is_none() {
        let backend = make_dragonfly().await;
        // Dragonfly does NOT support AtomicCacheWriter
        let atomic = backend.as_atomic_writer();
        assert!(atomic.is_none());
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[ignore = "requires Dragonfly server"]
    #[serial]
    async fn test_dragonfly_capacity_zero() {
        use crate::backend::CacheReader;
        let backend = make_dragonfly().await;
        let cap = backend.capacity().await.expect("capacity failed");
        assert_eq!(cap, 0);
        cleanup_insecure_env();
    }
}
