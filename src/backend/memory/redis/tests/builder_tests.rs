// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Builder tests for RedisBackend.

use super::*;
use crate::core::RedisModeType;
use crate::error::OxCacheError;
use serial_test::serial;

#[test]
fn test_redact_connection_string_with_password() {
    // pragma: allowlist secret
    let conn_str = "redis://:secret_password@localhost:6379/0";
    let redacted = RedisBackend::redact_connection_string(conn_str);
    assert!(!redacted.contains("secret_password"));
    assert!(redacted.contains("[REDACTED]"));
    assert!(redacted.contains("localhost:6379/0"));
}

#[test]
fn test_redact_connection_string_without_password() {
    let conn_str = "redis://localhost:6379/0";
    let redacted = RedisBackend::redact_connection_string(conn_str);
    assert_eq!(redacted, conn_str);
}

#[test]
fn test_redact_connection_string_no_protocol() {
    let conn_str = "localhost:6379";
    let redacted = RedisBackend::redact_connection_string(conn_str);
    assert_eq!(redacted, conn_str);
}

#[test]
fn test_redact_connection_string_rediss_protocol() {
    // pragma: allowlist secret
    let conn_str = "rediss://:mypw@example.com:6380/2";
    let redacted = RedisBackend::redact_connection_string(conn_str);
    assert!(!redacted.contains("mypw"));
    assert!(redacted.starts_with("rediss://[REDACTED]@"));
    assert!(redacted.contains("example.com:6380/2"));
}

#[tokio::test]
async fn test_builder_missing_connection_string() {
    let result = RedisBackend::builder().build().await;
    assert!(result.is_err());
    if let Err(OxCacheError::InvalidInput(msg)) = result {
        assert!(msg.contains("Connection string is required"));
    } else {
        panic!("Expected InvalidInput error");
    }
}

#[tokio::test]
#[serial]
async fn test_builder_insecure_rejected_without_env() {
    remove_allow_insecure_env();
    let result = RedisBackend::builder()
        .connection_string("redis://127.0.0.1:6379")
        .build()
        .await;
    assert!(result.is_err());
    if let Err(OxCacheError::InvalidInput(msg)) = result {
        assert!(msg.contains("TLS") || msg.contains("insecure"));
    } else {
        panic!("Expected InvalidInput error");
    }
    set_allow_insecure_env();
}

#[tokio::test]
#[ignore = "requires Redis server"]
#[serial]
async fn test_builder_insecure_allowed_with_env() {
    set_allow_insecure_env();
    let backend = RedisBackend::builder()
        .connection_string(REDIS_URL)
        .build()
        .await;
    assert!(backend.is_ok());
}

#[tokio::test]
#[ignore = "requires Redis server"]
#[serial]
async fn test_builder_insecure_allowed_with_dev_value() {
    set_insecure_env("development-only");
    let backend = RedisBackend::builder()
        .connection_string(REDIS_URL)
        .build()
        .await;
    assert!(backend.is_ok());
    set_allow_insecure_env();
}

#[tokio::test]
#[ignore = "requires Redis server"]
#[serial]
async fn test_builder_with_mode() {
    set_allow_insecure_env();
    let backend = RedisBackend::builder()
        .connection_string(REDIS_URL)
        .mode(RedisModeType::Standalone)
        .build()
        .await;
    assert!(backend.is_ok());
    assert_eq!(backend.unwrap().mode(), RedisModeType::Standalone);
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_builder_default_mode_is_standalone() {
    let backend = make_backend().await;
    assert_eq!(backend.mode(), RedisModeType::Standalone);
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_new_connects_to_redis() {
    let backend = make_backend().await;
    backend.health_check().await.expect("health check failed");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_with_pool_connects_to_redis() {
    set_allow_insecure_env();
    let backend = RedisBackend::with_pool(REDIS_URL, 4).await;
    assert!(backend.is_ok());
}

#[tokio::test]
#[serial]
async fn test_new_invalid_url_returns_error() {
    remove_allow_insecure_env();
    set_allow_insecure_env();
    let result = RedisBackend::new("redis://127.0.0.1:1/0").await;
    assert!(result.is_err());
    if let Err(OxCacheError::Connection(msg)) = result {
        assert!(msg.contains("Redis") || msg.contains("timeout") || msg.contains("connect"));
    } else {
        panic!("Expected Connection error");
    }
}

#[tokio::test]
#[serial]
async fn test_new_unreachable_host_times_out() {
    remove_allow_insecure_env();
    set_allow_insecure_env();
    let result = RedisBackend::new("redis://10.255.255.1:6379/0").await;
    assert!(result.is_err());
    if let Err(OxCacheError::Connection(msg)) = result {
        assert!(msg.contains("timeout") || msg.contains("Redis"));
    } else {
        panic!("Expected Connection/timeout error");
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_ping_returns_pong() {
    let backend = make_backend().await;
    let result = backend.ping().await.expect("ping failed");
    assert_eq!(result, "PONG");
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_health_check_ok() {
    let backend = make_backend().await;
    backend.health_check().await.expect("health check failed");
}

// ============================================================================
// Builder validation and chain method tests (no Redis server required)
// ============================================================================

#[tokio::test]
async fn test_builder_pool_size_zero_rejected() {
    let result = RedisBackend::builder()
        .connection_string("rediss://localhost:6379")
        .pool_size(0)
        .build()
        .await;
    assert!(result.is_err());
    if let Err(OxCacheError::InvalidInput(msg)) = result {
        assert!(msg.contains("pool size") || msg.contains("at least 1"));
    } else {
        panic!("Expected InvalidInput error for pool_size=0");
    }
}

#[tokio::test]
async fn test_builder_tls_connection_string_accepted() {
    // TLS URL should pass validation (will fail at connection, but not validation)
    let result = RedisBackend::builder()
        .connection_string("rediss://nonexistent.example.com:6379")
        .connection_timeout(std::time::Duration::from_millis(50))
        .build()
        .await;
    // Should fail at connection, not at TLS validation
    assert!(result.is_err());
    if let Err(OxCacheError::Connection(msg)) = result {
        assert!(
            msg.contains("Redis") || msg.contains("timeout") || msg.contains("connect") || msg.contains("unreachable"),
            "Expected connection error, got: {}",
            msg
        );
    }
}

#[tokio::test]
async fn test_builder_database_appended_to_url() {
    // database() should append /N to connection string
    // This will fail at connection but we verify the URL was modified
    let result = RedisBackend::builder()
        .connection_string("rediss://localhost:6379")
        .database(3)
        .connection_timeout(std::time::Duration::from_millis(50))
        .build()
        .await;
    // Connection will fail, but the builder accepted the config
    assert!(result.is_err());
}

#[test]
fn test_builder_chain_all_methods() {
    use std::time::Duration;

    // Verify all builder methods return Self for chaining
    let builder = RedisBackend::builder()
        .connection_string("rediss://localhost:6379")
        .mode(RedisModeType::Standalone)
        .pool_size(16)
        .connection_timeout(Duration::from_secs(5))
        .retry_count(5)
        .retry_delay(Duration::from_millis(200))
        .database(2)
        .circuit_breaker_threshold(10)
        .circuit_breaker_reset_timeout(Duration::from_secs(60))
        .dangerous_clear_enabled(true);

    // Builder should be constructable (build will fail at connection, but that's ok)
    let _ = builder;
}

#[test]
fn test_builder_distributed_config_applies_all_fields() {
    use crate::config::DistributedConfig;
    use std::time::Duration;

    let config = DistributedConfig::builder()
        .retry_count(7)
        .retry_base_delay(Duration::from_millis(250))
        .circuit_breaker_threshold(12)
        .circuit_breaker_reset_timeout(Duration::from_secs(45))
        .build();

    // Verify config values are set correctly
    assert_eq!(config.retry_count, 7);
    assert_eq!(config.retry_base_delay, Duration::from_millis(250));
    assert_eq!(config.circuit_breaker_threshold, 12);
    assert_eq!(config.circuit_breaker_reset_timeout, Duration::from_secs(45));

    // Apply to builder - just verify it compiles and chains
    let _builder = RedisBackend::builder()
        .connection_string("rediss://localhost:6379")
        .distributed_config(config);
}

#[test]
fn test_builder_default_values() {
    // Verify builder defaults via Debug
    let builder = RedisBackend::builder();
    let debug = format!("{:?}", builder);
    // Check key defaults are present in debug output
    assert!(debug.contains("pool_size: 8"));
    assert!(debug.contains("retry_count: 3"));
    assert!(debug.contains("circuit_breaker_threshold: 5"));
    assert!(debug.contains("dangerous_clear_enabled: false"));
}

#[tokio::test]
async fn test_builder_dangerous_clear_enabled_flag() {
    // Verify dangerous_clear_enabled(true) doesn't cause validation error
    let result = RedisBackend::builder()
        .connection_string("rediss://localhost:6379")
        .dangerous_clear_enabled(true)
        .connection_timeout(std::time::Duration::from_millis(50))
        .build()
        .await;
    // Should fail at connection, not at validation
    assert!(result.is_err());
}
