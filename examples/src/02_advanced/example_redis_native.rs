// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis 原生客户端示例
//!
//! 本示例演示如何使用 Oxcache 连接 Redis。
//!
//! 运行方式：
//! ```bash
//! # 本地开发需允许非 TLS 连接（生产请使用 rediss://）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --example redis_native
//! ```
//!（连接本机 Redis 时同样需要上述环境变量）

use oxcache::Cache;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_REDIS_NATIVE_1_CONNECT_TO_REDIS, MSG_EXAMPLE_REDIS_NATIVE_2_BASIC_CRUD_OPERATIONS,
    MSG_EXAMPLE_REDIS_NATIVE_3_BATCH_OPERATIONS, MSG_EXAMPLE_REDIS_NATIVE_4_CACHE_STATISTICS,
    MSG_EXAMPLE_REDIS_NATIVE_ADDING_USER, MSG_EXAMPLE_REDIS_NATIVE_ADDING_USERS_IN_BATCH,
    MSG_EXAMPLE_REDIS_NATIVE_BATCH_ADD_SUCCEEDED,
    MSG_EXAMPLE_REDIS_NATIVE_CACHE_STATISTICS_ARE_AVAILABLE, MSG_EXAMPLE_REDIS_NATIVE_CLEARED,
    MSG_EXAMPLE_REDIS_NATIVE_CLEARING_TEST_DATA, MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE,
    MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE_2, MSG_EXAMPLE_REDIS_NATIVE_DELETING_USER,
    MSG_EXAMPLE_REDIS_NATIVE_FAILED_TO_DELETE_THE,
    MSG_EXAMPLE_REDIS_NATIVE_FETCHING_USERS_IN_BATCH, MSG_EXAMPLE_REDIS_NATIVE_GETTING_USER,
    MSG_EXAMPLE_REDIS_NATIVE_REDIS_CONNECTED, MSG_EXAMPLE_REDIS_NATIVE_SEE_THE_METRICS_TEST,
    MSG_EXAMPLE_REDIS_NATIVE_UPDATING_USER, MSG_EXAMPLE_REDIS_NATIVE_USER,
    MSG_EXAMPLE_REDIS_NATIVE_USER_ADDED, MSG_EXAMPLE_REDIS_NATIVE_USER_DELETED,
    MSG_EXAMPLE_REDIS_NATIVE_USER_NOT_FOUND, MSG_EXAMPLE_REDIS_NATIVE_USER_RETRIEVED,
    MSG_EXAMPLE_REDIS_NATIVE_USER_UPDATED, t,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RedisUser {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE, &[]));

    println!("{}", t(MSG_EXAMPLE_REDIS_NATIVE_1_CONNECT_TO_REDIS, &[]));
    let cache: Cache<String, RedisUser> = Cache::redis("redis://127.0.0.1:6379").await?;
    println!("   {}\n", t(MSG_EXAMPLE_REDIS_NATIVE_REDIS_CONNECTED, &[]));

    println!(
        "{}",
        t(MSG_EXAMPLE_REDIS_NATIVE_2_BASIC_CRUD_OPERATIONS, &[])
    );
    let user = RedisUser {
        id: 1,
        name: "Alice".to_string(),
        email: "zhangsan@example.com".to_string(),
    };

    println!("  {}", t(MSG_EXAMPLE_REDIS_NATIVE_ADDING_USER, &[]));
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   {}", t(MSG_EXAMPLE_REDIS_NATIVE_USER_ADDED, &[]));

    println!("  {}", t(MSG_EXAMPLE_REDIS_NATIVE_GETTING_USER, &[]));
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(u) => println!(
            "   {}",
            t(
                MSG_EXAMPLE_REDIS_NATIVE_USER_RETRIEVED,
                &[("name", u.name.to_string()), ("email", u.email.to_string())]
            )
        ),
        None => println!("   {}", t(MSG_EXAMPLE_REDIS_NATIVE_USER_NOT_FOUND, &[])),
    }

    println!("  {}", t(MSG_EXAMPLE_REDIS_NATIVE_UPDATING_USER, &[]));
    let updated_user = RedisUser {
        id: 1,
        name: "Bob".to_string(),
        email: "zhangsanfeng@example.com".to_string(),
    };
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   {}", t(MSG_EXAMPLE_REDIS_NATIVE_USER_UPDATED, &[]));

    println!("  {}", t(MSG_EXAMPLE_REDIS_NATIVE_DELETING_USER, &[]));
    cache.delete(&"user:1".to_string()).await?;
    let retrieved = cache.get(&"user:1".to_string()).await?;
    match retrieved {
        Some(_) => println!(
            "   {}",
            t(MSG_EXAMPLE_REDIS_NATIVE_FAILED_TO_DELETE_THE, &[])
        ),
        None => println!("   {}", t(MSG_EXAMPLE_REDIS_NATIVE_USER_DELETED, &[])),
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_REDIS_NATIVE_3_BATCH_OPERATIONS, &[]));
    let users = vec![
        RedisUser {
            id: 2,
            name: "Charlie".to_string(),
            email: "lisi@example.com".to_string(),
        },
        RedisUser {
            id: 3,
            name: "David".to_string(),
            email: "wangwu@example.com".to_string(),
        },
    ];

    println!(
        "  {}",
        t(MSG_EXAMPLE_REDIS_NATIVE_ADDING_USERS_IN_BATCH, &[])
    );
    for user in &users {
        cache.set(&format!("user:{}", user.id), user).await?;
    }
    println!(
        "   {}",
        t(MSG_EXAMPLE_REDIS_NATIVE_BATCH_ADD_SUCCEEDED, &[])
    );

    println!(
        "  {}",
        t(MSG_EXAMPLE_REDIS_NATIVE_FETCHING_USERS_IN_BATCH, &[])
    );
    for user in &users {
        if let Some(u) = cache.get(&format!("user:{}", user.id)).await? {
            println!(
                "   {}",
                t(
                    MSG_EXAMPLE_REDIS_NATIVE_USER,
                    &[("id", u.id.to_string()), ("name", u.name.to_string())]
                )
            );
        }
    }

    // 清空（安全方式：仅删除本示例写入的测试键，避免全库 clear）
    println!("  {}", t(MSG_EXAMPLE_REDIS_NATIVE_CLEARING_TEST_DATA, &[]));
    for key in ["user:1", "user:2", "user:3"] {
        cache.delete(&key.to_string()).await?;
    }
    println!("   {}\n", t(MSG_EXAMPLE_REDIS_NATIVE_CLEARED, &[]));

    println!("{}", t(MSG_EXAMPLE_REDIS_NATIVE_4_CACHE_STATISTICS, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_REDIS_NATIVE_CACHE_STATISTICS_ARE_AVAILABLE, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_REDIS_NATIVE_SEE_THE_METRICS_TEST, &[])
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE_2, &[]));
    Ok(())
}
