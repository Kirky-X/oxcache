#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

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
        unsafe {
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        }
    }

    /// Remove the insecure Redis env var after Dragonfly tests.
    fn cleanup_insecure_env() {
        // SAFETY: Rust 2024 edition — remove_var is unsafe; tests are serialized via #[serial]
        unsafe {
            std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS");
        }
    }

    async fn make_dragonfly() -> Option<DragonflyBackend> {
        set_insecure_env();
        // 无外部 Dragonfly/Redis 时拉起进程内假 RESP 服务器（端口空闲才拉起，
        // 被占用则 PING 探测）；两者皆不可用才跳过
        if !crate::test_support::ensure_server(6380) {
            return None;
        }
        DragonflyBackend::new(DRAGONFLY_URL, 4).await.ok()
    }

    fn unique_key(prefix: &str) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}_{}", prefix, ts)
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_backend_kind() {
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);
        assert!(backend.backend_kind().is_distributed());
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_backend_score() {
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        assert_eq!(backend.score(), Scores::REDIS);
        assert!(backend.is_persistent());
        assert_eq!(backend.backend_name(), "dragonfly");
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_set_get_delete() {
        use crate::backend::{CacheReader, CacheWriter};
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        let key = unique_key("df_sg");

        // set
        backend
            .set(
                Arc::from(key.as_str()),
                Arc::new(b"dragonfly_value".to_vec()),
                None,
            )
            .await
            .expect("set failed");

        // get
        let val = backend.get(&key).await.expect("get failed");
        assert_eq!(val, Some(b"dragonfly_value".to_vec()));

        // exists
        assert!(backend.exists(&key).await.expect("exists failed"));

        // delete
        backend.delete(&key).await.expect("delete failed");
        assert!(
            !backend
                .exists(&key)
                .await
                .expect("exists after delete failed")
        );
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_set_with_ttl() {
        use crate::backend::{CacheReader, CacheWriter};
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        let key = unique_key("df_ttl");

        backend
            .set(
                Arc::from(key.as_str()),
                Arc::new(b"ttl_val".to_vec()),
                Some(Duration::from_secs(100)),
            )
            .await
            .expect("set with ttl failed");

        let ttl = backend.ttl(&key).await.expect("ttl failed");
        assert!(ttl.is_some());
        let secs = ttl.unwrap().as_secs();
        assert!(secs > 90 && secs <= 100, "ttl secs = {}", secs);

        backend.delete(&key).await.ok();
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_expire() {
        use crate::backend::CacheWriter;
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        let key = unique_key("df_exp");

        backend
            .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
            .await
            .expect("set failed");

        let ok = backend
            .expire(&key, Duration::from_secs(50))
            .await
            .expect("expire failed");
        assert!(ok);

        backend.delete(&key).await.ok();
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_health_check_and_stats() {
        use crate::backend::CacheConnector;
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        backend.health_check().await.expect("health check failed");
        backend.shutdown().await;
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_set_many_get_many() {
        use crate::backend::{CacheReader, CacheWriter};
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
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

        backend
            .delete_many(&keys)
            .await
            .expect("delete_many failed");
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_atomic_writer_is_none() {
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        // Dragonfly does NOT support AtomicCacheWriter
        let atomic = backend.as_atomic_writer();
        assert!(atomic.is_none());
        cleanup_insecure_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_dragonfly_capacity_zero() {
        use crate::backend::CacheReader;
        let Some(backend) = make_dragonfly().await else {
            cleanup_insecure_env();
            return;
        };
        let cap = backend.capacity().await.expect("capacity failed");
        assert_eq!(cap, 0);
        cleanup_insecure_env();
    }
}
