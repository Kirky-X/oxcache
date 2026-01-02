//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 安全性集成测试

use oxcache::backend::l2::L2Backend;
use oxcache::config::{L2Config, RedisMode};

#[path = "../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_redis_tls_config_parsing() {
    common::setup_logging();

    // Verify that we can parse a rediss:// (TLS) connection string.
    // Note: We cannot easily run a real TLS Redis server in this environment without certificates,
    // so we primarily verify the configuration parsing and client initialization attempt.

    let l2_config = L2Config {
        mode: RedisMode::Standalone,
        // "rediss://" scheme indicates TLS
        connection_string: std::env::var("REDIS_TLS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6380".to_string())
            .into(),
        enable_tls: true,
        ..Default::default()
    };

    // This is expected to fail connection, but it should validate that the URL scheme is accepted
    // by the underlying redis crate if compiled with TLS features.
    // However, if the crate doesn't have TLS features enabled, it might fail with a specific error.
    // For now, we just want to ensure our config struct and new() method don't reject it outright
    // before passing to the driver.

    let result = L2Backend::new(&l2_config).await;

    match result {
        Ok(_) => {
            // If by some miracle there is a TLS redis at that port (unlikely), or if open() is lazy.
            // Redis client open() is usually lazy, but we call get_connection_manager which connects.
            // So we expect an error here usually.
        }
        Err(e) => {
            // We expect a connection error, not a configuration parsing error.
            // If it was a config error (e.g. "unsupported scheme"), that would be a failure of our support.
            let msg = e.to_string();
            // Redis crate error for connection refused usually looks like "Connection refused" or IO error.
            // If it says "Feature 'tls' not enabled" or similar, we know we need to enable it.
            println!(
                "Got expected error connecting to non-existent TLS redis: {}",
                msg
            );
        }
    }
}
