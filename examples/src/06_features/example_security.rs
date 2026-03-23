// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
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

use oxcache::utils::redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted};
use oxcache::utils::security_log::sanitize_message;
use oxcache::Cache;

/// 演示安全功能的使用方法
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== oxcache 安全功能示例 ===\n");

    // 1. 敏感数据脱敏
    println!("--- 1. 敏感数据脱敏 ---\n");

    // 脱敏密码
    let password = "super_secret_password_123"; // pragma: allowlist secret
    let redacted = redact_value(password, 4);
    println!("原始密码: {}", password);
    println!("脱敏后:   {}", redacted);

    // 脱敏连接字符串
    println!("\n--- 连接字符串脱敏 ---");
    let redis_url = "redis://admin:my_password@localhost:6379"; // pragma: allowlist secret
    let safe_url = redact_connection_string(redis_url);
    println!("原始URL: {}", redis_url);
    println!("脱敏后:  {}", safe_url);

    // 脱敏缓存键
    println!("\n--- 敏感缓存键脱敏 ---");
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
    println!("\n--- 字段值脱敏 ---");
    let fields = [
        ("password", "user_password"),
        ("api_key", "sk-1234567890abcdef"),
        ("username", "john_doe"),
    ];

    for (field_name, value) in fields {
        let redacted = redact_field(field_name, value);
        println!("  {} = {} -> {}", field_name, value, redacted);
    }

    // 2. Redacted 包装器
    println!("\n--- 2. Redacted 包装器 ---\n");

    let secret = Redacted::new("my_api_key_12345");
    println!("Redacted 包装: {}", secret);

    let custom_redacted = Redacted::new("very_long_secret_value").with_visible_chars(6);
    println!("自定义可见字符: {}", custom_redacted);

    // 3. 安全日志处理
    println!("\n--- 3. 安全日志处理 ---\n");

    let log_messages = [
        "Connection established to redis://user:password123@redis.example.com:6379", // pragma: allowlist secret
        "Cache hit for key user_token_xyz",
        "Query executed successfully",
    ];

    for msg in log_messages {
        let sanitized = sanitize_message(msg);
        println!("原始: {}", msg);
        println!("安全: {}", sanitized);
        println!();
    }

    // 4. 实际缓存操作中的安全考虑
    println!("--- 4. 缓存操作安全示例 ---\n");

    // 创建缓存
    let cache: Cache<String, Vec<u8>> = Cache::builder()
        .capacity(100)
        .build()
        .await?;

    // 安全地存储敏感数据（应该先加密）
    // 注意：缓存不应直接存储明文敏感数据
    let user_token = String::from("user_session_token_abc123");
    let session_data = b"encrypted_session_data".to_vec();

    // 使用脱敏后的键记录日志
    println!("存储用户会话: {}", redact_cache_key(&user_token));

    // 存储数据
    cache.set(&user_token, &session_data).await?;

    // 获取数据
    if let Some(data) = cache.get(&user_token).await? {
        println!("获取到数据: {} 字节", data.len());
    }

    // 5. 安全最佳实践
    println!("\n--- 5. 安全最佳实践 ---\n");

    println!("✓ 永远不要在日志中记录敏感信息");
    println!("✓ 使用 redaction 模块脱敏敏感数据");
    println!("✓ 缓存中的敏感数据应先加密");
    println!("✓ 使用安全的连接字符串格式");
    println!("✓ 定期轮换缓存中的敏感令牌");
    println!("✓ 限制缓存键的命名避免泄露信息");

    // 6. 配置安全示例
    println!("\n--- 6. 配置安全示例 ---\n");

    // 展示安全和不安全的配置方式
    println!("❌ 不安全: 在配置中明文存储密码");
    println!("   config.password = \"my_password\"");

    println!("\n✓ 安全: 使用环境变量或密钥管理服务");
    println!("   config.password = std::env::var(\"CACHE_PASSWORD\")?");

    println!("\n✓ 安全: 日志中隐藏连接字符串密码");
    let safe_conn_str = redact_connection_string("redis://user:secret@localhost:6379");
    println!("   日志: 连接到 {}", safe_conn_str);

    // 清理
    cache.clear().await?;

    println!("\n示例完成！");
    Ok(())
}
