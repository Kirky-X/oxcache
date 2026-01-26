#![allow(deprecated)]
// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// HTTP 缓存集成测试
//
// 测试 HTTP 缓存功能，包括 ETags、条件请求等。

#[cfg(all(feature = "http-cache", feature = "redis"))]
#[cfg(feature = "redis")]
mod tests {
    use oxcache::http::HttpCacheResponse;
    use std::collections::HashMap;

    // ============================================================================
    // HTTP 响应结构测试
    // ============================================================================

    #[test]
    fn test_http_cache_response_basic() {
        let response = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: "Hello, World!".as_bytes().to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: None,
            etag: None,
            last_modified: None,
        };

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"Hello, World!".to_vec());
        assert!(response.etag.is_none());
    }

    #[test]
    fn test_http_cache_response_with_etag() {
        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"abc123\"".to_string());

        let response = HttpCacheResponse {
            status: 200,
            headers,
            body: "Data".as_bytes().to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"abc123\"".to_string()),
            last_modified: None,
        };

        assert_eq!(response.etag, Some("\"abc123\"".to_string()));
        assert!(response.headers.contains_key("ETag"));
    }

    #[test]
    fn test_http_cache_response_with_multiple_headers() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Cache-Control".to_string(), "max-age=3600".to_string());
        headers.insert(
            "Last-Modified".to_string(),
            "Wed, 21 Oct 2015 07:28:00 GMT".to_string(),
        );

        let response = HttpCacheResponse {
            status: 200,
            headers,
            body: "{}".as_bytes().to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: None,
            last_modified: None,
        };

        assert_eq!(response.headers.len(), 3);
    }

    #[test]
    fn test_http_cache_response_different_status_codes() {
        let status_codes = vec![200, 201, 204, 304, 404, 500];

        for status in status_codes {
            let response = HttpCacheResponse {
                status,
                headers: HashMap::new(),
                body: format!("Response {}", status).as_bytes().to_vec(),
                cached_at: chrono::Utc::now(),
                ttl: None,
                etag: None,
                last_modified: None,
            };

            assert_eq!(response.status, status);
        }
    }

    #[test]
    fn test_http_cache_response_serialization() {
        let response = HttpCacheResponse {
            status: 200,
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Content-Type".to_string(), "application/json".to_string());
                headers
            },
            body: r#"{"message": "test"}"#.as_bytes().to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"v1\"".to_string()),
            last_modified: None,
        };

        // 验证可以序列化
        let serialized = serde_json::to_string(&response);
        assert!(serialized.is_ok());

        // 验证可以反序列化
        let deserialized: Result<HttpCacheResponse, _> = serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());

        let deserialized_response = deserialized.unwrap();
        assert_eq!(deserialized_response.status, 200);
        assert_eq!(deserialized_response.etag, Some("\"v1\"".to_string()));
    }
}
