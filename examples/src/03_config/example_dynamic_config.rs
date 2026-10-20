// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 动态配置示例
//!
//! 本示例演示如何使用 Oxcache 管理动态配置。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example dynamic_config
//!

use oxcache::Cache;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_DYNAMIC_CONFIG_1_INITIALIZING_CONFIG, MSG_EXAMPLE_DYNAMIC_CONFIG_2_CONFIG_QUERIES,
    MSG_EXAMPLE_DYNAMIC_CONFIG_3_CONFIG_UPDATE_DEMONSTRATION,
    MSG_EXAMPLE_DYNAMIC_CONFIG_4_CONFIG_EXPORT, MSG_EXAMPLE_DYNAMIC_CONFIG_5_CONFIG_GROUPS_BY,
    MSG_EXAMPLE_DYNAMIC_CONFIG_6_CLEANING_UP_TEST, MSG_EXAMPLE_DYNAMIC_CONFIG_APP_THEME_UPDATED_TO,
    MSG_EXAMPLE_DYNAMIC_CONFIG_APPLICATION_CONFIG_ITEMS,
    MSG_EXAMPLE_DYNAMIC_CONFIG_CURRENT_APP_THEME, MSG_EXAMPLE_DYNAMIC_CONFIG_DATABASE_CONFIG_ITEMS,
    MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE, MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_DYNAMIC_CONFIG_EXPORTING_ALL_CONFIG, MSG_EXAMPLE_DYNAMIC_CONFIG_NOT_FOUND,
    MSG_EXAMPLE_DYNAMIC_CONFIG_TEST_DATA_CLEANED_UP, MSG_EXAMPLE_DYNAMIC_CONFIG_TYPE,
    MSG_EXAMPLE_DYNAMIC_CONFIG_UPDATING_APP_THEME, t,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ConfigValue {
    key: String,
    value: String,
    config_type: String,
    updated_at: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE, &[]));

    let cache: Cache<String, ConfigValue> = Cache::builder().build().await?;

    println!(
        "{}",
        t(MSG_EXAMPLE_DYNAMIC_CONFIG_1_INITIALIZING_CONFIG, &[])
    );
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

    println!("{}", t(MSG_EXAMPLE_DYNAMIC_CONFIG_2_CONFIG_QUERIES, &[]));
    let query_keys = ["app.mode", "app.theme", "database.timeout", "cache.size"];

    for key in &query_keys {
        if let Some(config) = cache.get(&key.to_string()).await? {
            println!(
                "   {}",
                t(
                    MSG_EXAMPLE_DYNAMIC_CONFIG_TYPE,
                    &[
                        ("key", config.key.to_string()),
                        ("value", config.value.to_string()),
                        ("config_type", config.config_type.to_string())
                    ]
                )
            );
        } else {
            println!(
                "   {}",
                t(
                    MSG_EXAMPLE_DYNAMIC_CONFIG_NOT_FOUND,
                    &[("key", key.to_string())]
                )
            );
        }
    }
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DYNAMIC_CONFIG_3_CONFIG_UPDATE_DEMONSTRATION,
            &[]
        )
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_DYNAMIC_CONFIG_UPDATING_APP_THEME, &[])
    );
    let updated_config = ConfigValue {
        key: "app.theme".to_string(),
        value: "light".to_string(),
        config_type: "app".to_string(),
        updated_at: chrono::Local::now().to_rfc3339(),
    };
    cache.set(&updated_config.key, &updated_config).await?;
    println!(
        "   {}",
        t(MSG_EXAMPLE_DYNAMIC_CONFIG_APP_THEME_UPDATED_TO, &[])
    );

    if let Some(config) = cache.get(&"app.theme".to_string()).await? {
        println!(
            "   {}",
            t(
                MSG_EXAMPLE_DYNAMIC_CONFIG_CURRENT_APP_THEME,
                &[("value", config.value.to_string())]
            )
        );
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_DYNAMIC_CONFIG_4_CONFIG_EXPORT, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_DYNAMIC_CONFIG_EXPORTING_ALL_CONFIG, &[])
    );
    // 注意: Cache 结构体不支持直接迭代，需要手动跟踪已添加的键
    for config in &configs {
        println!(
            "     {} = {} ({})",
            config.key, config.value, config.config_type
        );
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_DYNAMIC_CONFIG_5_CONFIG_GROUPS_BY, &[]));
    let mut app_configs = Vec::new();
    let mut db_configs = Vec::new();

    for config in &configs {
        match config.config_type.as_str() {
            "app" => app_configs.push(config),
            "database" => db_configs.push(config),
            _ => {}
        }
    }

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DYNAMIC_CONFIG_APPLICATION_CONFIG_ITEMS,
            &[("app_configs", app_configs.len().to_string())]
        )
    );
    for config in &app_configs {
        println!("     {} = {}", config.key, config.value);
    }
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DYNAMIC_CONFIG_DATABASE_CONFIG_ITEMS,
            &[("db_configs", db_configs.len().to_string())]
        )
    );
    for config in &db_configs {
        println!("     {} = {}", config.key, config.value);
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_DYNAMIC_CONFIG_6_CLEANING_UP_TEST, &[]));
    cache.clear().await?;
    println!(
        "   {}\n",
        t(MSG_EXAMPLE_DYNAMIC_CONFIG_TEST_DATA_CLEANED_UP, &[])
    );

    println!("{}", t(MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
