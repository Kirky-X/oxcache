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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_LUA_SCRIPT_1_ATOMIC_COUNTER_EVAL,
    MSG_EXAMPLE_LUA_SCRIPT_2_PRELOADING_SCRIPTS_SCRIPT,
    MSG_EXAMPLE_LUA_SCRIPT_3_EXECUTING_SCRIPTS_BY,
    MSG_EXAMPLE_LUA_SCRIPT_4_CONDITIONAL_UPDATES_COMPARE,
    MSG_EXAMPLE_LUA_SCRIPT_CALL_COUNTER_VALUE, MSG_EXAMPLE_LUA_SCRIPT_CONNECTING_TO_REDIS,
    MSG_EXAMPLE_LUA_SCRIPT_EVAL_LUA_RESULT, MSG_EXAMPLE_LUA_SCRIPT_EVAL_SHA_RESULT,
    MSG_EXAMPLE_LUA_SCRIPT_EXAMPLE_COMPLETE_TEST_DATA, MSG_EXAMPLE_LUA_SCRIPT_EXECUTION_EXAMPLE,
    MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V1_UPDATING_TO,
    MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V2_UPDATING_TO, MSG_EXAMPLE_LUA_SCRIPT_INITIAL_VALUE_V1,
    MSG_EXAMPLE_LUA_SCRIPT_REDIS_CONNECTED, MSG_EXAMPLE_LUA_SCRIPT_SCRIPT_SHA1, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_LUA_SCRIPT_EXECUTION_EXAMPLE, &[]));

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!(
        "{}",
        t(
            MSG_EXAMPLE_LUA_SCRIPT_CONNECTING_TO_REDIS,
            &[("redis_url", redis_url.to_string())]
        )
    );

    let backend = RedisBackend::new(&redis_url).await?;
    println!("{}\n", t(MSG_EXAMPLE_LUA_SCRIPT_REDIS_CONNECTED, &[]));

    println!("{}", t(MSG_EXAMPLE_LUA_SCRIPT_1_ATOMIC_COUNTER_EVAL, &[]));
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
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_LUA_SCRIPT_CALL_COUNTER_VALUE,
                &[("i", i.to_string()), ("result", format!("{:?}", result))]
            )
        );
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_LUA_SCRIPT_2_PRELOADING_SCRIPTS_SCRIPT, &[])
    );
    let hash_script = r#"
        local value = redis.call('GET', KEYS[1])
        if value == false then
            return nil
        end
        return string.len(value)
    "#;

    let sha = backend.script_load(hash_script).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_LUA_SCRIPT_SCRIPT_SHA1,
            &[("sha", sha.to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_LUA_SCRIPT_3_EXECUTING_SCRIPTS_BY, &[])
    );

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
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_LUA_SCRIPT_EVAL_LUA_RESULT,
            &[("result", format!("{:?}", result))]
        )
    );

    let result = backend.eval_sha(&sha, &["test:lua:length"], &[]).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_LUA_SCRIPT_EVAL_SHA_RESULT,
            &[("result", format!("{:?}", result))]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_LUA_SCRIPT_4_CONDITIONAL_UPDATES_COMPARE, &[])
    );
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

    backend
        .set("cas:key".into(), b"v1".to_vec().into(), None)
        .await?;
    println!("  {}", t(MSG_EXAMPLE_LUA_SCRIPT_INITIAL_VALUE_V1, &[]));

    let result = backend
        .eval_lua(cas_script, &["cas:key"], &["v2", "v3"])
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V2_UPDATING_TO,
            &[("result", format!("{:?}", result))]
        )
    );

    // 用正确的期望值更新
    let result = backend
        .eval_lua(cas_script, &["cas:key"], &["v1", "v2"])
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V1_UPDATING_TO,
            &[("result", format!("{:?}", result))]
        )
    );

    backend
        .delete_many_pipeline(&["counter:1", "test:lua:length", "cas:key"])
        .await?;
    println!(
        "\n{}",
        t(MSG_EXAMPLE_LUA_SCRIPT_EXAMPLE_COMPLETE_TEST_DATA, &[])
    );

    Ok(())
}
