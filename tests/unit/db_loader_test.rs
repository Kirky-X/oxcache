// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 数据库加载器模块测试

use async_trait::async_trait;
use oxcache::client::db_loader::{validate_cache_key, validate_sql_identifier, DbFallbackManager, DbLoader};
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
    assert!(validate_cache_key("user:123"));
    assert!(validate_cache_key("session-abc"));
    assert!(validate_cache_key("cache_key"));
    assert!(validate_cache_key("data.json"));
    assert!(validate_cache_key("path/to/data"));
    assert!(validate_cache_key("abc123"));
}

#[test]
fn test_validate_cache_key_invalid() {
    assert!(!validate_cache_key(""));
    assert!(!validate_cache_key(&"a".repeat(1025)));
    assert!(!validate_cache_key("key with spaces"));
    assert!(!validate_cache_key("key;drop"));
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
