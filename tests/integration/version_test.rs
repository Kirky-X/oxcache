// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 版本管理集成测试

#[cfg(feature = "redis")]
use oxcache::backend::memory::RedisBackend;

use std::sync::Arc;

use crate::common;

#[tokio::test]
#[cfg(feature = "redis")]
async fn test_version_control() {
    common::setup_logging();

    if !common::is_redis_available().await {
        println!("Skipping test_version_control because Redis is not available");
        return;
    }

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let l2: Arc<dyn oxcache::backend::CacheBackend> = match RedisBackend::new(&redis_url).await {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            println!("无法创建Redis后端: {:?}", e);
            return;
        }
    };

    // Ensure key is clean
    let _ = l2.delete("version_key").await;

    // First set should initialize
    l2.set("version_key", b"v1".to_vec(), None).await.expect("Set failed");

    // Get value
    let val1 = l2.get("version_key").await.expect("Get failed");
    assert_eq!(val1, Some(b"v1".to_vec()));

    // Second set should work
    l2.set("version_key", b"v2".to_vec(), None).await.expect("Set failed");

    // Get value and verify it changed
    let val2 = l2.get("version_key").await.expect("Get failed");
    assert_eq!(val2, Some(b"v2".to_vec()));

    // Clean up
    let _ = l2.delete("version_key").await;
}
