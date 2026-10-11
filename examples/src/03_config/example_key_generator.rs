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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Key generator example ===\n");

    println!("--- 1. Basic usage ---");
    let _gen = KeyGenerator::new();
    println!("  default namespace: 'default'");
    println!("  default max length: 256");

    println!("\n--- 2. Using namespaces ---");
    let user_gen = KeyGenerator::new().with_namespace("users");
    let order_gen = KeyGenerator::new().with_namespace("orders");

    let user_key = user_gen.namespaced_key("user:123");
    let order_key = order_gen.namespaced_key("order:456");
    println!("  user key: {}", user_key);
    println!("  order key: {}", order_key);

    println!("\n--- 3. Using prefixes ---");
    // 前缀按原样直拼（不加分隔符），需自带尾随冒号；
    // 且仅 generate_full/try_generate_full/json_hash_key 会套用前缀
    let cache_gen = KeyGenerator::with_prefix("cache:");
    let session_gen = KeyGenerator::with_prefix("session:");

    let key1 = cache_gen.generate_full("item:{id}", &[("id", "1")]);
    let key2 = session_gen.generate_full("token:{value}", &[("value", "abc")]);
    println!("  cache key: {}", key1); // cache:item:1
    println!("  session key: {}", key2); // session:token:abc

    println!("\n--- 4. Chaining (namespace + prefix) ---");
    let generator = KeyGenerator::new()
        .with_namespace("myapp")
        .with_prefix_str("v2:")
        .with_max_key_length(512);

    let key = generator.generate_full("user:profile:{id}", &[("id", "123")]);
    println!("  full key: {}", key); // myapp:v2:user:profile:123

    println!("\n--- 5. Template generation ---");
    let generator = KeyGenerator::new().with_namespace("api");

    // generate 只替换模板占位符，不包装命名空间/前缀
    let key = generator.generate("user:{id}:profile", &[("id", "42")]);
    println!("  generate('user:{{id}}:profile', id=42) -> {}", key); // user:42:profile

    // generate_full 在模板结果外追加前缀与命名空间
    let key = generator.generate_full(
        "search:{type}:{query}",
        &[("type", "products"), ("query", "laptop")],
    );
    println!("  generate_full('search:{{type}}:{{query}}') -> {}", key); // api:search:products:laptop

    println!("\n--- 6. Full key generation (generate_full) ---");
    let generator = KeyGenerator::new()
        .with_namespace("production")
        .with_prefix_str("cache:");

    let key = generator.generate_full("user:{id}", &[("id", "123")]);
    println!("  generate_full('user:{{id}}', id=123) -> {}", key); // production:cache:user:123

    println!("\n--- 7. Key validation ---");
    let generator = KeyGenerator::new();

    let valid_keys = ["user:123", "cache:item:abc", "session:token"];
    for key in &valid_keys {
        match generator.validate_key(key) {
            Ok(()) => println!("  ✓ '{}' is valid", key),
            Err(e) => println!("  ✗ '{}' is invalid: {}", key, e),
        }
    }

    let invalid_keys = ["", "key\0with\0null", "key\nwith\nnewline"];
    for key in &invalid_keys {
        match generator.validate_key(key) {
            Ok(()) => println!("  ✓ '{}' is valid", key),
            Err(e) => println!("  ✗ '{}' is invalid: {}", key, e),
        }
    }

    println!("\n--- 8. In practice: cache operations ---");
    use oxcache::Cache;

    let cache: Cache<String, String> = Cache::builder().build().await?;
    let key_gen = KeyGenerator::new()
        .with_namespace("demo")
        .with_prefix_str("v1:");

    // 使用 KeyGenerator 生成键
    let user_key = key_gen.generate_full("user:{id}:name", &[("id", "1")]);
    cache.set(&user_key, &"Alice".to_string()).await?;
    let name = cache.get(&user_key).await?;
    println!("  store: {} = {:?}", user_key, name);

    let user_key2 = key_gen.generate_full("user:{id}:name", &[("id", "2")]);
    cache.set(&user_key2, &"Bob".to_string()).await?;
    let name2 = cache.get(&user_key2).await?;
    println!("  store: {} = {:?}", user_key2, name2);

    println!("\n✓ Example complete");
    Ok(())
}
