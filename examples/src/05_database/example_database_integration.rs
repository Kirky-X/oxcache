// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据库集成使用示例
//!
//! 本示例演示了 Oxcache 的数据库集成功能：
//! - 缓存旁路模式
//! - 数据库加载器
//! - 连接字符串解析
//! - 分区策略
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_database_integration
//! ```

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_DATABASE_INTEGRATION_1_CREATING_A_CACHE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_2_CONNECTION_STRING_EXAMPLES,
    MSG_EXAMPLE_DATABASE_INTEGRATION_3_SIMULATING_A_DATABASE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_4_IMPLEMENTING_THE_CACHE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_5_TESTING_THE_CACHE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_6_BATCH_LOADING_EXAMPLE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_7_CACHE_UPDATES_AND,
    MSG_EXAMPLE_DATABASE_INTEGRATION_8_CACHE_INVALIDATION_EXAMPLE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_9_CACHE_STATISTICS,
    MSG_EXAMPLE_DATABASE_INTEGRATION_10_SIMULATING_A_PARTITIONING,
    MSG_EXAMPLE_DATABASE_INTEGRATION_ABSENT_FROM_THE_DATABASE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_BATCH_LOAD_RESULTS,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_ASIDE_CACHE_MISSES,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT, MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT_2,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_INVALIDATED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_MISS_QUERYING_THE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_127,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_HOST1,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_TYPE, MSG_EXAMPLE_DATABASE_INTEGRATION_CACHED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CAPACITY,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CONNECTION_MANAGEMENT_UNIFORM_CONNECTION,
    MSG_EXAMPLE_DATABASE_INTEGRATION_CORRECT_THE_USER_DOES,
    MSG_EXAMPLE_DATABASE_INTEGRATION_DATA_CONSISTENCY_PROMPT_UPDATES,
    MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_LOAD_SUCCEEDED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_ERROR,
    MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_SUCCEEDED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_ENTRIES, MSG_EXAMPLE_DATABASE_INTEGRATION_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_FAILED_TO_GET_STATISTICS,
    MSG_EXAMPLE_DATABASE_INTEGRATION_FIRST_QUERY_FOR_USER,
    MSG_EXAMPLE_DATABASE_INTEGRATION_IN_MEMORY_CACHE_CREATED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_INVALIDATING_THE_CACHE_FOR,
    MSG_EXAMPLE_DATABASE_INTEGRATION_KEY_BENEFITS_OF_DATABASE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_LESS_DATABASE_LOAD_HIGHER,
    MSG_EXAMPLE_DATABASE_INTEGRATION_LOAD_ERROR, MSG_EXAMPLE_DATABASE_INTEGRATION_NOT_FOUND,
    MSG_EXAMPLE_DATABASE_INTEGRATION_OXCACHE_CAN_CREATE_A,
    MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION,
    MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_ASSIGNMENT,
    MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_COUNT,
    MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITIONING_SUPPORTS_LARGE_SCALE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_PROCESSING_USER,
    MSG_EXAMPLE_DATABASE_INTEGRATION_QUERYING_THE_NONEXISTENT_USER,
    MSG_EXAMPLE_DATABASE_INTEGRATION_RELOAD_SUCCEEDED, MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT,
    MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_2, MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_CACHED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_SECOND_QUERY_FOR_USER,
    MSG_EXAMPLE_DATABASE_INTEGRATION_SEE_THE_EXAMPLE_REDIS,
    MSG_EXAMPLE_DATABASE_INTEGRATION_SIMULATED_DATABASE_LOADER_CREATED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_TRYING_TO_READ_FROM,
    MSG_EXAMPLE_DATABASE_INTEGRATION_UNEXPECTED_FOUND_THE_USER,
    MSG_EXAMPLE_DATABASE_INTEGRATION_UPDATING_THE_USER_RECORD,
    MSG_EXAMPLE_DATABASE_INTEGRATION_USAGE_EXAMPLE,
    MSG_EXAMPLE_DATABASE_INTEGRATION_USER_NOT_FOUND_IN,
    MSG_EXAMPLE_DATABASE_INTEGRATION_USER_RECORD_UPDATED,
    MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_INVALIDATION,
    MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_THE_UPDATE, t,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_USAGE_EXAMPLE, &[])
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_1_CREATING_A_CACHE, &[])
    );
    let cache: Cache<String, User> = Cache::builder().build().await?;
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_IN_MEMORY_CACHE_CREATED,
            &[]
        )
    );
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_2_CONNECTION_STRING_EXAMPLES,
            &[]
        )
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_OXCACHE_CAN_CREATE_A, &[])
    );
    println!(
        "   {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_127, &[])
    );
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_HOST1,
            &[]
        )
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_SEE_THE_EXAMPLE_REDIS, &[])
    );
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_3_SIMULATING_A_DATABASE,
            &[]
        )
    );

    struct MockDbLoader;

    impl MockDbLoader {
        async fn load_user_from_db(&self, user_id: &str) -> Result<Option<User>, String> {
            // 模拟数据库查询延迟
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // 模拟数据库中的一些用户数据
            match user_id {
                "user:1" => Ok(Some(User {
                    id: 1,
                    name: "Alice".to_string(),
                    email: "alice@example.com".to_string(),
                })),
                "user:2" => Ok(Some(User {
                    id: 2,
                    name: "Bob".to_string(),
                    email: "bob@example.com".to_string(),
                })),
                "user:3" => Ok(Some(User {
                    id: 3,
                    name: "Charlie".to_string(),
                    email: "charlie@example.com".to_string(),
                })),
                _ => Ok(None), // 模拟未找到用户
            }
        }
    }

    let db_loader = MockDbLoader;
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_SIMULATED_DATABASE_LOADER_CREATED,
            &[]
        )
    );
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_4_IMPLEMENTING_THE_CACHE,
            &[]
        )
    );

    async fn get_user_with_cache_bypass(
        cache: &Cache<String, User>,
        db_loader: &MockDbLoader,
        key: &str,
    ) -> Result<Option<User>, Box<dyn std::error::Error>> {
        // 首先尝试从缓存获取
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_TRYING_TO_READ_FROM,
                &[("key", key.to_string())]
            )
        );
        if let Some(user) = cache.get(&key.to_string()).await? {
            println!(
                "   {}",
                t(
                    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT,
                    &[("name", format!("{:?}", user.name))]
                )
            );
            return Ok(Some(user));
        }

        println!(
            "   {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_MISS_QUERYING_THE,
                &[]
            )
        );

        // 缓存未命中，从数据库加载
        match db_loader.load_user_from_db(key).await {
            Ok(Some(user)) => {
                println!(
                    "   {}",
                    t(
                        MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_SUCCEEDED,
                        &[("name", format!("{:?}", user.name))]
                    )
                );

                // 将结果存入缓存
                cache.set(&key.to_string(), &user).await?;
                println!(
                    "   {}",
                    t(MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_CACHED, &[])
                );

                Ok(Some(user))
            }
            Ok(None) => {
                println!(
                    "   {}",
                    t(MSG_EXAMPLE_DATABASE_INTEGRATION_USER_NOT_FOUND_IN, &[])
                );
                Ok(None)
            }
            Err(e) => {
                eprintln!(
                    "   {}",
                    t(
                        MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_ERROR,
                        &[("e", e.to_string())]
                    )
                );
                Err(e.into())
            }
        }
    }

    println!(
        "{}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_5_TESTING_THE_CACHE, &[])
    );

    // 首次查询（未缓存）
    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_FIRST_QUERY_FOR_USER, &[])
    );
    let user1 = get_user_with_cache_bypass(&cache, &db_loader, "user:1").await?;
    if let Some(user) = user1 {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT,
                &[("user", format!("{:?}", user))]
            )
        );
    }
    println!();

    // 第二次查询（缓存命中）
    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_SECOND_QUERY_FOR_USER, &[])
    );
    let user1_cached = get_user_with_cache_bypass(&cache, &db_loader, "user:1").await?;
    if let Some(user) = user1_cached {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_2,
                &[("user", format!("{:?}", user))]
            )
        );
    }
    println!();

    // 查询不存在的用户
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_QUERYING_THE_NONEXISTENT_USER,
            &[]
        )
    );
    let user_missing = get_user_with_cache_bypass(&cache, &db_loader, "user:999").await?;
    match user_missing {
        Some(user) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_UNEXPECTED_FOUND_THE_USER,
                &[("user", format!("{:?}", user))]
            )
        ),
        None => println!(
            "   {}",
            t(MSG_EXAMPLE_DATABASE_INTEGRATION_CORRECT_THE_USER_DOES, &[])
        ),
    }
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_6_BATCH_LOADING_EXAMPLE,
            &[]
        )
    );

    async fn batch_load_users_with_cache(
        cache: &Cache<String, User>,
        db_loader: &MockDbLoader,
        user_ids: &[&str],
    ) -> Result<Vec<Option<User>>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();

        for user_id in user_ids {
            println!(
                "  {}",
                t(
                    MSG_EXAMPLE_DATABASE_INTEGRATION_PROCESSING_USER,
                    &[("user_id", user_id.to_string())]
                )
            );

            if let Some(user) = cache.get(&user_id.to_string()).await? {
                println!(
                    "     {}",
                    t(MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT_2, &[])
                );
                results.push(Some(user));
                continue;
            }

            // 缓存未命中，从数据库加载
            match db_loader.load_user_from_db(user_id).await {
                Ok(Some(user)) => {
                    println!(
                        "     {}",
                        t(
                            MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_LOAD_SUCCEEDED,
                            &[]
                        )
                    );

                    // 存入缓存
                    cache.set(&user_id.to_string(), &user).await?;
                    println!("     {}", t(MSG_EXAMPLE_DATABASE_INTEGRATION_CACHED, &[]));

                    results.push(Some(user));
                }
                Ok(None) => {
                    println!(
                        "     {}",
                        t(
                            MSG_EXAMPLE_DATABASE_INTEGRATION_ABSENT_FROM_THE_DATABASE,
                            &[]
                        )
                    );
                    results.push(None);
                }
                Err(e) => {
                    eprintln!(
                        "     {}",
                        t(
                            MSG_EXAMPLE_DATABASE_INTEGRATION_LOAD_ERROR,
                            &[("e", e.to_string())]
                        )
                    );
                    results.push(None);
                }
            }
        }

        Ok(results)
    }

    let user_ids = vec!["user:1", "user:2", "user:3", "user:4"];
    let batch_results = batch_load_users_with_cache(&cache, &db_loader, &user_ids).await?;

    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_BATCH_LOAD_RESULTS, &[])
    );
    for (i, result) in batch_results.iter().enumerate() {
        match result {
            // CodeQL cleartext-logging：示例输出不去展示命中与否，但不回显明文数据
            Some(user) => println!(
                "     [batch entry {}]: <cached, {} chars>",
                i,
                user.name.len()
            ),
            None => println!(
                "     {}",
                t(
                    MSG_EXAMPLE_DATABASE_INTEGRATION_NOT_FOUND,
                    &[("i", user_ids[i].to_string())]
                )
            ),
        }
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_7_CACHE_UPDATES_AND, &[])
    );

    // 模拟用户信息更新
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_UPDATING_THE_USER_RECORD,
            &[]
        )
    );
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!(
        "   {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_USER_RECORD_UPDATED, &[])
    );

    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_THE_UPDATE,
                &[("name", format!("{:?}", cached_user.name))]
            )
        );
    }
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_8_CACHE_INVALIDATION_EXAMPLE,
            &[]
        )
    );

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_INVALIDATING_THE_CACHE_FOR,
            &[]
        )
    );
    cache.delete(&"user:2".to_string()).await?;
    println!(
        "   {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_INVALIDATED, &[])
    );

    // 验证失效 - 应该重新从数据库加载
    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_INVALIDATION, &[])
    );
    let user2_after_invalidation = get_user_with_cache_bypass(&cache, &db_loader, "user:2").await?;
    if let Some(user) = user2_after_invalidation {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_RELOAD_SUCCEEDED,
                &[("name", format!("{:?}", user.name))]
            )
        );
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_9_CACHE_STATISTICS, &[])
    );
    match cache.stats().await {
        Ok(stats) => {
            println!(
                "  {}",
                t(
                    MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_TYPE,
                    &[(
                        "stats",
                        (stats.get("type").unwrap_or(&"N/A".to_string())).to_string()
                    )]
                )
            );
            println!(
                "  {}",
                t(
                    MSG_EXAMPLE_DATABASE_INTEGRATION_ENTRIES,
                    &[(
                        "stats",
                        (stats.get("entry_count").unwrap_or(&"N/A".to_string())).to_string()
                    )]
                )
            );
            println!(
                "  {}",
                t(
                    MSG_EXAMPLE_DATABASE_INTEGRATION_CAPACITY,
                    &[(
                        "stats",
                        (stats.get("capacity").unwrap_or(&"N/A".to_string())).to_string()
                    )]
                )
            );
        }
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_FAILED_TO_GET_STATISTICS,
                &[("e", e.to_string())]
            )
        ),
    }
    println!();

    println!(
        "{}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_10_SIMULATING_A_PARTITIONING,
            &[]
        )
    );

    // 简单的哈希分区策略
    fn simple_partition(key: &str, num_partitions: usize) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash_value = hasher.finish();
        (hash_value as usize) % num_partitions
    }

    let partitions = 4;
    let sample_keys = vec!["user:1", "user:2", "user:3", "user:4", "user:5", "user:6"];

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_COUNT,
            &[("partitions", partitions.to_string())]
        )
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_ASSIGNMENT, &[])
    );

    for key in &sample_keys {
        let partition = simple_partition(key, partitions);
        println!(
            "     {}",
            t(
                MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION,
                &[
                    ("key", key.to_string()),
                    ("partition", partition.to_string())
                ]
            )
        );
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_DATABASE_INTEGRATION_EXAMPLE_COMPLETE, &[])
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_KEY_BENEFITS_OF_DATABASE,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_ASIDE_CACHE_MISSES,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_LESS_DATABASE_LOAD_HIGHER,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_DATA_CONSISTENCY_PROMPT_UPDATES,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITIONING_SUPPORTS_LARGE_SCALE,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DATABASE_INTEGRATION_CONNECTION_MANAGEMENT_UNIFORM_CONNECTION,
            &[]
        )
    );

    Ok(())
}
