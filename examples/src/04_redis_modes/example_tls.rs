//! Redis TLS 连接示例
//!
//! 本示例演示如何使用 Oxcache 连接启用 TLS 的 Redis 服务器。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_tls
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Session {
    id: String,
    user_id: u64,
    data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis TLS 连接示例 ===\n");

    // 创建 Redis 缓存 (使用 rediss:// 前缀启用 TLS)
    println!("创建 Redis TLS 缓存连接...");
    let cache: Cache<String, Session> = Cache::redis("rediss://127.0.0.1:6379").await?;
    println!("✓ Redis TLS 连接成功\n");

    // 基本操作
    println!("1. 会话存储演示");
    let session = Session {
        id: "sess_abc123".to_string(),
        user_id: 1001,
        data: r#"{"theme": "dark", "language": "zh-CN"}"#.to_string(),
    };

    // 存储会话
    println!("   存储会话...");
    cache.set("session:sess_abc123", &session, Some(3600)).await?;
    println!("   ✓ 会话存储成功");

    // 获取会话
    println!("   获取会话...");
    let retrieved = cache.get("session:sess_abc123").await?;
    match retrieved {
        Some(s) => println!("   ✓ 会话获取成功: 用户 {} (ID: {})", s.user_id, s.id),
        None => println!("   ✗ 会话未找到"),
    }

    // 更新会话数据
    println!("   更新会话数据...");
    let updated_session = Session {
        id: "sess_abc123".to_string(),
        user_id: 1001,
        data: r#"{"theme": "light", "language": "zh-CN"}"#.to_string(),
    };
    cache.set("session:sess_abc123", &updated_session, Some(3600)).await?;
    println!("   ✓ 会话更新成功");

    // 验证更新
    let retrieved = cache.get("session:sess_abc123").await?;
    match retrieved {
        Some(s) => println!("   ✓ 会话数据: {}", s.data),
        None => println!("   ✗ 会话未找到"),
    }

    // 删除会话
    println!("   删除会话...");
    cache.delete("session:sess_abc123").await?;
    println!("   ✓ 会话删除成功\n");

    // 统计信息
    println!("2. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    println!();

    println!("=== Redis TLS 连接示例完成 ===");
    Ok(())
}