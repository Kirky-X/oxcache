//! 动态配置示例
//!
//! 本示例演示如何使用 Oxcache 管理动态配置。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example dynamic_config
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ConfigValue {
    key: String,
    value: String,
    config_type: String,
    updated_at: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 动态配置示例 ===\n");

    // 创建配置缓存
    let cache: Cache<String, ConfigValue> = Cache::new().await?;

    // 1. 初始化配置
    println!("1. 初始化配置");
    let configs = vec![
        ConfigValue {
            key: "app.mode".to_string(),
            value: "production".to_string(),
            config_type: "app".to_string(),
            updated_at: chrono::Local::now().to_rfc3339(),
        },
        ConfigValue {
            key: "app.theme".to_string(),
            value: "dark".to_string(),
            config_type: "app".to_string(),
            updated_at: chrono::Local::now().to_rfc3339(),
        },
        ConfigValue {
            key: "database.timeout".to_string(),
            value: "30".to_string(),
            config_type: "database".to_string(),
            updated_at: chrono::Local::now().to_rfc3339(),
        },
        ConfigValue {
            key: "cache.size".to_string(),
            value: "10000".to_string(),
            config_type: "cache".to_string(),
            updated_at: chrono::Local::now().to_rfc3339(),
        },
    ];

    for config in &configs {
        cache.set(&config.key, config).await?;
        println!("   ✓ {} = {}", config.key, config.value);
    }
    println!();

    // 2. 配置查询
    println!("2. 配置查询");
    let query_keys = ["app.mode", "app.theme", "database.timeout", "cache.size"];

    for key in &query_keys {
        if let Some(config) = cache.get(&key.to_string()).await? {
            println!("   ✓ {} = {} (类型: {})", config.key, config.value, config.config_type);
        } else {
            println!("   ✗ {} 未找到", key);
        }
    }
    println!();

    // 3. 配置更新
    println!("3. 配置更新演示");
    println!("   更新 app.theme...");
    let updated_config = ConfigValue {
        key: "app.theme".to_string(),
        value: "light".to_string(),
        config_type: "app".to_string(),
        updated_at: chrono::Local::now().to_rfc3339(),
    };
    cache.set(&updated_config.key, &updated_config).await?;
    println!("   ✓ app.theme 更新为 light");

    // 验证更新
    if let Some(config) = cache.get(&"app.theme".to_string()).await? {
        println!("   ✓ 当前 app.theme = {}", config.value);
    }
    println!();

    // 4. 批量配置导出
    println!("4. 配置导出");
    println!("   导出所有配置:");
    // 注意: Cache 结构体不支持直接迭代，需要手动跟踪已添加的键
    for config in &configs {
        println!("     {} = {} ({})", config.key, config.value, config.config_type);
    }
    println!();

    // 5. 配置类型分组
    println!("5. 配置类型分组");
    let mut app_configs = Vec::new();
    let mut db_configs = Vec::new();

    for config in &configs {
        match config.config_type.as_str() {
            "app" => app_configs.push(config),
            "database" => db_configs.push(config),
            _ => {}
        }
    }

    println!("   应用配置 ({} 个):", app_configs.len());
    for config in &app_configs {
        println!("     {} = {}", config.key, config.value);
    }
    println!("   数据库配置 ({} 个):", db_configs.len());
    for config in &db_configs {
        println!("     {} = {}", config.key, config.value);
    }
    println!();

    // 清理
    println!("6. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 动态配置示例完成 ===");
    Ok(())
}
