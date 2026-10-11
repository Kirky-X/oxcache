// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! CacheKey trait 示例
//!
//! 本示例演示 CacheKey trait 的使用：
//! - 内置类型的 CacheKey 实现
//! - 自定义类型的 CacheKey 实现
//! - 在缓存操作中使用 CacheKey
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_cache_key
//! ```

use oxcache::Cache;
use oxcache::traits::CacheKey;
use serde::{Deserialize, Serialize};

// 自定义类型：用户 ID
#[derive(Debug, Clone)]
struct UserId(u64);

impl CacheKey for UserId {
    fn to_key_string(&self) -> String {
        format!("user:{}", self.0)
    }
}

// 自定义类型：复合键
#[derive(Debug, Clone)]
struct CacheCompositeKey {
    namespace: String,
    entity: String,
    id: u64,
}

impl CacheKey for CacheCompositeKey {
    fn to_key_string(&self) -> String {
        format!("{}:{}:{}", self.namespace, self.entity, self.id)
    }
}

// 自定义类型：带哈希的键
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Product {
    id: u64,
    name: String,
    category: String,
}

impl CacheKey for Product {
    fn to_key_string(&self) -> String {
        format!("product:{}:{}", self.category, self.id)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CacheKey trait example ===\n");

    println!("--- 1. CacheKey for built-in types ---");

    let string_key: String = "user:123".to_string();
    let str_key: &str = "user:456";
    let u64_key: u64 = 789;
    let i64_key: i64 = -100;
    let u32_key: u32 = 42;
    let i32_key: i32 = -42;
    let usize_key: usize = 1000;
    let isize_key: isize = -500;

    println!(
        "  String: '{}' -> '{}'",
        string_key,
        string_key.to_key_string()
    );
    println!("  &str: '{}' -> '{}'", str_key, str_key.to_key_string());
    println!("  u64: {} -> '{}'", u64_key, u64_key.to_key_string());
    println!("  i64: {} -> '{}'", i64_key, i64_key.to_key_string());
    println!("  u32: {} -> '{}'", u32_key, u32_key.to_key_string());
    println!("  i32: {} -> '{}'", i32_key, i32_key.to_key_string());
    println!("  usize: {} -> '{}'", usize_key, usize_key.to_key_string());
    println!("  isize: {} -> '{}'", isize_key, isize_key.to_key_string());
    println!();

    println!("--- 2. CacheKey for custom types ---");

    let user_id = UserId(12345);
    println!("  UserId(12345) -> '{}'", user_id.to_key_string());

    let composite_key = CacheCompositeKey {
        namespace: "app".to_string(),
        entity: "session".to_string(),
        id: 999,
    };
    println!("  CacheCompositeKey -> '{}'", composite_key.to_key_string());

    let product = Product {
        id: 100,
        name: "Laptop".to_string(),
        category: "electronics".to_string(),
    };
    println!("  Product -> '{}'", product.to_key_string());
    println!();

    println!("--- 3. Using CacheKey in the cache ---");

    let cache: Cache<String, Product> = Cache::builder().build().await?;

    // 使用 CacheKey 生成缓存键
    let key1 = product.to_key_string();
    cache.set(&key1, &product).await?;
    println!("  store: key='{}', product={:?}", key1, product);

    // 从缓存读取
    let cached: Option<Product> = cache.get(&key1).await?;
    println!("  read: {:?}", cached);

    // 使用不同的键格式
    let key2 = UserId(100).to_key_string();
    let another_product = Product {
        id: 100,
        name: "Phone".to_string(),
        category: "electronics".to_string(),
    };
    cache.set(&key2, &another_product).await?;
    println!("  store: key='{}', product={:?}", key2, another_product);
    println!();

    println!("--- 4. Batch operations ---");

    let users = vec![UserId(1), UserId(2), UserId(3)];

    for user_id in &users {
        let key = user_id.to_key_string();
        let product = Product {
            id: user_id.0,
            name: format!("Product {}", user_id.0),
            category: "general".to_string(),
        };
        cache.set(&key, &product).await?;
        println!("  store: key='{}'", key);
    }

    // 批量读取
    let keys: Vec<String> = users.iter().map(|u| u.to_key_string()).collect();
    let results = cache.get_many(keys.iter()).await?;
    println!("  batch read: {} results", results.len());
    println!();

    println!("--- 5. Key format consistency check ---");

    let key_a = UserId(100).to_key_string();
    let key_b = UserId(100).to_key_string();
    let key_c = UserId(200).to_key_string();

    println!(
        "  UserId(100) == UserId(100): {} (should be true)",
        key_a == key_b
    );
    println!(
        "  UserId(100) == UserId(200): {} (should be false)",
        key_a == key_c
    );
    println!(
        "  same value produces same key: {}",
        UserId(100).to_key_string() == UserId(100).to_key_string()
    );
    println!();

    // 清理
    cache.clear().await?;

    println!("=== CacheKey trait example complete ===");
    println!("  key points:");
    println!("  - built-in types (String, u64, i64, etc.) already implement CacheKey");
    println!("  - custom types can generate cache keys by implementing the CacheKey trait");
    println!("  - to_key_string() should return a deterministic, unique string representation");
    println!("  - CacheKey can be used with set/get/delete operations");

    Ok(())
}
