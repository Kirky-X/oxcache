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
        // 单次查找：克隆 value 与 expires_at 后立即释放不可变借用
        let entry = data.get(key).map(|(v, exp)| (v.clone(), *exp));
        if let Some((value, expires_at)) = entry {
            if let Some(exp) = expires_at {
                if exp <= now {
                    // lazy 过期清理
                    data.remove(key);
                    return Ok(None);
                }
            }
            return Ok(Some(value));
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

    async fn keys(&self, pattern: &str) -> crate::error::OxCacheResult<Vec<String>> {
        let now = Instant::now();
        let mut data = self.data.write().await;
        // Lazy 过期清理 + pattern 匹配
        let expired_keys: Vec<String> = data
            .iter()
            .filter(|(_, (_, exp))| exp.is_some_and(|e| e <= now))
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired_keys {
            data.remove(&k);
        }
        let matched: Vec<String> = data
            .keys()
            .filter(|k| simple_glob(pattern, k))
            .cloned()
            .collect();
        Ok(matched)
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
        // 单次查找：避免 contains_key + get_mut 的双重哈希探测
        if let Some(entry) = data.get_mut(key) {
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

    fn as_atomic_writer(&self) -> Option<&dyn crate::backend::AtomicCacheWriter> {
        Some(self)
    }
}

// CacheBackend is automatically implemented via blanket implementation

/// Simple glob matching: `*` = any chars, `?` = single char.
fn simple_glob(pattern: &str, text: &str) -> bool {
    let mut p = pattern.chars().peekable();
    let mut t = text.chars().peekable();
    while let Some(pc) = p.peek() {
        match pc {
            '*' => {
                p.next();
                if p.peek().is_none() { return true; }
                let rem_p: String = p.collect();
                let rem_t: String = t.collect();
                for i in 0..=rem_t.len() {
                    if simple_glob(&rem_p, &rem_t[i..]) { return true; }
                }
                return false;
            }
            '?' => { if t.next().is_none() { return false; } p.next(); }
            _ => { match t.next() { Some(tc) if tc == *pc => { p.next(); } _ => return false } }
        }
    }
    t.peek().is_none()
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::AtomicCacheWriter for MockBackend {
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> crate::error::OxCacheResult<i64> {
        let mut data = self.data.write().await;
        let current = data
            .get(key)
            .and_then(|(v, _)| String::from_utf8(v.clone()).ok())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let new_val = current + delta;
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(key.to_string(), (new_val.to_string().into_bytes(), expires_at));
        Ok(new_val)
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<bool> {
        let mut data = self.data.write().await;
        match expected {
            None => {
                // SETNX: set only if absent
                if data.contains_key(key) {
                    Ok(false)
                } else {
                    let expires_at = ttl.map(|d| Instant::now() + d);
                    data.insert(key.to_string(), (new, expires_at));
                    Ok(true)
                }
            }
            Some(exp_bytes) => {
                match data.get(key) {
                    Some((current_val, _)) if current_val == exp_bytes => {
                        let expires_at = ttl.map(|d| Instant::now() + d);
                        data.insert(key.to_string(), (new, expires_at));
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
        }
    }

    async fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<bool> {
        let mut data = self.data.write().await;
        if data.contains_key(key) {
            return Ok(false);
        }
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(key.to_string(), (value, expires_at));
        Ok(true)
    }
}

#[cfg(test)]
impl crate::backend::SyncAtomicCacheWriter for MockBackend {
    fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> crate::error::OxCacheResult<i64> {
        let mut data = self.data.blocking_write();
        let current = data
            .get(key)
            .and_then(|(v, _)| String::from_utf8(v.clone()).ok())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let new_val = current + delta;
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(key.to_string(), (new_val.to_string().into_bytes(), expires_at));
        Ok(new_val)
    }

    fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<bool> {
        let mut data = self.data.blocking_write();
        match expected {
            None => {
                if data.contains_key(key) {
                    Ok(false)
                } else {
                    let expires_at = ttl.map(|d| Instant::now() + d);
                    data.insert(key.to_string(), (new, expires_at));
                    Ok(true)
                }
            }
            Some(exp_bytes) => {
                match data.get(key) {
                    Some((current_val, _)) if current_val == exp_bytes => {
                        let expires_at = ttl.map(|d| Instant::now() + d);
                        data.insert(key.to_string(), (new, expires_at));
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
        }
    }

    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<bool> {
        let mut data = self.data.blocking_write();
        if data.contains_key(key) {
            return Ok(false);
        }
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(key.to_string(), (value, expires_at));
        Ok(true)
    }
}

#[cfg(test)]
mod mock_tests {
    use super::*;
    use crate::backend::{AtomicCacheWriter, BackendScore, CacheConnector, CacheReader, CacheWriter};

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
        CacheWriter::set(&backend, Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
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
        CacheWriter::set(&backend, Arc::from("k1"), Arc::new(b"v1".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
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

    // ========================================================================
    // simple_glob unit tests
    // ========================================================================

    #[test]
    fn test_simple_glob_exact_match() {
        assert!(simple_glob("hello", "hello"));
        assert!(!simple_glob("hello", "world"));
        assert!(!simple_glob("hello", "hell"));
        assert!(!simple_glob("hell", "hello"));
    }

    #[test]
    fn test_simple_glob_star_matches_any() {
        assert!(simple_glob("*", ""));
        assert!(simple_glob("*", "anything"));
        assert!(simple_glob("hello*", "hello"));
        assert!(simple_glob("hello*", "helloworld"));
        assert!(simple_glob("*world", "helloworld"));
        assert!(simple_glob("he*ld", "helloworld"));
        assert!(!simple_glob("he*ld", "hello"));
    }

    #[test]
    fn test_simple_glob_question_mark() {
        assert!(simple_glob("h?llo", "hello"));
        assert!(simple_glob("?????", "hello"));
        assert!(!simple_glob("????", "hello"));
        assert!(!simple_glob("??????", "hello"));
        assert!(simple_glob("h?llo", "hallo"));
        assert!(!simple_glob("?", ""));
    }

    #[test]
    fn test_simple_glob_combined_patterns() {
        assert!(simple_glob("h*o", "hello"));
        assert!(simple_glob("h*o", "ho"));
        assert!(simple_glob("h?l*w", "hellow"));
        assert!(simple_glob("*?*", "a"));
        assert!(!simple_glob("*?*", ""));
        assert!(simple_glob("a*b*c", "abc"));
        assert!(simple_glob("a*b*c", "aXbYc"));
        assert!(!simple_glob("a*b*c", "aXbY"));
    }

    #[test]
    fn test_simple_glob_empty() {
        assert!(simple_glob("", ""));
        assert!(!simple_glob("", "a"));
        assert!(simple_glob("*", ""));
    }

    // ========================================================================
    // keys() with glob patterns
    // ========================================================================

    #[tokio::test]
    async fn test_mock_keys_returns_matching_keys() {
        let backend = MockBackend::new("test", 50, false);
        backend.set(Arc::from("user:1"), Arc::new(b"a".to_vec()), None).await.unwrap();
        backend.set(Arc::from("user:2"), Arc::new(b"b".to_vec()), None).await.unwrap();
        backend.set(Arc::from("session:1"), Arc::new(b"c".to_vec()), None).await.unwrap();

        let all_keys = backend.keys("*").await.unwrap();
        assert_eq!(all_keys.len(), 3);

        let user_keys = backend.keys("user:*").await.unwrap();
        assert_eq!(user_keys.len(), 2);

        let session_keys = backend.keys("session:*").await.unwrap();
        assert_eq!(session_keys.len(), 1);

        let no_match = backend.keys("nope:*").await.unwrap();
        assert!(no_match.is_empty());
    }

    #[tokio::test]
    async fn test_mock_keys_lazy_expires_during_scan() {
        let backend = MockBackend::new("test", 50, false);
        backend.set(Arc::from("alive"), Arc::new(b"v".to_vec()), None).await.unwrap();
        backend.set(Arc::from("dying"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(30))).await.unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;

        let keys = backend.keys("*").await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "alive");
    }

    // ========================================================================
    // capacity() and as_atomic_writer()
    // ========================================================================

    #[tokio::test]
    async fn test_mock_capacity_returns_zero() {
        let backend = MockBackend::new("test", 50, false);
        assert_eq!(backend.capacity().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mock_as_atomic_writer_returns_some() {
        let backend = MockBackend::new("test", 50, false);
        let writer = CacheConnector::as_atomic_writer(&backend);
        assert!(writer.is_some(), "MockBackend should implement AtomicCacheWriter");
    }

    // ========================================================================
    // AtomicCacheWriter direct tests (async)
    // ========================================================================

    #[tokio::test]
    async fn test_mock_atomic_incr_from_zero() {
        let backend = MockBackend::new("test", 50, false);
        let val = AtomicCacheWriter::incr(&backend, "counter", 1, None).await.unwrap();
        assert_eq!(val, 1);
    }

    #[tokio::test]
    async fn test_mock_atomic_incr_accumulates() {
        let backend = MockBackend::new("test", 50, false);
        AtomicCacheWriter::incr(&backend, "c", 10, None).await.unwrap();
        let val = AtomicCacheWriter::incr(&backend, "c", 5, None).await.unwrap();
        assert_eq!(val, 15);
    }

    #[tokio::test]
    async fn test_mock_atomic_incr_negative_delta() {
        let backend = MockBackend::new("test", 50, false);
        AtomicCacheWriter::incr(&backend, "c", 10, None).await.unwrap();
        let val = AtomicCacheWriter::incr(&backend, "c", -3, None).await.unwrap();
        assert_eq!(val, 7);
    }

    #[tokio::test]
    async fn test_mock_atomic_cas_setnx() {
        let backend = MockBackend::new("test", 50, false);
        // CAS with None = SETNX
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v1".to_vec(), None).await.unwrap();
        assert!(ok);
        // Second SETNX should fail
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v2".to_vec(), None).await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_mock_atomic_cas_with_expected() {
        let backend = MockBackend::new("test", 50, false);
        AtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v1".to_vec(), None).await.unwrap();
        // CAS with correct expected → success
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "k", Some(b"v1"), b"v2".to_vec(), None).await.unwrap();
        assert!(ok);
        // CAS with wrong expected → fail
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "k", Some(b"v1"), b"v3".to_vec(), None).await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn test_mock_atomic_cas_missing_key_with_expected() {
        let backend = MockBackend::new("test", 50, false);
        let ok = AtomicCacheWriter::compare_and_swap(&backend, "missing", Some(b"v1"), b"v2".to_vec(), None).await.unwrap();
        assert!(!ok, "CAS on missing key with expected should fail");
    }

    #[tokio::test]
    async fn test_mock_atomic_set_if_absent() {
        let backend = MockBackend::new("test", 50, false);
        let ok = AtomicCacheWriter::set_if_absent(&backend, "k", b"v".to_vec(), None).await.unwrap();
        assert!(ok);
        let ok = AtomicCacheWriter::set_if_absent(&backend, "k", b"v2".to_vec(), None).await.unwrap();
        assert!(!ok);
    }

    // ========================================================================
    // SyncAtomicCacheWriter direct tests
    // ========================================================================

    #[test]
    fn test_mock_sync_atomic_incr() {
        let backend = MockBackend::new("test", 50, false);
        let val = crate::backend::SyncAtomicCacheWriter::incr(&backend, "c", 5, None).unwrap();
        assert_eq!(val, 5);
        let val = crate::backend::SyncAtomicCacheWriter::incr(&backend, "c", 3, None).unwrap();
        assert_eq!(val, 8);
    }

    #[test]
    fn test_mock_sync_atomic_cas() {
        let backend = MockBackend::new("test", 50, false);
        // SETNX
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(&backend, "k", None, b"v1".to_vec(), None).unwrap();
        assert!(ok);
        // CAS correct expected
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(&backend, "k", Some(b"v1"), b"v2".to_vec(), None).unwrap();
        assert!(ok);
        // CAS wrong expected
        let ok = crate::backend::SyncAtomicCacheWriter::compare_and_swap(&backend, "k", Some(b"v1"), b"v3".to_vec(), None).unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_mock_sync_atomic_set_if_absent() {
        let backend = MockBackend::new("test", 50, false);
        let ok = crate::backend::SyncAtomicCacheWriter::set_if_absent(&backend, "k", b"v".to_vec(), None).unwrap();
        assert!(ok);
        let ok = crate::backend::SyncAtomicCacheWriter::set_if_absent(&backend, "k", b"v2".to_vec(), None).unwrap();
        assert!(!ok);
    }

    // ========================================================================
    // exists() with expired entry
    // ========================================================================

    #[tokio::test]
    async fn test_mock_exists_lazy_expires() {
        let backend = MockBackend::new("test", 50, false);
        backend.set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_millis(30))).await.unwrap();
        assert!(backend.exists("k").await.unwrap());
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(!backend.exists("k").await.unwrap(), "expired entry should report not exists");
    }
}
