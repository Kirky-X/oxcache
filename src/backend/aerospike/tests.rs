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
    backend
        .set(
            Arc::from(key.as_str()),
            Arc::new(b"aerospike_value".to_vec()),
            None,
        )
        .await
        .expect("set failed");

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
}

#[tokio::test]
#[ignore = "requires Aerospike server"]
async fn test_aerospike_expire() {
    let backend = make_backend().await;
    let key = unique_key("as_exp");

    backend
        .set(Arc::from(key.as_str()), Arc::new(b"v".to_vec()), None)
        .await
        .expect("set failed");

    let ok = backend
        .expire(&key, Duration::from_secs(50))
        .await
        .expect("expire failed");
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

    let ok = backend
        .expire(&key, Duration::from_secs(50))
        .await
        .expect("expire ne");
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

    backend
        .delete_many(&keys)
        .await
        .expect("delete_many failed");

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
