// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 公共 MockBackend 实现 - 用于测试

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Mock 后端 - 用于测试的模拟缓存后端
///
/// 提供可配置的分数和持久化属性，用于测试链式缓存和后端排序。
///
/// # Example
///
/// ```rust,ignore
/// use common::mock_backend::MockBackend;
///
/// let backend = MockBackend::new("test", 80, false);
/// let backend_with_data = MockBackend::with_data("test", 80, false);
/// ```
#[allow(dead_code)]
pub struct MockBackend {
    name: &'static str,
    score: u8,
    persistent: bool,
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MockBackend {
    /// 创建新的 MockBackend（无数据存储）
    ///
    /// # Arguments
    ///
    /// * `name` - 后端名称
    /// * `score` - 后端分数 (0-100)
    /// * `persistent` - 是否持久化
    #[allow(dead_code)]
    pub fn new(name: &'static str, score: u8, persistent: bool) -> Self {
        Self {
            name,
            score,
            persistent,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建带数据存储的 MockBackend
    ///
    /// 与 `new` 相同，但明确表示支持数据存储操作。
    #[allow(dead_code)]
    pub fn with_data(name: &'static str, score: u8, persistent: bool) -> Self {
        Self::new(name, score, persistent)
    }
}

impl oxcache::backend::BackendScore for MockBackend {
    fn score(&self) -> u8 {
        self.score
    }

    fn is_persistent(&self) -> bool {
        self.persistent
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl oxcache::backend::CacheBackend for MockBackend {
    async fn get(&self, key: &str) -> oxcache::error::Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> oxcache::error::Result<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }

    async fn clear(&self) -> oxcache::error::Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }

    async fn close(&self) -> oxcache::error::Result<()> {
        Ok(())
    }

    async fn ttl(&self, _key: &str) -> oxcache::error::Result<Option<Duration>> {
        Ok(None)
    }

    async fn expire(&self, _key: &str, _ttl: Duration) -> oxcache::error::Result<bool> {
        Ok(false)
    }

    async fn health_check(&self) -> oxcache::error::Result<bool> {
        Ok(true)
    }

    async fn stats(&self) -> oxcache::error::Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "mock".to_string());
        stats.insert("name".to_string(), self.name.to_string());
        Ok(stats)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn len(&self) -> oxcache::error::Result<u64> {
        let data = self.data.read().await;
        Ok(data.len() as u64)
    }

    async fn is_empty(&self) -> oxcache::error::Result<bool> {
        let data = self.data.read().await;
        Ok(data.is_empty())
    }

    async fn capacity(&self) -> oxcache::error::Result<u64> {
        Ok(1000)
    }
}
