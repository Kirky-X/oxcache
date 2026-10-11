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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_SECURITY_1_SENSITIVE_DATA_REDACTION, MSG_EXAMPLE_SECURITY_2_THE_REDACTED_WRAPPER,
    MSG_EXAMPLE_SECURITY_3_SAFE_LOG_HANDLING, MSG_EXAMPLE_SECURITY_4_CACHE_OPERATION_SAFETY,
    MSG_EXAMPLE_SECURITY_5_SECURITY_BEST_PRACTICES, MSG_EXAMPLE_SECURITY_6_CONFIGURATION_SAFETY,
    MSG_EXAMPLE_SECURITY_CONNECTION_STRING_REDACTION,
    MSG_EXAMPLE_SECURITY_CUSTOM_VISIBLE_CHARACTERS, MSG_EXAMPLE_SECURITY_DATA_READ_BACK_BYTES,
    MSG_EXAMPLE_SECURITY_ENCRYPT_SENSITIVE_DATA_BEFORE, MSG_EXAMPLE_SECURITY_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_SECURITY_FIELD_VALUE_REDACTION, MSG_EXAMPLE_SECURITY_KEEP_CACHE_KEY_NAMES,
    MSG_EXAMPLE_SECURITY_LOG_CONNECTING_TO, MSG_EXAMPLE_SECURITY_NEVER_LOG_SENSITIVE_INFORMATION,
    MSG_EXAMPLE_SECURITY_OXCACHE_SECURITY_FEATURES_EXAMPLE, MSG_EXAMPLE_SECURITY_RAW,
    MSG_EXAMPLE_SECURITY_RAW_PASSWORD, MSG_EXAMPLE_SECURITY_RAW_URL, MSG_EXAMPLE_SECURITY_REDACTED,
    MSG_EXAMPLE_SECURITY_REDACTED_2, MSG_EXAMPLE_SECURITY_REDACTED_WRAPPER,
    MSG_EXAMPLE_SECURITY_ROTATE_SENSITIVE_TOKENS_IN, MSG_EXAMPLE_SECURITY_SAFE,
    MSG_EXAMPLE_SECURITY_SAFE_HIDE_CONNECTION_STRING,
    MSG_EXAMPLE_SECURITY_SAFE_USE_ENVIRONMENT_VARIABLES,
    MSG_EXAMPLE_SECURITY_SENSITIVE_CACHE_KEY_REDACTION,
    MSG_EXAMPLE_SECURITY_STORING_THE_USER_SESSION,
    MSG_EXAMPLE_SECURITY_UNSAFE_STORING_PASSWORDS_IN,
    MSG_EXAMPLE_SECURITY_USE_SECURE_CONNECTION_STRING,
    MSG_EXAMPLE_SECURITY_USE_THE_REDACTION_MODULE, t,
};

/// 演示安全功能的使用方法
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!(
        "{}\n",
        t(MSG_EXAMPLE_SECURITY_OXCACHE_SECURITY_FEATURES_EXAMPLE, &[])
    );

    println!(
        "{}\n",
        t(MSG_EXAMPLE_SECURITY_1_SENSITIVE_DATA_REDACTION, &[])
    );

    // 脱敏密码
    let password = "super_secret_password_123"; // pragma: allowlist secret
    let redacted = redact_value(password, 4);
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_RAW_PASSWORD,
            &[("password", password.to_string())]
        )
    );
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_REDACTED,
            &[("redacted", redacted.to_string())]
        )
    );

    // 脱敏连接字符串
    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_CONNECTION_STRING_REDACTION, &[])
    );
    let redis_url = "redis://admin:my_password@localhost:6379"; // pragma: allowlist secret
    let safe_url = redact_connection_string(redis_url);
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_RAW_URL,
            &[("redis_url", redis_url.to_string())]
        )
    );
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_REDACTED_2,
            &[("safe_url", safe_url.to_string())]
        )
    );

    // 脱敏缓存键
    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_SENSITIVE_CACHE_KEY_REDACTION, &[])
    );
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
    println!("\n{}", t(MSG_EXAMPLE_SECURITY_FIELD_VALUE_REDACTION, &[]));
    let fields = [
        ("password", "user_password"),
        ("api_key", "sk-1234567890abcdef"),
        ("username", "john_doe"),
    ];

    for (field_name, value) in fields {
        let redacted = redact_field(field_name, value);
        println!("  {} = {} -> {}", field_name, value, redacted);
    }

    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_2_THE_REDACTED_WRAPPER, &[])
    );

    let secret = Redacted::new("my_api_key_12345");
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_REDACTED_WRAPPER,
            &[("secret", secret.to_string())]
        )
    );

    let custom_redacted = Redacted::new("very_long_secret_value").with_visible_chars(6);
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_CUSTOM_VISIBLE_CHARACTERS,
            &[("custom_redacted", custom_redacted.to_string())]
        )
    );

    println!("\n{}\n", t(MSG_EXAMPLE_SECURITY_3_SAFE_LOG_HANDLING, &[]));

    let log_messages = [
        "Connection established to redis://user:password123@redis.example.com:6379", // pragma: allowlist secret
        "Cache hit for key user_token_xyz",
        "Query executed successfully",
    ];

    for msg in log_messages {
        let sanitized = sanitize_message(msg);
        println!(
            "{}",
            t(MSG_EXAMPLE_SECURITY_RAW, &[("msg", msg.to_string())])
        );
        println!(
            "{}",
            t(
                MSG_EXAMPLE_SECURITY_SAFE,
                &[("sanitized", sanitized.to_string())]
            )
        );
        println!();
    }

    println!(
        "{}\n",
        t(MSG_EXAMPLE_SECURITY_4_CACHE_OPERATION_SAFETY, &[])
    );

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    // 安全地存储敏感数据（应该先加密）
    // 注意：缓存不应直接存储明文敏感数据
    let user_token = String::from("user_session_token_abc123");
    let session_data = b"encrypted_session_data".to_vec();

    // 使用脱敏后的键记录日志
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SECURITY_STORING_THE_USER_SESSION,
            &[("user_token", redact_cache_key(&user_token).to_string())]
        )
    );

    cache.set(&user_token, &session_data).await?;

    if let Some(data) = cache.get(&user_token).await? {
        println!(
            "{}",
            t(
                MSG_EXAMPLE_SECURITY_DATA_READ_BACK_BYTES,
                &[("data", data.len().to_string())]
            )
        );
    }

    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_5_SECURITY_BEST_PRACTICES, &[])
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_SECURITY_NEVER_LOG_SENSITIVE_INFORMATION, &[])
    );
    println!("{}", t(MSG_EXAMPLE_SECURITY_USE_THE_REDACTION_MODULE, &[]));
    println!(
        "{}",
        t(MSG_EXAMPLE_SECURITY_ENCRYPT_SENSITIVE_DATA_BEFORE, &[])
    );
    println!(
        "{}",
        t(MSG_EXAMPLE_SECURITY_USE_SECURE_CONNECTION_STRING, &[])
    );
    println!(
        "{}",
        t(MSG_EXAMPLE_SECURITY_ROTATE_SENSITIVE_TOKENS_IN, &[])
    );
    println!("{}", t(MSG_EXAMPLE_SECURITY_KEEP_CACHE_KEY_NAMES, &[]));

    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_6_CONFIGURATION_SAFETY, &[])
    );

    // 展示安全和不安全的配置方式
    println!(
        "{}",
        t(MSG_EXAMPLE_SECURITY_UNSAFE_STORING_PASSWORDS_IN, &[])
    );
    println!("   config.password = \"my_password\"");

    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_SAFE_USE_ENVIRONMENT_VARIABLES, &[])
    );
    println!("   config.password = std::env::var(\"CACHE_PASSWORD\")?");

    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_SAFE_HIDE_CONNECTION_STRING, &[])
    );
    let safe_conn_str = redact_connection_string("redis://user:secret@localhost:6379");
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_SECURITY_LOG_CONNECTING_TO,
            &[("safe_conn_str", safe_conn_str.to_string())]
        )
    );

    cache.clear().await?;

    println!("\n{}", t(MSG_EXAMPLE_SECURITY_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
