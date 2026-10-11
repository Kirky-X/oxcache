// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 序列化选项示例：JSON（人类可读）与 Bincode（二进制、更高效）

use oxcache::Cache;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_SERIALIZATION_1_JSON_SERIALIZATION_DEFAULT,
    MSG_EXAMPLE_SERIALIZATION_2_SERIALIZATION_FEATURES,
    MSG_EXAMPLE_SERIALIZATION_BINCODE_COMPACT_BINARY_FORMAT, MSG_EXAMPLE_SERIALIZATION_CONFIG,
    MSG_EXAMPLE_SERIALIZATION_EXAMPLE_COMPLETE, MSG_EXAMPLE_SERIALIZATION_GOOD_FOR_DEBUGGING,
    MSG_EXAMPLE_SERIALIZATION_HUMAN_READABLE_FORMAT,
    MSG_EXAMPLE_SERIALIZATION_JSON_STANDARD_READABLE_FORMAT,
    MSG_EXAMPLE_SERIALIZATION_OPTIONS_EXAMPLE, MSG_EXAMPLE_SERIALIZATION_RETRIEVED,
    MSG_EXAMPLE_SERIALIZATION_SLIGHTLY_LARGER_SIZE, MSG_EXAMPLE_SERIALIZATION_STORED_USER,
    MSG_EXAMPLE_SERIALIZATION_SUPPORTS_CUSTOM_SERIALIZERS, t,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    profile: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, User> = Cache::builder().build().await?;

    println!("{}", t(MSG_EXAMPLE_SERIALIZATION_OPTIONS_EXAMPLE, &[]));
    println!("========================\n");

    // 使用JSON创建测试数据
    let user = User {
        id: 1,
        name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        profile: serde_json::json!({
            "age": 30,
            "city": "New York",
            "skills": ["Rust", "Redis", "Cache"]
        }),
    };

    println!(
        "{}",
        t(MSG_EXAMPLE_SERIALIZATION_1_JSON_SERIALIZATION_DEFAULT, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SERIALIZATION_HUMAN_READABLE_FORMAT, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_SERIALIZATION_GOOD_FOR_DEBUGGING, &[]));
    println!(
        "  {}\n",
        t(MSG_EXAMPLE_SERIALIZATION_SLIGHTLY_LARGER_SIZE, &[])
    );

    // 使用JSON存储
    cache.set(&"user:json:1".to_string(), &user).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_SERIALIZATION_STORED_USER,
            &[
                ("name", user.name.to_string()),
                ("email", user.email.to_string())
            ]
        )
    );

    // 检索
    if let Some(cached) = cache.get(&"user:json:1".to_string()).await? {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_SERIALIZATION_RETRIEVED,
                &[
                    ("name", cached.name.to_string()),
                    ("email", cached.email.to_string())
                ]
            )
        );
        println!(
            "  {}\n",
            t(
                MSG_EXAMPLE_SERIALIZATION_CONFIG,
                &[("profile", format!("{:?}", cached.profile))]
            )
        );
    }

    println!(
        "{}",
        t(MSG_EXAMPLE_SERIALIZATION_2_SERIALIZATION_FEATURES, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SERIALIZATION_JSON_STANDARD_READABLE_FORMAT, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SERIALIZATION_BINCODE_COMPACT_BINARY_FORMAT, &[])
    );
    println!(
        "  {}\n",
        t(MSG_EXAMPLE_SERIALIZATION_SUPPORTS_CUSTOM_SERIALIZERS, &[])
    );

    println!("{}", t(MSG_EXAMPLE_SERIALIZATION_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
