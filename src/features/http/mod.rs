//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 缓存模块
//!
//! 提供 HTTP 响应缓存适配层、键生成、条件请求处理和 Axum 中间件。

use serde::{Deserialize, Serialize};

// Submodules
pub mod adapter;
pub mod axum;
pub mod conditional;
pub mod key;
pub mod matcher;
pub mod policy;
pub mod tags;

// Re-exports
pub use adapter::HttpCacheAdapter;
pub use axum::{CacheMiddlewareConfig, CacheMiddlewareState};
pub use conditional::{ConditionalRequestHandler, ConditionalRequestResult};
pub use key::{HttpCacheKeyGenerator, HttpRequest};
pub use matcher::PathPatternMatcher;
pub use policy::HttpCachePolicy;
pub use tags::CacheTagManager;

/// HTTP 缓存响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpCacheResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
    pub cached_at: chrono::DateTime<chrono::Utc>,
    pub ttl: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}
