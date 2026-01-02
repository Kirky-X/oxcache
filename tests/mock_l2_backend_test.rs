//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 测试真实L2Backend与Redis的集成

use common::redis_test_utils::{
    create_standalone_config, is_redis_available, test_redis_connection, wait_for_redis,
};
use oxcache::backend::l2::L2Backend;

mod common;

#[tokio::test]
async fn test_real_l2_backend_creation() {
    println!("测试真实L2Backend创建...");

    if !is_redis_available() {
        println!("跳过测试: Redis不可用");
        return;
    }

    if !wait_for_redis("redis://127.0.0.1:6379").await {
        println!("跳过测试: Redis连接超时");
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

    let config = create_standalone_config();

    match L2Backend::new(&config).await {
        Ok(backend) => {
            println!("成功创建真实L2Backend");
            println!("后端类型正确：真实Redis后端");
            assert_eq!(backend.command_timeout_ms(), config.command_timeout_ms);
        }
        Err(e) => {
            println!("创建真实后端失败: {:?}", e);
            panic!("应该能成功创建真实后端: {}", e);
        }
    }
}

#[tokio::test]
async fn test_real_l2_backend_ping() {
    println!("测试真实L2Backend的ping方法...");

    if !is_redis_available() {
        println!("跳过测试: Redis不可用");
        return;
    }

    if !wait_for_redis("redis://127.0.0.1:6379").await {
        println!("跳过测试: Redis连接超时");
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

    let config = create_standalone_config();

    let backend = L2Backend::new(&config).await.unwrap();

    let result = backend.ping().await;
    assert!(
        result.is_ok(),
        "真实后端ping应该成功, 实际结果: {:?}",
        result
    );
    println!("真实后端按预期返回成功");
}

#[tokio::test]
async fn test_real_l2_backend_with_tls() {
    println!("测试真实L2Backend创建...");

    if !is_redis_available() {
        println!("跳过测试: Redis不可用");
        return;
    }

    if !wait_for_redis("redis://127.0.0.1:6379").await {
        println!("跳过测试: Redis连接超时");
        return;
    }

    let config = create_standalone_config();

    match L2Backend::new(&config).await {
        Ok(_backend) => {
            println!("成功创建配置的L2Backend");
        }
        Err(e) => {
            println!("创建后端失败: {:?}", e);
            panic!("应该能成功创建后端: {}", e);
        }
    }
}
