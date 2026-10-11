// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全功能示例
//!
//! 本示例演示 oxcache 的安全功能，包括：
//! - 敏感数据脱敏
//! - 安全日志记录
//! - 连接字符串保护
//!
//! # 安全特性
//!
//! oxcache 提供多层安全保护：
//! 1. 敏感数据脱敏 - 自动隐藏密码、令牌等敏感信息
//! 2. 安全日志 - 确保日志中不泄露敏感信息
//! 3. 配置验证 - 防止路径遍历等安全漏洞

use oxcache::Cache;
use oxcache::sanitize_message;
use oxcache::{Redacted, redact_cache_key, redact_connection_string, redact_field, redact_value};

/// 演示安全功能的使用方法
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== oxcache security features example ===\n");

    println!("--- 1. Sensitive data redaction ---\n");

    // 脱敏密码
    let password = "super_secret_password_123"; // pragma: allowlist secret
    let redacted = redact_value(password, 4);
    println!("raw password: {}", password);
    println!("redacted:   {}", redacted);

    // 脱敏连接字符串
    println!("\n--- Connection string redaction ---");
    let redis_url = "redis://admin:my_password@localhost:6379"; // pragma: allowlist secret
    let safe_url = redact_connection_string(redis_url);
    println!("raw URL: {}", redis_url);
    println!("redacted:  {}", safe_url);

    // 脱敏缓存键
    println!("\n--- Sensitive cache key redaction ---");
    let sensitive_keys = [
        "user_token_abc123",
        "api_key_secret",
        "password_reset_token",
        "normal_cache_key",
    ];

    for key in sensitive_keys {
        let redacted_key = redact_cache_key(key);
        println!("  {} -> {}", key, redacted_key);
    }

    // 脱敏字段
    println!("\n--- Field value redaction ---");
    let fields = [
        ("password", "user_password"),
        ("api_key", "sk-1234567890abcdef"),
        ("username", "john_doe"),
    ];

    for (field_name, value) in fields {
        let redacted = redact_field(field_name, value);
        println!("  {} = {} -> {}", field_name, value, redacted);
    }

    println!("\n--- 2. The Redacted wrapper ---\n");

    let secret = Redacted::new("my_api_key_12345");
    println!("Redacted wrapper: {}", secret);

    let custom_redacted = Redacted::new("very_long_secret_value").with_visible_chars(6);
    println!("custom visible characters: {}", custom_redacted);

    println!("\n--- 3. Safe log handling ---\n");

    let log_messages = [
        "Connection established to redis://user:password123@redis.example.com:6379", // pragma: allowlist secret
        "Cache hit for key user_token_xyz",
        "Query executed successfully",
    ];

    for msg in log_messages {
        let sanitized = sanitize_message(msg);
        println!("raw: {}", msg);
        println!("safe: {}", sanitized);
        println!();
    }

    println!("--- 4. Cache operation safety ---\n");

    // 创建缓存
    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    // 安全地存储敏感数据（应该先加密）
    // 注意：缓存不应直接存储明文敏感数据
    let user_token = String::from("user_session_token_abc123");
    let session_data = b"encrypted_session_data".to_vec();

    // 使用脱敏后的键记录日志
    println!(
        "storing the user session: {}",
        redact_cache_key(&user_token)
    );

    // 存储数据
    cache.set(&user_token, &session_data).await?;

    // 获取数据
    if let Some(data) = cache.get(&user_token).await? {
        println!("data read back: {} bytes", data.len());
    }

    println!("\n--- 5. Security best practices ---\n");

    println!("✓ never log sensitive information");
    println!("✓ use the redaction module to mask sensitive data");
    println!("✓ encrypt sensitive data before caching it");
    println!("✓ use secure connection string formats");
    println!("✓ rotate sensitive tokens in the cache regularly");
    println!("✓ keep cache key names from leaking information");

    println!("\n--- 6. Configuration safety ---\n");

    // 展示安全和不安全的配置方式
    println!("❌ unsafe: storing passwords in plaintext in config");
    println!("   config.password = \"my_password\"");

    println!("\n✓ safe: use environment variables or a secret management service");
    println!("   config.password = std::env::var(\"CACHE_PASSWORD\")?");

    println!("\n✓ safe: hide connection string passwords in logs");
    let safe_conn_str = redact_connection_string("redis://user:secret@localhost:6379");
    println!("   log: connecting to {}", safe_conn_str);

    // 清理
    cache.clear().await?;

    println!("\nExample complete!");
    Ok(())
}
