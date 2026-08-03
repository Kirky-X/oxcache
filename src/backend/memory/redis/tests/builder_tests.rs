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
async fn test_new_invalid_url_returns_error() {
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
async fn test_new_unreachable_host_times_out() {
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
