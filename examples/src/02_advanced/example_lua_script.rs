// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Lua 脚本执行示例
//!
//! 本示例演示 oxcache 的 Lua 脚本执行功能：
//! - 使用 eval_lua() 执行原子操作
//! - 使用 script_load() 预加载脚本
//! - 使用 eval_sha() 通过 SHA1 哈希执行脚本
//!
//! 运行方式：
//! ```bash
//! # 本地开发需允许非 TLS 连接（生产请使用 rediss://）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --example example_lua_script
//! ```

use oxcache::backend::RedisBackend;
use oxcache::backend::{CacheWriter, LuaExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Lua script execution example ===\n");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("Connecting to Redis: {}", redis_url);

    let backend = RedisBackend::new(&redis_url).await?;
    println!("✓ Redis connected\n");

    println!("--- 1. Atomic counter (eval_lua) ---");
    let counter_script = r#"
        local current = redis.call('GET', KEYS[1])
        if current == false then
            current = 0
        else
            current = tonumber(current)
        end
        current = current + 1
        redis.call('SET', KEYS[1], current)
        return current
    "#;

    for i in 1..=3 {
        let result = backend
            .eval_lua(counter_script, &["counter:1"], &[])
            .await?;
        println!("  call {}: counter value: {:?}", i, result);
    }

    println!("\n--- 2. Preloading scripts (script_load) ---");
    let hash_script = r#"
        local value = redis.call('GET', KEYS[1])
        if value == false then
            return nil
        end
        return string.len(value)
    "#;

    let sha = backend.script_load(hash_script).await?;
    println!("  script SHA1: {}", sha);

    println!("\n--- 3. Executing scripts by SHA (eval_sha) ---");

    // 先设置一个值
    backend
        .set(
            "test:lua:length".into(),
            b"hello world".to_vec().into(),
            None,
        )
        .await?;

    let result = backend
        .eval_lua(hash_script, &["test:lua:length"], &[])
        .await?;
    println!("  eval_lua result: {:?}", result);

    let result = backend.eval_sha(&sha, &["test:lua:length"], &[]).await?;
    println!("  eval_sha result: {:?}", result);

    println!("\n--- 4. Conditional updates (compare-and-swap) ---");
    let cas_script = r#"
        local current = redis.call('GET', KEYS[1])
        if current == false then
            return -1
        end
        if current == ARGV[1] then
            redis.call('SET', KEYS[1], ARGV[2])
            return 1
        end
        return 0
    "#;

    // 设置初始值
    backend
        .set("cas:key".into(), b"v1".to_vec().into(), None)
        .await?;
    println!("  Initial value: v1");

    // 尝试用错误的期望值更新
    let result = backend
        .eval_lua(cas_script, &["cas:key"], &["v2", "v3"])
        .await?;
    println!(
        "  expecting v2, updating to v3: {:?} (should be 0, meaning failure)",
        result
    );

    // 用正确的期望值更新
    let result = backend
        .eval_lua(cas_script, &["cas:key"], &["v1", "v2"])
        .await?;
    println!(
        "  expecting v1, updating to v2: {:?} (should be 1, meaning success)",
        result
    );

    backend
        .delete_many_pipeline(&["counter:1", "test:lua:length", "cas:key"])
        .await?;
    println!("\n✓ Example complete, test data cleaned up");

    Ok(())
}
