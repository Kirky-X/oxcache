// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 键生成器示例
//!
//! 本示例演示 oxcache 的 KeyGenerator 工具：
//! - 使用命名空间隔离不同应用的键
//! - 使用前缀组织键的层次结构
//! - 使用模板生成动态键
//! - 验证键的合法性
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_key_generator
//! ```

use oxcache::KeyGenerator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 键生成器示例 ===\n");

    // 1. 基本用法
    println!("--- 1. 基本用法 ---");
    let _gen = KeyGenerator::new();
    println!("  默认命名空间: 'default'");
    println!("  默认最大长度: 256");

    // 2. 使用命名空间
    println!("\n--- 2. 使用命名空间 ---");
    let user_gen = KeyGenerator::new().with_namespace("users");
    let order_gen = KeyGenerator::new().with_namespace("orders");

    let user_key = user_gen.namespaced_key("user:123");
    let order_key = order_gen.namespaced_key("order:456");
    println!("  用户键: {}", user_key);
    println!("  订单键: {}", order_key);

    // 3. 使用前缀
    println!("\n--- 3. 使用前缀 ---");
    let cache_gen = KeyGenerator::with_prefix("cache");
    let session_gen = KeyGenerator::with_prefix("session");

    let key1 = cache_gen.namespaced_key("item:1");
    let key2 = session_gen.namespaced_key("token:abc");
    println!("  缓存键: {}", key1);
    println!("  会话键: {}", key2);

    // 4. 链式设置
    println!("\n--- 4. 链式设置（命名空间 + 前缀） ---");
    let gen = KeyGenerator::new()
        .with_namespace("myapp")
        .with_prefix_str("v2")
        .with_max_key_length(512);

    let key = gen.namespaced_key("user:profile:123");
    println!("  完整键: {}", key);

    // 5. 模板生成
    println!("\n--- 5. 模板生成 ---");
    let gen = KeyGenerator::new().with_namespace("api");

    let key = gen.generate("user:{id}:profile", &[("id", "42")]);
    println!("  模板 'user:{{id}}:profile' + id=42 -> {}", key);

    let key = gen.generate("search:{type}:{query}", &[("type", "products"), ("query", "laptop")]);
    println!("  模板 'search:{{type}}:{{query}}' -> {}", key);

    // 6. 完整键生成（带命名空间和前缀）
    println!("\n--- 6. 完整键生成（generate_full） ---");
    let gen = KeyGenerator::new()
        .with_namespace("production")
        .with_prefix_str("cache");

    let key = gen.generate_full("user:{id}", &[("id", "123")]);
    println!("  generate_full('user:{{id}}', id=123) -> {}", key);

    // 7. 键验证
    println!("\n--- 7. 键验证 ---");
    let gen = KeyGenerator::new();

    let valid_keys = ["user:123", "cache:item:abc", "session:token"];
    for key in &valid_keys {
        match gen.validate_key(key) {
            Ok(()) => println!("  ✓ '{}' 有效", key),
            Err(e) => println!("  ✗ '{}' 无效: {}", key, e),
        }
    }

    let invalid_keys = ["", "key\0with\0null", "key\nwith\nnewline"];
    for key in &invalid_keys {
        match gen.validate_key(key) {
            Ok(()) => println!("  ✓ '{}' 有效", key),
            Err(e) => println!("  ✗ '{}' 无效: {}", key, e),
        }
    }

    // 8. 实际缓存操作中的应用
    println!("\n--- 8. 实际应用：缓存操作 ---");
    use oxcache::Cache;

    let cache: Cache<String, String> = Cache::builder().build().await?;
    let key_gen = KeyGenerator::new().with_namespace("demo").with_prefix_str("v1");

    // 使用 KeyGenerator 生成键
    let user_key = key_gen.generate_full("user:{id}:name", &[("id", "1")]);
    cache.set(&user_key, &"Alice".to_string()).await?;
    let name = cache.get(&user_key).await?;
    println!("  存储: {} = {:?}", user_key, name);

    let user_key2 = key_gen.generate_full("user:{id}:name", &[("id", "2")]);
    cache.set(&user_key2, &"Bob".to_string()).await?;
    let name2 = cache.get(&user_key2).await?;
    println!("  存储: {} = {:?}", user_key2, name2);

    println!("\n✓ 示例完成");
    Ok(())
}
