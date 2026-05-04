//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! HTTP 条件请求处理器

use http::header::HeaderMap;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_cached_response(etag: Option<String>, last_modified: Option<String>) -> HttpCacheResponse {
        HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: b"Hello, World!".to_vec(),
            cached_at: chrono::DateTime::parse_from_rfc2822("Mon, 01 Jan 2024 12:00:00 +0000")
                .unwrap()
                .with_timezone(&chrono::Utc),
            ttl: Some(3600),
            etag,
            last_modified,
        }
    }

    #[test]
    fn test_check_conditional_etag_match_with_quotes() {
        // cached_etag = "\"abc123\"", request_etag = "\"abc123\""
        // Logic: request_etag == cached_etag.trim_matches('"') || request_etag == cached_etag
        // trim_matches('"') on "\"abc123\"" -> "abc123"
        // So: "\"abc123\"" == "abc123" -> false, "\"abc123\"" == "\"abc123\"" -> true
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(Some("\"abc123\"".to_string()), None);
        let result = handler.check_conditional(&response, Some("\"abc123\""), None);
        assert!(matches!(result, ConditionalRequestResult::NotModified));
    }

    #[test]
    fn test_check_conditional_etag_match_without_quotes() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(Some("abc123".to_string()), None);
        let result = handler.check_conditional(&response, Some("abc123"), None);
        assert!(matches!(result, ConditionalRequestResult::NotModified));
    }

    #[test]
    fn test_check_conditional_etag_no_match() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(Some("abc123".to_string()), None);
        let result = handler.check_conditional(&response, Some("\"xyz789\""), None);
        assert!(matches!(result, ConditionalRequestResult::FullResponse(_)));
    }

    #[test]
    fn test_check_conditional_etag_cached_has_none() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(None, None);
        let result = handler.check_conditional(&response, Some("abc123"), None);
        assert!(matches!(result, ConditionalRequestResult::FullResponse(_)));
    }

    #[test]
    fn test_check_conditional_no_conditionals_returns_full() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(Some("abc123".to_string()), None);
        let result = handler.check_conditional(&response, None, None);
        match result {
            ConditionalRequestResult::FullResponse(resp) => assert_eq!(resp.etag, Some("abc123".to_string())),
            _ => panic!("Expected FullResponse"),
        }
    }

    #[test]
    fn test_check_conditional_if_modified_since_not_modified() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(None, None);
        let result = handler.check_conditional(&response, None, Some("Tue, 02 Jan 2024 12:00:00 +0000"));
        assert!(matches!(result, ConditionalRequestResult::NotModified));
    }

    #[test]
    fn test_check_conditional_if_modified_since_still_modified() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(None, None);
        let result = handler.check_conditional(&response, None, Some("Sun, 31 Dec 2023 12:00:00 +0000"));
        assert!(matches!(result, ConditionalRequestResult::FullResponse(_)));
    }

    #[test]
    fn test_check_conditional_if_modified_since_invalid_date() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(None, None);
        let result = handler.check_conditional(&response, None, Some("not-a-date"));
        assert!(matches!(result, ConditionalRequestResult::FullResponse(_)));
    }

    #[test]
    fn test_check_conditional_etag_takes_precedence() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(Some("abc123".to_string()), None);
        let result = handler.check_conditional(&response, Some("abc123"), Some("Sun, 31 Dec 2023 12:00:00 +0000"));
        assert!(matches!(result, ConditionalRequestResult::NotModified));
    }

    #[test]
    fn test_create_not_modified_response_with_etag_and_last_modified() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(
            Some("\"myetag\"".to_string()),
            Some("Mon, 01 Jan 2024 12:00:00 GMT".to_string()),
        );
        let result = handler.create_not_modified_response(&response);
        assert_eq!(result.status, StatusCode::NOT_MODIFIED.as_u16());
        assert_eq!(result.headers.get("ETag"), Some(&"\"myetag\"".to_string()));
        assert_eq!(
            result.headers.get("Last-Modified"),
            Some(&"Mon, 01 Jan 2024 12:00:00 GMT".to_string())
        );
        assert!(result.headers.contains_key("Date"));
        assert!(result.body.is_empty());
    }

    #[test]
    fn test_create_not_modified_response_without_etag_or_last_modified() {
        let handler = ConditionalRequestHandler::new();
        let response = make_cached_response(None, None);
        let result = handler.create_not_modified_response(&response);
        assert_eq!(result.status, StatusCode::NOT_MODIFIED.as_u16());
        assert!(!result.headers.contains_key("ETag"));
        assert!(!result.headers.contains_key("Last-Modified"));
        assert!(result.headers.contains_key("Date"));
        assert!(result.body.is_empty());
    }

    #[test]
    fn test_extract_conditionals_both_present() {
        let handler = ConditionalRequestHandler::new();
        let mut headers = HeaderMap::new();
        headers.insert(http::header::IF_NONE_MATCH, "\"abc123\"".parse().unwrap());
        headers.insert(
            http::header::IF_MODIFIED_SINCE,
            "Mon, 01 Jan 2024 12:00:00 GMT".parse().unwrap(),
        );
        let (if_none_match, if_modified_since) = handler.extract_conditionals(&headers);
        assert_eq!(if_none_match, Some("\"abc123\"".to_string()));
        assert_eq!(if_modified_since, Some("Mon, 01 Jan 2024 12:00:00 GMT".to_string()));
    }

    #[test]
    fn test_extract_conditionals_neither_present() {
        let handler = ConditionalRequestHandler::new();
        let headers = HeaderMap::new();
        let (if_none_match, if_modified_since) = handler.extract_conditionals(&headers);
        assert!(if_none_match.is_none());
        assert!(if_modified_since.is_none());
    }

    #[test]
    fn test_extract_conditionals_only_if_none_match() {
        let handler = ConditionalRequestHandler::new();
        let mut headers = HeaderMap::new();
        headers.insert(http::header::IF_NONE_MATCH, "\"xyz\"".parse().unwrap());
        let (if_none_match, if_modified_since) = handler.extract_conditionals(&headers);
        assert_eq!(if_none_match, Some("\"xyz\"".to_string()));
        assert!(if_modified_since.is_none());
    }

    #[test]
    #[cfg(not(feature = "sha2"))]
    fn test_generate_strong_etag_md5() {
        let handler = ConditionalRequestHandler::new();
        let etag = handler.generate_strong_etag(b"hello");
        assert!(etag.starts_with('"'));
        assert!(etag.ends_with('"'));
        assert_eq!(etag.len(), 34);
    }

    #[test]
    #[cfg(not(feature = "sha2"))]
    fn test_generate_weak_etag_md5() {
        let handler = ConditionalRequestHandler::new();
        let etag = handler.generate_weak_etag(b"hello");
        assert!(etag.starts_with("W/\""));
        assert!(etag.ends_with('"'));
    }

    #[test]
    #[cfg(not(feature = "sha2"))]
    fn test_generate_strong_etag_empty_body() {
        let handler = ConditionalRequestHandler::new();
        let etag = handler.generate_strong_etag(b"");
        assert!(!etag.is_empty());
    }

    #[test]
    #[cfg(not(feature = "sha2"))]
    fn test_generate_strong_etag_deterministic() {
        let handler = ConditionalRequestHandler::new();
        let etag1 = handler.generate_strong_etag(b"same content");
        let etag2 = handler.generate_strong_etag(b"same content");
        assert_eq!(etag1, etag2);
    }

    #[test]
    #[cfg(not(feature = "sha2"))]
    fn test_generate_strong_etag_different_content() {
        let handler = ConditionalRequestHandler::new();
        let etag1 = handler.generate_strong_etag(b"content1");
        let etag2 = handler.generate_strong_etag(b"content2");
        assert_ne!(etag1, etag2);
    }
}
