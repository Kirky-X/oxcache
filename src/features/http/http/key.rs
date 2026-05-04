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
