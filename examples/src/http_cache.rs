// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

//! HTTP 缓存集成示例
//!
//! 本示例演示如何将 Oxcache 用于 HTTP 缓存场景。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example http_cache
//!

use oxcache::Cache;

// 模拟 HTTP 响应
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== HTTP 缓存集成示例 ===
");

    // 创建缓存
    let cache: Cache<String, HttpResponse> = Cache::builder().build().await?;

    // 1. 缓存 API 响应
    println!("1. 缓存 API 响应");

    let response = HttpResponse {
        status: 200,
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Cache-Control".to_string(), "max-age=3600".to_string()),
        ],
        body: r#"{"message": "Hello, World!", "data": [1, 2, 3]}"#.to_string(),
    };

    cache.set("/api/hello", &response, Some(3600)).await?;
    println!("   ✓ 缓存 API 响应: /api/hello");

    // 2. 模拟请求
    println!("
2. 模拟 HTTP 请求");

    let requests = vec![
        "/api/hello",
        "/api/hello", // 重复请求，命中缓存
        "/api/users",
        "/api/hello", // 再次请求，命中缓存
    ];

    for path in &requests {
        let start = std::time::Instant::new();
        let cached = cache.get(path).await?;
        let elapsed = start.elapsed();

        match cached {
            Some(resp) => {
                println!(
                    "   GET {} - 状态: {} (缓存命中, 耗时: {:?})",
                    path, resp.status, elapsed
                );
            }
            None => {
                println!("   GET {} - 缓存未命中", path);
            }
        }
    }

    // 3. 缓存控制
    println!("
3. 缓存控制示例");

    // 设置缓存响应
    let private_response = HttpResponse {
        status: 200,
        headers: vec![("Cache-Control".to_string(), "private, max-age=300".to_string())],
        body: r#"{"user": "private_data"}"#.to_string(),
    };
    cache.set("/api/user/profile", &private_response, Some(300)).await?;
    println!("   ✓ 设置私有缓存: /api/user/profile (5分钟)");

    // 强制刷新
    println!("   强制刷新 /api/hello...");
    cache.delete("/api/hello").await?;
    println!("   ✓ 缓存已清除");

    // 4. 统计信息
    println!("
4. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    if stats.hit_count() + stats.miss_count() > 0 {
        let hit_rate = stats.hit_count() as f64
            / (stats.hit_count() + stats.miss_count()) as f64
            * 100.0;
        println!("   - 命中率: {:.2}%", hit_rate);
    }

    // 清理
    println!("
5. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理
");

    println!("=== HTTP 缓存集成示例完成 ===");
    Ok(())
}
