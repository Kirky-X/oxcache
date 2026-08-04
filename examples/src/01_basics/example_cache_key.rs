// Copyright (c) 2025-2026 Kirky.X
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
    println!("=== CacheKey trait 示例 ===\n");

    // 1. 内置类型演示
    println!("--- 1. 内置类型的 CacheKey 实现 ---");

    let string_key: String = "user:123".to_string();
    let str_key: &str = "user:456";
    let u64_key: u64 = 789;
    let i64_key: i64 = -100;
    let u32_key: u32 = 42;
    let i32_key: i32 = -42;
    let usize_key: usize = 1000;
    let isize_key: isize = -500;

    println!("  String: '{}' -> '{}'", string_key, string_key.to_key_string());
    println!("  &str: '{}' -> '{}'", str_key, str_key.to_key_string());
    println!("  u64: {} -> '{}'", u64_key, u64_key.to_key_string());
    println!("  i64: {} -> '{}'", i64_key, i64_key.to_key_string());
    println!("  u32: {} -> '{}'", u32_key, u32_key.to_key_string());
    println!("  i32: {} -> '{}'", i32_key, i32_key.to_key_string());
    println!("  usize: {} -> '{}'", usize_key, usize_key.to_key_string());
    println!("  isize: {} -> '{}'", isize_key, isize_key.to_key_string());
    println!();

    // 2. 自定义类型演示
    println!("--- 2. 自定义类型的 CacheKey 实现 ---");

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

    // 3. 在缓存中使用 CacheKey
    println!("--- 3. 在缓存中使用 CacheKey ---");

    let cache: Cache<String, Product> = Cache::builder().build().await?;

    // 使用 CacheKey 生成缓存键
    let key1 = product.to_key_string();
    cache.set(&key1, &product).await?;
    println!("  存储: key='{}', product={:?}", key1, product);

    // 从缓存读取
    let cached: Option<Product> = cache.get(&key1).await?;
    println!("  读取: {:?}", cached);

    // 使用不同的键格式
    let key2 = UserId(100).to_key_string();
    let another_product = Product {
        id: 100,
        name: "Phone".to_string(),
        category: "electronics".to_string(),
    };
    cache.set(&key2, &another_product).await?;
    println!("  存储: key='{}', product={:?}", key2, another_product);
    println!();

    // 4. 批量操作演示
    println!("--- 4. 批量操作演示 ---");

    let users = vec![UserId(1), UserId(2), UserId(3)];

    for user_id in &users {
        let key = user_id.to_key_string();
        let product = Product {
            id: user_id.0,
            name: format!("Product {}", user_id.0),
            category: "general".to_string(),
        };
        cache.set(&key, &product).await?;
        println!("  存储: key='{}'", key);
    }

    // 批量读取
    let keys: Vec<String> = users.iter().map(|u| u.to_key_string()).collect();
    let results = cache.get_many(keys.iter()).await?;
    println!("  批量读取: {} 个结果", results.len());
    println!();

    // 5. 键格式一致性验证
    println!("--- 5. 键格式一致性验证 ---");

    let key_a = UserId(100).to_key_string();
    let key_b = UserId(100).to_key_string();
    let key_c = UserId(200).to_key_string();

    println!("  UserId(100) == UserId(100): {} (应该为 true)", key_a == key_b);
    println!("  UserId(100) == UserId(200): {} (应该为 false)", key_a == key_c);
    println!(
        "  相同值产生相同键: {}",
        UserId(100).to_key_string() == UserId(100).to_key_string()
    );
    println!();

    // 清理
    cache.clear().await?;

    println!("=== CacheKey trait 示例完成 ===");
    println!("  关键点:");
    println!("  - 内置类型 (String, u64, i64 等) 已有 CacheKey 实现");
    println!("  - 自定义类型可以通过实现 CacheKey trait 生成缓存键");
    println!("  - to_key_string() 应该返回确定性的、唯一的字符串表示");
    println!("  - CacheKey 可以在 set/get/delete 等操作中使用");

    Ok(())
}
