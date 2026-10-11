// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 基本 CRUD 操作示例（get / set / delete）

use oxcache::Cache;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_BASIC_OPERATIONS_DELETE_REMOVE_A_VALUE, MSG_EXAMPLE_BASIC_OPERATIONS_DELETING_USER,
    MSG_EXAMPLE_BASIC_OPERATIONS_EXAMPLE_COMPLETE, MSG_EXAMPLE_BASIC_OPERATIONS_GET_READ_A_VALUE,
    MSG_EXAMPLE_BASIC_OPERATIONS_GETTING_USER, MSG_EXAMPLE_BASIC_OPERATIONS_GOT_USER,
    MSG_EXAMPLE_BASIC_OPERATIONS_SET_STORE_A_VALUE, MSG_EXAMPLE_BASIC_OPERATIONS_SET_USER,
    MSG_EXAMPLE_BASIC_OPERATIONS_UPDATE_OVERWRITE_THE_EXISTING,
    MSG_EXAMPLE_BASIC_OPERATIONS_UPDATED_USER, MSG_EXAMPLE_BASIC_OPERATIONS_UPDATING_USER, t,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    println!(
        "{}",
        t(
            MSG_EXAMPLE_BASIC_OPERATIONS_SET_USER,
            &[("name", user.name.to_string())]
        )
    );
    cache.set(&"user:1".to_string(), &user).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_some());

    println!("{}", t(MSG_EXAMPLE_BASIC_OPERATIONS_GETTING_USER, &[]));
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!(
            "{}",
            t(
                MSG_EXAMPLE_BASIC_OPERATIONS_GOT_USER,
                &[
                    ("name", cached_user.name.to_string()),
                    ("email", cached_user.email.to_string())
                ]
            )
        );
        assert_eq!(cached_user.id, 1);
    }

    println!("{}", t(MSG_EXAMPLE_BASIC_OPERATIONS_DELETING_USER, &[]));
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());

    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("{}", t(MSG_EXAMPLE_BASIC_OPERATIONS_UPDATING_USER, &[]));
    cache.set(&"user:1".to_string(), &updated_user).await?;
    if let Some(cached) = cache.get(&"user:1".to_string()).await? {
        println!(
            "{}",
            t(
                MSG_EXAMPLE_BASIC_OPERATIONS_UPDATED_USER,
                &[
                    ("name", cached.name.to_string()),
                    ("email", cached.email.to_string())
                ]
            )
        );
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_BASIC_OPERATIONS_EXAMPLE_COMPLETE, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_BASIC_OPERATIONS_SET_STORE_A_VALUE, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_BASIC_OPERATIONS_GET_READ_A_VALUE, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_BASIC_OPERATIONS_DELETE_REMOVE_A_VALUE, &[])
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BASIC_OPERATIONS_UPDATE_OVERWRITE_THE_EXISTING,
            &[]
        )
    );
    Ok(())
}
