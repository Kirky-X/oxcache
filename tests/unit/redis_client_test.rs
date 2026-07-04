// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis client unit tests extracted from client.rs

#![cfg(feature = "redis")]

use oxcache::backend::memory::redis::{RedisBackendBuilder, RedisMode};

#[test]
fn test_redis_mode_default() {
    assert_eq!(RedisMode::Standalone, RedisMode::default());
}

#[test]
fn test_redis_mode_variants() {
    let _standalone = RedisMode::Standalone;
    let _sentinel = RedisMode::Sentinel;
    let _cluster = RedisMode::Cluster;
}

#[test]
fn test_redis_backend_builder_build_without_connection_string() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async { RedisBackendBuilder::default().build().await });
    assert!(result.is_err());
    if let Err(e) = result {
        let err_msg = e.to_string();
        assert!(err_msg.contains("Connection string is required"));
    }
}

mod security_tests {
    use oxcache::backend::memory::redis::RedisBackend;

    #[test]
    fn test_redact_connection_string_with_password() {
        let conn_str = "redis://:secret_password@localhost:6379/0";
        let redacted = RedisBackend::redact_connection_string(conn_str);

        assert!(!redacted.contains("secret_password"), "Password should be redacted");
        assert!(redacted.contains("[REDACTED]"), "Should contain REDACTED marker");
        assert!(redacted.contains("localhost:6379"), "Host should be visible");
    }

    #[test]
    fn test_redact_connection_string_with_user_and_password() {
        let conn_str = "redis://user:mypassword@redis.example.com:6379/1";
        let redacted = RedisBackend::redact_connection_string(conn_str);

        assert!(!redacted.contains("mypassword"), "Password should be redacted");
        assert!(redacted.contains("[REDACTED]"), "Should contain REDACTED marker");
        assert!(redacted.contains("redis.example.com:6379"), "Host should be visible");
    }

    #[test]
    fn test_redact_connection_string_no_password() {
        let conn_str = "redis://localhost:6379/0";
        let redacted = RedisBackend::redact_connection_string(conn_str);

        assert_eq!(conn_str, redacted, "Should not change if no password");
    }

    #[test]
    fn test_redact_connection_string_empty() {
        let conn_str = "";
        let redacted = RedisBackend::redact_connection_string(conn_str);
        assert_eq!(conn_str, redacted, "Empty string should remain empty");
    }

    #[test]
    fn test_redact_connection_string_rediss() {
        let conn_str = "rediss://:secure_password@localhost:6379/0";
        let redacted = RedisBackend::redact_connection_string(conn_str);

        assert!(!redacted.contains("secure_password"));
        assert!(redacted.contains("[REDACTED]"));
        assert!(redacted.starts_with("rediss://"));
    }
}
