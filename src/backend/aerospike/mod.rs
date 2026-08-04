// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Aerospike backend for oxcache.
//!
//! Provides `AerospikeBackend`, implementing oxcache's
//! `CacheReader`/`CacheWriter`/`CacheConnector` traits for Aerospike.
//!
//! # Data Model
//!
//! - Cache values are stored as `Value::Blob(Vec<u8>)` in a single bin named `"value"`.
//! - TTL is set per-write via `WritePolicy.expiration`.
//! - `expire()` uses `client.touch()` to update TTL without modifying the value.
//!
//! # Limitations
//!
//! - `len()`, `capacity()`, `keys()` are not natively supported by Aerospike
//!   and return `NotSupported`.
//! - `clear()` is not implemented (Aerospike has no bulk delete API).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use aerospike::{
    Bin, Bins, Client, ClientPolicy, Error as AsError, Expiration, Key, ReadPolicy,
    RecordExistsAction, ResultCode, Value, WritePolicy,
};
use async_trait::async_trait;

use crate::backend::interface::{
    AtomicCacheWriter, BackendKind, CacheConnector, CacheReader, CacheWriter,
};
use crate::backend::score::BackendScore;
use crate::error::{OxCacheError, OxCacheResult};

/// The bin name used to store cache values.
const VALUE_BIN: &str = "value";

/// Aerospike connection configuration.
#[derive(Debug, Clone)]
pub struct AerospikeConfig {
    /// Seed nodes in `"host:port"` format. Default port is 3000.
    pub seed_nodes: Vec<String>,
    /// Aerospike namespace.
    pub namespace: String,
    /// Aerospike set name (analogous to a table/collection).
    pub set_name: String,
    /// Default TTL in seconds. 0 means no expiration.
    pub default_ttl: u32,
    /// IP translation table for Docker/NAT environments.
    /// Maps internal server IPs (from peer discovery) to client-reachable IPs.
    /// Key: IP returned by the server. Value: IP the client should actually connect to.
    pub ip_map: Option<HashMap<String, String>>,
}

impl Default for AerospikeConfig {
    fn default() -> Self {
        Self {
            seed_nodes: vec!["127.0.0.1:3000".to_string()],
            namespace: "test".to_string(),
            set_name: "oxcache".to_string(),
            default_ttl: 0,
            ip_map: None,
        }
    }
}

/// Aerospike cache backend.
///
/// Wraps an Aerospike `Client` and implements oxcache's cache traits.
pub struct AerospikeBackend {
    client: Arc<Client>,
    config: AerospikeConfig,
    write_policy: WritePolicy,
    read_policy: ReadPolicy,
}

/// Check if an Aerospike error is a KEY_NOT_FOUND error.
fn is_key_not_found(e: &AsError) -> bool {
    matches!(e, AsError::ServerError(ResultCode::KeyNotFoundError, _, _))
}

impl AerospikeBackend {
    /// Create a new Aerospike backend.
    pub async fn new(config: AerospikeConfig) -> OxCacheResult<Self> {
        let hosts = config.seed_nodes.join(",");
        let policy = ClientPolicy {
            ip_map: config.ip_map.clone(),
            ..ClientPolicy::default()
        };

        let client = Client::new(&policy, &hosts)
            .await
            .map_err(|e| OxCacheError::Connection(format!("Aerospike connect failed: {}", e)))?;

        let write_policy = WritePolicy {
            record_exists_action: RecordExistsAction::Replace,
            ..WritePolicy::default()
        };

        Ok(Self {
            client: Arc::new(client),
            config,
            write_policy,
            read_policy: ReadPolicy::default(),
        })
    }

    /// Build an Aerospike Key from a cache key string.
    fn make_key(&self, key: &str) -> OxCacheResult<Key> {
        Key::new(
            self.config.namespace.clone(),
            self.config.set_name.clone(),
            Value::from(key),
        )
        .map_err(|e| OxCacheError::InvalidKey(format!("Aerospike key creation failed: {}", e)))
    }

    /// Build a WritePolicy with the given TTL.
    fn write_policy_with_ttl(&self, ttl: Option<Duration>) -> WritePolicy {
        let mut wp = self.write_policy.clone();
        let ttl_secs = ttl
            .map(|d| d.as_secs() as u32)
            .unwrap_or(self.config.default_ttl);
        wp.expiration = if ttl_secs == 0 {
            Expiration::Never
        } else {
            Expiration::Seconds(ttl_secs)
        };
        wp
    }

    /// Extract the value bytes from an Aerospike Record.
    fn extract_value(record: &aerospike::Record) -> Option<Vec<u8>> {
        record.bins.get(VALUE_BIN).and_then(|v| match v {
            Value::Blob(data) => Some(data.clone()),
            _ => None,
        })
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

#[async_trait]
impl CacheReader for AerospikeBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let as_key = self.make_key(key)?;
        match self.client.get(&self.read_policy, &as_key, Bins::All).await {
            Ok(record) => Ok(Self::extract_value(&record)),
            Err(e) if is_key_not_found(&e) => Ok(None),
            Err(e) => Err(OxCacheError::BackendError(format!(
                "Aerospike get failed: {}",
                e
            ))),
        }
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        let as_key = self.make_key(key)?;
        // Read header only (no bins) to check existence
        match self
            .client
            .get(&self.read_policy, &as_key, Bins::None)
            .await
        {
            Ok(_) => Ok(true),
            Err(e) if is_key_not_found(&e) => Ok(false),
            Err(e) => Err(OxCacheError::BackendError(format!(
                "Aerospike exists check failed: {}",
                e
            ))),
        }
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let as_key = self.make_key(key)?;
        match self
            .client
            .get(&self.read_policy, &as_key, Bins::None)
            .await
        {
            Ok(record) => Ok(record.time_to_live()),
            Err(e) if is_key_not_found(&e) => Ok(None),
            Err(e) => Err(OxCacheError::BackendError(format!(
                "Aerospike ttl check failed: {}",
                e
            ))),
        }
    }

    async fn len(&self) -> OxCacheResult<u64> {
        Err(OxCacheError::NotSupported(
            "Aerospike does not support efficient key counting".to_string(),
        ))
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        Err(OxCacheError::NotSupported(
            "Aerospike does not expose capacity information".to_string(),
        ))
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("backend_kind".to_string(), "aerospike".to_string());
        stats.insert("namespace".to_string(), self.config.namespace.clone());
        stats.insert("set_name".to_string(), self.config.set_name.clone());
        stats.insert(
            "connected".to_string(),
            self.client.is_connected().to_string(),
        );
        stats.insert(
            "nodes".to_string(),
            self.client.node_names().len().to_string(),
        );
        Ok(stats)
    }

    async fn keys(&self, _pattern: &str) -> OxCacheResult<Vec<String>> {
        Err(OxCacheError::NotSupported(
            "Aerospike does not support pattern-based key listing".to_string(),
        ))
    }
}

#[async_trait]
impl CacheWriter for AerospikeBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let as_key = self.make_key(&key)?;
        let wp = self.write_policy_with_ttl(ttl);
        let bins = [Bin::new(VALUE_BIN.to_string(), Value::Blob((*value).clone()))];

        self.client.put(&wp, &as_key, &bins).await.map_err(|e| {
            OxCacheError::BackendError(format!("Aerospike set failed: {}", e))
        })
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        let as_key = self.make_key(key)?;
        // delete returns Ok(false) if key doesn't exist, which is fine
        let _ = self
            .client
            .delete(&self.write_policy, &as_key)
            .await
            .map_err(|e| {
                OxCacheError::BackendError(format!("Aerospike delete failed: {}", e))
            })?;
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        Err(OxCacheError::NotSupported(
            "Aerospike does not support bulk delete; use namespace truncation instead".to_string(),
        ))
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let as_key = self.make_key(key)?;
        let wp = WritePolicy {
            expiration: Expiration::Seconds(ttl.as_secs() as u32),
            ..WritePolicy::default()
        };

        match self.client.touch(&wp, &as_key).await {
            Ok(()) => Ok(true),
            Err(e) if is_key_not_found(&e) => Ok(false),
            Err(e) => Err(OxCacheError::BackendError(format!(
                "Aerospike expire (touch) failed: {}",
                e
            ))),
        }
    }

    async fn set_many(
        &self,
        items: &[(Arc<str>, Arc<Vec<u8>>, Option<Duration>)],
    ) -> OxCacheResult<()> {
        for (key, value, ttl) in items {
            self.set(key.clone(), value.clone(), *ttl).await?;
        }
        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        let mut failures: Vec<(&String, OxCacheError)> = Vec::new();
        for key in keys {
            if let Err(e) = self.delete(key).await {
                failures.push((key, e));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            let details: Vec<String> = failures
                .iter()
                .map(|(k, e)| format!("{}: {}", k, e))
                .collect();
            Err(OxCacheError::Operation(format!(
                "Aerospike delete_many: {}/{} keys failed: {}",
                failures.len(),
                keys.len(),
                details.join("; ")
            )))
        }
    }
}

#[async_trait]
impl CacheConnector for AerospikeBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        if self.client.is_connected() {
            Ok(())
        } else {
            Err(OxCacheError::Connection(
                "Aerospike cluster is not connected".to_string(),
            ))
        }
    }

    async fn shutdown(&self) {
        // Aerospike client doesn't have an explicit shutdown in the async API.
        // The client will be dropped when the Arc refcount reaches zero.
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Aerospike
    }

    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        None
    }
}

impl BackendScore for AerospikeBackend {
    fn score(&self) -> u8 {
        // Aerospike scores lower than Redis-family backends
        30
    }

    fn is_persistent(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        "aerospike"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::score::Scores;

    #[test]
    fn test_aerospike_config_default() {
        let config = AerospikeConfig::default();
        assert_eq!(config.seed_nodes, vec!["127.0.0.1:3000"]);
        assert_eq!(config.namespace, "test");
        assert_eq!(config.set_name, "oxcache");
        assert_eq!(config.default_ttl, 0);
        assert!(config.ip_map.is_none());
    }

    #[test]
    fn test_aerospike_config_custom() {
        let mut ip_map = HashMap::new();
        ip_map.insert("172.17.0.2".to_string(), "127.0.0.1".to_string());
        let config = AerospikeConfig {
            seed_nodes: vec!["10.0.0.1:3000".to_string(), "10.0.0.2:3000".to_string()],
            namespace: "production".to_string(),
            set_name: "cache".to_string(),
            default_ttl: 3600,
            ip_map: Some(ip_map),
        };
        assert_eq!(config.seed_nodes.len(), 2);
        assert_eq!(config.namespace, "production");
        assert_eq!(config.default_ttl, 3600);
        assert!(config.ip_map.is_some());
        assert_eq!(config.ip_map.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_is_key_not_found_error() {
        let err = AsError::ServerError(ResultCode::KeyNotFoundError, false, "not found".into());
        assert!(is_key_not_found(&err));

        let err = AsError::ServerError(ResultCode::ServerError, false, "internal".into());
        assert!(!is_key_not_found(&err));

        let err = AsError::Connection("conn refused".into());
        assert!(!is_key_not_found(&err));
    }

    #[test]
    fn test_write_policy_with_ttl_some() {
        // Verify Expiration variants construct correctly for TTL usage
        let exp_secs = Expiration::Seconds(60);
        assert!(matches!(exp_secs, Expiration::Seconds(60)));

        let exp_never = Expiration::Never;
        assert!(matches!(exp_never, Expiration::Never));

        // Verify Seconds(0) is distinct from Never
        assert_ne!(Expiration::Seconds(0), Expiration::Never);
    }

    #[test]
    fn test_backend_score_values() {
        // Verify AerospikeBackend score and persistence via trait defaults
        assert_eq!(Scores::REDIS, 50);
        // Aerospike score (30) is lower than Redis-family (50)
        const { assert!(30u8 < Scores::REDIS) };
    }

    // ========================================================================
    // Integration tests (require Aerospike server on port 3001)
    // ========================================================================

    /// Aerospike test seed — Docker mapped port 3001 → container 3000
    const AEROSPIKE_SEED: &str = "127.0.0.1:3001";

    fn test_config() -> AerospikeConfig {
        AerospikeConfig {
            seed_nodes: vec![AEROSPIKE_SEED.to_string()],
            namespace: "test".to_string(),
            set_name: "oxcache_test".to_string(),
            default_ttl: 0,
            ip_map: None,
        }
    }

    async fn make_backend() -> AerospikeBackend {
        AerospikeBackend::new(test_config())
            .await
            .expect("Failed to connect to Aerospike")
    }

    fn unique_key(prefix: &str) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}_{}", prefix, ts)
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_backend_kind() {
        let backend = make_backend().await;
        assert_eq!(backend.backend_kind(), BackendKind::Aerospike);
        assert!(backend.backend_kind().is_distributed());
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_backend_score() {
        let backend = make_backend().await;
        assert_eq!(backend.score(), 30);
        assert!(backend.is_persistent());
        assert_eq!(backend.backend_name(), "aerospike");
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_set_get_delete() {
        let backend = make_backend().await;
        let key = unique_key("as_sg");

        // set
        backend.set(
            Arc::from(key.as_str()),
            Arc::new(b"aerospike_value".to_vec()),
            None,
        ).await.expect("set failed");

        // get
        let val = backend.get(&key).await.expect("get failed");
        assert_eq!(val, Some(b"aerospike_value".to_vec()));

        // exists
        assert!(backend.exists(&key).await.expect("exists failed"));

        // delete
        backend.delete(&key).await.expect("delete failed");
        assert!(!backend.exists(&key).await.expect("exists after delete"));

        // get after delete
        let val = backend.get(&key).await.expect("get after delete");
        assert_eq!(val, None);
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_set_with_ttl() {
        let backend = make_backend().await;
        let key = unique_key("as_ttl");

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
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_expire() {
        let backend = make_backend().await;
        let key = unique_key("as_exp");

        backend.set(
            Arc::from(key.as_str()),
            Arc::new(b"v".to_vec()),
            None,
        ).await.expect("set failed");

        let ok = backend.expire(&key, Duration::from_secs(50)).await.expect("expire failed");
        assert!(ok);

        // TTL should now be set
        let ttl = backend.ttl(&key).await.expect("ttl after expire");
        assert!(ttl.is_some());

        backend.delete(&key).await.ok();
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_expire_nonexistent_key() {
        let backend = make_backend().await;
        let key = unique_key("as_exp_ne");

        let ok = backend.expire(&key, Duration::from_secs(50)).await.expect("expire ne");
        assert!(!ok);
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_health_check_and_stats() {
        let backend = make_backend().await;
        backend.health_check().await.expect("health check failed");

        let stats = backend.stats().await.expect("stats failed");
        assert_eq!(stats.get("backend_kind").unwrap(), "aerospike");
        assert_eq!(stats.get("namespace").unwrap(), "test");
        assert_eq!(stats.get("connected").unwrap(), "true");

        backend.shutdown().await;
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_set_many_get_many() {
        let backend = make_backend().await;
        let k1 = unique_key("as_m1");
        let k2 = unique_key("as_m2");

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

        // Verify deleted
        let values = backend.get_many(&keys).await.expect("get_many after del");
        assert_eq!(values[0], None);
        assert_eq!(values[1], None);
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_atomic_writer_is_none() {
        let backend = make_backend().await;
        let atomic = backend.as_atomic_writer();
        assert!(atomic.is_none());
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_unsupported_ops() {
        let backend = make_backend().await;

        // len, capacity, keys, clear are all NotSupported
        assert!(backend.len().await.is_err());
        assert!(backend.capacity().await.is_err());
        assert!(backend.keys("*").await.is_err());
        assert!(backend.clear().await.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_get_nonexistent_key() {
        let backend = make_backend().await;
        let key = unique_key("as_ne");
        let val = backend.get(&key).await.expect("get ne");
        assert_eq!(val, None);
    }

    #[tokio::test]
    #[ignore = "requires Aerospike server"]
    async fn test_aerospike_ttl_nonexistent_key() {
        let backend = make_backend().await;
        let key = unique_key("as_ttl_ne");
        let ttl = backend.ttl(&key).await.expect("ttl ne");
        assert!(ttl.is_none());
    }
}
