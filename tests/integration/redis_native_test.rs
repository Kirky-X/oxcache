// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis 原生操作集成测试
//
// 测试 Redis 后端的原生功能，包括 Lua 脚本执行等
//
// 注意：部分高级功能（如 ZADD、ZRANGE 等 Sorted Set 操作）需要直接访问 Redis 命令
// 这些功能可以通过 Lua 脚本实现，或者使用 redis crate 的原生功能

use crate::common::{is_redis_available, setup_logging};
use oxcache::backend::client::RedisBackend;
use oxcache::backend::CacheBackend;
use oxcache::security::validate_lua_script;
use std::time::Duration;

/// 测试 Redis Lua 脚本验证功能
///
/// 验证 Lua 脚本的安全检查机制
#[tokio::test]
async fn test_redis_lua_script_validation() {
    setup_logging();

    // 有效的 Lua 脚本应该能通过验证
    let valid_script = r#"
        local key = KEYS[1]
        local value = ARGV[1]
        return redis.call('GET', key)
    "#;
    assert!(validate_lua_script(valid_script, 1).is_ok());

    // 包含禁止命令的脚本应该被拒绝
    let dangerous_script = r#"
        return redis.call('FLUSHALL')
    "#;
    assert!(validate_lua_script(dangerous_script, 0).is_err());

    // 包含 KEYS 命令的脚本应该被拒绝
    let keys_script = r#"
        local keys = redis.call('KEYS', '*')
        return keys
    "#;
    assert!(validate_lua_script(keys_script, 0).is_err());
}

/// 测试 Redis Lua 脚本最大长度限制
///
/// 验证超长脚本会被拒绝
#[tokio::test]
async fn test_redis_lua_script_max_length() {
    setup_logging();

    // 创建超过限制的脚本
    let long_script = "local x = 1\n".repeat(10000);
    assert!(validate_lua_script(&long_script, 1).is_err());
}

/// 测试 Redis Lua 脚本最大 key 数量限制
///
/// 验证超过最大 key 数量的脚本会被拒绝
#[tokio::test]
async fn test_redis_lua_script_max_keys() {
    setup_logging();

    // 创建声明大量 KEYS 的脚本
    let script = "local result = 0\n".to_string()
        + "for i = 1, 101 do\n"
        + "    local key = KEYS[i]\n"
        + "    result = result + 1\n"
        + "end\n"
        + "return result";
    assert!(validate_lua_script(&script, 101).is_err());
}

/// 测试基本的 Redis 操作（使用新 API）
///
/// 验证基本的 Redis 操作是否正常工作
#[tokio::test]
async fn test_basic_redis_operations_new_api() {
    setup_logging();

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = "redis://127.0.0.1:6381";
    let backend = RedisBackend::new(redis_url).await;

    if let Err(e) = backend {
        println!("Redis 连接失败: {:?}", e);
        return;
    }

    let backend = backend.unwrap();

    // 测试 PING
    let ping_result = backend.ping().await;
    assert!(ping_result.is_ok(), "PING should succeed");
    println!("✅ PING 测试通过");

    // 测试 SET/GET
    let test_key = "test:native:key1";
    let test_value = b"test_value_12345";

    let set_result = backend
        .set(test_key, test_value.to_vec(), Some(Duration::from_secs(60)))
        .await;
    assert!(set_result.is_ok(), "SET should succeed");
    println!("✅ SET 测试通过");

    let get_result = backend.get(test_key).await;
    assert!(get_result.is_ok(), "GET should succeed");
    assert_eq!(get_result.unwrap(), Some(test_value.to_vec()));
    println!("✅ GET 测试通过");

    // 测试 EXISTS
    let exists_result = backend.exists(test_key).await;
    assert!(exists_result.is_ok(), "EXISTS should succeed");
    assert!(exists_result.unwrap(), "Key should exist");
    println!("✅ EXISTS 测试通过");

    // 测试 TTL
    let ttl_result = backend.ttl(test_key).await;
    assert!(ttl_result.is_ok(), "TTL should succeed");
    assert!(ttl_result.unwrap().is_some(), "TTL should be positive");
    println!("✅ TTL 测试通过");

    // 测试 DELETE
    let delete_result = backend.delete(test_key).await;
    assert!(delete_result.is_ok(), "DELETE should succeed");
    println!("✅ DELETE 测试通过");

    // 验证删除后 key 不存在
    let get_after_delete = backend.get(test_key).await;
    assert!(get_after_delete.is_ok());
    assert!(get_after_delete.unwrap().is_none());
    println!("✅ 删除验证测试通过");

    println!("🎉 所有基本 Redis 操作测试通过！");
}

/// 测试 Lua 脚本执行（使用新 API）
///
/// 验证 Lua 脚本功能是否正常工作
#[tokio::test]
async fn test_redis_lua_script_execution() {
    setup_logging();

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = "redis://127.0.0.1:6381";
    let backend = RedisBackend::new(redis_url).await;

    if let Err(e) = backend {
        println!("Redis 连接失败: {:?}", e);
        return;
    }

    let backend = backend.unwrap();

    // 先设置一个测试值
    let test_key = "test:lua:key1";
    let test_value = b"hello";
    let _ = backend
        .set(test_key, test_value.to_vec(), Some(Duration::from_secs(60)))
        .await;

    // 执行一个简单的 Lua 脚本
    let lua_script = r#"
        local key = KEYS[1]
        local value = redis.call('GET', key)
        return value
    "#;

    let eval_result = backend.eval_lua(lua_script, &[test_key], &[]).await;
    assert!(eval_result.is_ok(), "Lua script execution should succeed");
    println!("✅ Lua 脚本执行测试通过");
}

/// 测试 Sorted Set 操作（通过 Lua 脚本）
///
/// 验证可以使用 Lua 脚本执行 Sorted Set 操作
///
/// 注意：这不是直接测试 ZADD/ZRANGE 命令，而是测试通过 Lua 脚本实现这些操作
#[tokio::test]
async fn test_redis_sorted_set_via_lua() {
    setup_logging();

    if !is_redis_available().await {
        println!("跳过测试: Redis不可用");
        return;
    }

    let redis_url = "redis://127.0.0.1:6381";
    let backend = RedisBackend::new(redis_url).await;

    if let Err(e) = backend {
        println!("Redis 连接失败: {:?}", e);
        return;
    }

    let backend = backend.unwrap();

    let test_key = "test:sorted:set1";

    // 通过 Lua 脚本模拟 ZADD 操作
    let zadd_script = r#"
        local key = KEYS[1]
        local score = tonumber(ARGV[1])
        local member = ARGV[2]
        redis.call('ZADD', key, score, member)
        return 1
    "#;

    let zadd_result = backend.eval_lua(zadd_script, &[test_key], &["1.0", "member1"]).await;
    assert!(zadd_result.is_ok(), "ZADD via Lua should succeed");
    println!("✅ ZADD via Lua 测试通过");

    // 通过 Lua 脚本模拟 ZRANGE 操作
    let zrange_script = r#"
        local key = KEYS[1]
        local start = tonumber(ARGV[1])
        local stop = tonumber(ARGV[2])
        return redis.call('ZRANGE', key, start, stop)
    "#;

    let zrange_result = backend.eval_lua(zrange_script, &[test_key], &["0", "-1"]).await;
    assert!(zrange_result.is_ok(), "ZRANGE via Lua should succeed");
    println!("✅ ZRANGE via Lua 测试通过");

    // 清理测试数据
    let _ = backend.delete(test_key).await;
    println!("✅ Sorted Set 操作测试通过");
}
