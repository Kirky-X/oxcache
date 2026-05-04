// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// MockBackend 实现 - 用于测试

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
use tokio::sync::RwLock;

/// Mock 后端 - 用于测试的模拟缓存后端
#[cfg(test)]
#[allow(dead_code)]
pub struct MockBackend {
    name: &'static str,
    score: u8,
    persistent: bool,
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

#[cfg(test)]
impl MockBackend {
    pub fn new(name: &'static str, score: u8, persistent: bool) -> Self {
        Self {
            name,
            score,
            persistent,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn with_data(name: &'static str, score: u8, persistent: bool) -> Self {
        Self::new(name, score, persistent)
    }
}

#[cfg(test)]
impl crate::backend::BackendScore for MockBackend {
    fn score(&self) -> u8 {
        self.score
    }

    fn is_persistent(&self) -> bool {
        self.persistent
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::CacheReader for MockBackend {
    async fn get(&self, key: &str) -> crate::error::Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn exists(&self, key: &str) -> crate::error::Result<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }

    async fn ttl(&self, _key: &str) -> crate::error::Result<Option<Duration>> {
        Ok(None)
    }

    async fn len(&self) -> crate::error::Result<u64> {
        let data = self.data.read().await;
        Ok(data.len() as u64)
    }

    async fn is_empty(&self) -> crate::error::Result<bool> {
        let data = self.data.read().await;
        Ok(data.is_empty())
    }

    async fn capacity(&self) -> crate::error::Result<u64> {
        Ok(0)
    }

    async fn stats(&self) -> crate::error::Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), self.name.to_string());
        Ok(stats)
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::CacheWriter for MockBackend {
    async fn set(&self, key: &str, value: Vec<u8>, _ttl: Option<Duration>) -> crate::error::Result<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> crate::error::Result<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> crate::error::Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }

    async fn expire(&self, _key: &str, _ttl: Duration) -> crate::error::Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::CacheConnector for MockBackend {
    async fn health_check(&self) -> crate::error::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) {}

    fn backend_kind(&self) -> crate::backend::interface::BackendKind {
        crate::backend::interface::BackendKind::Mock
    }
}

// CacheBackend is automatically implemented via blanket implementation

#[cfg(test)]
mod mock_tests {
    use super::*;
    use crate::backend::{BackendScore, CacheConnector, CacheReader, CacheWriter};

    #[tokio::test]
    async fn test_mock_backend_new() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(BackendScore::score(&backend), 50);
        assert_eq!(BackendScore::backend_name(&backend), "test");
        assert!(!BackendScore::is_persistent(&backend));
    }

    #[tokio::test]
    async fn test_mock_backend_set_get() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(&backend, "key", b"value".to_vec(), None)
            .await
            .unwrap();
        let result = CacheReader::get(&backend, "key").await.unwrap();
        assert_eq!(result, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_mock_backend_delete() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(&backend, "key", b"value".to_vec(), None)
            .await
            .unwrap();
        CacheWriter::delete(&backend, "key").await.unwrap();
        assert!(CacheReader::get(&backend, "key").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mock_backend_clear() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(&backend, "k1", b"v1".to_vec(), None).await.unwrap();
        CacheWriter::clear(&backend).await.unwrap();
        assert!(CacheReader::is_empty(&backend).await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_backend_exists() {
        let backend = MockBackend::new("test", 50, false);
        assert!(!CacheReader::exists(&backend, "key").await.unwrap());
        CacheWriter::set(&backend, "key", b"value".to_vec(), None)
            .await
            .unwrap();
        assert!(CacheReader::exists(&backend, "key").await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_backend_len() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(CacheReader::len(&backend).await.unwrap(), 0);
        CacheWriter::set(&backend, "k1", b"v1".to_vec(), None).await.unwrap();
        assert_eq!(CacheReader::len(&backend).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_mock_backend_stats() {
        let backend = MockBackend::new("test", 50, false);
        let stats = CacheReader::stats(&backend).await.unwrap();
        assert_eq!(stats.get("type"), Some(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_mock_backend_health_check() {
        let backend = MockBackend::new("test", 50, false);
        assert!(CacheConnector::health_check(&backend).await.is_ok());
    }

    #[tokio::test]
    async fn test_mock_backend_shutdown() {
        let backend = MockBackend::new("test", 50, false);
        CacheConnector::shutdown(&backend).await;
    }

    #[test]
    fn test_mock_backend_kind() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(
            CacheConnector::backend_kind(&backend),
            crate::backend::interface::BackendKind::Mock
        );
    }

    #[tokio::test]
    async fn test_mock_backend_persistent() {
        let backend = MockBackend::new("test", 50, true);
        assert!(BackendScore::is_persistent(&backend));
    }
}
