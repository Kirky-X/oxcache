//! SQLite 缓存配置示例
//!
//! 本示例演示如何使用 Oxcache 缓存配置数据 (类似 SQLite 的键值存储)。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_sqlite_cache
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Config {
    key: String,
    value: String,
    #[serde(default)]
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SQLite 缓存配置示例 ===\n");

    // 创建内存缓存 (模拟 SQLite 的本地存储)
    println!("创建内存缓存...");
    let cache: Cache<String, Config> = Cache::new().await?;
    println!("✓ 缓存创建成功\n");

    // 应用配置
    println!("1. 应用配置管理");
    let configs = vec![
        Config {
            key: "app.name".to_string(),
            value: "Oxcache Demo".to_string(),
            description: "应用名称".to_string(),
        },
        Config {
            key: "app.version".to_string(),
            value: "1.0.0".to_string(),
            description: "应用版本".to_string(),
        },
        Config {
            key: "database.path".to_string(),
            value: "./data.db".to_string(),
            description: "数据库路径".to_string(),
        },
        Config {
            key: "cache.size".to_string(),
            value: "10000".to_string(),
            description: "缓存大小".to_string(),
        },
    ];

    println!("   添加配置项...");
    for config in &configs {
        cache.set(&config.key, config, None).await?;
        println!("   ✓ {} = {} ({})", config.key, config.value, config.description);
    }
    println!();

    // 配置查询
    println!("2. 配置查询");
    let query_keys = ["app.name", "app.version", "database.path", "cache.size"];

    for key in &query_keys {
        let start = std::time::Instant::new();
        let config = cache.get(key).await?;
        let elapsed = start.elapsed();

        match config {
            Some(c) => println!("   ✓ {} = {} (耗时: {:?})", c.key, c.value, elapsed),
            None => println!("   ✗ {} 未找到", key),
        }
    }
    println!();

    // 配置更新
    println!("3. 配置更新演示");
    println!("   更新 app.version...");
    let new_version = Config {
        key: "app.version".to_string(),
        value: "1.1.0".to_string(),
        description: "应用版本".to_string(),
    };
    cache.set("app.version", &new_version, None).await?;
    println!("   ✓ app.version 更新为 1.1.0");

    // 验证更新
    if let Some(c) = cache.get("app.version").await? {
        println!("   ✓ 当前 app.version = {}", c.value);
    }
    println!();

    // 批量导出
    println!("4. 配置导出");
    println!("   导出所有配置项:");
    let all_configs = cache.iter().await?;
    println!("   导出 {} 个配置项:", all_configs.len());
    for (key, config) in all_configs {
        println!("     {} = {}", key, config.value);
    }
    println!();

    // 清理
    println!("5. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    // 统计信息
    println!("6. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    println!();

    println!("=== SQLite 缓存配置示例完成 ===");
    Ok(())
}