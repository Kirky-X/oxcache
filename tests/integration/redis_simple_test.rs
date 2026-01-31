// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

// Simplified integration test - verifies core functionality
// Run with: REDIS_URL=redis://127.0.0.1:6381 cargo test --lib test_redis_integration --all-features
// Or set OXCACHE_ALLOW_INSECURE_REDIS=1 to allow non-TLS connections

#[cfg(test)]
#[cfg(feature = "redis")]
mod redis_integration_tests {
    use oxcache::backend::client::RedisBackend;
    use oxcache::backend::CacheBackend;
    use std::time::Duration;

    /// Check if Redis is available for testing
    async fn is_redis_available() -> bool {
        let redis_url = "redis://127.0.0.1:6381";
        RedisBackend::new(redis_url).await.is_ok()
    }

    #[tokio::test]
    async fn test_redis_connection() {
        let redis_url = "redis://127.0.0.1:6381";

        // Skip if Redis is not available
        if !is_redis_available().await {
            println!(
                "⚠️  Skipping Redis test - Redis not available at {}",
                redis_url
            );
            return;
        }

        // Test 1: Create backend
        let backend = RedisBackend::new(redis_url).await;
        assert!(backend.is_ok(), "Failed to create Redis backend");
        println!("✅ Redis backend created");

        // Test 2: Set operation
        let backend = backend.unwrap();
        let result = backend
            .set(
                "test:key",
                b"test_value".to_vec(),
                Some(Duration::from_secs(60)),
            )
            .await;
        assert!(result.is_ok(), "SET operation failed");
        println!("✅ SET operation successful");

        // Test 3: Get operation
        let result = backend.get("test:key").await;
        assert!(result.is_ok(), "GET operation failed");
        let value = result.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()), "Value mismatch");
        println!("✅ GET operation successful");

        // Test 4: Delete operation
        let result = backend.delete("test:key").await;
        assert!(result.is_ok(), "DELETE operation failed");
        println!("✅ DELETE operation successful");

        // Test 5: Verify deletion
        let result = backend.get("test:key").await;
        assert!(result.is_ok(), "GET after DELETE failed");
        assert_eq!(result.unwrap(), None, "Key should be None after deletion");
        println!("✅ Deletion verified");

        println!(
            "
🎉 All Redis integration tests passed!"
        );
    }

    #[tokio::test]
    async fn test_redis_ttl() {
        let redis_url = "redis://127.0.0.1:6381";

        // Skip if Redis is not available
        if !is_redis_available().await {
            println!(
                "⚠️  Skipping Redis TTL test - Redis not available at {}",
                redis_url
            );
            return;
        }

        let backend = RedisBackend::new(redis_url).await.unwrap();

        // Set with 2 second TTL
        let result = backend
            .set("ttl:test", b"value".to_vec(), Some(Duration::from_secs(2)))
            .await;
        assert!(result.is_ok());

        // Verify value exists
        let result = backend.get("ttl:test").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Verify expiration
        let result = backend.get("ttl:test").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        println!("✅ TTL operation test passed");
    }

    #[tokio::test]
    async fn test_redis_batch_operations() {
        let redis_url = "redis://127.0.0.1:6381";

        // Skip if Redis is not available
        if !is_redis_available().await {
            println!(
                "⚠️  Skipping Redis batch test - Redis not available at {}",
                redis_url
            );
            return;
        }

        let backend = RedisBackend::new(redis_url).await.unwrap();

        // Test batch operations
        for i in 0..10 {
            let key = format!("batch:test:{}", i);
            let value = format!("value_{}", i).into_bytes();
            let result = backend
                .set(&key, value, Some(Duration::from_secs(60)))
                .await;
            assert!(result.is_ok(), "Batch SET {} failed", i);
        }

        // Verify all values
        for i in 0..10 {
            let key = format!("batch:test:{}", i);
            let result = backend.get(&key).await;
            assert!(result.is_ok(), "Batch GET {} failed", i);
            assert!(result.unwrap().is_some(), "Batch GET {} returned None", i);
        }

        // Cleanup
        for i in 0..10 {
            let key = format!("batch:test:{}", i);
            let _ = backend.delete(&key).await;
        }

        println!("✅ Batch operations test passed");
    }
}
