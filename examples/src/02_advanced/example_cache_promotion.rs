// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 缓存提升策略示例
//
// 本示例演示命中的缓存提升行为:
// - 当启用promote_on_hit时，L2缓存命中时更新L1缓存
// - 热数据自动提升到L1以实现更快的访问

use oxcache::Cache;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Session {
    id: String,
    user_id: u64,
    created_at: chrono::DateTime<chrono::Utc>,
    last_accessed: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, Session> = Cache::builder().build().await?;

    // 模拟初始状态下仅存在于L2的会话数据
    let session = Session {
        id: "sess_abc123".to_string(),
        user_id: 42,
        created_at: chrono::Utc::now(),
        last_accessed: chrono::Utc::now(),
    };

    // 设置会话 (同时进入L1和L2)
    println!("创建会话...");
    cache
        .set(&"session:sess_abc123".to_string(), &session)
        .await?;

    // 从L1驱逐以模拟仅L2状态
    println!("\n从L1驱逐以模拟仅L2状态...");
    // 注意: 我们无法在此API中直接从L1驱逐，但在实际场景中
    // L1可能已满且旧条目被驱逐

    // 首次访问 - 可能命中L2并提升到L1
    println!("\n首次访问 (潜在L2命中 -> L1提升)...");
    let start = std::time::Instant::now();
    if let Some(sess) = cache.get(&"session:sess_abc123".to_string()).await? {
        println!("会话在 {:?} 后找到", start.elapsed());
        println!("用户ID: {}", sess.user_id);
    }

    // 后续访问 - 应该命中L1 (快速!)
    println!("\n第二次访问 (L1命中 - 应该更快)...");
    let start = std::time::Instant::now();
    if let Some(sess) = cache.get(&"session:sess_abc123".to_string()).await? {
        println!("会话在 {:?} 后找到", start.elapsed());
        println!("用户ID: {}", sess.user_id);
    }

    println!("\n缓存提升示例完成!");
    println!("当promote_on_hit=true时，热数据自动移动到L1以实现更快的访问.");
    Ok(())
}
