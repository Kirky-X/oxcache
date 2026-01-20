//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! # Redis 原生操作示例
//!
//! 演示如何使用 Redis 原生操作（有序集合、Lua 脚本、批量操作等）。
//!
//! 需要启用 `l2-redis` 特性。

use oxcache::backend::l2::L2Backend;
use oxcache::client::redis_native::L2NativeOperations;
use oxcache::config::{L2Config, RedisMode};
use secrecy::SecretString;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!("=== Redis 原生操作示例 ===\n");

    // ===========================================================================
    // 初始化 Redis 连接
    // ===========================================================================
    println!("1. 初始化 Redis 连接");

    let config = L2Config {
        connection_string: SecretString::new("redis://127.0.0.1:6379".to_string()),
        mode: RedisMode::Standalone,
        default_ttl: Some(300),
        ..Default::default()
    };

    let backend = Arc::new(
        L2Backend::new(&config)
            .await
            .expect("Failed to connect to Redis"),
    );
    println!("   - 连接到 Redis: 127.0.0.1:6379");
    println!();

    // ===========================================================================
    // 2. 有序集合操作
    // ===========================================================================
    println!("2. 有序集合 (Sorted Set) 操作");

    let zset_key = "example:leaderboard";

    // 添加成员
    println!("   - 添加排行榜成员:");
    let members = [
        (100.0, "alice"),
        (85.0, "bob"),
        (95.0, "charlie"),
        (70.0, "david"),
        (90.0, "eve"),
    ];

    for (score, member) in &members {
        backend.zadd(zset_key, *score, member).await.expect("zadd failed");
        println!("     - {}: {} -> score {}", zset_key, member, score);
    }

    // 获取成员数量
    let count = backend.zcard(zset_key).await.expect("zcard failed");
    println!("   - 成员数量: {}", count);

    // 获取分数范围内的成员
    println!("   - 分数 80-100 的成员:");
    let top_members = backend.zrange_by_score(zset_key, 80.0, 100.0).await.expect("zrange failed");
    for member in &top_members {
        let score = backend.zscore(zset_key, member).await.expect("zscore failed");
        println!("     - {}: score={:.1}", member, score.unwrap_or(0.0));
    }

    // 获取特定成员的分数
    let alice_score = backend.zscore(zset_key, "alice").await.expect("zscore failed");
    println!("   - alice 的分数: {:?}", alice_score);

    // 移除成员
    let removed = backend.zrem(zset_key, "david").await.expect("zrem failed");
    println!("   - 移除 david: {}", removed);
    println!();

    // ===========================================================================
    // 3. Lua 脚本执行
    // ===========================================================================
    println!("3. Lua 脚本执行");

    let counter_key = "example:counter";

    // 加载 Lua 脚本
    let increment_script = r#"
        local key = KEYS[1]
        local increment = tonumber(ARGV[1])
        local current = redis.call('GET', key) or 0
        local new_value = current + increment
        redis.call('SET', key, new_value)
        return new_value
    "#;

    let script_sha = backend.script_load(increment_script).await.expect("script_load failed");
    println!("   - 脚本 SHA: {}", &script_sha[..16]); // 只显示前 16 个字符

    // 初始值
    backend.set(counter_key, b"0", None).await.expect("set failed");

    // 使用 SHA 执行脚本
    for i in 1..=5 {
        let result = backend.evalsha(&script_sha, &[counter_key], &[&i.to_string()]).await;
        match result {
            Ok(val) => println!("   - 第 {} 次 increment: {}", i, val),
            Err(e) => println!("   - 第 {} 次 increment 失败: {}", i, e),
        }
    }

    // 直接执行 EVAL（无需预编译）
    let eval_result = backend.eval("return redis.call('GET', KEYS[1])", &[counter_key], &[])
        .await
        .expect("eval failed");
    println!("   - 最终计数器值: {}", eval_result);
    println!();

    // ===========================================================================
    // 4. 批量操作
    // ===========================================================================
    println!("4. 批量操作");

    let batch_prefix = "example:batch";

    // 批量设置
    println!("   - 批量设置 10 个键值对:");
    let mut kvs = Vec::new();
    for i in 1..=10 {
        kvs.push((
            format!("{}:key{}", batch_prefix, i),
            format!("batch_value_{}", i),
        ));
    }
    backend.set_many(&kvs, Some(300)).await.expect("set_many failed");
    println!("     - 完成设置 {} 个键", kvs.len());

    // 批量获取
    let keys: Vec<String> = kvs.iter().map(|(k, _)| k.clone()).collect();
    let values = backend.get_many(&keys).await.expect("get_many failed");
    println!("   - 批量获取结果:");
    for (i, value) in values.iter().enumerate() {
        match value {
            Some(v) => println!("     - {}: {}", keys[i], String::from_utf8_lossy(v)),
            None => println!("     - {}: (不存在)", keys[i]),
        }
    }

    // 按模式删除
    let deleted = backend.del_pattern(&format!("{}:*", batch_prefix)).await.expect("del_pattern failed");
    println!("   - 按模式删除 {} 个键", deleted);
    println!();

    // ===========================================================================
    // 5. 键扫描
    // ===========================================================================
    println!("5. 键扫描操作");

    // 先创建一些测试键
    for i in 1..=20 {
        let key = format!("example:scan:user:{}", i);
        backend.set(&key, b"test", Some(60)).await.expect("set failed");
    }

    // 扫描键
    println!("   - 扫描 example:scan:user:* 键:");
    let (keys, cursor) = backend.scan_keys("example:scan:user:*", 10).await.expect("scan_keys failed");
    println!("     - 第一批获取 {} 个键, cursor={}", keys.len(), cursor);

    for key in &keys[..keys.len().min(5)] {
        println!("       - {}", key);
    }
    if keys.len() > 5 {
        println!("       - ... 还有 {} 个键", keys.len() - 5);
    }

    // 清理测试键
    backend.del_pattern("example:scan:*").await.expect("del_pattern failed");
    println!();

    // ===========================================================================
    // 6. 计数器操作
    // ===========================================================================
    println!("6. 计数器操作");

    let counter_key = "example:page_views";

    // INCR BY
    let result = backend.incr_by(counter_key, 100).await.expect("incr_by failed");
    println!("   - incr_by {}: {}", counter_key, result);

    let result = backend.incr_by(counter_key, 50).await.expect("incr_by failed");
    println!("   - incr_by {}: {}", counter_key, result);

    // DECR BY
    let result = backend.decr_by(counter_key, 30).await.expect("decr_by failed");
    println!("   - decr_by {}: {}", counter_key, result);

    // GET COUNTER
    let count = backend.get_counter(counter_key).await.expect("get_counter failed");
    println!("   - get_counter {}: {}", counter_key, count);

    // 清理
    backend.del_pattern("example:counter:*").await.expect("del_pattern failed");
    backend.del_pattern("example:leaderboard").await.expect("del_pattern failed");
    backend.del_pattern(counter_key).await.expect("del_pattern failed");
    println!();

    println!("=== Redis 原生操作示例完成 ===");
}
