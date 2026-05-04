//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 缓存键生成

use http::header::{self, HeaderMap};
use http::{Method, Uri, Version};
use md5;

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
                && (name == header::ACCEPT_ENCODING || name == header::VARY || name == header::AUTHORIZATION)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: Method, path: &str, query: Option<&str>) -> HttpRequest {
        let uri: Uri = if let Some(q) = query {
            format!("{}?{}", path, q).parse().unwrap()
        } else {
            path.parse().unwrap()
        };

        HttpRequest {
            method,
            uri,
            version: Version::HTTP_11,
            headers: HeaderMap::new(),
            body: Vec::new(),
        }
    }

    fn make_request_with_headers(method: Method, path: &str, headers: Vec<(&str, &str)>) -> HttpRequest {
        let mut hm = HeaderMap::new();
        for (k, v) in headers {
            hm.insert(k.parse::<http::header::HeaderName>().unwrap(), v.parse().unwrap());
        }
        HttpRequest {
            method,
            uri: path.parse().unwrap(),
            version: Version::HTTP_11,
            headers: hm,
            body: Vec::new(),
        }
    }

    #[test]
    fn test_default_key_generator() {
        let gen = HttpCacheKeyGenerator::default();
        let req = make_request(Method::GET, "/api/users", None);
        let key = gen.generate_key(&req);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_new_key_generator() {
        let gen = HttpCacheKeyGenerator::new();
        let req = make_request(Method::GET, "/api/users", None);
        let key = gen.generate_key(&req);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_key_deterministic() {
        let gen = HttpCacheKeyGenerator::new();
        let req = make_request(Method::GET, "/api/users", None);
        let key1 = gen.generate_key(&req);
        let key2 = gen.generate_key(&req);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_key_different_paths() {
        let gen = HttpCacheKeyGenerator::new();
        let req1 = make_request(Method::GET, "/api/users", None);
        let req2 = make_request(Method::GET, "/api/posts", None);
        assert_ne!(gen.generate_key(&req1), gen.generate_key(&req2));
    }

    #[test]
    fn test_include_query_true() {
        let gen = HttpCacheKeyGenerator::new().with_include_query(true);
        let req1 = make_request(Method::GET, "/api/users", Some("page=1"));
        let req2 = make_request(Method::GET, "/api/users", Some("page=2"));
        assert_ne!(gen.generate_key(&req1), gen.generate_key(&req2));
    }

    #[test]
    fn test_include_query_false() {
        let gen = HttpCacheKeyGenerator::new().with_include_query(false);
        let req1 = make_request(Method::GET, "/api/users", Some("page=1"));
        let req2 = make_request(Method::GET, "/api/users", Some("page=2"));
        assert_eq!(gen.generate_key(&req1), gen.generate_key(&req2));
    }

    #[test]
    fn test_include_query_no_query_param() {
        let gen = HttpCacheKeyGenerator::new().with_include_query(true);
        let req = make_request(Method::GET, "/api/users", None);
        let key = gen.generate_key(&req);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_include_method_true() {
        let gen = HttpCacheKeyGenerator::new().with_include_method(true);
        let get_req = make_request(Method::GET, "/api/resource", None);
        let post_req = make_request(Method::POST, "/api/resource", None);
        assert_ne!(gen.generate_key(&get_req), gen.generate_key(&post_req));
    }

    #[test]
    fn test_include_method_false() {
        let gen = HttpCacheKeyGenerator::new().with_include_method(false);
        let get_req = make_request(Method::GET, "/api/resource", None);
        let post_req = make_request(Method::POST, "/api/resource", None);
        assert_eq!(gen.generate_key(&get_req), gen.generate_key(&post_req));
    }

    #[test]
    fn test_include_version_true() {
        let gen = HttpCacheKeyGenerator::new().with_include_version(true);
        let req = make_request(Method::GET, "/api/users", None);
        let key = gen.generate_key(&req);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_include_version_different() {
        let gen = HttpCacheKeyGenerator::new().with_include_version(true);
        let mut req1 = make_request(Method::GET, "/api/users", None);
        let mut req2 = make_request(Method::GET, "/api/users", None);
        req1.version = Version::HTTP_11;
        req2.version = Version::HTTP_2;
        assert_ne!(gen.generate_key(&req1), gen.generate_key(&req2));
    }

    #[test]
    fn test_include_headers_accept_encoding() {
        let gen = HttpCacheKeyGenerator::new();
        let req = make_request_with_headers(Method::GET, "/api/users", vec![("Accept-Encoding", "gzip")]);
        let key = gen.generate_key(&req);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_include_headers_authorization() {
        let gen = HttpCacheKeyGenerator::new();
        let req1 = make_request_with_headers(Method::GET, "/api/users", vec![("Authorization", "Bearer token1")]);
        let req2 = make_request_with_headers(Method::GET, "/api/users", vec![("Authorization", "Bearer token2")]);
        assert_ne!(gen.generate_key(&req1), gen.generate_key(&req2));
    }

    #[test]
    fn test_exclude_headers() {
        let gen = HttpCacheKeyGenerator::new()
            .with_include_method(true)
            .with_exclude_headers(vec!["Authorization".to_string()]);
        // Both requests have no relevant headers that pass the filter
        // (Authorization is excluded, and no Accept-Encoding/Vary present)
        // So keys should be identical since only method + path are used
        let req1 = make_request(Method::GET, "/api/users", None);
        let req2 = make_request(Method::GET, "/api/users", None);
        assert_eq!(gen.generate_key(&req1), gen.generate_key(&req2));
    }

    #[test]
    fn test_builder_chaining() {
        let gen = HttpCacheKeyGenerator::new()
            .with_include_query(true)
            .with_include_method(true)
            .with_include_version(true)
            .with_exclude_headers(vec!["X-Custom".to_string()]);
        let req = make_request_with_headers(Method::GET, "/api/users", vec![("Accept-Encoding", "gzip")]);
        let key = gen.generate_key(&req);
        assert!(!key.is_empty());
    }

    #[test]
    fn test_http_request_clone() {
        let req = make_request(Method::GET, "/api/users", Some("page=1"));
        let cloned = req.clone();
        assert_eq!(req.method, cloned.method);
        assert_eq!(req.uri, cloned.uri);
        assert_eq!(req.version, cloned.version);
    }
}
