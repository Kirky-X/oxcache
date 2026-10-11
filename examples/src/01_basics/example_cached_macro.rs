// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! #[cached] 宏使用示例
//!
//! 本示例演示了 Oxcache 的 #[cached] 宏功能：
//! - 零样板缓存装饰
//! - 自动序列化/反序列化
//! - 灵活的键生成策略
//! - TTL 控制
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_cached_macro
//! ```

use oxcache::Cache;
use oxcache::macros::cached;
use serde::{Deserialize, Serialize};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CACHED_MACRO_1_BASIC_CACHING_DEMONSTRATION,
    MSG_EXAMPLE_CACHED_MACRO_2_CUSTOM_KEY_FORMAT, MSG_EXAMPLE_CACHED_MACRO_3_SHORT_TTL_CACHING,
    MSG_EXAMPLE_CACHED_MACRO_4_PERFORMANCE_COMPARISON,
    MSG_EXAMPLE_CACHED_MACRO_5_MULTI_ARGUMENT_FUNCTION,
    MSG_EXAMPLE_CACHED_MACRO_AUTOMATIC_SERIALIZATION_NO_MANUAL,
    MSG_EXAMPLE_CACHED_MACRO_CACHE_HIT_TOOK, MSG_EXAMPLE_CACHED_MACRO_CACHE_HITS_ARE_TYPICALLY,
    MSG_EXAMPLE_CACHED_MACRO_CACHE_REGISTERED_FOR_USE, MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT,
    MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_2, MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_3,
    MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA, MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA_2,
    MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM,
    MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM_2,
    MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_WITH,
    MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_DIFFERENT_ARGUMENTS,
    MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_THE_SAME,
    MSG_EXAMPLE_CACHED_MACRO_DIFFERENT_ARGUMENT_RESULT, MSG_EXAMPLE_CACHED_MACRO_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION,
    MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_2,
    MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_3,
    MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_4,
    MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_OF_GET, MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_RESULT,
    MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_TOOK,
    MSG_EXAMPLE_CACHED_MACRO_FLEXIBLE_CONFIGURATION_SUPPORTS_MULTIPLE,
    MSG_EXAMPLE_CACHED_MACRO_MAIN_ADVANTAGES_OF_THE,
    MSG_EXAMPLE_CACHED_MACRO_PERFORMANCE_SIGNIFICANTLY_IMPROVES_CACHE,
    MSG_EXAMPLE_CACHED_MACRO_RESULT, MSG_EXAMPLE_CACHED_MACRO_RESULT_2,
    MSG_EXAMPLE_CACHED_MACRO_RESULT_3, MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_OF_GET,
    MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_RESULT, MSG_EXAMPLE_CACHED_MACRO_SPEEDUP_X,
    MSG_EXAMPLE_CACHED_MACRO_TYPE_SAFE_CHECKED_AT, MSG_EXAMPLE_CACHED_MACRO_USAGE_EXAMPLE,
    MSG_EXAMPLE_CACHED_MACRO_ZERO_BOILERPLATE_CACHING_LOGIC, t,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

// 使用 #[cached] 宏一行代码启用缓存
#[cached(service = "user_cache", ttl = 600)]
async fn get_user(id: u64) -> Result<User, String> {
    // 模拟耗时的数据库查询
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION,
            &[]
        )
    );
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

// 使用自定义键格式的缓存函数
#[cached(service = "user_cache", ttl = 600, key_prefix = "user")]
async fn get_user_custom_key(id: u64) -> Result<User, String> {
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_2,
            &[]
        )
    );
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("Custom Key User {}", id),
    })
}

// 使用较短 TTL 的缓存函数（演示不同的 TTL 配置）
#[cached(service = "user_cache", ttl = 300)]
async fn get_hot_data(id: u64) -> Result<String, String> {
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_3,
            &[]
        )
    );
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(format!("Hot Data for {}", id))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_CACHED_MACRO_USAGE_EXAMPLE, &[]));

    // 初始化缓存
    // 注意：register_for_macro 仅支持 Cache<String, Vec<u8>>
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // 注册缓存实例到全局管理器（供宏使用）
    cache.register_for_macro("user_cache").await?;
    println!(
        "{}\n",
        t(MSG_EXAMPLE_CACHED_MACRO_CACHE_REGISTERED_FOR_USE, &[])
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_CACHED_MACRO_1_BASIC_CACHING_DEMONSTRATION, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_OF_GET, &[]));
    let user = get_user(1).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_RESULT,
            &[("user", format!("{:?}", user))]
        )
    );

    println!("  {}", t(MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_OF_GET, &[]));
    let cached_user = get_user(1).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_RESULT,
            &[("cached_user", format!("{:?}", cached_user))]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_CACHED_MACRO_2_CUSTOM_KEY_FORMAT, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM, &[])
    );
    let user_custom = get_user_custom_key(2).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_RESULT,
            &[("user_custom", format!("{:?}", user_custom))]
        )
    );

    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM_2, &[])
    );
    let cached_custom = get_user_custom_key(2).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT,
            &[("cached_custom", format!("{:?}", cached_custom))]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_CACHED_MACRO_3_SHORT_TTL_CACHING, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA, &[])
    );
    let hot_data = get_hot_data(1).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_RESULT_2,
            &[("hot_data", hot_data.to_string())]
        )
    );

    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA_2, &[])
    );
    let cached_hot_data = get_hot_data(1).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_2,
            &[("cached_hot_data", cached_hot_data.to_string())]
        )
    );
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_CACHED_MACRO_4_PERFORMANCE_COMPARISON, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CACHE_HITS_ARE_TYPICALLY, &[])
    );

    // 首次调用（未缓存）
    let start = std::time::Instant::now();
    let _first_call = get_user(3).await?;
    let first_duration = start.elapsed();
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_TOOK,
            &[("first_duration", format!("{:?}", first_duration))]
        )
    );

    // 第二次调用（缓存命中）
    let start = std::time::Instant::now();
    let _second_call = get_user(3).await?;
    let second_duration = start.elapsed();
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_CACHE_HIT_TOOK,
            &[("second_duration", format!("{:?}", second_duration))]
        )
    );

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_SPEEDUP_X,
            &[(
                "f64",
                format!(
                    "{:.2}",
                    first_duration.as_millis() as f64 / second_duration.as_millis() as f64
                )
            )]
        )
    );
    println!();

    #[cached(service = "user_cache", ttl = 600)]
    async fn get_user_with_role(user_id: u64, role: String) -> Result<String, String> {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_4,
                &[]
            )
        );
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        Ok(format!("User {} with role {}", user_id, role))
    }

    println!(
        "{}",
        t(MSG_EXAMPLE_CACHED_MACRO_5_MULTI_ARGUMENT_FUNCTION, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_WITH, &[])
    );
    let result = get_user_with_role(4, "admin".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_RESULT_3,
            &[("result", result.to_string())]
        )
    );

    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_THE_SAME, &[])
    );
    let cached_result = get_user_with_role(4, "admin".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_3,
            &[("cached_result", cached_result.to_string())]
        )
    );

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_DIFFERENT_ARGUMENTS,
            &[]
        )
    );
    let diff_result = get_user_with_role(4, "user".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_DIFFERENT_ARGUMENT_RESULT,
            &[("diff_result", diff_result.to_string())]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_CACHED_MACRO_EXAMPLE_COMPLETE, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_MAIN_ADVANTAGES_OF_THE, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_ZERO_BOILERPLATE_CACHING_LOGIC, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHED_MACRO_TYPE_SAFE_CHECKED_AT, &[])
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_FLEXIBLE_CONFIGURATION_SUPPORTS_MULTIPLE,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_AUTOMATIC_SERIALIZATION_NO_MANUAL,
            &[]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHED_MACRO_PERFORMANCE_SIGNIFICANTLY_IMPROVES_CACHE,
            &[]
        )
    );

    Ok(())
}
