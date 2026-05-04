//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 缓存策略配置

use http::header::{self, HeaderMap};
use http::StatusCode;

/// HTTP 缓存策略配置
#[derive(Debug, Clone, Default)]
pub struct HttpCachePolicy {
    /// 要缓存的状态码
    pub cache_status_codes: Vec<StatusCode>,
    /// 默认 TTL（秒）
    pub default_ttl: u64,
    /// 基于响应头 TTL 的优先级
    pub use_header_ttl: bool,
    /// 需要忽略的路径模式
    pub ignore_patterns: Vec<String>,
    /// 缓存键前缀
    pub key_prefix: String,
}

impl HttpCachePolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加要缓存的状态码
    pub fn with_cache_status_codes(mut self, codes: Vec<StatusCode>) -> Self {
        self.cache_status_codes = codes;
        self
    }

    /// 设置默认 TTL
    pub fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// 设置是否使用响应头 TTL
    pub fn with_use_header_ttl(mut self, use_header: bool) -> Self {
        self.use_header_ttl = use_header;
        self
    }

    /// 添加忽略路径模式
    pub fn with_ignore_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = patterns;
        self
    }

    /// 检查是否应该缓存响应
    pub fn should_cache_response(&self, status: StatusCode) -> bool {
        self.cache_status_codes.contains(&status)
    }

    /// 从响应头提取 TTL
    pub fn extract_ttl_from_headers(&self, headers: &HeaderMap) -> Option<u64> {
        if !self.use_header_ttl {
            return None;
        }

        if let Some(cache_control) = headers.get(header::CACHE_CONTROL) {
            if let Ok(value) = cache_control.to_str() {
                for directive in value.split(',') {
                    let directive = directive.trim();
                    if directive.starts_with("max-age=") {
                        if let Some(age) = directive.strip_prefix("max-age=") {
                            return age.parse().ok();
                        }
                    }
                }
            }
        }

        None
    }
}
