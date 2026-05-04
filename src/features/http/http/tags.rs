//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存标签管理器

use std::sync::Arc;

use crate::features::http::HttpCacheAdapter;

/// 缓存标签管理器
#[derive(Clone)]
pub struct CacheTagManager {
    tag_mapping: Arc<dashmap::DashMap<String, dashmap::DashSet<String>>>,
    adapter: Arc<dyn HttpCacheAdapter>,
}

impl CacheTagManager {
    /// 创建新的缓存标签管理器
    pub fn new(adapter: Arc<dyn HttpCacheAdapter>) -> Self {
        Self {
            tag_mapping: Arc::new(dashmap::DashMap::new()),
            adapter,
        }
    }

    /// 为缓存项添加标签
    pub async fn add_tags(&self, cache_key: &str, tags: &[&str]) -> Result<(), crate::error::CacheError> {
        for tag in tags {
            let tag_set = self.tag_mapping.entry(tag.to_string()).or_default();
            tag_set.insert(cache_key.to_string());
        }
        Ok(())
    }

    /// 使具有指定标签的所有缓存项失效
    pub async fn invalidate_by_tag(&self, tag: &str) -> Result<u64, crate::error::CacheError> {
        if let Some((_, keys)) = self.tag_mapping.remove(tag) {
            let count = keys.len() as u64;
            for key in keys {
                let _ = self.adapter.delete_response(&key).await;
            }
            return Ok(count);
        }
        Ok(0)
    }

    /// 使匹配模式的所有缓存项失效
    pub async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64, crate::error::CacheError> {
        self.adapter.invalidate_by_pattern(pattern).await
    }

    /// 清除所有标签映射
    pub fn clear(&self) {
        self.tag_mapping.clear();
    }
}
