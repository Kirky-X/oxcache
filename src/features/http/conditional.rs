//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 条件请求处理器

use http::header::{self, HeaderMap};
use http::StatusCode;
use std::collections::HashMap;

use crate::features::http::HttpCacheResponse;

/// 条件请求处理结果
#[derive(Debug, Clone)]
pub enum ConditionalRequestResult {
    FullResponse(HttpCacheResponse),
    NotModified,
    PreconditionFailed,
}

/// HTTP 条件请求处理器
#[derive(Debug, Clone, Default)]
pub struct ConditionalRequestHandler;

impl ConditionalRequestHandler {
    /// 创建新的条件请求处理器
    pub fn new() -> Self {
        Self
    }

    /// 检查条件请求并返回处理结果
    pub fn check_conditional(
        &self,
        cached_response: &HttpCacheResponse,
        if_none_match: Option<&str>,
        if_modified_since: Option<&str>,
    ) -> ConditionalRequestResult {
        if let Some(request_etag) = if_none_match {
            if let Some(cached_etag) = &cached_response.etag {
                if request_etag == cached_etag.trim_matches('"') || request_etag == cached_etag {
                    return ConditionalRequestResult::NotModified;
                }
            }
        }

        if let Some(imf) = if_modified_since {
            if let Ok(modified_since) = chrono::DateTime::parse_from_rfc2822(imf) {
                if modified_since >= cached_response.cached_at {
                    return ConditionalRequestResult::NotModified;
                }
            }
        }

        ConditionalRequestResult::FullResponse(cached_response.clone())
    }

    /// 从缓存响应生成 304 Not Modified 响应
    pub fn create_not_modified_response(&self, cached_response: &HttpCacheResponse) -> HttpCacheResponse {
        let mut headers = HashMap::new();

        if let Some(etag) = &cached_response.etag {
            headers.insert("ETag".to_string(), etag.clone());
        }

        if let Some(lm) = &cached_response.last_modified {
            headers.insert("Last-Modified".to_string(), lm.clone());
        }

        let now = chrono::Utc::now();
        headers.insert("Date".to_string(), now.format("%a, %d %b %Y %H:%M:%S GMT").to_string());

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

    /// 生成强 ETag (使用 SHA-256)
    #[cfg(feature = "sha2")]
    pub fn generate_strong_etag(&self, body: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(body);
        let result = hasher.finalize();
        format!("\"{:x}\"", result)
    }

    /// 生成弱 ETag (使用 SHA-256)
    #[cfg(feature = "sha2")]
    pub fn generate_weak_etag(&self, body: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(body);
        let result = hasher.finalize();
        format!("W/\"{:x}\"", result)
    }

    /// 生成强 ETag (使用 MD5)
    #[cfg(not(feature = "sha2"))]
    pub fn generate_strong_etag(&self, body: &[u8]) -> String {
        let digest = md5::compute(body);
        format!("\"{:x}\"", digest)
    }

    /// 生成弱 ETag (使用 MD5)
    #[cfg(not(feature = "sha2"))]
    pub fn generate_weak_etag(&self, body: &[u8]) -> String {
        let digest = md5::compute(body);
        format!("W/\"{:x}\"", digest)
    }
}
