//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 缓存适配器 trait

use std::collections::HashMap;

use crate::features::http::HttpCacheResponse;

/// HTTP 缓存适配器
///
/// 提供 HTTP 响应缓存的底层存储接口。
#[async_trait::async_trait]
pub trait HttpCacheAdapter: Send + Sync {
    /// 获取缓存的响应
    async fn get_response(&self, key: &str) -> Result<Option<HttpCacheResponse>, crate::error::CacheError>;

    /// 设置缓存响应
    async fn set_response(&self, key: &str, response: &HttpCacheResponse) -> Result<(), crate::error::CacheError>;

    /// 删除缓存的响应
    async fn delete_response(&self, key: &str) -> Result<bool, crate::error::CacheError>;

    /// 按模式失效
    async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64, crate::error::CacheError>;

    /// 按路径模式失效
    async fn invalidate_by_path_pattern(&self, path_pattern: &str) -> Result<u64, crate::error::CacheError> {
        self.invalidate_by_pattern(path_pattern).await
    }

    /// 批量获取
    async fn get_responses(
        &self,
        keys: &[&str],
    ) -> Result<HashMap<String, HttpCacheResponse>, crate::error::CacheError>;
}
