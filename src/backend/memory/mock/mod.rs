// Copyright (c) 2025-2026 Kirky.X🌠
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
    /// 为 true 时 `expire` 返回错误（模拟 TTL 调整写入失败路径）
    pub fail_expire: bool,
}

/// Mock 后端 - 用于测试的模拟缓存后端
///
/// 内部数据结构存储 `(value, expires_at)`：`expires_at=None` 表示永不过期，
/// `Some(Instant)` 表示在该时刻过期（`get` 时 lazy 校验并清理）。
#[cfg(test)]
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

    /// 注入故障：`expire` 返回错误
    pub fn with_fail_expire(mut self) -> Self {
        self.fault.fail_expire = true;
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
            if let Some(exp) = expires_at
                && exp <= now
            {
                // lazy 过期清理
                data.remove(key);
                return Ok(None);
            }
            return Ok(Some(value));
        }
        Ok(None)
    }

    async fn exists(&self, key: &str) -> crate::error::OxCacheResult<bool> {
        let now = Instant::now();
        let mut data = self.data.write().await;
        if let Some((_v, expires_at)) = data.get(key) {
            if let Some(exp) = expires_at
                && *exp <= now
            {
                data.remove(key);
                return Ok(false);
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

        // Lazy 过期清理
        let expired_keys: Vec<String> = data
            .iter()
            .filter(|(_, (_, exp))| exp.is_some_and(|e| e <= now))
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired_keys {
            data.remove(k);
        }

        // Pattern 匹配（在清理后执行）
        let matched: Vec<String> = data
            .keys()
            .filter(|k| crate::backend::interface::glob_match(pattern, k))
            .cloned()
            .collect();

        Ok(matched)
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::CacheWriter for MockBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<()> {
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
        if self.fault.fail_expire {
            return Err(crate::error::OxCacheError::Operation(
                "mock expire failure injected".to_string(),
            ));
        }
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

#[cfg(test)]
#[async_trait::async_trait]
impl crate::backend::AtomicCacheWriter for MockBackend {
    async fn incr(
        &self,
        key: &str,
        delta: i64,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<i64> {
        let mut data = self.data.write().await;
        let current = match data.get(key) {
            Some((v, _)) => {
                let s = String::from_utf8(v.clone()).map_err(|e| {
                    crate::error::OxCacheError::Operation(format!(
                        "incr: invalid UTF-8 in stored value for key '{}': {}",
                        key, e
                    ))
                })?;
                s.parse::<i64>().map_err(|e| {
                    crate::error::OxCacheError::Operation(format!(
                        "incr: invalid integer in stored value for key '{}': {}",
                        key, e
                    ))
                })?
            }
            None => 0,
        };
        let new_val = current.checked_add(delta).ok_or_else(|| {
            crate::error::OxCacheError::Operation(format!(
                "incr: i64 overflow for key '{}': {} + {}",
                key, current, delta
            ))
        })?;
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(
            key.to_string(),
            (new_val.to_string().into_bytes(), expires_at),
        );
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
            Some(exp_bytes) => match data.get(key) {
                Some((current_val, _)) if current_val == exp_bytes => {
                    let expires_at = ttl.map(|d| Instant::now() + d);
                    data.insert(key.to_string(), (new, expires_at));
                    Ok(true)
                }
                _ => Ok(false),
            },
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
    fn incr(
        &self,
        key: &str,
        delta: i64,
        ttl: Option<Duration>,
    ) -> crate::error::OxCacheResult<i64> {
        // Mirror the async incr: propagate UTF-8/parse/overflow errors instead
        // of silently treating corrupt data as 0.
        let mut data = self.data.blocking_write();
        let current = match data.get(key) {
            Some((v, _)) => {
                let s = String::from_utf8(v.clone()).map_err(|e| {
                    crate::error::OxCacheError::Operation(format!(
                        "incr: invalid UTF-8 in stored value for key '{}': {}",
                        key, e
                    ))
                })?;
                s.parse::<i64>().map_err(|e| {
                    crate::error::OxCacheError::Operation(format!(
                        "incr: invalid integer in stored value for key '{}': {}",
                        key, e
                    ))
                })?
            }
            None => 0,
        };
        let new_val = current.checked_add(delta).ok_or_else(|| {
            crate::error::OxCacheError::Operation(format!(
                "incr: i64 overflow for key '{}': {} + {}",
                key, current, delta
            ))
        })?;
        let expires_at = ttl.map(|d| Instant::now() + d);
        data.insert(
            key.to_string(),
            (new_val.to_string().into_bytes(), expires_at),
        );
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
            Some(exp_bytes) => match data.get(key) {
                Some((current_val, _)) if current_val == exp_bytes => {
                    let expires_at = ttl.map(|d| Instant::now() + d);
                    data.insert(key.to_string(), (new, expires_at));
                    Ok(true)
                }
                _ => Ok(false),
            },
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
mod tests;
