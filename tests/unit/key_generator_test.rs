// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 键生成器单元测试

use oxcache::KeyGenerator;

#[test]
fn test_key_generator_new() {
    let gen = KeyGenerator::new();
    let key = gen.generate("user:{id}", &[("id", "123")]);
    assert_eq!(key, "user:123");
}

#[test]
fn test_key_generator_with_prefix() {
    let gen = KeyGenerator::with_prefix("session:");
    let key = gen.generate_full("user:{id}", &[("id", "123")]);
    assert_eq!(key, "session:user:123");
}

#[test]
fn test_key_generator_with_namespace() {
    let gen = KeyGenerator::new().with_namespace("app:v1");
    let key = gen.generate_full("user:{id}", &[("id", "456")]);
    assert_eq!(key, "app:v1:user:456");
}

#[test]
fn test_key_generator_with_prefix_str() {
    let gen = KeyGenerator::new().with_prefix_str("cache:");
    let key = gen.generate_full("item:{id}", &[("id", "789")]);
    assert_eq!(key, "cache:item:789");
}

#[test]
fn test_key_generator_with_max_key_length() {
    let gen = KeyGenerator::new().with_max_key_length(10);
    assert!(gen.validate_key("short").is_ok());
    assert!(gen.validate_key("this_is_a_very_long_key").is_err());
}

#[test]
fn test_key_generator_validate_key_empty() {
    let gen = KeyGenerator::new();
    let result = gen.validate_key("");
    assert!(result.is_err());
}

#[test]
fn test_key_generator_validate_key_valid() {
    let gen = KeyGenerator::new();
    let result = gen.validate_key("valid_key_123");
    assert!(result.is_ok());
}

#[test]
fn test_key_generator_validate_key_invalid_chars() {
    let gen = KeyGenerator::new();
    let result = gen.validate_key("key with spaces");
    assert!(result.is_err());
}

#[test]
fn test_key_generator_validate_key_special_chars() {
    let gen = KeyGenerator::new();
    assert!(gen.validate_key("key:with:colons").is_ok());
    assert!(gen.validate_key("key/with/slashes").is_ok());
    assert!(gen.validate_key("key.with.dots").is_ok());
    assert!(gen.validate_key("key-with-dashes").is_ok());
    assert!(gen.validate_key("key_with_underscores").is_ok());
    assert!(gen.validate_key("key@email").is_ok());
}

#[test]
fn test_key_generator_validate_key_invalid_special_chars() {
    let gen = KeyGenerator::new();
    assert!(gen.validate_key("key with spaces").is_err());
    assert!(gen.validate_key("key,with,commas").is_err());
    assert!(gen.validate_key("key;with;semicolons").is_err());
}

#[test]
fn test_key_generator_generate_multiple_params() {
    let gen = KeyGenerator::new();
    let key = gen.generate(
        "user:{user_id}:post:{post_id}",
        &[("user_id", "1"), ("post_id", "42")],
    );
    assert_eq!(key, "user:1:post:42");
}

#[test]
fn test_key_generator_generate_no_params() {
    let gen = KeyGenerator::new();
    let key = gen.generate("simple_key", &[]);
    assert_eq!(key, "simple_key");
}

#[test]
fn test_key_generator_generate_missing_param() {
    let gen = KeyGenerator::new();
    let key = gen.generate("user:{id}:name", &[("other", "value")]);
    assert_eq!(key, "user:{id}:name");
}

#[test]
fn test_key_generator_namespaced_key_default() {
    let gen = KeyGenerator::new();
    let key = gen.namespaced_key("test_key");
    assert_eq!(key, "test_key");
}

#[test]
fn test_key_generator_namespaced_key_custom() {
    let gen = KeyGenerator::new().with_namespace("myapp");
    let key = gen.namespaced_key("test_key");
    assert_eq!(key, "myapp:test_key");
}

#[test]
fn test_key_generator_chained_builders() {
    let gen = KeyGenerator::new()
        .with_namespace("app:v2")
        .with_prefix_str("cache:")
        .with_max_key_length(100);

    let key = gen.generate_full("item:{id}", &[("id", "123")]);
    assert_eq!(key, "app:v2:cache:item:123");
}

#[test]
fn test_key_generator_default_trait() {
    let gen1 = KeyGenerator::default();
    let gen2 = KeyGenerator::new();
    let key1 = gen1.generate("test", &[]);
    let key2 = gen2.generate("test", &[]);
    assert_eq!(key1, key2);
}

#[test]
fn test_key_generator_clone() {
    let gen1 = KeyGenerator::new().with_namespace("test");
    let gen2 = gen1.clone();
    let key1 = gen1.generate_full("key", &[]);
    let key2 = gen2.generate_full("key", &[]);
    assert_eq!(key1, key2);
}

#[test]
fn test_key_generator_debug() {
    let gen = KeyGenerator::new().with_namespace("test");
    let debug_str = format!("{:?}", gen);
    assert!(debug_str.contains("KeyGenerator"));
    assert!(debug_str.contains("test"));
}
