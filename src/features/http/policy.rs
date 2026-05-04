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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = HttpCachePolicy::default();
        assert_eq!(policy.default_ttl, 0);
        assert!(!policy.use_header_ttl);
        assert!(policy.ignore_patterns.is_empty());
        assert!(policy.key_prefix.is_empty());
    }

    #[test]
    fn test_new_policy() {
        let policy = HttpCachePolicy::new();
        assert_eq!(policy.default_ttl, 0);
    }

    #[test]
    fn test_with_cache_status_codes() {
        let policy = HttpCachePolicy::new().with_cache_status_codes(vec![StatusCode::OK, StatusCode::NOT_MODIFIED]);
        assert!(policy.should_cache_response(StatusCode::OK));
        assert!(policy.should_cache_response(StatusCode::NOT_MODIFIED));
        assert!(!policy.should_cache_response(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn test_with_default_ttl() {
        let policy = HttpCachePolicy::new().with_default_ttl(7200);
        assert_eq!(policy.default_ttl, 7200);
    }

    #[test]
    fn test_with_use_header_ttl() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        assert!(policy.use_header_ttl);
    }

    #[test]
    fn test_with_ignore_patterns() {
        let policy = HttpCachePolicy::new().with_ignore_patterns(vec!["/health".to_string(), "/metrics".to_string()]);
        assert_eq!(policy.ignore_patterns.len(), 2);
        assert_eq!(policy.ignore_patterns[0], "/health");
    }

    #[test]
    fn test_should_cache_response_ok() {
        let policy = HttpCachePolicy::new().with_cache_status_codes(vec![StatusCode::OK]);
        assert!(policy.should_cache_response(StatusCode::OK));
        assert!(!policy.should_cache_response(StatusCode::NOT_FOUND));
    }

    #[test]
    fn test_should_cache_response_empty_codes() {
        let policy = HttpCachePolicy::new();
        assert!(!policy.should_cache_response(StatusCode::OK));
    }

    #[test]
    fn test_extract_ttl_disabled() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(false);
        let mut headers = HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "max-age=300".parse().unwrap());
        assert!(policy.extract_ttl_from_headers(&headers).is_none());
    }

    #[test]
    fn test_extract_ttl_from_max_age() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        let mut headers = HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "max-age=300, public".parse().unwrap());
        let ttl = policy.extract_ttl_from_headers(&headers);
        assert_eq!(ttl, Some(300));
    }

    #[test]
    fn test_extract_ttl_no_cache_control() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        let headers = HeaderMap::new();
        assert!(policy.extract_ttl_from_headers(&headers).is_none());
    }

    #[test]
    fn test_extract_ttl_invalid_max_age() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        let mut headers = HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "max-age=not-a-number".parse().unwrap());
        assert!(policy.extract_ttl_from_headers(&headers).is_none());
    }

    #[test]
    fn test_extract_ttl_no_max_age_directive() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        let mut headers = HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "no-cache, private".parse().unwrap());
        assert!(policy.extract_ttl_from_headers(&headers).is_none());
    }

    #[test]
    fn test_extract_ttl_zero() {
        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        let mut headers = HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "max-age=0".parse().unwrap());
        let ttl = policy.extract_ttl_from_headers(&headers);
        assert_eq!(ttl, Some(0));
    }

    #[test]
    fn test_chained_builder() {
        let policy = HttpCachePolicy::new()
            .with_cache_status_codes(vec![StatusCode::OK, StatusCode::CREATED])
            .with_default_ttl(3600)
            .with_use_header_ttl(true)
            .with_ignore_patterns(vec!["/admin/*".to_string()]);

        assert!(policy.should_cache_response(StatusCode::OK));
        assert_eq!(policy.default_ttl, 3600);
        assert!(policy.use_header_ttl);
        assert_eq!(policy.ignore_patterns, vec!["/admin/*".to_string()]);
    }

    #[test]
    fn test_clone() {
        let policy = HttpCachePolicy::new().with_default_ttl(100);
        let cloned = policy.clone();
        assert_eq!(cloned.default_ttl, 100);
    }
}
