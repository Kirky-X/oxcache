// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Lua script execution tests for RedisBackend.

use super::*;
use crate::backend::LuaExecutor;
use crate::error::OxCacheError;
use std::sync::Arc;

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_eval_lua_simple_return() {
    let backend = make_backend().await;
    let script = "return 'hello'";
    let result = backend.eval_lua(script, &[], &[]).await.expect("eval_lua failed");
    match result {
        redis::Value::BulkString(s) => assert_eq!(s, b"hello"),
        redis::Value::SimpleString(s) => assert_eq!(s, "hello"),
        other => panic!("Expected string, got {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_eval_lua_returns_int() {
    let backend = make_backend().await;
    let script = "return 42";
    let result = backend.eval_lua(script, &[], &[]).await.expect("eval_lua failed");
    match result {
        redis::Value::Int(n) => assert_eq!(n, 42),
        other => panic!("Expected Int(42), got {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_eval_lua_with_keys_and_args() {
    let backend = make_backend().await;
    let key = unique_key("lua_key");
    backend
        .set(Arc::from(key.as_str()), Arc::new(b"100".to_vec()), None)
        .await
        .unwrap();

    let script = "local v = redis.call('GET', KEYS[1]); return tonumber(v) + tonumber(ARGV[1])";
    let result = backend
        .eval_lua(script, &[&key], &["5"])
        .await
        .expect("eval_lua failed");
    match result {
        redis::Value::Int(n) => assert_eq!(n, 105),
        other => panic!("Expected Int(105), got {:?}", other),
    }
    cleanup(&backend, &key).await;
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_script_load_and_eval_sha() {
    let backend = make_backend().await;
    let script = "return 1 + 1";
    let sha = backend.script_load(script).await.expect("script_load failed");
    assert_eq!(sha.len(), 40);
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));

    let result = backend.eval_sha(&sha, &[], &[]).await.expect("eval_sha failed");
    match result {
        redis::Value::Int(n) => assert_eq!(n, 2),
        other => panic!("Expected Int(2), got {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_eval_sha_invalid_format_rejected() {
    let backend = make_backend().await;
    let result = backend.eval_sha("abc123", &[], &[]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        OxCacheError::InvalidInput(msg) => assert!(msg.contains("SHA")),
        other => panic!("Expected InvalidInput, got {:?}", other),
    }

    let result = backend
        .eval_sha("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", &[], &[])
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_eval_lua_forbidden_command_rejected() {
    let backend = make_backend().await;
    let script = "redis.call('FLUSHALL')";
    let result = backend.eval_lua(script, &[], &[]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        OxCacheError::InvalidInput(_) => {}
        other => panic!("Expected InvalidInput, got {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_eval_lua_too_many_keys_rejected() {
    let backend = make_backend().await;
    let keys: Vec<&str> = (0..200).map(|_| "k").collect();
    let result = backend.eval_lua("return 1", &keys, &[]).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires Redis server"]
async fn test_as_lua_executor_returns_some() {
    use crate::backend::CacheConnector;
    let backend = make_backend().await;
    let executor = backend.as_lua_executor();
    assert!(executor.is_some());
}
