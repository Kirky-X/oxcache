// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// MockBackend 实现 - 用于测试

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::time::{Duration, Instant};
#[cfg(test)]
use tokio::sync::RwLock;

/// 单条 Mock 缓存条目：(value, expires_at)，`None` 表示永不过期。
#[cfg(test)]
type MockEntry = (Vec<u8>, Option<Instant>);

/// 故障注入标志：控制 MockBackend 的失败行为（用于测试降级/错误路径）
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct MockFaultConfig {
    /// 为 true 时 `get` 返回错误（模拟 L1 故障）
    pub fail_get: bool,
    /// 为 true 时 `set` 返回错误
    pub fail_set: bool,
    /// 为 true 时 `health_check` 返回错误
    pub fail_health: bool,
}

/// Mock 后端 - 用于测试的模拟缓存后端
///
/// 内部数据结构存储 `(value, expires_at)`：`expires_at=None` 表示永不过期，
/// `Some(Instant)` 表示在该时刻过期（`get` 时 lazy 校验并清理）。
#[cfg(test)]
#[allow(dead_code)]
pub struct MockBackend {
    name: &'static str,
    score: u8,
    persistent: bool,
    data: Arc<RwLock<HashMap<String, MockEntry>>>,
    fault: MockFaultConfig,
}

#[cfg(test)]
impl MockBackend {
    pub fn new(name: &'static str, score: u8, persistent: bool) -> Self {
        Self {
            name,
            score,
            persistent,
            data: Arc::new(RwLock::new(HashMap::new())),
            fault: MockFaultConfig::default(),
        }
    }

    /// 注入故障：`get` 返回错误
    pub fn with_fail_get(mut self) -> Self {
        self.fault.fail_get = true;
        self
    }

    /// 注入故障：`set` 返回错误
    pub fn with_fail_set(mut self) -> Self {
        self.fault.fail_set = true;
        self
    }

    /// 注入故障：`health_check` 返回错误
    pub fn with_fail_health(mut self) -> Self {
        self.fault.fail_health = true;
        self
    }

    /// 检查是否配置了 `get` 故障
    pub fn fails_get(&self) -> bool {
        self.fault.fail_get
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
    async fn get(&self, key: &str) -> crate::error::OxCacheResult<Option<Vec<u8>>> {
        if self.fault.fail_get {
            return Err(crate::error::OxCacheError::Operation(
                "MockBackend get fault injected".to_string(),
            ));
        }
        let now = Instant::now();
        let mut data = self.data.write().await;
        if let Some((_v, expires_at)) = data.get(key) {
            if let Some(exp) = expires_at {
                if *exp <= now {
                    // lazy 过期清理
                    data.remove(key);
                    return Ok(None);
                }
            }
            return Ok(Some(data.get(key).unwrap().0.clone()));
        }
        Ok(None)
    }

    async fn exists(&self, key: &str) -> crate::error::OxCacheResult<bool> {
        let now = Instant::now();
        let mut data = self.data.write().await;
        if let Some((_v, expires_at)) = data.get(key) {
            if let Some(exp) = expires_at {
                if *exp <= now {
                    data.remove(key);
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    async fn ttl(&self, key: &str) -> crate::error::OxCacheResult<Option<Duration>> {
        let now = Instant::now();
        let data = self.data.read().await;
        if let Some((_v, Some(exp))) = data.get(key) {
            return Ok(exp.checked_duration_since(now));
        }
        Ok(None)
    }

    async fn len(&self) -> crate::error::OxCacheResult<u64> {
        let data = self.data.read().await;
        Ok(data.len() as u64)
    }

    async fn is_empty(&self) -> crate::error::OxCacheResult<bool> {
        let data = self.data.read().await;
        Ok(data.is_empty())
    }

    async fn capacity(&self) -> crate::error::OxCacheResult<u64> {
        Ok(0)
    }

    async fn stats(&self) -> crate::error::OxCacheResult<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), self.name.to_string());
        Ok(stats)
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::CacheWriter for MockBackend {
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> crate::error::OxCacheResult<()> {
        if self.fault.fail_set {
            return Err(crate::error::OxCacheError::Operation(
                "MockBackend set fault injected".to_string(),
            ));
        }
        let mut data = self.data.write().await;
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(key.to_string(), ((*value).clone(), expires_at));
        Ok(())
    }

    async fn delete(&self, key: &str) -> crate::error::OxCacheResult<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> crate::error::OxCacheResult<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> crate::error::OxCacheResult<bool> {
        let mut data = self.data.write().await;
        if data.contains_key(key) {
            let entry = data.get_mut(key).unwrap();
            entry.1 = Some(Instant::now() + ttl);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::CacheConnector for MockBackend {
    async fn health_check(&self) -> crate::error::OxCacheResult<()> {
        if self.fault.fail_health {
            return Err(crate::error::OxCacheError::Operation(
                "MockBackend health fault injected".to_string(),
            ));
        }
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
        CacheWriter::set(&backend, Arc::from("key"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();
        let result = CacheReader::get(&backend, "key").await.unwrap();
        assert_eq!(result, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_mock_backend_delete() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(&backend, Arc::from("key"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();
        CacheWriter::delete(&backend, "key").await.unwrap();
        assert!(CacheReader::get(&backend, "key").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mock_backend_clear() {
        let backend = MockBackend::new("test", 50, false);
        CacheWriter::set(&backend, Arc::from("k1"), Arc::new(b"v1".to_vec()), None).await.unwrap();
        CacheWriter::clear(&backend).await.unwrap();
        assert!(CacheReader::is_empty(&backend).await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_backend_exists() {
        let backend = MockBackend::new("test", 50, false);
        assert!(!CacheReader::exists(&backend, "key").await.unwrap());
        CacheWriter::set(&backend, Arc::from("key"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();
        assert!(CacheReader::exists(&backend, "key").await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_backend_len() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(CacheReader::len(&backend).await.unwrap(), 0);
        CacheWriter::set(&backend, Arc::from("k1"), Arc::new(b"v1".to_vec()), None).await.unwrap();
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

    // ========================================================================
    // 故障注入测试 (问题 5.1 / 7.3)
    // ========================================================================

    #[tokio::test]
    async fn test_mock_backend_fault_injected_get_returns_err() {
        let backend = MockBackend::new("failing", 50, false).with_fail_get();
        assert!(backend.fails_get());
        let result = CacheReader::get(&backend, "key").await;
        assert!(result.is_err(), "fail_get 注入后 get 应返回错误");
    }

    #[tokio::test]
    async fn test_mock_backend_fault_injected_set_returns_err() {
        let backend = MockBackend::new("failing", 50, false).with_fail_set();
        let result = CacheWriter::set(&backend, Arc::from("key"), Arc::new(b"v".to_vec()), None).await;
        assert!(result.is_err(), "fail_set 注入后 set 应返回错误");
    }

    #[tokio::test]
    async fn test_mock_backend_fault_injected_health_returns_err() {
        let backend = MockBackend::new("failing", 50, false).with_fail_health();
        let result = CacheConnector::health_check(&backend).await;
        assert!(result.is_err(), "fail_health 注入后 health_check 应返回错误");
    }

    #[tokio::test]
    async fn test_mock_backend_persistent() {
        let backend = MockBackend::new("test", 50, true);
        assert!(BackendScore::is_persistent(&backend));
    }

    // ========================================================================
    // Per-entry TTL tests (spec: universal-per-entry-ttl)
    // ========================================================================

    #[tokio::test]
    async fn test_mock_set_with_ttl_expires_after_timeout() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(50)))
            .await
            .unwrap();
        // 立即可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
        // 等待 100ms 后应过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(backend.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_set_without_ttl_never_expires() {
        let backend = MockBackend::new("test", 50, false);
        backend.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_mock_ttl_returns_remaining() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ttl = backend.ttl("k").await.unwrap().expect("ttl should be Some");
        // 58s < ttl <= 60s
        assert!(
            ttl > Duration::from_secs(58),
            "ttl={} should be > 58s",
            ttl.as_secs_f64()
        );
        assert!(
            ttl <= Duration::from_secs(60),
            "ttl={} should be <= 60s",
            ttl.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_mock_ttl_returns_none_for_missing_key() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(backend.ttl("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_ttl_returns_none_for_no_ttl_key() {
        let backend = MockBackend::new("test", 50, false);
        backend.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).await.unwrap();
        assert_eq!(backend.ttl("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_expire_extends_ttl() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ok = backend.expire("k", Duration::from_secs(120)).await.unwrap();
        assert!(ok, "expire on existing key should return true");
        let ttl = backend
            .ttl("k")
            .await
            .unwrap()
            .expect("ttl should be Some after expire");
        assert!(
            ttl > Duration::from_secs(118),
            "ttl={} should be > 118s",
            ttl.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_mock_expire_missing_key_returns_false() {
        let backend = MockBackend::new("test", 50, false);
        let ok = backend.expire("missing", Duration::from_secs(60)).await.unwrap();
        assert!(!ok, "expire on missing key should return false");
    }

    #[tokio::test]
    async fn test_mock_lazy_cleanup_removes_expired_entry() {
        let backend = MockBackend::new("test", 50, false);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(50)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        // 触发 lazy 过期清理
        let _ = backend.get("k").await.unwrap();
        // 内部 HashMap 中 "k" 应已删除
        let data = backend.data.read().await;
        assert!(!data.contains_key("k"), "expired entry should be lazily removed");
    }
}
