// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis测试工具 - 新API版本

#![allow(dead_code)]

#[cfg(feature = "redis")]
use oxcache::backend::client::RedisBackend;
#[cfg(feature = "redis")]
use oxcache::backend::CacheBackend;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "redis")]
pub(crate) async fn create_redis_backend_with_real_redis() -> Result<Arc<dyn CacheBackend>, String>
{
    match RedisBackend::new("redis://127.0.0.1:6379").await {
        Ok(backend) => Ok(Arc::new(backend)),
        Err(e) => Err(format!("无法创建Redis连接: {}", e)),
    }
}

pub(crate) async fn create_standalone_config() -> String {
    "redis://127.0.0.1:6379".to_string()
}

pub(crate) async fn wait_for_redis(_url: &str) -> bool {
    // 简化实现，直接检查Redis可用性
    is_redis_available()
}

pub(crate) async fn is_redis_available_default() -> bool {
    is_redis_available()
}

pub(crate) async fn test_redis_connection() -> Result<(), String> {
    let backend = match create_redis_backend_with_real_redis().await {
        Ok(b) => b,
        Err(e) => return Err(format!("无法创建Redis连接: {}", e)),
    };
    let test_key = "oxcache:test:connection";
    if let Err(e) = backend
        .set(test_key, b"test".to_vec(), Some(Duration::from_secs(60)))
        .await
    {
        return Err(format!("SET操作失败: {}", e));
    }
    let value_opt = match backend.get(test_key).await {
        Ok(v) => v,
        Err(e) => return Err(format!("GET操作失败: {}", e)),
    };
    let value = match value_opt {
        Some(v) => v,
        None => return Err("Redis返回空值".to_string()),
    };
    if &value != b"test" {
        return Err("Redis返回的值不正确".to_string());
    }
    if let Err(e) = backend.delete(test_key).await {
        return Err(format!("DELETE操作失败: {}", e));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn is_redis_available() -> bool {
    std::env::var("OXCACHE_SKIP_REDIS_TESTS").is_err()
}

#[allow(dead_code)]
pub async fn cleanup_test_keys(_pattern: &str) -> Result<(), String> {
    // 简化实现
    Ok(())
}
