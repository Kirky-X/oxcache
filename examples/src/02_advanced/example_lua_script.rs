// Copyright (c) 2025-2026 Kirky.X
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
//! cd examples && cargo run --example example_lua_script
//! ```

use oxcache::backend::RedisBackend;
use oxcache::backend::{CacheWriter, LuaExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Lua 脚本执行示例 ===\n");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("连接 Redis: {}", redis_url);

    let backend = RedisBackend::new(&redis_url).await?;
    println!("✓ Redis 连接成功\n");

    // 1. 使用 eval_lua 执行原子计数器
    println!("--- 1. 原子计数器（eval_lua） ---");
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
        let result = backend.eval_lua(counter_script, &["counter:1"], &[]).await?;
        println!("  第 {} 次调用，计数器值: {:?}", i, result);
    }

    // 2. 使用 script_load 预加载脚本
    println!("\n--- 2. 预加载脚本（script_load） ---");
    let hash_script = r#"
        local value = redis.call('GET', KEYS[1])
        if value == false then
            return nil
        end
        return string.len(value)
    "#;

    let sha = backend.script_load(hash_script).await?;
    println!("  脚本 SHA1: {}", sha);

    // 3. 使用 eval_sha 执行已加载的脚本
    println!("\n--- 3. 通过 SHA 执行脚本（eval_sha） ---");

    // 先设置一个值
    backend.set("test:lua:length", b"hello world".to_vec(), None).await?;

    let result = backend.eval_lua(hash_script, &["test:lua:length"], &[]).await?;
    println!("  eval_lua 结果: {:?}", result);

    let result = backend.eval_sha(&sha, &["test:lua:length"], &[]).await?;
    println!("  eval_sha 结果: {:?}", result);

    // 4. 条件更新示例（CAS 操作）
    println!("\n--- 4. 条件更新（Compare-And-Swap） ---");
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
    backend.set("cas:key", b"v1".to_vec(), None).await?;
    println!("  初始值: v1");

    // 尝试用错误的期望值更新
    let result = backend.eval_lua(cas_script, &["cas:key"], &["v2", "v3"]).await?;
    println!("  期望 v2 更新为 v3: {:?} (应为 0，表示失败)", result);

    // 用正确的期望值更新
    let result = backend.eval_lua(cas_script, &["cas:key"], &["v1", "v2"]).await?;
    println!("  期望 v1 更新为 v2: {:?} (应为 1，表示成功)", result);

    // 5. 清理
    backend
        .delete_many_pipeline(&["counter:1", "test:lua:length", "cas:key"])
        .await?;
    println!("\n✓ 示例完成，已清理测试数据");

    Ok(())
}
