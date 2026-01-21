//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 缓存模块
//!
//! 提供 HTTP 响应缓存适配层、键生成、条件请求处理和 Axum 中间件。

use http::header::{self, HeaderMap};
use http::{Method, StatusCode, Uri, Version};
use md5;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub mod axum;

pub use self::axum::{CacheMiddlewareConfig, CacheMiddlewareState};
// Note: HttpCachePolicy, HttpCacheResponse, HttpRequest, HttpCacheKeyGenerator, HttpCacheAdapter
// are defined in this module and don't need to be re-exported

/// HTTP 缓存响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpCacheResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub cached_at: chrono::DateTime<chrono::Utc>,
    pub ttl: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// HTTP 缓存键生成器
#[derive(Debug, Clone, Default)]
pub struct HttpCacheKeyGenerator {
    include_query: bool,
    exclude_headers: Vec<String>,
    include_method: bool,
    include_version: bool,
}

impl HttpCacheKeyGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置是否包含查询参数
    pub fn with_include_query(mut self, include: bool) -> Self {
        self.include_query = include;
        self
    }

    /// 设置要排除的请求头
    pub fn with_exclude_headers(mut self, headers: Vec<String>) -> Self {
        self.exclude_headers = headers;
        self
    }

    /// 设置是否包含 HTTP 方法
    pub fn with_include_method(mut self, include: bool) -> Self {
        self.include_method = include;
        self
    }

    /// 设置是否包含 HTTP 版本
    pub fn with_include_version(mut self, include: bool) -> Self {
        self.include_version = include;
        self
    }

    /// 生成缓存键
    pub fn generate_key(&self, request: &HttpRequest) -> String {
        let mut key_parts = Vec::new();

        if self.include_method {
            key_parts.push(request.method.to_string());
        }

        key_parts.push(request.uri.path().to_string());

        if self.include_query && request.uri.query().is_some() {
            key_parts.push(request.uri.query().unwrap_or("").to_string());
        }

        if self.include_version {
            key_parts.push(format!("{:?}", request.version));
        }

        // 考虑重要的请求头
        for (name, value) in &request.headers {
            if !self.exclude_headers.iter().any(|h| name.as_str() == h)
                && (name == header::ACCEPT_ENCODING
                    || name == header::VARY
                    || name == header::AUTHORIZATION)
            {
                key_parts.push(format!("{}:{}", name, value.to_str().unwrap_or("")));
            }
        }

        // 使用哈希以保持键长度可控
        let key_string = key_parts.join(":");
        let hash = md5::compute(&key_string);
        format!("{:x}", hash)
    }
}

/// 简化的 HTTP 请求
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub uri: Uri,
    pub version: Version,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

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

/// HTTP 缓存适配器
#[async_trait::async_trait]
pub trait HttpCacheAdapter {
    /// 获取缓存的响应
    async fn get_response(
        &self,
        key: &str,
    ) -> Result<Option<HttpCacheResponse>, crate::error::CacheError>;

    /// 设置缓存响应
    async fn set_response(
        &self,
        key: &str,
        response: &HttpCacheResponse,
    ) -> Result<(), crate::error::CacheError>;

    /// 删除缓存的响应
    async fn delete_response(&self, key: &str) -> Result<bool, crate::error::CacheError>;

    /// 按模式失效
    async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64, crate::error::CacheError>;

    /// 按路径模式失效
    ///
    /// # 参数
    ///
    /// * `path_pattern` - 路径匹配模式，支持 glob 风格匹配
    ///                   例如: "/api/users/*", "/api/products/**", "/api/*/detail"
    ///
    /// # 返回值
    ///
    /// 返回失效的缓存项数量
    async fn invalidate_by_path_pattern(
        &self,
        path_pattern: &str,
    ) -> Result<u64, crate::error::CacheError> {
        // 默认实现：使用通用的模式匹配
        self.invalidate_by_pattern(path_pattern).await
    }

    /// 批量获取
    async fn get_responses(
        &self,
        keys: &[&str],
    ) -> Result<HashMap<String, HttpCacheResponse>, crate::error::CacheError>;
}

/// 路径模式匹配器
///
/// 支持 glob 风格的路径模式匹配。
#[derive(Debug, Clone, Default)]
pub struct PathPatternMatcher;

impl PathPatternMatcher {
    /// 创建新的模式匹配器
    pub fn new() -> Self {
        Self
    }

    /// 检查路径是否匹配模式
    ///
    /// # 参数
    ///
    /// * `path` - 要检查的路径
    /// * `pattern` - 匹配模式
    ///
    /// # 返回值
    ///
    /// 如果匹配返回 true，否则返回 false
    pub fn matches(&self, path: &str, pattern: &str) -> bool {
        // 处理通配符
        if pattern.contains("**") {
            // 处理 **（匹配任意数量的目录）
            self.matches_double_star(path, pattern)
        } else if pattern.contains('*') {
            // 处理 *（匹配单个目录段）
            self.matches_single_star(path, pattern)
        } else {
            // 精确匹配
            path == pattern
        }
    }

    /// 匹配单个 *（匹配单个目录段）
    fn matches_single_star(&self, path: &str, pattern: &str) -> bool {
        let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();

        if path_parts.len() != pattern_parts.len() {
            return false;
        }

        for (path_part, pattern_part) in path_parts.iter().zip(pattern_parts.iter()) {
            if !self.matches_segment(pattern_part, path_part) {
                return false;
            }
        }

        true
    }

    /// 匹配 **（匹配任意数量的目录）
    fn matches_double_star(&self, path: &str, pattern: &str) -> bool {
        // 简化实现：将 ** 替换为 .* 并使用正则表达式
        let regex_pattern = pattern
            .replace("**", "§§§") // 临时标记
            .replace("*", "[^/]*") // * 匹配非 / 字符
            .replace("§§§", ".*"); // ** 匹配任意字符

        if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
            re.is_match(path)
        } else {
            false
        }
    }

    /// 匹配单个段落的通配符
    fn matches_segment(&self, pattern: &str, segment: &str) -> bool {
        // 处理模式中的 *
        let regex_pattern: String = pattern
            .chars()
            .map(|c| {
                match c {
                    '*' => ".*".to_string(), // * 匹配任意字符序列
                    c => regex::escape(&c.to_string()),
                }
            })
            .collect();

        if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
            re.is_match(segment)
        } else {
            pattern == segment
        }
    }
}

/// 条件请求处理结果
#[derive(Debug, Clone)]
pub enum ConditionalRequestResult {
    FullResponse(HttpCacheResponse),
    NotModified,
    PreconditionFailed,
}

/// HTTP 条件请求处理器
///
/// 处理 If-None-Match 和 If-Modified-Since 请求头，
/// 返回 304 Not Modified 或完整的缓存响应。
#[derive(Debug, Clone, Default)]
pub struct ConditionalRequestHandler;

impl ConditionalRequestHandler {
    /// 创建新的条件请求处理器
    pub fn new() -> Self {
        Self
    }

    /// 检查条件请求并返回处理结果
    ///
    /// # 参数
    ///
    /// * `cached_response` - 缓存的响应
    /// * `if_none_match` - 请求的 If-None-Match 头
    /// * `if_modified_since` - 请求的 If-Modified-Since 头
    ///
    /// # 返回值
    ///
    /// 返回条件请求处理结果
    pub fn check_conditional(
        &self,
        cached_response: &HttpCacheResponse,
        if_none_match: Option<&str>,
        if_modified_since: Option<&str>,
    ) -> ConditionalRequestResult {
        // 检查 If-None-Match
        if let Some(request_etag) = if_none_match {
            if let Some(cached_etag) = &cached_response.etag {
                if request_etag == cached_etag.trim_matches('"') || request_etag == cached_etag {
                    return ConditionalRequestResult::NotModified;
                }
            }
        }

        // 检查 If-Modified-Since
        if let Some(imf) = if_modified_since {
            // 解析 If-Modified-Since 头
            if let Ok(modified_since) = chrono::DateTime::parse_from_rfc2822(imf) {
                let cached_time = cached_response.cached_at;
                if modified_since >= cached_time {
                    return ConditionalRequestResult::NotModified;
                }
            } else if let Ok(modified_since) = chrono::DateTime::parse_from_rfc2822(&format!(
                "{}, 01 Jan 1970 00:00:00 GMT",
                imf.trim()
            )) {
                // 尝试其他格式
                if modified_since >= cached_response.cached_at {
                    return ConditionalRequestResult::NotModified;
                }
            }
        }

        ConditionalRequestResult::FullResponse(cached_response.clone())
    }

    /// 从缓存响应生成 304 Not Modified 响应
    ///
    /// # 参数
    ///
    /// * `cached_response` - 缓存的响应
    ///
    /// # 返回值
    ///
    /// 返回 304 响应的 HttpCacheResponse
    pub fn create_not_modified_response(
        &self,
        cached_response: &HttpCacheResponse,
    ) -> HttpCacheResponse {
        let mut headers = HashMap::new();

        // 复制 ETag
        if let Some(etag) = &cached_response.etag {
            headers.insert("ETag".to_string(), etag.clone());
        }

        // 复制 Last-Modified
        if let Some(lm) = &cached_response.last_modified {
            headers.insert("Last-Modified".to_string(), lm.clone());
        }

        // 添加 Date 头
        let now = chrono::Utc::now();
        headers.insert(
            "Date".to_string(),
            now.format("%a, %d %b %Y %H:%M:%S GMT").to_string(),
        );

        HttpCacheResponse {
            status: StatusCode::NOT_MODIFIED.as_u16(),
            headers,
            body: Vec::new(),
            cached_at: now,
            ttl: cached_response.ttl,
            etag: cached_response.etag.clone(),
            last_modified: cached_response.last_modified.clone(),
        }
    }

    /// 从请求中提取条件请求头
    ///
    /// # 参数
    ///
    /// * `headers` - 请求头
    ///
    /// # 返回值
    ///
    /// 返回提取的 If-None-Match 和 If-Modified-Since 值
    pub fn extract_conditionals(&self, headers: &HeaderMap) -> (Option<String>, Option<String>) {
        let if_none_match = headers
            .get(http::header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let if_modified_since = headers
            .get(http::header::IF_MODIFIED_SINCE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        (if_none_match, if_modified_since)
    }

    /// 生成强 ETag
    pub fn generate_strong_etag(&self, body: &[u8]) -> String {
        let digest = md5::compute(body);
        format!("\"{:x}\"", digest)
    }

    /// 生成弱 ETag
    pub fn generate_weak_etag(&self, body: &[u8]) -> String {
        let digest = md5::compute(body);
        format!("W/\"{:x}\"", digest)
    }
}

/// 缓存标签管理器
///
/// 用于管理和失效带有特定标签的缓存项。
#[derive(Clone)]
pub struct CacheTagManager {
    /// 标签到缓存键的映射
    tag_mapping: Arc<dashmap::DashMap<String, dashmap::DashSet<String>>>,
    /// 缓存适配器
    adapter: Arc<dyn HttpCacheAdapter + Send + Sync>,
}

impl CacheTagManager {
    /// 创建新的缓存标签管理器
    pub fn new(adapter: Arc<dyn HttpCacheAdapter + Send + Sync>) -> Self {
        Self {
            tag_mapping: Arc::new(dashmap::DashMap::new()),
            adapter,
        }
    }

    /// 为缓存项添加标签
    ///
    /// # 参数
    ///
    /// * `cache_key` - 缓存键
    /// * `tags` - 要添加的标签列表
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn add_tags(
        &self,
        cache_key: &str,
        tags: &[&str],
    ) -> Result<(), crate::error::CacheError> {
        for tag in tags {
            let tag_set = self.tag_mapping.entry(tag.to_string()).or_default();
            tag_set.insert(cache_key.to_string());
        }
        Ok(())
    }

    /// 使具有指定标签的所有缓存项失效
    ///
    /// # 参数
    ///
    /// * `tag` - 要失效的标签
    ///
    /// # 返回值
    ///
    /// 返回失效的缓存项数量
    pub async fn invalidate_by_tag(&self, tag: &str) -> Result<u64, crate::error::CacheError> {
        if let Some((_, keys)) = self.tag_mapping.remove(tag) {
            let count = keys.len() as u64;
            // 删除所有关联的缓存响应
            for key in keys {
                let _ = self.adapter.delete_response(&key).await;
            }
            return Ok(count);
        }
        Ok(0)
    }

    /// 使匹配模式的所有缓存项失效
    ///
    /// # 参数
    ///
    /// * `pattern` - 匹配模式（支持 glob 风格匹配）
    ///
    /// # 返回值
    ///
    /// 返回失效的缓存项数量
    pub async fn invalidate_by_pattern(
        &self,
        pattern: &str,
    ) -> Result<u64, crate::error::CacheError> {
        self.adapter.invalidate_by_pattern(pattern).await
    }

    /// 清除所有标签映射
    pub fn clear(&self) {
        self.tag_mapping.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_cache_key_generator() {
        let generator = HttpCacheKeyGenerator::new()
            .with_include_method(true)
            .with_include_query(true);

        let request = HttpRequest {
            method: Method::GET,
            uri: "/api/users?id=123".parse().unwrap(),
            version: Version::HTTP_11,
            headers: HeaderMap::new(),
            body: vec![],
        };

        let key = generator.generate_key(&request);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_http_cache_policy_should_cache() {
        let policy = HttpCachePolicy::new()
            .with_cache_status_codes(vec![StatusCode::OK, StatusCode::NOT_FOUND]);

        assert!(policy.should_cache_response(StatusCode::OK));
        assert!(policy.should_cache_response(StatusCode::NOT_FOUND));
        assert!(!policy.should_cache_response(StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn test_http_cache_policy_extract_ttl() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "max-age=3600".parse().unwrap());

        let policy = HttpCachePolicy::new().with_use_header_ttl(true);
        let ttl = policy.extract_ttl_from_headers(&headers);
        assert_eq!(ttl, Some(3600));
    }

    #[test]
    fn test_http_cache_response() {
        let response = HttpCacheResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())]
                .into_iter()
                .collect(),
            body: vec![1, 2, 3],
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("abc123".to_string()),
            last_modified: None,
        };

        assert_eq!(response.status, 200);
        assert_eq!(response.ttl, Some(3600));
    }

    #[test]
    fn test_conditional_request_handler_etag_match() {
        let handler = ConditionalRequestHandler::new();
        let cached = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: vec![1, 2, 3],
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"abc123\"".to_string()),
            last_modified: None,
        };

        // ETag 匹配，应该返回 NotModified
        let result = handler.check_conditional(&cached, Some("\"abc123\""), None);
        match result {
            ConditionalRequestResult::NotModified => {}
            _ => panic!("Expected NotModified"),
        }
    }

    #[test]
    fn test_conditional_request_handler_etag_mismatch() {
        let handler = ConditionalRequestHandler::new();
        let cached = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: vec![1, 2, 3],
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"abc123\"".to_string()),
            last_modified: None,
        };

        // ETag 不匹配，应该返回 FullResponse
        let result = handler.check_conditional(&cached, Some("\"different\""), None);
        match result {
            ConditionalRequestResult::FullResponse(_) => {}
            _ => panic!("Expected FullResponse"),
        }
    }

    #[test]
    fn test_conditional_request_handler_if_modified_since() {
        let handler = ConditionalRequestHandler::new();
        let old_time = chrono::DateTime::from_timestamp(1000000, 0).expect("Invalid timestamp");
        let cached = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: vec![1, 2, 3],
            cached_at: old_time,
            ttl: Some(3600),
            etag: None,
            last_modified: Some("Mon, 01 Jan 2001 00:00:00 GMT".to_string()),
        };

        // 请求时间晚于缓存时间，应该返回 NotModified
        let recent_time = "Tue, 01 Jan 2002 00:00:00 GMT";
        let result = handler.check_conditional(&cached, None, Some(recent_time));
        match result {
            ConditionalRequestResult::NotModified => {}
            _ => panic!("Expected NotModified"),
        }
    }

    #[test]
    fn test_generate_etag() {
        let handler = ConditionalRequestHandler::new();
        let body = b"hello world";

        let strong = handler.generate_strong_etag(body);
        assert!(strong.starts_with('"'));
        assert!(strong.ends_with('"'));

        let weak = handler.generate_weak_etag(body);
        assert!(weak.starts_with("W/\""));
        assert!(weak.ends_with('"'));
    }

    #[test]
    fn test_create_not_modified_response() {
        let handler = ConditionalRequestHandler::new();
        let cached = HttpCacheResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())]
                .into_iter()
                .collect(),
            body: vec![1, 2, 3],
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"abc123\"".to_string()),
            last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
        };

        let not_modified = handler.create_not_modified_response(&cached);

        assert_eq!(not_modified.status, 304);
        assert!(not_modified.body.is_empty());
        assert_eq!(not_modified.etag, cached.etag);
        assert_eq!(not_modified.last_modified, cached.last_modified);
    }

    #[test]
    fn test_path_pattern_matcher_exact() {
        let matcher = PathPatternMatcher::new();
        assert!(matcher.matches("/api/users", "/api/users"));
        assert!(!matcher.matches("/api/users", "/api/products"));
    }

    #[test]
    fn test_path_pattern_matcher_single_star() {
        let matcher = PathPatternMatcher::new();
        assert!(matcher.matches("/api/users/123", "/api/users/*"));
        assert!(matcher.matches("/api/users/abc", "/api/users/*"));
        assert!(!matcher.matches("/api/users/123/profile", "/api/users/*"));
    }

    #[test]
    fn test_path_pattern_matcher_double_star() {
        let matcher = PathPatternMatcher::new();
        assert!(matcher.matches("/api/users/123", "/api/**"));
        assert!(matcher.matches("/api/users/123/profile", "/api/**"));
        assert!(matcher.matches("/api/a/b/c/d", "/api/**"));
        assert!(!matcher.matches("/other/users", "/api/**"));
    }

    #[test]
    fn test_path_pattern_matcher_mixed() {
        let matcher = PathPatternMatcher::new();
        assert!(matcher.matches("/api/users/123", "/api/users/*"));
        assert!(matcher.matches("/api/products/456", "/api/products/*"));
        assert!(matcher.matches("/api/users/123/profile", "/api/users/*/profile"));
        assert!(!matcher.matches("/api/users/123/extra", "/api/users/*/profile"));
    }
}
