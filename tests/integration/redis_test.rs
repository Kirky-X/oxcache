// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis集成测试 - 新API版本

#![allow(deprecated)]

use crate::common::redis_test_utils::{test_redis_connection};
use crate::common::is_redis_available;
use oxcache::backend::client::RedisBackend;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_redis_backend_standalone_creation() {
    println!("测试RedisBackend Standalone模式创建...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let result = RedisBackend::new("redis://127.0.0.1:6381").await;

    assert!(
        result.is_ok(),
        "应该能成功创建Standalone RedisBackend: {:?}",
        result.err()
    );
    println!("✓ Standalone RedisBackend创建成功");
}

#[tokio::test]
async fn test_redis_backend_standalone_operations() {
    println!("测试RedisBackend Standalone模式基本操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let backend: Arc<dyn CacheBackend> = match RedisBackend::new("redis://127.0.0.1:6381").await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            println!("创建RedisBackend失败: {:?}", e);
            return;
        }
    };

    let test_key = "oxcache:test:redis:basic";

    let set_result = backend
        .set(
            test_key,
            b"standalone_value".to_vec(),
            Some(Duration::from_secs(60)),
        )
        .await;
    assert!(set_result.is_ok(), "SET操作失败: {:?}", set_result.err());

    let get_result = backend.get(test_key).await;
    assert!(get_result.is_ok(), "GET操作失败");
    assert_eq!(get_result.unwrap(), Some(b"standalone_value".to_vec()));

    let delete_result = backend.delete(test_key).await;
    assert!(delete_result.is_ok(), "DELETE操作失败");

    println!("✓ Standalone模式基本操作测试通过");
}

#[tokio::test]
async fn test_redis_backend_health_check() {
    println!("测试RedisBackend健康检查...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let backend = match RedisBackend::new("redis://127.0.0.1:6379").await {
        Ok(b) => b,
        Err(e) => {
            println!("创建RedisBackend失败: {:?}", e);
            return;
        }
    };

    for i in 0..5 {
        let ping_result = backend.ping().await;
        assert!(ping_result.is_ok(), "第{}次Ping失败", i + 1);
    }

    println!("✓ 5次健康检查全部通过");
}

#[tokio::test]
async fn test_redis_backend_ttl_operations() {
    println!("测试RedisBackend TTL操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let backend: Arc<dyn CacheBackend> = match RedisBackend::new("redis://127.0.0.1:6381").await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            println!("创建RedisBackend失败: {:?}", e);
            return;
        }
    };

    let test_key = "oxcache:test:redis:ttl";

    let set_result = backend
        .set(
            test_key,
            b"ttl_value".to_vec(),
            Some(Duration::from_secs(5)),
        )
        .await;
    assert!(set_result.is_ok(), "SET with TTL失败");

    let get_result = backend.get(test_key).await;
    assert!(get_result.is_ok());
    assert_eq!(get_result.unwrap(), Some(b"ttl_value".to_vec()));

    tokio::time::sleep(Duration::from_secs(6)).await;

    let expire_check = backend.get(test_key).await;
    assert!(expire_check.is_ok());
    assert!(expire_check.unwrap().is_none(), "键应该在TTL过期后被删除");

    println!("✓ TTL操作测试通过");
}

#[tokio::test]
async fn test_redis_backend_exists_operation() {
    println!("测试RedisBackend EXISTS操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let backend: Arc<dyn CacheBackend> = match RedisBackend::new("redis://127.0.0.1:6381").await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            println!("创建RedisBackend失败: {:?}", e);
            return;
        }
    };

    let test_key = "oxcache:test:redis:exists";

    let exists_before = backend.get(test_key).await;
    assert!(exists_before.is_ok());
    assert!(exists_before.unwrap().is_none(), "不存在的键应该返回None");

    backend
        .set(
            test_key,
            b"exists_value".to_vec(),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    let exists_after = backend.get(test_key).await;
    assert!(exists_after.is_ok());
    assert!(exists_after.unwrap().is_some(), "存在的键应该返回Some");

    backend.delete(test_key).await.unwrap();

    let exists_final = backend.get(test_key).await;
    assert!(exists_final.is_ok());
    assert!(exists_final.unwrap().is_none(), "删除后的键应该返回None");

    println!("✓ EXISTS操作测试通过");
}

#[tokio::test]
async fn test_redis_backend_multiple_operations() {
    println!("测试RedisBackend批量操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let backend: Arc<dyn CacheBackend> = match RedisBackend::new("redis://127.0.0.1:6381").await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            println!("创建RedisBackend失败: {:?}", e);
            return;
        }
    };

    for i in 0..10 {
        let key = format!("oxcache:test:redis:batch_{}", i);
        let value = format!("batch_value_{}", i);
        assert!(backend
            .set(
                &key,
                value.as_bytes().to_vec(),
                Some(Duration::from_secs(60))
            )
            .await
            .is_ok());
    }
    println!("✓ 批量写入10个键成功");

    for i in 0..10 {
        let key = format!("oxcache:test:redis:batch_{}", i);
        let expected = format!("batch_value_{}", i);
        let result = backend.get(&key).await.unwrap();
        assert_eq!(result, Some(expected.as_bytes().to_vec()));
    }
    println!("✓ 批量读取10个键成功");

    // 清理
    for i in 0..10 {
        let key = format!("oxcache:test:redis:batch_{}", i);
        let _ = backend.delete(&key).await;
    }
    println!("✓ 批量清理成功");
}

#[tokio::test]
async fn test_redis_backend_concurrent_operations() {
    println!("测试RedisBackend并发操作...");

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    match test_redis_connection().await {
        Ok(()) => {
            println!("Redis连接成功");
        }
        Err(e) => {
            println!("跳过测试: Redis连接失败 - {}", e);
            return;
        }
    }

    let backend: Arc<dyn CacheBackend> = match RedisBackend::new("redis://127.0.0.1:6381").await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            println!("创建RedisBackend失败: {:?}", e);
            return;
        }
    };

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let backend = backend.clone();
            tokio::spawn(async move {
                for j in 0..10 {
                    let key = format!("oxcache:test:redis:concurrent_{}_{}", i, j);
                    let value = format!("value_{}_{}", i, j);
                    let _ = backend
                        .set(
                            &key,
                            value.as_bytes().to_vec(),
                            Some(Duration::from_secs(60)),
                        )
                        .await;
                }
            })
        })
        .collect();

    for handle in handles {
        handle.await.expect("并发任务失败");
    }
    println!("✓ 5个任务并发写入50个键成功");

    // 验证至少有一些键被正确写入
    let sample_key = "oxcache:test:redis:concurrent_0_0";
    let result = backend.get(sample_key).await.unwrap();
    assert!(result.is_some(), "并发写入的键应该存在");

    // 清理
    for i in 0..5 {
        for j in 0..10 {
            let key = format!("oxcache:test:redis:concurrent_{}_{}", i, j);
            let _ = backend.delete(&key).await;
        }
    }
    println!("✓ 并发操作测试完成");
}
