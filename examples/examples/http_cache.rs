//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! # HTTP 缓存示例
//!
//! 演示如何使用 HTTP 缓存适配器、ETag、条件请求和 Axum 中间件。
//!
//! 需要启用 `http-cache` 特性。

use axum::{
    body::Body,
    extract::{Query, State},
    http::{Response, StatusCode},
    routing::get,
    Router,
};
use oxcache::http::{
    CacheMiddleware, CacheMiddlewareConfig, CacheTagManager, ConditionalRequestHandler,
    HttpCacheAdapter, HttpCacheKeyGenerator, HttpCachePolicy, HttpCacheResponse, PathPatternMatcher,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// Mock HTTP 缓存存储（简化版）
// ============================================================================

#[derive(Clone, Default)]
struct HttpCacheStore {
    store: Arc<Mutex<HashMap<String, HttpCacheResponse>>>,
}

impl HttpCacheStore {
    fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl HttpCacheAdapter for HttpCacheStore {
    async fn get_response(&self, key: &str) -> Result<Option<HttpCacheResponse>, oxcache::error::CacheError> {
        let store = self.store.lock().await;
        Ok(store.get(key).cloned())
    }

    async fn set_response(&self, key: &str, response: &HttpCacheResponse) -> Result<(), oxcache::error::CacheError> {
        let mut store = self.store.lock().await;
        store.insert(key.to_string(), response.clone());
        Ok(())
    }

    async fn delete_response(&self, key: &str) -> Result<bool, oxcache::error::CacheError> {
        let mut store = self.store.lock().await;
        Ok(store.remove(key).is_some())
    }

    async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64, oxcache::error::CacheError> {
        let mut store = self.store.lock().await;
        let keys: Vec<String> = store.keys().filter(|k| k.contains(pattern)).cloned().collect();
        for key in &keys {
            store.remove(key);
        }
        Ok(keys.len() as u64)
    }

    async fn get_responses(
        &self,
        keys: &[&str],
    ) -> Result<HashMap<String, HttpCacheResponse>, oxcache::error::CacheError> {
        let store = self.store.lock().await;
        let mut result = HashMap::new();
        for &key in keys {
            if let Some(response) = store.get(key) {
                result.insert(key.to_string(), response.clone());
            }
        }
        Ok(result)
    }
}

// ============================================================================
// 示例数据模型
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ApiResponse<T> {
    data: T,
    cached_at: String,
}

// ============================================================================
// 示例应用
// ============================================================================

#[tokio::main]
async fn main() {
    println!("=== HTTP 缓存示例 ===\n");

    // ===========================================================================
    // 1. HTTP 缓存键生成器
    // ===========================================================================
    println!("1. HTTP 缓存键生成器");

    let key_generator = HttpCacheKeyGenerator::new()
        .with_include_method(true)
        .with_include_query(true);

    println!("   - 创建键生成器（包含方法和查询参数）");
    println!();

    // ===========================================================================
    // 2. 路径模式匹配
    // ===========================================================================
    println!("2. 路径模式匹配");

    let matcher = PathPatternMatcher::new();

    println!("   - 测试模式匹配:");
    println!("     /api/users/123 匹配 /api/users/* -> {}", matcher.matches("/api/users/123", "/api/users/*"));
    println!("     /api/users/123 匹配 /api/** -> {}", matcher.matches("/api/users/123", "/api/**"));
    println!("     /api/users/123/profile 匹配 /api/users/*/profile -> {}", matcher.matches("/api/users/123/profile", "/api/users/*/profile"));
    println!();

    // ===========================================================================
    // 3. 条件请求处理 (ETag)
    // ===========================================================================
    println!("3. 条件请求处理 (ETag)");

    let handler = ConditionalRequestHandler::new();

    // 模拟缓存的响应
    let cached_response = HttpCacheResponse {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())]
            .into_iter()
            .collect(),
        body: r#"{"id": 1, "name": "Alice", "email": "alice@example.com"}"#.as_bytes().to_vec(),
        cached_at: chrono::Utc::now(),
        ttl: Some(3600),
        etag: Some("\"abc123\"".to_string()),
        last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
    };

    // 生成 ETag
    let strong_etag = handler.generate_strong_etag(&cached_response.body);
    println!("   - 生成强 ETag: {}", strong_etag);

    // 模拟 If-None-Match 请求
    let result = handler.check_conditional(&cached_response, Some(&strong_etag), None);
    match result {
        oxcache::http::ConditionalRequestResult::NotModified => {
            println!("   - ETag 匹配，返回 304 Not Modified");
        }
        oxcache::http::ConditionalRequestResult::FullResponse(_) => {
            println!("   - ETag 不匹配，返回完整响应");
        }
        _ => {}
    }

    // 模拟 If-Modified-Since 请求
    let old_time = "Tue, 01 Jan 2025 00:00:00 GMT";
    let result = handler.check_conditional(&cached_response, None, Some(old_time));
    match result {
        oxcache::http::ConditionalRequestResult::NotModified => {
            println!("   - 资源未修改，返回 304 Not Modified");
        }
        _ => {}
    }
    println!();

    // ===========================================================================
    // 4. 缓存标签管理
    // ===========================================================================
    println!("4. 缓存标签管理");

    let cache_store = Arc::new(HttpCacheStore::new());
    let tag_manager = CacheTagManager::new(cache_store.clone());

    // 模拟缓存用户数据并添加标签
    let user1_key = "cache:users:1";
    let user2_key = "cache:users:2";

    tag_manager.add_tags(user1_key, &["users", "vip"]).await.unwrap();
    tag_manager.add_tags(user2_key, &["users"]).await.unwrap();

    println!("   - user1 添加标签: users, vip");
    println!("   - user2 添加标签: users");

    // 按标签失效
    let count = tag_manager.invalidate_by_tag("users").await.unwrap();
    println!("   - 按 'users' 标签失效: {} 个缓存项", count);

    // 验证
    let user1 = cache_store.get_response(user1_key).await.unwrap();
    let user2 = cache_store.get_response(user2_key).await.unwrap();
    println!("   - user1 缓存状态: {}", if user1.is_some() { "存在" } else { "已删除" });
    println!("   - user2 缓存状态: {}", if user2.is_some() { "存在" } else { "已删除" });
    println!();

    // ===========================================================================
    // 5. HTTP 缓存策略配置
    // ===========================================================================
    println!("5. HTTP 缓存策略配置");

    let cache_policy = HttpCachePolicy::new()
        .with_cache_status_codes(vec![StatusCode::OK, StatusCode::NOT_FOUND])
        .with_default_ttl(3600)
        .with_use_header_ttl(true)
        .with_ignore_patterns(vec!["/api/health".to_string()]);

    println!("   - 缓存状态码: [200, 404]");
    println!("   - 默认 TTL: {} 秒", cache_policy.default_ttl);
    println!("   - 使用响应头 TTL: {}", cache_policy.use_header_ttl);
    println!("   - 忽略路径: {:?}", cache_policy.ignore_patterns);
    println!();

    // ===========================================================================
    // 6. Axum 中间件示例（伪代码展示）
    // ===========================================================================
    println!("6. Axum 中间件使用示例");
    println!();
    println!("   // 创建缓存存储");
    println!("   let cache_store = Arc::new(HttpCacheStore::new());");
    println!();
    println!("   // 创建中间件配置");
    println!("   let config = CacheMiddlewareConfig::new()");
    println!("       .with_ttl(3600)");
    println!("       .with_etag(true);");
    println!();
    println!("   // 创建中间件");
    println!("   let cache_middleware = CacheMiddleware::new(cache_store.clone(), config);");
    println!();
    println!("   // 在路由中使用");
    println!("   let app = Router::new()");
    println!("       .route(\"/api/users/:id\", get(get_user))");
    println!("       .layer(cache_middleware);");

    println!();
    println!("=== HTTP 缓存示例完成 ===");
}
