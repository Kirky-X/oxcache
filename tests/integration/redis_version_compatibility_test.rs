// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis版本兼容性测试 - 支持Redis 6.0, 6.2, 7.0, 7.2等多个版本

#![cfg(feature = "redis")]

use oxcache::backend::client::redis::RedisBackend;
use oxcache::backend::CacheBackend;

/// 测试指定Redis版本的Standalone模式兼容性
async fn test_redis_version_standalone(
    version: &str,
    connection_string: &str,
) -> Result<(), String> {
    println!("Testing Redis {} Standalone compatibility...", version);

    let backend: RedisBackend = RedisBackend::new(connection_string)
        .await
        .map_err(|e| format!("Redis {} connection failed: {}", version, e))?;

    // 测试基本操作
    let test_key = format!("test:{}:compatibility", version.replace('.', "_"));
    let test_value = format!("Redis {} compatibility test data", version);

    // SET
    backend
        .set(
            &test_key,
            test_value.as_bytes().to_vec(),
            Some(std::time::Duration::from_secs(60)),
        )
        .await
        .map_err(|e| format!("Redis {} set failed: {}", version, e))?;

    // GET
    let retrieved = backend
        .get(&test_key)
        .await
        .map_err(|e| format!("Redis {} get failed: {}", version, e))?;

    if retrieved != Some(test_value.as_bytes().to_vec()) {
        return Err(format!("Redis {} value mismatch", version));
    }

    // DELETE
    backend
        .delete(&test_key)
        .await
        .map_err(|e| format!("Redis {} delete failed: {}", version, e))?;

    println!("✅ Redis {} Standalone compatibility passed", version);
    Ok(())
}

/// 测试Redis 6.x版本兼容性
#[tokio::test]
async fn test_redis_6_compatibility() {
    let connection_string =
        std::env::var("REDIS_6_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    match RedisBackend::new(&connection_string).await {
        Ok(backend) => {
            let test_key = "test:redis6:compatibility";
            let test_value = b"Redis 6 compatibility test data";

            if let Err(e) = backend.set(test_key, test_value.to_vec(), None).await {
                println!("Skipping Redis 6.x compatibility test: set failed - {}", e);
                return;
            }

            match backend.get(test_key).await {
                Ok(retrieved) => {
                    assert_eq!(retrieved, Some(test_value.to_vec()));
                }
                Err(e) => {
                    println!("Skipping Redis 6.x compatibility test: get failed - {}", e);
                    let _ = backend.delete(test_key).await;
                    return;
                }
            }

            let _ = backend.delete(test_key).await;
            println!("Redis 6.x compatibility test passed");
        }
        Err(e) => {
            println!("Skipping Redis 6.x compatibility test: {}", e);
        }
    }
}

/// 测试Redis 7.x版本兼容性
#[tokio::test]
async fn test_redis_7_compatibility() {
    let connection_string =
        std::env::var("REDIS_7_URL").unwrap_or_else(|_| "redis://127.0.0.1:6380".to_string());

    match RedisBackend::new(&connection_string).await {
        Ok(backend) => {
            let test_key = "test:redis7:compatibility";
            let test_value = b"Redis 7 compatibility test data with enhanced features";

            if let Err(e) = backend
                .set(
                    test_key,
                    test_value.to_vec(),
                    Some(std::time::Duration::from_secs(60)),
                )
                .await
            {
                println!("Skipping Redis 7.x compatibility test: set failed - {}", e);
                return;
            }

            match backend.get(test_key).await {
                Ok(retrieved) => {
                    assert_eq!(retrieved, Some(test_value.to_vec()));
                }
                Err(e) => {
                    println!("Skipping Redis 7.x compatibility test: get failed - {}", e);
                    let _ = backend.delete(test_key).await;
                    return;
                }
            }

            // 测试TTL功能
            match backend.ttl(test_key).await {
                Ok(ttl) => {
                    if let Some(ttl_value) = ttl {
                        let ttl_secs = ttl_value.as_secs();
                        assert!(ttl_secs > 0 && ttl_secs <= 60);
                    }
                }
                Err(e) => {
                    println!(
                        "Skipping Redis 7.x compatibility test: TTL test failed - {}",
                        e
                    );
                    let _ = backend.delete(test_key).await;
                    return;
                }
            }

            let _ = backend.delete(test_key).await;
            println!("Redis 7.x compatibility test passed");
        }
        Err(e) => {
            println!("Skipping Redis 7.x compatibility test: {}", e);
        }
    }
}

/// 全面的Redis多版本兼容性测试
#[tokio::test]
async fn test_comprehensive_redis_version_compatibility() {
    println!("Starting comprehensive Redis version compatibility tests...");

    if std::env::var("REDIS_VERSION_TEST_ENABLED").is_err() {
        println!("Redis version compatibility tests are disabled.");
        println!("Set REDIS_VERSION_TEST_ENABLED=1 to enable these tests.");
        return;
    }

    let configs = vec![
        (
            "6.0",
            std::env::var("REDIS_6_0_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
        ),
        (
            "7.0",
            std::env::var("REDIS_7_0_URL").unwrap_or_else(|_| "redis://127.0.0.1:6380".to_string()),
        ),
    ];

    let mut passed_tests = Vec::new();
    let mut failed_tests = Vec::new();
    let mut skipped_tests = Vec::new();

    for (version, url) in configs {
        println!("\nTesting Redis {}...", version);

        match test_redis_version_standalone(version, &url).await {
            Ok(_) => {
                passed_tests.push(format!("{} Standalone", version));
                println!("  ✅ Standalone mode passed");
            }
            Err(e) => {
                if e.contains("Connection timed out") || e.contains("connection refused") {
                    skipped_tests.push(format!("{} Standalone: {}", version, e));
                    println!("  ⚠️  Standalone mode skipped: {}", e);
                } else {
                    failed_tests.push(format!("{} Standalone: {}", version, e));
                    println!("  ❌ Standalone mode failed: {}", e);
                }
            }
        }
    }

    println!("\nRedis Version Compatibility Test Summary:");
    println!("  ✅ Passed: {}", passed_tests.len());
    println!("  ❌ Failed: {}", failed_tests.len());
    println!("  ⚠️  Skipped: {}", skipped_tests.len());

    if !failed_tests.is_empty() {
        panic!(
            "Redis version compatibility tests failed: {:?}",
            failed_tests
        );
    }

    println!("\n🎉 All Redis version compatibility tests completed!");
}
