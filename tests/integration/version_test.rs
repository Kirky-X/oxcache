//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 版本管理集成测试

use oxcache::backend::l2::L2Backend;
use oxcache::config::L2Config;
use std::sync::Arc;

#[path = "../common/mod.rs"]
mod common;

#[tokio::test]
async fn test_version_control() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping test_version_control because Redis is not available");
        return;
    }

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let l2_config = L2Config {
        connection_string: redis_url.into(),
        ..Default::default()
    };
    let l2 = Arc::new(
        L2Backend::new(&l2_config)
            .await
            .expect("Failed to create L2"),
    );

    // Ensure key is clean
    let _ = l2.delete("version_key").await;

    // First set should initialize version
    l2.set_with_version("version_key", b"v1".to_vec(), None)
        .await
        .expect("Set failed");

    // Get value and verify version
    let (val1, ver1) = l2
        .get_with_version("version_key")
        .await
        .expect("Get failed")
        .expect("Value missing");
    assert_eq!(val1, b"v1");

    // Second set should increment version
    l2.set_with_version("version_key", b"v2".to_vec(), None)
        .await
        .expect("Set failed");

    // Get value and verify version incremented
    let (val2, ver2) = l2
        .get_with_version("version_key")
        .await
        .expect("Get failed")
        .expect("Value missing");
    assert_eq!(val2, b"v2");
    assert!(ver2 > ver1, "Version should increment: {} > {}", ver2, ver1);

    // Clean up
    let _ = l2.delete("version_key").await;
}
