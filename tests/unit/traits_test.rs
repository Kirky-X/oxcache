// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Traits 单元测试

use oxcache::traits::CacheKey;
use serde::{Deserialize, Serialize};

#[test]
fn test_cache_key_string() {
    let key = "user:123".to_string();
    assert_eq!(key.to_key_string(), "user:123");
}

#[test]
fn test_cache_key_str() {
    let key: &str = "session:abc";
    assert_eq!(key.to_key_string(), "session:abc");
}

#[test]
fn test_cache_key_u64() {
    let key: u64 = 12345;
    assert_eq!(key.to_key_string(), "12345");
}

#[test]
fn test_cache_key_u128() {
    let key: u128 = 12345678901234567890;
    assert_eq!(key.to_key_string(), "12345678901234567890");
}

#[test]
fn test_cache_key_i64() {
    let key: i64 = -98765;
    assert_eq!(key.to_key_string(), "-98765");
}

#[test]
fn test_cache_key_i64_positive() {
    let key: i64 = 98765;
    assert_eq!(key.to_key_string(), "98765");
}

#[test]
fn test_cache_key_i32() {
    let key: i32 = -123;
    assert_eq!(key.to_key_string(), "-123");
}

#[test]
fn test_cache_key_u32() {
    let key: u32 = 456;
    assert_eq!(key.to_key_string(), "456");
}

#[test]
fn test_cache_key_usize() {
    let key: usize = 789;
    assert_eq!(key.to_key_string(), "789");
}

#[test]
fn test_cache_key_isize() {
    let key: isize = -101;
    assert_eq!(key.to_key_string(), "-101");
}

#[test]
fn test_cache_key_zero() {
    assert_eq!(0u64.to_key_string(), "0");
    assert_eq!(0i64.to_key_string(), "0");
    assert_eq!(0usize.to_key_string(), "0");
}

#[test]
fn test_cache_key_max_values() {
    assert_eq!(u64::MAX.to_key_string(), u64::MAX.to_string());
    assert_eq!(i64::MAX.to_key_string(), i64::MAX.to_string());
    assert_eq!(i64::MIN.to_key_string(), i64::MIN.to_string());
}

#[test]
fn test_cache_key_empty_string() {
    let key = String::new();
    assert_eq!(key.to_key_string(), "");
}

#[test]
fn test_cache_key_with_special_chars() {
    let key = "user:123:profile";
    assert_eq!(key.to_key_string(), "user:123:profile");
}

#[test]
fn test_cache_key_with_slashes() {
    let key = "path/to/resource";
    assert_eq!(key.to_key_string(), "path/to/resource");
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
}

#[test]
fn test_cacheable_user_struct() {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
    };
    assert_eq!(user.id, 123);
    assert_eq!(user.name, "Alice");
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Product {
    id: u32,
    name: String,
    price: f64,
    tags: Vec<String>,
}

#[test]
fn test_cacheable_product_struct() {
    let product = Product {
        id: 1,
        name: "Widget".to_string(),
        price: 9.99,
        tags: vec!["tag1".to_string(), "tag2".to_string()],
    };
    assert_eq!(product.id, 1);
    assert_eq!(product.price, 9.99);
    assert_eq!(product.tags.len(), 2);
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Status {
    Active,
    Inactive,
    Pending,
}

#[test]
fn test_cacheable_enum() {
    let status = Status::Active;
    assert_eq!(status, Status::Active);
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct NestedStruct {
    user: User,
    status: Status,
    metadata: std::collections::HashMap<String, String>,
}

#[test]
fn test_cacheable_nested_struct() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("key1".to_string(), "value1".to_string());

    let nested = NestedStruct {
        user: User {
            id: 1,
            name: "Test".to_string(),
        },
        status: Status::Active,
        metadata,
    };
    assert_eq!(nested.user.id, 1);
    assert_eq!(nested.metadata.len(), 1);
}

#[test]
fn test_cache_key_consistency() {
    let key1 = "test_key".to_string();
    let key2 = "test_key".to_string();
    assert_eq!(key1.to_key_string(), key2.to_key_string());
}

#[test]
fn test_cache_key_numeric_consistency() {
    let key1: u64 = 12345;
    let key2: u64 = 12345;
    assert_eq!(key1.to_key_string(), key2.to_key_string());
}
