// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 简化集成测试 - 验证核心功能

#[cfg(test)]
#[cfg(feature = "redis")]
mod redis_integration_tests {
    use crate::common::{get_redis_url, is_redis_available};
    use oxcache::backend::client::RedisBackend;
    use oxcache::backend::CacheBackend;
    use std::time::Duration;

    #[tokio::test]
    async fn test_redis_connection() {
        if !is_redis_available().await {
            println!("⚠️  Skipping Redis test - Redis not available");
            return;
        }

        let redis_url = get_redis_url();
        println!("Testing Redis at: {}", redis_url);

        let backend = RedisBackend::new(&redis_url).await;
        assert!(backend.is_ok(), "Failed to create Redis backend");
        println!("✅ Redis backend created");

        let backend = backend.unwrap();
        let result = backend
            .set("test:key", b"test_value".to_vec(), Some(Duration::from_secs(60)))
            .await;
        assert!(result.is_ok(), "SET operation failed");
        println!("✅ SET operation successful");

        let result = backend.get("test:key").await;
        assert!(result.is_ok(), "GET operation failed");
        let value = result.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()), "Value mismatch");
        println!("✅ GET operation successful");

        let result = backend.delete("test:key").await;
        assert!(result.is_ok(), "DELETE operation failed");
        println!("✅ DELETE operation successful");

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
        if !is_redis_available().await {
            println!("⚠️  Skipping Redis TTL test - Redis not available");
            return;
        }

        let redis_url = get_redis_url();
        let backend = RedisBackend::new(&redis_url).await.unwrap();

        let result = backend
            .set("ttl:test", b"value".to_vec(), Some(Duration::from_secs(2)))
            .await;
        assert!(result.is_ok());

        let result = backend.get("ttl:test").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        tokio::time::sleep(Duration::from_secs(3)).await;

        let result = backend.get("ttl:test").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        println!("✅ TTL operation test passed");
    }

    #[tokio::test]
    async fn test_redis_batch_operations() {
        if !is_redis_available().await {
            println!("⚠️  Skipping Redis batch test - Redis not available");
            return;
        }

        let redis_url = get_redis_url();
        let backend = RedisBackend::new(&redis_url).await.unwrap();

        for i in 0..10 {
            let key = format!("batch:test:{}", i);
            let value = format!("value_{}", i).into_bytes();
            let result = backend.set(&key, value, Some(Duration::from_secs(60))).await;
            assert!(result.is_ok(), "Batch SET {} failed", i);
        }

        for i in 0..10 {
            let key = format!("batch:test:{}", i);
            let result = backend.get(&key).await;
            assert!(result.is_ok(), "Batch GET {} failed", i);
            assert!(result.unwrap().is_some(), "Batch GET {} returned None", i);
        }

        for i in 0..10 {
            let key = format!("batch:test:{}", i);
            let _ = backend.delete(&key).await;
        }

        println!("✅ Batch operations test passed");
    }
}
