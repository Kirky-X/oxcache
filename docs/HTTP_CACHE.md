# HTTP 缓存集成指南

## 概述

Oxcache 提供了完整的 HTTP 缓存集成功能，支持标准的 HTTP 缓存头（Cache-Control、ETag、Last-Modified 等），可以轻松地将 Oxcache 集成到 Web 应用中，实现高效的 HTTP 缓存策略。

### 核心特性

- ✅ **标准 HTTP 缓存**：支持 Cache-Control、ETag、Last-Modified
- ✅ **条件请求**：支持 If-None-Match、If-Modified-Since
- ✅ **缓存验证**：自动验证缓存有效性
- ✅ **Vary 支持**：支持基于 Vary 头的缓存键
- ✅ **Stale-While-Revalidate**：支持后台刷新
- ✅ **框架集成**：支持 Axum、Actix-web 等 Web 框架

## 工作原理

### HTTP 缓存流程

```
1. 客户端请求 → HTTP 服务器
   ↓
2. 检查缓存
   ↓
3. 缓存命中且有效：
   - 返回 304 Not Modified（如果支持条件请求）
   - 或返回缓存内容
   ↓
4. 缓存未命中或过期：
   - 从数据源获取数据
   - 更新缓存
   - 返回响应
```

### 条件请求流程

```
1. 客户端发送 If-None-Match: "abc123"
   ↓
2. 服务器检查 ETag
   ↓
3. ETag 匹配：
   - 返回 304 Not Modified
   - 不返回响应体
   ↓
4. ETag 不匹配：
   - 返回 200 OK
   - 返回新内容和新的 ETag
```

## 使用方式

### Axum 集成

```rust
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use oxcache::{Cache, CacheOps};
use oxcache::http::axum::CacheLayer;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

// 创建缓存
let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;

// 创建缓存层
let cache_layer = CacheLayer::new(cache)
    .with_ttl(3600)  // 缓存 1 小时
    .with_etag(true) // 启用 ETag
    .with_stale_while_revalidate(300); // 过期后 5 分钟内继续使用

// 定义路由
async fn get_user(
    State(cache): State<Cache<String, User>>,
    axum::extract::Path(id): axum::extract::Path<u64>,
) -> Result<Response, StatusCode> {
    let key = format!("user:{}", id);
    
    // 尝试从缓存获取
    if let Some(user) = cache.get(&key).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        // 生成 ETag
        let etag = format!("\"{}\"", compute_etag(&user));
        
        // 检查条件请求
        if let Some(if_none_match) = headers.get("if-none-match") {
            if if_none_match.to_str().unwrap() == etag {
                return Ok((StatusCode::NOT_MODIFIED, []).into_response());
            }
        }
        
        // 返回缓存内容
        return Ok((
            StatusCode::OK,
            [(axum::http::header::ETAG, etag)],
            axum::Json(user),
        ).into_response());
    }
    
    // 缓存未命中，从数据库查询
    let user = database::query_user(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // 更新缓存
    cache.set(&key, &user, Some(3600)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // 返回响应
    let etag = format!("\"{}\"", compute_etag(&user));
    Ok((
        StatusCode::OK,
        [(axum::http::header::ETAG, etag)],
        axum::Json(user),
    ).into_response())
}
```

### 使用中间件

```rust
use axum::{
    middleware,
    Router,
    http::StatusCode,
};
use oxcache::http::axum::CacheMiddleware;

// 创建缓存中间件
let cache_middleware = CacheMiddleware::new(cache)
    .with_ttl(3600)
    .with_cacheable_paths(vec![
        "/api/users/*",
        "/api/products/*",
    ]);

// 应用中间件
let app = Router::new()
    .route("/api/users/:id", get_user_handler)
    .route("/api/products/:id", get_product_handler)
    .layer(middleware::from_fn(cache_middleware));
```

### 手动 HTTP 缓存

```rust
use oxcache::{Cache, CacheOps};
use oxcache::http::{HttpCache, CacheControl, ETag};

// 创建 HTTP 缓存
let http_cache = HttpCache::new(cache);

// 存储响应
let response = Response {
    status: 200,
    headers: Headers {
        cache_control: CacheControl::public()
            .max_age(3600)
            .stale_while_revalidate(300),
        etag: ETag::weak("abc123"),
    },
    body: user_data,
};

http_cache.set("/api/users/123", &response).await?;

// 获取响应
let cached_response = http_cache.get("/api/users/123").await?;

if let Some(response) = cached_response {
    // 检查是否过期
    if response.is_fresh() {
        return Ok(response);
    }
    
    // 过期但可以使用
    if response.can_stale() {
        // 后台刷新
        tokio::spawn(async move {
            refresh_cache("/api/users/123").await;
        });
        
        return Ok(response);
    }
}

// 缓存无效，重新获取
let fresh_response = fetch_from_origin("/api/users/123").await?;
http_cache.set("/api/users/123", &fresh_response).await?;

Ok(fresh_response)
```

## 缓存头配置

### Cache-Control

```rust
use oxcache::http::CacheControl;

// 公共缓存
let cache_control = CacheControl::public()
    .max_age(3600)
    .stale_while_revalidate(300);

// 私有缓存
let cache_control = CacheControl::private()
    .max_age(600);

// 不缓存
let cache_control = CacheControl::no_cache()
    .no_store();

// 必须重新验证
let cache_control = CacheControl::no_cache()
    .must_revalidate();
```

### ETag

```rust
use oxcache::http::ETag;

// 强 ETag
let etag = ETag::strong("abc123");

// 弱 ETag
let etag = ETag::weak("abc123");

// 从内容生成 ETag
fn compute_etag<T: Serialize>(data: &T) -> String {
    let serialized = serde_json::to_vec(data).unwrap();
    let hash = md5::compute(&serialized);
    format!("{:x}", hash)
}
```

### Last-Modified

```rust
use chrono::{DateTime, Utc};

// 设置 Last-Modified
let last_modified = DateTime::<Utc>::from_utc(
    NaiveDateTime::from_timestamp(1234567890, 0).unwrap(),
    Utc
);

// 检查条件请求
if let Some(if_modified_since) = headers.get("if-modified-since") {
    let if_modified_since = DateTime::parse_from_rfc2822(
        if_modified_since.to_str().unwrap()
    ).unwrap();
    
    if last_modified <= if_modified_since {
        return Ok(StatusCode::NOT_MODIFIED);
    }
}
```

## 高级用法

### Vary 支持

```rust
use oxcache::http::HttpCache;

// 创建支持 Vary 的缓存
let http_cache = HttpCache::new(cache)
    .with_vary_headers(vec![
        "Accept".to_string(),
        "Accept-Encoding".to_string(),
        "Accept-Language".to_string(),
    ]);

// 缓存会根据 Vary 头生成不同的缓存键
let response = http_cache.get("/api/users/123", headers).await?;
```

### Stale-While-Revalidate

```rust
use oxcache::http::CacheControl;

// 启用过期后继续使用
let cache_control = CacheControl::public()
    .max_age(3600)
    .stale_while_revalidate(300);  // 过期后 5 分钟内继续使用

// 后台刷新
async fn get_with_background_refresh(
    http_cache: &HttpCache,
    path: &str,
) -> Result<Response> {
    let response = http_cache.get(path).await?;
    
    if let Some(response) = response {
        if response.is_stale() && response.can_stale() {
            // 立即返回过期内容
            let stale_response = response.clone();
            
            // 后台刷新
            let path = path.to_string();
            tokio::spawn(async move {
                if let Ok(fresh) = fetch_from_origin(&path).await {
                    http_cache.set(&path, &fresh).await;
                }
            });
            
            return Ok(stale_response);
        }
    }
    
    // 缓存无效，重新获取
    let fresh_response = fetch_from_origin(path).await?;
    http_cache.set(path, &fresh_response).await?;
    
    Ok(fresh_response)
}
```

### 缓存失效

```rust
use oxcache::http::HttpCache;

// 按路径失效
http_cache.invalidate("/api/users/123").await?;

// 按模式失效
http_cache.invalidate_pattern("/api/users/*").await?;

// 全部失效
http_cache.invalidate_all().await?;

// 按标签失效
http_cache.invalidate_tag("user_data").await?;
```

### 缓存预热

```rust
use oxcache::http::HttpCache;

// 预热缓存
async fn warmup_cache(http_cache: &HttpCache) -> Result<()> {
    // 预热热门 API
    let hot_paths = vec![
        "/api/users/1",
        "/api/users/2",
        "/api/products/1",
        "/api/products/2",
    ];
    
    for path in hot_paths {
        let response = fetch_from_origin(path).await?;
        http_cache.set(path, &response).await?;
        println!("预热缓存: {}", path);
    }
    
    Ok(())
}
```

## 最佳实践

### ✅ 推荐做法

1. **合理设置 TTL**：根据数据更新频率设置合理的过期时间
2. **使用 ETag**：启用 ETag 支持条件请求，减少带宽消耗
3. **Stale-While-Revalidate**：使用过期后继续使用策略，提升用户体验
4. **Vary 正确使用**：只在必要时使用 Vary，避免缓存碎片化
5. **监控缓存命中率**：定期检查缓存命中率，优化缓存策略

### ❌ 避免做法

1. **缓存敏感数据**：不要缓存个人隐私、支付等敏感数据
2. **忽略缓存失效**：不要忽略缓存失效逻辑，导致数据不一致
3. **过长的 TTL**：不要设置过长的 TTL，导致数据过期
4. **过度使用 Vary**：不要过度使用 Vary，增加缓存复杂度
5. **忽略错误处理**：不要忽略缓存错误，导致服务不可用

## 性能优化

### 缓存键优化

```rust
// 使用简洁的缓存键
let key = format!("user:{}", user_id);  // ✅ 好

// 避免使用复杂的缓存键
let key = format!("api/v1/users/id/{}/name/{}/email/{}", 
    user_id, name, email);  // ❌ 差
```

### 压缩响应

```rust
use oxcache::http::HttpCache;

// 启用响应压缩
let http_cache = HttpCache::new(cache)
    .with_compression(true)
    .with_compression_threshold(1024);  // 超过 1KB 才压缩
```

### 批量预热

```rust
// 批量预热缓存
async fn batch_warmup(http_cache: &HttpCache) -> Result<()> {
    let tasks: Vec<_> = (1..=1000)
        .map(|id| {
            let http_cache = http_cache.clone();
            tokio::spawn(async move {
                let path = format!("/api/users/{}", id);
                if let Ok(response) = fetch_from_origin(&path).await {
                    http_cache.set(&path, &response).await;
                }
            })
        })
        .collect();
    
    for task in tasks {
        task.await?;
    }
    
    Ok(())
}
```

## 监控与统计

```rust
use oxcache::http::HttpCacheStats;

// 获取 HTTP 缓存统计
let stats = http_cache.get_stats().await?;

println!("HTTP 缓存统计：");
println!("  缓存命中: {}", stats.hits);
println!("  缓存未命中: {}", stats.misses);
println!("  命中率: {:.2}%", stats.hit_rate * 100.0);
println!("  条件请求命中: {}", stats.conditional_hits);
println!("  过期响应: {}", stats.stale_responses);
println!("  平均响应时间: {:?}", stats.avg_response_time);
```

## 完整示例

```rust
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use oxcache::{Cache, CacheOps};
use oxcache::http::axum::CacheLayer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== HTTP 缓存集成示例 ===\n");
    
    // 1. 创建缓存
    println!("1. 创建缓存...");
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    println!("   ✅ 缓存创建成功\n");
    
    // 2. 创建缓存层
    println!("2. 创建缓存层...");
    let cache_layer = CacheLayer::new(cache.clone())
        .with_ttl(3600)
        .with_etag(true)
        .with_stale_while_revalidate(300);
    println!("   ✅ 缓存层创建成功\n");
    
    // 3. 创建路由
    println!("3. 创建路由...");
    let app = Router::new()
        .route("/api/users/:id", get_user)
        .with_state(Arc::new(cache));
    
    // 4. 启动服务器
    println!("4. 启动服务器...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("   ✅ 服务器启动成功: http://localhost:3000\n");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn get_user(
    State(cache): State<Arc<Cache<String, User>>>,
    Path(id): Path<u64>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let key = format!("user:{}", id);
    
    // 尝试从缓存获取
    if let Some(user) = cache.get(&key).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        // 生成 ETag
        let etag = format!("\"{}\"", compute_etag(&user));
        
        // 检查条件请求
        if let Some(if_none_match) = headers.get("if-none-match") {
            if if_none_match.to_str().unwrap() == etag {
                println!("💾 缓存命中，返回 304: user_id={}", id);
                return Ok((
                    StatusCode::NOT_MODIFIED,
                    [(axum::http::header::ETAG, etag)],
                ).into_response());
            }
        }
        
        println!("💾 缓存命中: user_id={}", id);
        
        // 返回缓存内容
        return Ok((
            StatusCode::OK,
            [
                (axum::http::header::ETAG, etag),
                (axum::http::header::CACHE_CONTROL, "public, max-age=3600, stale-while-revalidate=300"),
            ],
            Json(user),
        ).into_response());
    }
    
    println!("🔍 缓存未命中，查询数据库: user_id={}", id);
    
    // 缓存未命中，从数据库查询
    let user = database::query_user(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // 更新缓存
    cache.set(&key, &user, Some(3600)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    println!("✅ 数据库查询成功，已更新缓存");
    
    // 返回响应
    let etag = format!("\"{}\"", compute_etag(&user));
    Ok((
        StatusCode::OK,
        [
            (axum::http::header::ETAG, etag),
            (axum::http::header::CACHE_CONTROL, "public, max-age=3600, stale-while-revalidate=300"),
        ],
        Json(user),
    ).into_response())
}

fn compute_etag(user: &User) -> String {
    let serialized = serde_json::to_vec(user).unwrap();
    let hash = md5::compute(&serialized);
    format!("{:x}", hash)
}
```

## 故障排除

### 问题：缓存不生效

**原因**：
- 缓存头配置错误
- 缓存键不一致
- TTL 设置过短

**解决方案**：
1. 检查 Cache-Control 头配置
2. 确保缓存键一致
3. 增加 TTL

### 问题：ETag 不匹配

**原因**：
- ETag 生成逻辑不一致
- 内容序列化方式不同
- 时区问题

**解决方案**：
1. 统一 ETag 生成逻辑
2. 使用相同的序列化方式
3. 使用 UTC 时间

### 问题：条件请求失败

**原因**：
- If-None-Match 头格式错误
- ETag 比较逻辑错误
- 缓存未正确存储

**解决方案**：
1. 检查 If-None-Match 头格式
2. 修复 ETag 比较逻辑
3. 确保缓存正确存储

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [Axum 文档](https://docs.rs/axum/)

## 示例代码

- `examples/src/http_cache.rs` - HTTP 缓存完整示例
- `src/http/axum.rs` - Axum 集成实现
- `src/http/mod.rs` - HTTP 缓存实现