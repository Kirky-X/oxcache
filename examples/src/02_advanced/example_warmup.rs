//! 缓存预热示例
//!
//! 本示例演示了 Oxcache 的缓存预热功能：
//! - 应用启动时预热缓存
//! - 预加载热点数据
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_warmup
//! ```

use std::sync::Arc;
use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct AppConfig {
    key: String,
    value: String,
    description: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    username: String,
    role: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 缓存预热示例 ===\n");

    // 创建缓存
    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);

    // 1. 模拟从数据库加载配置
    println!("1. 模拟应用配置预热");
    let configs = vec![
        ("app:theme", "dark", "应用主题"),
        ("app:language", "zh-CN", "默认语言"),
        ("app:timezone", "Asia/Shanghai", "时区"),
        ("app:max_connections", "100", "最大连接数"),
        ("app:session_timeout", "3600", "会话超时时间"),
    ];

    println!("   从数据库加载配置...");
    for (key, value, desc) in &configs {
        // 模拟数据库查询延迟
        // tokio::time::sleep(Duration::from_millis(10)).await;
        cache.set(key, value, None).await?;
        println!("     加载配置: {} = {} ({})", key, value, desc);
    }
    println!("   ✓ 配置预热完成 ({} 个配置项)\n", configs.len());

    // 2. 模拟预加载热点用户数据
    println!("2. 模拟热点用户数据预热");
    let hot_users = vec![1, 2, 3, 4, 5, 10, 100, 101];

    println!("   预加载热点用户...");
    let start = std::time::Instant::new();
    let mut handles = Vec::new();

    for user_id in &hot_users {
        let cache = cache.clone();
        let id = *user_id;
        let handle = tokio::spawn(async move {
            // 模拟从数据库查询用户
            // let user = db.query_user(id).await?;
            let username = format!("user_{}", id);
            let role = if id == 1 { "admin" } else { "user" };
            cache
                .set(&format!("user:{}", id), &format!("{}:{}", username, role), None)
                .await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!(
        "   ✓ 热点用户预热完成 ({} 个用户, 耗时: {:?})\n",
        hot_users.len(),
        elapsed
    );

    // 3. 验证预热数据
    println!("3. 验证预热数据");
    println!("   配置验证:");
    for (key, value, _) in &configs {
        let retrieved = cache.get(key).await?;
        match retrieved {
            Some(v) if v == *value => println!("     ✓ {} = {}", key, v),
            Some(v) => println!("     ✗ {} = {} (期望: {})", key, v, value),
            None => println!("     ✗ {} 未找到", key),
        }
    }

    println!("   \n   用户验证:");
    for user_id in &hot_users {
        let key = format!("user:{}", user_id);
        let retrieved = cache.get(&key).await?;
        match retrieved {
            Some(v) => println!("     ✓ {} = {}", key, v),
            None => println!("     ✗ {} 未找到", key),
        }
    }
    println!();

    // 4. 模拟缓存重建（故障恢复后）
    println!("4. 模拟缓存重建场景");
    println!("   清空缓存...");
    cache.clear().await?;

    println!("   重新预热...");
    let start = std::time::Instant::new();

    // 并发重新加载所有配置
    let mut handles = Vec::new();
    for (key, value, _) in &configs {
        let cache = cache.clone();
        let k = key.clone();
        let v = value.clone();
        let handle = tokio::spawn(async move {
            cache.set(&k, &v, None).await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!("   ✓ 缓存重建完成，耗时: {:?}", elapsed);
    println!();

    // 5. 统计信息
    println!("5. 预热后统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!();

    println!("=== 缓存预热示例完成 ===");
    Ok(())
}