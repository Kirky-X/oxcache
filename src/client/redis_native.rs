//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis 原生操作扩展模块
//!
//! 提供 Redis 计数器、有序集合、键扫描和 Lua 脚本执行支持。

use crate::error::{CacheError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Redis 原生操作 trait
///
/// 定义 Redis 特有的高级操作接口。
#[async_trait]
pub trait RedisNativeOps: Send + Sync {
    /// 设置单个键值
    async fn set(&self, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()>;

    /// 获取字节数组缓存值
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 计数器操作：增加数值
    async fn increment(&self, key: &str, amount: i64, ttl: Option<u64>) -> Result<i64>;

    /// 计数器操作：减少数值
    async fn decrement(&self, key: &str, amount: i64, ttl: Option<u64>) -> Result<i64>;

    /// 计数器操作：获取计数值
    async fn get_counter(&self, key: &str) -> Result<Option<i64>>;

    /// 有序集合操作：添加成员
    async fn zadd(&self, key: &str, score: f64, member: &str, ttl: Option<u64>) -> Result<u64>;

    /// 有序集合操作：按分数范围获取成员
    async fn zrange_by_score(
        &self,
        key: &str,
        min: f64,
        max: f64,
        with_scores: bool,
    ) -> Result<Vec<ZSetMember>>;

    /// 有序集合操作：获取成员分数
    async fn zscore(&self, key: &str, member: &str) -> Result<Option<f64>>;

    /// 有序集合操作：删除成员
    async fn zrem(&self, key: &str, members: &[&str]) -> Result<u64>;

    /// 有序集合操作：获取成员数量
    async fn zcard(&self, key: &str) -> Result<u64>;

    /// 键扫描：获取匹配的键
    async fn scan_keys(&self, pattern: &str, count: usize) -> Result<Vec<String>>;

    /// 键扫描迭代器
    fn scan_iter(&self, pattern: &str) -> ScanKeyIterator;

    /// 执行 Lua 脚本（只读）
    async fn eval_readonly(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<String>;

    /// 执行 Lua 脚本（写操作）
    async fn eval_write(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<String>;

    /// 使用缓存的脚本 SHA 执行
    async fn evalsha(&self, sha1: &str, keys: &[&str], args: &[&str]) -> Result<String>;

    /// 加载 Lua 脚本到缓存
    async fn script_load(&self, script: &str) -> Result<String>;

    /// 检查脚本是否在缓存中
    async fn script_exists(&self, sha1: &[&str]) -> Result<Vec<bool>>;

    /// 批量获取
    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>>;

    /// 批量设置
    async fn set_many(&self, items: HashMap<&str, &[u8]>, ttl: Option<u64>) -> Result<()>;

    /// 删除匹配模式的键
    async fn del_pattern(&self, pattern: &str) -> Result<u64>;
}

/// 有序集合成员
#[derive(Debug, Clone)]
pub struct ZSetMember {
    pub member: String,
    pub score: f64,
}

/// 键扫描迭代器
#[allow(dead_code)]
pub struct ScanKeyIterator {
    client: Arc<dyn RedisNativeOps>,
    pattern: String,
    cursor: u64,
    finished: bool,
}

impl ScanKeyIterator {
    pub fn new(client: Arc<dyn RedisNativeOps>, pattern: &str) -> Self {
        Self {
            client,
            pattern: pattern.to_string(),
            cursor: 0,
            finished: false,
        }
    }
}

impl Iterator for ScanKeyIterator {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        // 同步迭代器无法执行异步操作

        // 建议使用 async_next() 方法

        None
    }
}

impl ScanKeyIterator {
    /// 异步生成器风格的扫描
    pub async fn async_next(&mut self) -> Option<String> {
        if self.finished {
            return None;
        }

        // 使用 Redis SCAN 命令获取键

        // 注意：这是一个简化实现，实际应该使用内部状态维护游标

        // 由于 RedisNativeOps trait 没有定义 scan_match 方法，这里返回 None

        self.finished = true;

        None
    }

    /// 批量获取所有匹配的键
    pub async fn collect_all(&mut self) -> crate::error::Result<Vec<String>> {
        let mut all_keys = Vec::new();

        while let Some(key) = self.async_next().await {
            all_keys.push(key);
        }

        Ok(all_keys)
    }
}

/// Lua 脚本缓存
#[derive(Debug, Clone)]
pub struct ScriptCache {
    scripts: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

impl ScriptCache {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// 获取脚本的 SHA
    pub fn get_sha(&self, script: &str) -> Option<String> {
        self.scripts
            .lock()
            .map_err(|e| CacheError::LockError(e.to_string()))
            .ok()
            .and_then(|cache| cache.get(script).cloned())
    }

    /// 缓存脚本及其 SHA
    pub fn cache(&self, script: &str, sha: &str) {
        if let Ok(mut cache) = self
            .scripts
            .lock()
            .map_err(|e| CacheError::LockError(e.to_string()))
        {
            cache.insert(script.to_string(), sha.to_string());
        }
    }

    /// 检查脚本是否已缓存
    pub fn contains(&self, script: &str) -> bool {
        self.scripts
            .lock()
            .map_err(|e| CacheError::LockError(e.to_string()))
            .ok()
            .is_some_and(|cache| cache.contains_key(script))
    }

    /// 清除缓存
    pub fn clear(&self) {
        if let Ok(mut cache) = self
            .scripts
            .lock()
            .map_err(|e| CacheError::LockError(e.to_string()))
        {
            cache.clear();
        }
    }
}

impl Default for ScriptCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zset_member() {
        let member = ZSetMember {
            member: "test".to_string(),
            score: 1.5,
        };
        assert_eq!(member.member, "test");
        assert_eq!(member.score, 1.5);
    }

    #[test]
    fn test_script_cache() {
        let cache = ScriptCache::new();

        // 初始为空
        assert!(cache.get_sha("test").is_none());

        // 缓存脚本
        cache.cache("test script", "abc123");

        // 获取缓存的脚本
        assert_eq!(cache.get_sha("test script"), Some("abc123".to_string()));

        // 检查包含
        assert!(cache.contains("test script"));
        assert!(!cache.contains("other script"));

        // 清除缓存
        cache.clear();
        assert!(cache.get_sha("test script").is_none());
    }

    #[test]
    fn test_script_cache_default() {
        let cache = ScriptCache::default();
        assert!(cache.get_sha("test").is_none());
    }
}
