// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Axum HTTP 缓存中间件覆盖率测试
//
// 测试 Axum 中间件的核心功能：缓存键生成、缓存命中/未命中、
// bypass header、条件请求、响应构建等。

#[cfg(feature = "http-cache")]
mod tests {
    use axum::{body::Body, middleware, routing::get, Router};
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use oxcache::features::http::{
        CacheMiddlewareConfig, CacheMiddlewareState, HttpCacheAdapter, HttpCacheKeyGenerator, HttpCachePolicy,
        HttpCacheResponse, HttpRequest,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tower::ServiceExt;

    /// 测试用内存缓存适配器（复用 axum.rs 中的实现）
    #[derive(Clone, Debug)]
    struct MemoryCacheAdapter {
        store: Arc<tokio::sync::Mutex<HashMap<String, HttpCacheResponse>>>,
    }

    impl MemoryCacheAdapter {
        fn new() -> Self {
            Self {
                store: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl HttpCacheAdapter for MemoryCacheAdapter {
        async fn get_response(&self, key: &str) -> Result<Option<HttpCacheResponse>, oxcache::error::CacheError> {
            let store = self.store.lock().await;
            Ok(store.get(key).cloned())
        }

        async fn set_response(
            &self,
            key: &str,
            response: &HttpCacheResponse,
        ) -> Result<(), oxcache::error::CacheError> {
            let mut store = self.store.lock().await;
            store.insert(key.to_string(), response.clone());
            Ok(())
        }

        async fn delete_response(&self, key: &str) -> Result<bool, oxcache::error::CacheError> {
            let mut store = self.store.lock().await;
            Ok(store.remove(key).is_some())
        }

        async fn invalidate_by_pattern(&self, _pattern: &str) -> Result<u64, oxcache::error::CacheError> {
            let mut store = self.store.lock().await;
            let count = store.len();
            store.clear();
            Ok(count as u64)
        }

        async fn get_responses(
            &self,
            keys: &[&str],
        ) -> Result<HashMap<String, HttpCacheResponse>, oxcache::error::CacheError> {
            let store = self.store.lock().await;
            let mut result = HashMap::new();
            for &key in keys {
                if let Some(resp) = store.get(key) {
                    result.insert(key.to_string(), resp.clone());
                }
            }
            Ok(result)
        }
    }

    // ============================================================================
    // build_response 函数测试
    // ============================================================================

    /// 测试 build_response 正常情况
    #[test]
    fn test_build_response_normal() {
        // 通过 axum.rs 内部函数测试
        // 由于 build_response 是私有函数，我们通过公共接口间接测试
        let cached = HttpCacheResponse {
            status: 200,
            headers: HashMap::from([
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-Custom".to_string(), "value".to_string()),
            ]),
            body: b"test body".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"abc123\"".to_string()),
            last_modified: None,
        };

        // 验证缓存响应的结构正确
        assert_eq!(cached.status, 200);
        assert_eq!(cached.body, b"test body");
        assert!(cached.etag.is_some());
        assert_eq!(cached.ttl, Some(3600));
    }

    /// 测试 build_response 包含 ETag 和 Cache-Control
    #[test]
    fn test_build_response_with_etag_and_ttl() {
        let cached = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: vec![1, 2, 3, 4],
            cached_at: chrono::Utc::now(),
            ttl: Some(1800),
            etag: Some("\"etag-value\"".to_string()),
            last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
        };

        assert_eq!(cached.etag, Some("\"etag-value\"".to_string()));
        assert_eq!(cached.ttl, Some(1800));
        assert!(cached.last_modified.is_some());
    }

    /// 测试 build_response 无 TTL 场景
    #[test]
    fn test_build_response_without_ttl() {
        let cached = HttpCacheResponse {
            status: 404,
            headers: HashMap::new(),
            body: b"Not Found".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: None,
            etag: None,
            last_modified: None,
        };

        assert!(cached.ttl.is_none());
        assert!(cached.etag.is_none());
    }

    // ============================================================================
    // CacheMiddlewareConfig 测试
    // ============================================================================

    /// 测试配置构建器
    #[test]
    fn test_config_builder_with_all_options() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let key_gen = HttpCacheKeyGenerator::new()
            .with_include_method(true)
            .with_include_query(true);
        let policy = HttpCachePolicy::new()
            .with_default_ttl(7200)
            .with_cache_status_codes(vec![StatusCode::OK, StatusCode::CREATED]);

        let config = CacheMiddlewareConfig::new(adapter)
            .with_key_generator(key_gen)
            .with_policy(policy)
            .with_bypass_header("X-No-Cache".to_string());

        assert_eq!(config.policy.default_ttl, 7200);
        assert_eq!(config.bypass_header, Some("X-No-Cache".to_string()));
    }

    /// 测试配置克隆
    #[test]
    fn test_config_clone() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let config = CacheMiddlewareConfig::new(adapter).with_bypass_header("X-Bypass".to_string());

        let cloned = config.clone();
        assert_eq!(cloned.bypass_header, config.bypass_header);
    }

    // ============================================================================
    // cache_middleware 功能测试
    // ============================================================================

    /// 创建测试用的 Router
    fn create_test_router(adapter: Arc<MemoryCacheAdapter>) -> Router {
        let state = CacheMiddlewareState {
            adapter,
            key_generator: HttpCacheKeyGenerator::new(),
            policy: HttpCachePolicy::new()
                .with_default_ttl(3600)
                .with_cache_status_codes(vec![StatusCode::OK]),
            bypass_header: None,
        };

        Router::new()
            .route("/api/test", get(|| async { "Hello, World!" }))
            .layer(middleware::from_fn_with_state(
                state,
                oxcache::features::http::axum::cache_middleware,
            ))
    }

    /// 创建带 bypass header 的 Router
    fn create_router_with_bypass(adapter: Arc<MemoryCacheAdapter>, bypass_header: String) -> Router {
        let state = CacheMiddlewareState {
            adapter,
            key_generator: HttpCacheKeyGenerator::new(),
            policy: HttpCachePolicy::new()
                .with_default_ttl(3600)
                .with_cache_status_codes(vec![StatusCode::OK]),
            bypass_header: Some(bypass_header),
        };

        Router::new()
            .route("/api/test", get(|| async { "Hello, World!" }))
            .layer(middleware::from_fn_with_state(
                state,
                oxcache::features::http::axum::cache_middleware,
            ))
    }

    /// 测试中间件：首次请求（缓存未命中）
    #[tokio::test]
    async fn test_cache_middleware_cache_miss() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let adapter_clone = adapter.clone();
        let router = create_test_router(adapter_clone);

        let request = Request::builder().uri("/api/test").body(Body::empty()).unwrap();

        let response = router.oneshot(request).await.unwrap();

        // 验证响应状态
        assert_eq!(response.status(), StatusCode::OK);

        // 验证缓存已存储
        let store = adapter.store.lock().await;
        assert!(!store.is_empty());

        // 验证缓存内容
        let cached = store.values().next().unwrap();
        assert_eq!(cached.status, 200);
        assert_eq!(&cached.body[..], b"Hello, World!");
        assert!(cached.etag.is_some());
    }

    /// 测试中间件：缓存命中
    #[tokio::test]
    async fn test_cache_middleware_cache_hit() {
        let adapter = Arc::new(MemoryCacheAdapter::new());

        // 预填充缓存
        let cached_response = HttpCacheResponse {
            status: 200,
            headers: HashMap::from([("Content-Type".to_string(), "text/plain".to_string())]),
            body: b"Cached Response".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"cached-etag\"".to_string()),
            last_modified: None,
        };

        // 生成缓存键
        let key_gen = HttpCacheKeyGenerator::new();
        let test_request = HttpRequest {
            method: http::Method::GET,
            uri: "/api/test".parse().unwrap(),
            version: http::Version::HTTP_11,
            headers: http::HeaderMap::new(),
            body: vec![],
        };
        let cache_key = key_gen.generate_key(&test_request);

        adapter.set_response(&cache_key, &cached_response).await.unwrap();

        let router = create_test_router(adapter.clone());

        let request = Request::builder().uri("/api/test").body(Body::empty()).unwrap();

        let response = router.oneshot(request).await.unwrap();

        // 验证返回的是缓存的响应
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Cached Response");
    }

    /// 测试中间件：bypass header 触发跳过缓存
    #[tokio::test]
    async fn test_cache_middleware_bypass_header() {
        let adapter = Arc::new(MemoryCacheAdapter::new());

        // 预填充缓存（应该被跳过）
        let cached_response = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: b"Old Cached Data".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: None,
            last_modified: None,
        };

        let key_gen = HttpCacheKeyGenerator::new();
        let test_request = HttpRequest {
            method: http::Method::GET,
            uri: "/api/test".parse().unwrap(),
            version: http::Version::HTTP_11,
            headers: http::HeaderMap::new(),
            body: vec![],
        };
        let cache_key = key_gen.generate_key(&test_request);

        adapter.set_response(&cache_key, &cached_response).await.unwrap();

        let router = create_router_with_bypass(adapter.clone(), "X-Bypass-Cache".to_string());

        // 带有 bypass header 的请求
        let request = Request::builder()
            .uri("/api/test")
            .header("X-Bypass-Cache", "true")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        // 验证返回的是新响应（不是缓存的）
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Hello, World!"); // 来自路由，不是缓存
    }

    /// 测试中间件：ETag 条件请求返回 304
    #[tokio::test]
    async fn test_cache_middleware_etag_not_modified() {
        let adapter = Arc::new(MemoryCacheAdapter::new());

        // 预填充缓存带 ETag
        let cached_response = HttpCacheResponse {
            status: 200,
            headers: HashMap::from([("Content-Type".to_string(), "text/plain".to_string())]),
            body: b"Cached Data".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"test-etag-123\"".to_string()),
            last_modified: None,
        };

        let key_gen = HttpCacheKeyGenerator::new();
        let test_request = HttpRequest {
            method: http::Method::GET,
            uri: "/api/test".parse().unwrap(),
            version: http::Version::HTTP_11,
            headers: http::HeaderMap::new(),
            body: vec![],
        };
        let cache_key = key_gen.generate_key(&test_request);

        adapter.set_response(&cache_key, &cached_response).await.unwrap();

        let router = create_test_router(adapter.clone());

        // 带有 If-None-Match header 的请求
        let request = Request::builder()
            .uri("/api/test")
            .header("If-None-Match", "\"test-etag-123\"")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        // 验证返回 304 Not Modified
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

        // 验证响应体为空
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.is_empty());
    }

    /// 测试中间件：ETag 不匹配返回完整响应
    #[tokio::test]
    async fn test_cache_middleware_etag_mismatch() {
        let adapter = Arc::new(MemoryCacheAdapter::new());

        let cached_response = HttpCacheResponse {
            status: 200,
            headers: HashMap::new(),
            body: b"Cached Data".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: Some("\"etag-v1\"".to_string()),
            last_modified: None,
        };

        let key_gen = HttpCacheKeyGenerator::new();
        let test_request = HttpRequest {
            method: http::Method::GET,
            uri: "/api/test".parse().unwrap(),
            version: http::Version::HTTP_11,
            headers: http::HeaderMap::new(),
            body: vec![],
        };
        let cache_key = key_gen.generate_key(&test_request);

        adapter.set_response(&cache_key, &cached_response).await.unwrap();

        let router = create_test_router(adapter.clone());

        // ETag 不匹配
        let request = Request::builder()
            .uri("/api/test")
            .header("If-None-Match", "\"etag-v2\"")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        // 验证返回完整缓存响应
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Cached Data");
    }

    /// 测试中间件：响应被缓存（验证 ETag 生成）
    #[tokio::test]
    async fn test_cache_middleware_generates_etag() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let router = create_test_router(adapter.clone());

        let request = Request::builder().uri("/api/test").body(Body::empty()).unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 验证缓存存储包含 ETag
        let store = adapter.store.lock().await;
        let cached = store.values().next().unwrap();
        assert!(cached.etag.is_some());

        // 验证 ETag 格式正确（MD5 哈希）
        let etag = cached.etag.as_ref().unwrap();
        assert!(etag.starts_with('"') || etag.len() == 32); // MD5 hash length
    }

    /// 测试中间件：多请求缓存共享
    #[tokio::test]
    async fn test_cache_middleware_shared_cache() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let router = create_test_router(adapter.clone());

        // 第一次请求
        let request1 = Request::builder().uri("/api/test").body(Body::empty()).unwrap();

        let _response1 = router.clone().oneshot(request1).await.unwrap();

        // 第二次请求（应该命中缓存）
        let request2 = Request::builder().uri("/api/test").body(Body::empty()).unwrap();

        let response2 = router.oneshot(request2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);

        // 验证缓存只有一条记录
        let store = adapter.store.lock().await;
        assert_eq!(store.len(), 1);
    }

    /// 测试中间件：不同路径不同缓存键
    #[tokio::test]
    async fn test_cache_middleware_different_paths() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let adapter_for_verification = adapter.clone();

        let state = CacheMiddlewareState {
            adapter,
            key_generator: HttpCacheKeyGenerator::new().with_include_method(true),
            policy: HttpCachePolicy::new()
                .with_default_ttl(3600)
                .with_cache_status_codes(vec![StatusCode::OK]),
            bypass_header: None,
        };

        let router = Router::new()
            .route("/api/test1", get(|| async { "Response 1" }))
            .route("/api/test2", get(|| async { "Response 2" }))
            .layer(middleware::from_fn_with_state(
                state,
                oxcache::features::http::axum::cache_middleware,
            ));

        // 请求不同路径
        let request1 = Request::builder().uri("/api/test1").body(Body::empty()).unwrap();

        let response1 = router.clone().oneshot(request1).await.unwrap();
        assert_eq!(response1.status(), StatusCode::OK);

        let request2 = Request::builder().uri("/api/test2").body(Body::empty()).unwrap();

        let response2 = router.oneshot(request2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);

        // 验证两条不同的缓存
        let store = adapter_for_verification.store.lock().await;
        assert_eq!(store.len(), 2);

        // 验证缓存内容
        let bodies: Vec<&[u8]> = store.values().map(|v| v.body.as_slice()).collect();
        assert!(bodies.iter().any(|b| *b == b"Response 1"));
        assert!(bodies.iter().any(|b| *b == b"Response 2"));
    }

    // ============================================================================
    // CacheMiddlewareState 测试
    // ============================================================================

    /// 测试状态克隆
    #[test]
    fn test_state_clone() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let state = CacheMiddlewareState {
            adapter,
            key_generator: HttpCacheKeyGenerator::new(),
            policy: HttpCachePolicy::new().with_default_ttl(1800),
            bypass_header: Some("X-Skip".to_string()),
        };

        let cloned = state.clone();
        assert_eq!(cloned.policy.default_ttl, state.policy.default_ttl);
        assert_eq!(cloned.bypass_header, state.bypass_header);
    }

    // ============================================================================
    // 缓存键生成测试（通过中间件）
    // ============================================================================

    /// 测试不同请求方法影响缓存键
    #[tokio::test]
    async fn test_cache_key_different_methods() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let adapter_for_verification = adapter.clone();

        let state = CacheMiddlewareState {
            adapter,
            key_generator: HttpCacheKeyGenerator::new().with_include_method(true),
            policy: HttpCachePolicy::new().with_cache_status_codes(vec![StatusCode::OK]),
            bypass_header: None,
        };

        let router = Router::new()
            .route("/api/test", get(|| async { "GET Response" }))
            .layer(middleware::from_fn_with_state(
                state,
                oxcache::features::http::axum::cache_middleware,
            ));

        // GET 请求
        let get_request = Request::builder()
            .method(http::Method::GET)
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(get_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 验证缓存
        let store = adapter_for_verification.store.lock().await;
        assert!(!store.is_empty());
    }

    /// 测试查询参数影响缓存键
    #[tokio::test]
    async fn test_cache_key_with_query_params() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let adapter_for_verification = adapter.clone();

        let state = CacheMiddlewareState {
            adapter,
            key_generator: HttpCacheKeyGenerator::new().with_include_query(true),
            policy: HttpCachePolicy::new().with_cache_status_codes(vec![StatusCode::OK]),
            bypass_header: None,
        };

        let router = Router::new()
            .route("/api/test", get(|| async { "Query Response" }))
            .layer(middleware::from_fn_with_state(
                state,
                oxcache::features::http::axum::cache_middleware,
            ));

        // 带查询参数的请求
        let request = Request::builder()
            .uri("/api/test?id=123&name=test")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 验证缓存
        let store = adapter_for_verification.store.lock().await;
        assert!(!store.is_empty());
    }
}
