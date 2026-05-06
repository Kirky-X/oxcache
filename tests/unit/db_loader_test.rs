// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 数据库加载器模块测试

use async_trait::async_trait;
use oxcache::error::CacheError;
use oxcache::infra::{
    validate_cache_key, validate_sql_identifier, DbConnectionPool, DbFallbackConfig, DbFallbackManager, DbLoader,
    SqlDbLoader,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 测试用的模拟数据库加载器
#[derive(Debug)]
struct MockDbLoader {
    healthy: AtomicBool,
    data: std::collections::HashMap<String, Vec<u8>>,
}

impl MockDbLoader {
    fn new() -> Self {
        Self {
            healthy: AtomicBool::new(true),
            data: std::collections::HashMap::new(),
        }
    }

    fn with_data(data: Vec<(String, Vec<u8>)>) -> Self {
        let mut loader = Self::new();
        for (key, value) in data {
            loader.data.insert(key, value);
        }
        loader
    }

    fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::SeqCst);
    }
}

#[async_trait]
impl DbLoader for MockDbLoader {
    async fn load(&self, key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        if !self.healthy.load(Ordering::SeqCst) {
            return Err(oxcache::error::CacheError::DatabaseError(
                "Loader not healthy".to_string(),
            ));
        }
        Ok(self.data.get(key).cloned())
    }

    async fn load_batch(&self, keys: Vec<String>) -> oxcache::error::Result<Vec<(String, Vec<u8>)>> {
        if !self.healthy.load(Ordering::SeqCst) {
            return Err(oxcache::error::CacheError::DatabaseError(
                "Loader not healthy".to_string(),
            ));
        }
        let mut results = Vec::new();
        for key in keys {
            if let Some(value) = self.data.get(&key) {
                results.push((key, value.clone()));
            }
        }
        Ok(results)
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }
}

#[test]
fn test_validate_sql_identifier_valid() {
    assert!(validate_sql_identifier("table_name"));
    assert!(validate_sql_identifier("TableName"));
    assert!(validate_sql_identifier("_table"));
    assert!(validate_sql_identifier("table123"));
    assert!(validate_sql_identifier("a"));
    assert!(validate_sql_identifier("_"));
}

#[test]
fn test_validate_sql_identifier_invalid() {
    assert!(!validate_sql_identifier(""));
    assert!(!validate_sql_identifier("123table"));
    assert!(!validate_sql_identifier("table-name"));
    assert!(!validate_sql_identifier("table.name"));
    assert!(!validate_sql_identifier("table name"));
    assert!(!validate_sql_identifier("table;drop"));
}

#[test]
fn test_validate_cache_key_valid() {
    assert!(validate_cache_key("user:123").is_ok());
    assert!(validate_cache_key("session-abc").is_ok());
    assert!(validate_cache_key("cache_key").is_ok());
    assert!(validate_cache_key("data.json").is_ok());
    assert!(validate_cache_key("path/to/data").is_ok());
    assert!(validate_cache_key("abc123").is_ok());
}

#[test]
fn test_validate_cache_key_invalid() {
    assert!(validate_cache_key("").is_err());
    assert!(validate_cache_key(&"a".repeat(1025)).is_err());
    assert!(validate_cache_key("key with spaces").is_err());
    assert!(validate_cache_key("key;drop").is_err());
}

#[tokio::test]
async fn test_db_fallback_manager_disabled() {
    let loader = Arc::new(MockDbLoader::new());
    let manager = DbFallbackManager::new(loader, false, 1000, 3);

    let result = manager.fallback_load("test_key").await.unwrap();
    assert!(result.is_none());
    assert!(!manager.is_enabled());
}

#[tokio::test]
async fn test_db_fallback_manager_not_healthy() {
    let loader = Arc::new(MockDbLoader::new());
    loader.set_healthy(false);

    let manager = DbFallbackManager::new(loader, true, 1000, 3);

    let result = manager.fallback_load("test_key").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_db_fallback_manager_load_success() {
    let loader = Arc::new(MockDbLoader::with_data(vec![(
        "test_key".to_string(),
        b"test_value".to_vec(),
    )]));

    let manager = DbFallbackManager::new(loader, true, 1000, 3);

    let result = manager.fallback_load("test_key").await.unwrap();
    assert_eq!(result, Some(b"test_value".to_vec()));
}

#[tokio::test]
async fn test_db_fallback_manager_load_not_found() {
    let loader = Arc::new(MockDbLoader::new());
    let manager = DbFallbackManager::new(loader, true, 1000, 3);

    let result = manager.fallback_load("nonexistent_key").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_db_fallback_manager_batch_load_success() {
    let loader = Arc::new(MockDbLoader::with_data(vec![
        ("key1".to_string(), b"value1".to_vec()),
        ("key2".to_string(), b"value2".to_vec()),
    ]));

    let manager = DbFallbackManager::new(loader, true, 1000, 3);

    let results = manager
        .fallback_load_batch(vec!["key1".to_string(), "key2".to_string()])
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_db_fallback_manager_batch_load_disabled() {
    let loader = Arc::new(MockDbLoader::new());
    let manager = DbFallbackManager::new(loader, false, 1000, 3);

    let results = manager.fallback_load_batch(vec!["key1".to_string()]).await.unwrap();

    assert!(results.is_empty());
}

#[test]
fn test_db_fallback_manager_is_enabled() {
    let loader = Arc::new(MockDbLoader::new());

    let manager_enabled = DbFallbackManager::new(loader.clone(), true, 1000, 3);
    assert!(manager_enabled.is_enabled());

    let manager_disabled = DbFallbackManager::new(loader, false, 1000, 3);
    assert!(!manager_disabled.is_enabled());
}

/// 测试用的模拟数据库连接池
#[derive(Debug)]
struct MockDbConnectionPool {
    healthy: AtomicBool,
    data: std::collections::HashMap<String, Vec<u8>>,
}

impl MockDbConnectionPool {
    fn new() -> Self {
        Self {
            healthy: AtomicBool::new(true),
            data: std::collections::HashMap::new(),
        }
    }

    fn with_data(data: Vec<(String, Vec<u8>)>) -> Self {
        let mut pool = Self::new();
        for (key, value) in data {
            pool.data.insert(key, value);
        }
        pool
    }

    fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::SeqCst);
    }
}

#[async_trait]
impl DbConnectionPool for MockDbConnectionPool {
    async fn execute_query(&self, _query: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        if !self.healthy.load(Ordering::SeqCst) {
            return Err(CacheError::DatabaseError("Pool not healthy".to_string()));
        }
        Ok(self.data.values().next().cloned())
    }

    async fn execute_batch_query(&self, _query: &str) -> oxcache::error::Result<Vec<(String, Vec<u8>)>> {
        if !self.healthy.load(Ordering::SeqCst) {
            return Err(CacheError::DatabaseError("Pool not healthy".to_string()));
        }
        Ok(self.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }
}

#[test]
fn test_sql_db_loader_new_valid_params() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let result = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    );

    assert!(result.is_ok());
    let loader = result.unwrap();
    assert!(loader.is_healthy());
}

#[test]
fn test_sql_db_loader_new_invalid_table_name() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let result = SqlDbLoader::new(
        pool,
        "123invalid".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::InvalidInput(_)));
    assert!(err.to_string().contains("Invalid table name"));
}

#[test]
fn test_sql_db_loader_new_invalid_key_column() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let result = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "invalid-key".to_string(),
        "cache_value".to_string(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::InvalidInput(_)));
    assert!(err.to_string().contains("Invalid key column name"));
}

#[test]
fn test_sql_db_loader_new_invalid_value_column() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let result = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "invalid;column".to_string(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::InvalidInput(_)));
    assert!(err.to_string().contains("Invalid value column name"));
}

#[tokio::test]
async fn test_sql_db_loader_load_valid_key() {
    let pool = Arc::new(MockDbConnectionPool::with_data(vec![(
        "user:123".to_string(),
        b"test_data".to_vec(),
    )]));

    let loader = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    )
    .unwrap();

    let result = loader.load("user:123").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), b"test_data");
}

#[tokio::test]
async fn test_sql_db_loader_load_invalid_key() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let loader = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    )
    .unwrap();

    let result = loader.load("key with spaces").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::InvalidInput(_)));
}

#[tokio::test]
async fn test_sql_db_loader_load_batch_valid_keys() {
    let pool = Arc::new(MockDbConnectionPool::with_data(vec![
        ("key1".to_string(), b"value1".to_vec()),
        ("key2".to_string(), b"value2".to_vec()),
    ]));

    let loader = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    )
    .unwrap();

    let result = loader
        .load_batch(vec!["key1".to_string(), "key2".to_string()])
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_sql_db_loader_load_batch_empty_keys() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let loader = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    )
    .unwrap();

    let result = loader.load_batch(vec![]).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_sql_db_loader_load_batch_invalid_key() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let loader = SqlDbLoader::new(
        pool,
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    )
    .unwrap();

    let result = loader
        .load_batch(vec!["valid_key".to_string(), "invalid;key".to_string()])
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::InvalidInput(_)));
}

#[test]
fn test_sql_db_loader_is_healthy() {
    let pool = Arc::new(MockDbConnectionPool::new());
    let loader = SqlDbLoader::new(
        pool.clone(),
        "cache_table".to_string(),
        "cache_key".to_string(),
        "cache_value".to_string(),
    )
    .unwrap();

    assert!(loader.is_healthy());

    pool.set_healthy(false);
    assert!(!loader.is_healthy());
}

/// 总是失败的 Mock 加载器
#[derive(Debug)]
struct FailingMockLoader {
    healthy: AtomicBool,
    fail_count: std::sync::Mutex<usize>,
}

impl FailingMockLoader {
    fn new() -> Self {
        Self {
            healthy: AtomicBool::new(true),
            fail_count: std::sync::Mutex::new(0),
        }
    }
}

#[async_trait]
impl DbLoader for FailingMockLoader {
    async fn load(&self, _key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        let mut count = self.fail_count.lock().unwrap();
        *count += 1;
        Err(CacheError::DatabaseError(format!("Simulated failure #{}", *count)))
    }

    async fn load_batch(&self, _keys: Vec<String>) -> oxcache::error::Result<Vec<(String, Vec<u8>)>> {
        Err(CacheError::DatabaseError("Batch failure".to_string()))
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn test_db_fallback_manager_retry_on_error() {
    let loader = Arc::new(FailingMockLoader::new());
    let manager = DbFallbackManager::new(loader, true, 10, 2);

    let result = manager.fallback_load("test_key").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::DatabaseError(_)));
}

/// 延迟成功的 Mock 加载器
#[derive(Debug)]
struct DelayedSuccessLoader {
    attempt: std::sync::Mutex<u32>,
    succeed_after: u32,
}

impl DelayedSuccessLoader {
    fn new(succeed_after: u32) -> Self {
        Self {
            attempt: std::sync::Mutex::new(0),
            succeed_after,
        }
    }
}

#[async_trait]
impl DbLoader for DelayedSuccessLoader {
    async fn load(&self, _key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        let mut attempt = self.attempt.lock().unwrap();
        *attempt += 1;
        if *attempt >= self.succeed_after {
            Ok(Some(b"success".to_vec()))
        } else {
            Err(CacheError::DatabaseError(format!("Attempt {} failed", *attempt)))
        }
    }

    async fn load_batch(&self, _keys: Vec<String>) -> oxcache::error::Result<Vec<(String, Vec<u8>)>> {
        Ok(vec![("key".to_string(), b"value".to_vec())])
    }

    fn is_healthy(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn test_db_fallback_manager_retry_eventually_succeeds() {
    let loader = Arc::new(DelayedSuccessLoader::new(2));
    let manager = DbFallbackManager::new(loader, true, 100, 3);

    let result = manager.fallback_load("test_key").await.unwrap();

    assert_eq!(result, Some(b"success".to_vec()));
}

/// 永远阻塞的 Mock 加载器
#[derive(Debug)]
struct HangingMockLoader;

#[async_trait]
impl DbLoader for HangingMockLoader {
    async fn load(&self, _key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        std::future::pending().await
    }

    async fn load_batch(&self, _keys: Vec<String>) -> oxcache::error::Result<Vec<(String, Vec<u8>)>> {
        std::future::pending().await
    }

    fn is_healthy(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn test_db_fallback_manager_timeout_on_load() {
    let loader = Arc::new(HangingMockLoader);
    let manager = DbFallbackManager::new(loader, true, 50, 0);

    let start = std::time::Instant::now();
    let result = manager.fallback_load("test_key").await.unwrap();

    assert!(result.is_none());
    assert!(start.elapsed() < std::time::Duration::from_millis(200));
}

#[tokio::test]
async fn test_db_fallback_manager_timeout_on_batch_load() {
    let loader = Arc::new(HangingMockLoader);
    let manager = DbFallbackManager::new(loader, true, 50, 0);

    let start = std::time::Instant::now();
    let result = manager.fallback_load_batch(vec!["key1".to_string()]).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CacheError::Timeout(_)));
    assert!(start.elapsed() < std::time::Duration::from_millis(200));
}

#[test]
fn test_db_fallback_config_default() {
    let config = DbFallbackConfig::default();

    assert!(!config.enabled);
    assert_eq!(config.timeout_ms, 5000);
    assert_eq!(config.max_retries, 3);
    assert!(config.connection_string.is_empty());
    assert_eq!(config.table_name, "cache_table");
    assert_eq!(config.key_column, "cache_key");
    assert_eq!(config.value_column, "cache_value");
}

#[test]
fn test_db_fallback_config_serialization() {
    let config = DbFallbackConfig {
        enabled: true,
        timeout_ms: 3000,
        max_retries: 5,
        connection_string: "postgres://localhost/db".to_string(),
        table_name: "my_cache".to_string(),
        key_column: "key".to_string(),
        value_column: "value".to_string(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: DbFallbackConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.enabled, config.enabled);
    assert_eq!(deserialized.timeout_ms, config.timeout_ms);
    assert_eq!(deserialized.max_retries, config.max_retries);
    assert_eq!(deserialized.connection_string, config.connection_string);
    assert_eq!(deserialized.table_name, config.table_name);
    assert_eq!(deserialized.key_column, config.key_column);
    assert_eq!(deserialized.value_column, config.value_column);
}

#[test]
fn test_db_fallback_manager_debug_format() {
    let loader = Arc::new(MockDbLoader::new());
    let manager = DbFallbackManager::new(loader, true, 5000, 3);

    let debug_str = format!("{:?}", manager);

    assert!(debug_str.contains("DbFallbackManager"));
    assert!(debug_str.contains("enabled"));
    assert!(debug_str.contains("true"));
    assert!(debug_str.contains("timeout_ms"));
    assert!(debug_str.contains("5000"));
    assert!(debug_str.contains("max_retries"));
    assert!(debug_str.contains("3"));
    assert!(debug_str.contains("loader_healthy"));
}

#[test]
fn test_db_fallback_manager_debug_unhealthy_loader() {
    let loader = Arc::new(MockDbLoader::new());
    loader.set_healthy(false);

    let manager = DbFallbackManager::new(loader, false, 1000, 2);

    let debug_str = format!("{:?}", manager);

    assert!(debug_str.contains("loader_healthy"));
    assert!(debug_str.contains("false"));
}
