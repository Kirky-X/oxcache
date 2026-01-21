//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Axum HTTP 缓存中间件
//!
//! 提供与 Axum 框架兼容的缓存中间件。

use crate::http::{
    HttpCacheAdapter, HttpCacheKeyGenerator, HttpCachePolicy, HttpCacheResponse, HttpRequest,
};
use axum::{body::Body, extract::State, response::Response};
use http::Request;
use http::StatusCode;
use http_body_util::BodyExt;
use std::collections::HashMap;
use std::sync::Arc;

/// Axum 缓存中间件配置
#[derive(Debug, Clone)]
pub struct CacheMiddlewareConfig<A: HttpCacheAdapter + Send + Sync + 'static> {
    pub cache_adapter: Arc<A>,
    pub key_generator: HttpCacheKeyGenerator,
    pub policy: HttpCachePolicy,
    pub bypass_header: Option<String>,
}

impl<A: HttpCacheAdapter + Send + Sync + 'static> CacheMiddlewareConfig<A> {
    pub fn new(cache_adapter: Arc<A>) -> Self {
        Self {
            cache_adapter,
            key_generator: HttpCacheKeyGenerator::new(),
            policy: HttpCachePolicy::new(),
            bypass_header: None,
        }
    }

    pub fn with_key_generator(mut self, key_generator: HttpCacheKeyGenerator) -> Self {
        self.key_generator = key_generator;
        self
    }

    pub fn with_policy(mut self, policy: HttpCachePolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_bypass_header(mut self, header: String) -> Self {
        self.bypass_header = Some(header);
        self
    }
}

/// 中间件状态
#[derive(Clone, Debug)]
pub struct CacheMiddlewareState<A: HttpCacheAdapter + Send + Sync + 'static> {
    pub adapter: Arc<A>,
    pub key_generator: HttpCacheKeyGenerator,
    pub policy: HttpCachePolicy,
    pub bypass_header: Option<String>,
}

/// 缓存中间件处理函数
pub async fn cache_middleware<A: HttpCacheAdapter + Send + Sync + 'static>(
    State(state): State<CacheMiddlewareState<A>>,
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    // 检查是否跳过缓存
    if let Some(ref header) = state.bypass_header {
        if request.headers().contains_key(header) {
            return next.run(request).await;
        }
    }

    // 生成缓存键
    let http_request = HttpRequest {
        method: request.method().clone(),
        uri: request.uri().clone(),
        version: request.version(),
        headers: request.headers().clone(),
        body: vec![],
    };

    let cache_key = state.key_generator.generate_key(&http_request);

    // 检查缓存
    if let Ok(Some(cached_response)) = state.adapter.get_response(&cache_key).await {
        // 检查条件请求
        if let Some(etag) = &cached_response.etag {
            if let Some(if_none_match) = request.headers().get("If-None-Match") {
                if if_none_match == etag {
                    return Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .body(Body::empty())
                        .unwrap();
                }
            }
        }

        // 返回缓存的响应
        return build_response(&cached_response);
    }

    // 执行请求
    let mut response = next.run(request).await;

    // 检查是否应该缓存
    let status = response.status();
    if state.policy.should_cache_response(status) {
        let body_bytes = response.body_mut().collect().await.unwrap().to_bytes();
        let body = body_bytes.to_vec();

        // 构建缓存响应
        let mut headers = HashMap::new();
        for (name, value) in response.headers().iter() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // 提取 TTL
        let ttl = state
            .policy
            .extract_ttl_from_headers(response.headers())
            .or(Some(state.policy.default_ttl));

        // 生成 ETag
        let etag = Some(format!("{:x}", md5::compute(&body)));

        let cached_response = HttpCacheResponse {
            status: status.as_u16(),
            headers,
            body,
            cached_at: chrono::Utc::now(),
            ttl,
            etag,
            last_modified: None,
        };

        // 缓存响应
        let _ = state
            .adapter
            .set_response(&cache_key, &cached_response)
            .await;
    }

    response
}

/// 从缓存响应构建 Axum 响应
fn build_response(cached: &HttpCacheResponse) -> Response {
    let mut builder = http::Response::builder().status(cached.status);

    // 添加响应头
    for (name, value) in &cached.headers {
        builder = builder.header(name, value);
    }

    if let Some(ref etag) = cached.etag {
        builder = builder.header("ETag", etag);
    }

    if let Some(ttl) = cached.ttl {
        builder = builder.header("Cache-Control", format!("max-age={}", ttl));
    }

    builder.body(Body::from(cached.body.clone())).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;

    /// 测试用内存缓存适配器
    #[derive(Clone, Debug)]
    struct MemoryCacheAdapter {
        store: Arc<std::sync::Mutex<HashMap<String, HttpCacheResponse>>>,
    }

    impl MemoryCacheAdapter {
        fn new() -> Self {
            Self {
                store: Arc::new(std::sync::Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl HttpCacheAdapter for MemoryCacheAdapter {
        async fn get_response(
            &self,
            key: &str,
        ) -> Result<Option<HttpCacheResponse>, crate::error::CacheError> {
            Ok(self
                .store
                .lock()
                .expect("MemoryCacheAdapter store lock poisoned")
                .get(key)
                .cloned())
        }

        async fn set_response(
            &self,
            key: &str,
            response: &HttpCacheResponse,
        ) -> Result<(), crate::error::CacheError> {
            let mut store = self
                .store
                .lock()
                .expect("MemoryCacheAdapter store lock poisoned");
            store.insert(key.to_string(), response.clone());
            Ok(())
        }

        async fn delete_response(&self, key: &str) -> Result<bool, crate::error::CacheError> {
            let mut store = self
                .store
                .lock()
                .expect("MemoryCacheAdapter store lock poisoned");
            Ok(store.remove(key).is_some())
        }

        async fn invalidate_by_pattern(
            &self,
            _pattern: &str,
        ) -> Result<u64, crate::error::CacheError> {
            let mut store = self
                .store
                .lock()
                .expect("MemoryCacheAdapter store lock poisoned");
            let count = store.len();
            store.clear();
            Ok(count as u64)
        }

        async fn get_responses(
            &self,
            keys: &[&str],
        ) -> Result<HashMap<String, HttpCacheResponse>, crate::error::CacheError> {
            let store = self
                .store
                .lock()
                .expect("MemoryCacheAdapter store lock poisoned");
            let mut result = HashMap::new();
            for &key in keys {
                if let Some(resp) = store.get(key) {
                    result.insert(key.to_string(), resp.clone());
                }
            }
            Ok(result)
        }
    }

    #[test]
    fn test_cache_middleware_config() {
        let adapter = Arc::new(MemoryCacheAdapter::new());
        let config = CacheMiddlewareConfig::new(adapter)
            .with_bypass_header("X-Cache-Bypass".to_string())
            .with_policy(HttpCachePolicy::new().with_default_ttl(3600));

        assert_eq!(config.policy.default_ttl, 3600);
        assert_eq!(config.bypass_header, Some("X-Cache-Bypass".to_string()));
    }

    #[tokio::test]
    async fn test_memory_cache_adapter() {
        let adapter = MemoryCacheAdapter::new();

        let response = HttpCacheResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())]
                .into_iter()
                .collect(),
            body: b"Hello, World!".to_vec(),
            cached_at: chrono::Utc::now(),
            ttl: Some(3600),
            etag: None,
            last_modified: None,
        };

        // 设置缓存
        adapter.set_response("test_key", &response).await.unwrap();

        // 获取缓存
        let cached = adapter.get_response("test_key").await.unwrap().unwrap();
        assert_eq!(cached.status, 200);
        assert_eq!(cached.body, b"Hello, World!");

        // 删除缓存
        assert!(adapter.delete_response("test_key").await.unwrap());

        // 验证删除
        assert!(adapter.get_response("test_key").await.unwrap().is_none());
    }
}
