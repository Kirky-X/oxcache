// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 键生成器示例
//!
//! 本示例演示 oxcache 的 KeyGenerator 工具：
//! - 使用命名空间隔离不同应用的键
//! - 使用前缀组织键的层次结构（前缀按原样直拼，需自带尾随冒号）
//! - 使用模板生成动态键，并区分 generate 与 generate_full 的包装行为
//! - 验证键的合法性
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_key_generator
//! ```

use oxcache::KeyGenerator;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_KEY_GENERATOR_1_BASIC_USAGE, MSG_EXAMPLE_KEY_GENERATOR_2_USING_NAMESPACES,
    MSG_EXAMPLE_KEY_GENERATOR_3_USING_PREFIXES,
    MSG_EXAMPLE_KEY_GENERATOR_4_CHAINING_NAMESPACE_PREFIX,
    MSG_EXAMPLE_KEY_GENERATOR_5_TEMPLATE_GENERATION,
    MSG_EXAMPLE_KEY_GENERATOR_6_FULL_KEY_GENERATION, MSG_EXAMPLE_KEY_GENERATOR_7_KEY_VALIDATION,
    MSG_EXAMPLE_KEY_GENERATOR_8_IN_PRACTICE_CACHE, MSG_EXAMPLE_KEY_GENERATOR_CACHE_KEY,
    MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_MAX_LENGTH_256,
    MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_NAMESPACE_DEFAULT, MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE,
    MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE_COMPLETE, MSG_EXAMPLE_KEY_GENERATOR_FULL_KEY,
    MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_SEARCH_TYPE,
    MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_USER_ID,
    MSG_EXAMPLE_KEY_GENERATOR_GENERATE_USER_ID_PROFILE, MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID,
    MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID_2, MSG_EXAMPLE_KEY_GENERATOR_IS_VALID,
    MSG_EXAMPLE_KEY_GENERATOR_IS_VALID_2, MSG_EXAMPLE_KEY_GENERATOR_ORDER_KEY,
    MSG_EXAMPLE_KEY_GENERATOR_SESSION_KEY, MSG_EXAMPLE_KEY_GENERATOR_STORE,
    MSG_EXAMPLE_KEY_GENERATOR_STORE_2, MSG_EXAMPLE_KEY_GENERATOR_USER_KEY, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE, &[]));

    println!("{}", t(MSG_EXAMPLE_KEY_GENERATOR_1_BASIC_USAGE, &[]));
    let _gen = KeyGenerator::new();
    println!(
        "  {}",
        t(MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_NAMESPACE_DEFAULT, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_MAX_LENGTH_256, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_KEY_GENERATOR_2_USING_NAMESPACES, &[]));
    let user_gen = KeyGenerator::new().with_namespace("users");
    let order_gen = KeyGenerator::new().with_namespace("orders");

    let user_key = user_gen.namespaced_key("user:123");
    let order_key = order_gen.namespaced_key("order:456");
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_USER_KEY,
            &[("user_key", user_key.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_ORDER_KEY,
            &[("order_key", order_key.to_string())]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_KEY_GENERATOR_3_USING_PREFIXES, &[]));
    // 前缀按原样直拼（不加分隔符），需自带尾随冒号；
    // 且仅 generate_full/try_generate_full/json_hash_key 会套用前缀
    let cache_gen = KeyGenerator::with_prefix("cache:");
    let session_gen = KeyGenerator::with_prefix("session:");

    let key1 = cache_gen.generate_full("item:{id}", &[("id", "1")]);
    let key2 = session_gen.generate_full("token:{value}", &[("value", "abc")]);
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_CACHE_KEY,
            &[("key1", key1.to_string())]
        )
    ); // cache:item:1
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_SESSION_KEY,
            &[("key2", key2.to_string())]
        )
    ); // session:token:abc

    println!(
        "\n{}",
        t(MSG_EXAMPLE_KEY_GENERATOR_4_CHAINING_NAMESPACE_PREFIX, &[])
    );
    let generator = KeyGenerator::new()
        .with_namespace("myapp")
        .with_prefix_str("v2:")
        .with_max_key_length(512);

    let key = generator.generate_full("user:profile:{id}", &[("id", "123")]);
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_FULL_KEY,
            &[("key", key.to_string())]
        )
    ); // myapp:v2:user:profile:123

    println!(
        "\n{}",
        t(MSG_EXAMPLE_KEY_GENERATOR_5_TEMPLATE_GENERATION, &[])
    );
    let generator = KeyGenerator::new().with_namespace("api");

    // generate 只替换模板占位符，不包装命名空间/前缀
    let key = generator.generate("user:{id}:profile", &[("id", "42")]);
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_GENERATE_USER_ID_PROFILE,
            &[("key", key.to_string())]
        )
    ); // user:42:profile

    // generate_full 在模板结果外追加前缀与命名空间
    let key = generator.generate_full(
        "search:{type}:{query}",
        &[("type", "products"), ("query", "laptop")],
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_SEARCH_TYPE,
            &[("key", key.to_string())]
        )
    ); // api:search:products:laptop

    println!(
        "\n{}",
        t(MSG_EXAMPLE_KEY_GENERATOR_6_FULL_KEY_GENERATION, &[])
    );
    let generator = KeyGenerator::new()
        .with_namespace("production")
        .with_prefix_str("cache:");

    let key = generator.generate_full("user:{id}", &[("id", "123")]);
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_USER_ID,
            &[("key", key.to_string())]
        )
    ); // production:cache:user:123

    println!("\n{}", t(MSG_EXAMPLE_KEY_GENERATOR_7_KEY_VALIDATION, &[]));
    let generator = KeyGenerator::new();

    let valid_keys = ["user:123", "cache:item:abc", "session:token"];
    for key in &valid_keys {
        match generator.validate_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_KEY_GENERATOR_IS_VALID,
                    &[("key", key.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID,
                    &[("key", key.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    let invalid_keys = ["", "key\0with\0null", "key\nwith\nnewline"];
    for key in &invalid_keys {
        match generator.validate_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_KEY_GENERATOR_IS_VALID_2,
                    &[("key", key.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID_2,
                    &[("key", key.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_KEY_GENERATOR_8_IN_PRACTICE_CACHE, &[])
    );
    use oxcache::Cache;

    let cache: Cache<String, String> = Cache::builder().build().await?;
    let key_gen = KeyGenerator::new()
        .with_namespace("demo")
        .with_prefix_str("v1:");

    // 使用 KeyGenerator 生成键
    let user_key = key_gen.generate_full("user:{id}:name", &[("id", "1")]);
    cache.set(&user_key, &"Alice".to_string()).await?;
    let name = cache.get(&user_key).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_STORE,
            &[
                ("user_key", user_key.to_string()),
                ("name", format!("{:?}", name))
            ]
        )
    );

    let user_key2 = key_gen.generate_full("user:{id}:name", &[("id", "2")]);
    cache.set(&user_key2, &"Bob".to_string()).await?;
    let name2 = cache.get(&user_key2).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_KEY_GENERATOR_STORE_2,
            &[
                ("user_key2", user_key2.to_string()),
                ("name2", format!("{:?}", name2))
            ]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
