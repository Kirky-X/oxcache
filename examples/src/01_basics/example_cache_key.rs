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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CACHE_KEY_1_CACHEKEY_FOR_BUILT, MSG_EXAMPLE_CACHE_KEY_2_CACHEKEY_FOR_CUSTOM,
    MSG_EXAMPLE_CACHE_KEY_3_USING_CACHEKEY_IN, MSG_EXAMPLE_CACHE_KEY_4_BATCH_OPERATIONS,
    MSG_EXAMPLE_CACHE_KEY_5_KEY_FORMAT_CONSISTENCY, MSG_EXAMPLE_CACHE_KEY_BATCH_READ_RESULTS,
    MSG_EXAMPLE_CACHE_KEY_BUILT_IN_TYPES_STRING, MSG_EXAMPLE_CACHE_KEY_CACHEKEY_CAN_BE_USED,
    MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE,
    MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_CACHE_KEY_CUSTOM_TYPES_CAN_GENERATE, MSG_EXAMPLE_CACHE_KEY_KEY_POINTS,
    MSG_EXAMPLE_CACHE_KEY_READ, MSG_EXAMPLE_CACHE_KEY_SAME_VALUE_PRODUCES_SAME,
    MSG_EXAMPLE_CACHE_KEY_STORE_KEY, MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT,
    MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT_2, MSG_EXAMPLE_CACHE_KEY_TO_KEY_STRING_SHOULD,
    MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_100, MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_200, t,
};

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
    println!("{}\n", t(MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE, &[]));

    println!("{}", t(MSG_EXAMPLE_CACHE_KEY_1_CACHEKEY_FOR_BUILT, &[]));

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

    println!("{}", t(MSG_EXAMPLE_CACHE_KEY_2_CACHEKEY_FOR_CUSTOM, &[]));

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

    println!("{}", t(MSG_EXAMPLE_CACHE_KEY_3_USING_CACHEKEY_IN, &[]));

    let cache: Cache<String, Product> = Cache::builder().build().await?;

    // 使用 CacheKey 生成缓存键
    let key1 = product.to_key_string();
    cache.set(&key1, &product).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT,
            &[
                ("key1", key1.to_string()),
                ("product", format!("{:?}", product))
            ]
        )
    );

    let cached: Option<Product> = cache.get(&key1).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_READ,
            &[("cached", format!("{:?}", cached))]
        )
    );

    // 使用不同的键格式
    let key2 = UserId(100).to_key_string();
    let another_product = Product {
        id: 100,
        name: "Phone".to_string(),
        category: "electronics".to_string(),
    };
    cache.set(&key2, &another_product).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT_2,
            &[
                ("key2", key2.to_string()),
                ("another_product", format!("{:?}", another_product))
            ]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_CACHE_KEY_4_BATCH_OPERATIONS, &[]));

    let users = vec![UserId(1), UserId(2), UserId(3)];

    for user_id in &users {
        let key = user_id.to_key_string();
        let product = Product {
            id: user_id.0,
            name: format!("Product {}", user_id.0),
            category: "general".to_string(),
        };
        cache.set(&key, &product).await?;
        println!(
            "  {}",
            t(MSG_EXAMPLE_CACHE_KEY_STORE_KEY, &[("key", key.to_string())])
        );
    }

    // 批量读取
    let keys: Vec<String> = users.iter().map(|u| u.to_key_string()).collect();
    let results = cache.get_many(keys.iter()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_BATCH_READ_RESULTS,
            &[("results", results.len().to_string())]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_CACHE_KEY_5_KEY_FORMAT_CONSISTENCY, &[]));

    let key_a = UserId(100).to_key_string();
    let key_b = UserId(100).to_key_string();
    let key_c = UserId(200).to_key_string();

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_100,
            &[("key_b", (key_a == key_b).to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_200,
            &[("key_c", (key_a == key_c).to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_KEY_SAME_VALUE_PRODUCES_SAME,
            &[(
                "to_key_string",
                (UserId(100).to_key_string() == UserId(100).to_key_string()).to_string()
            )]
        )
    );
    println!();

    cache.clear().await?;

    println!(
        "{}",
        t(MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE_COMPLETE, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_CACHE_KEY_KEY_POINTS, &[]));
    println!("  {}", t(MSG_EXAMPLE_CACHE_KEY_BUILT_IN_TYPES_STRING, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_KEY_CUSTOM_TYPES_CAN_GENERATE, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_CACHE_KEY_TO_KEY_STRING_SHOULD, &[]));
    println!("  {}", t(MSG_EXAMPLE_CACHE_KEY_CACHEKEY_CAN_BE_USED, &[]));

    Ok(())
}
