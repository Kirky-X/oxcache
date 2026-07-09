// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 同步 API 示例
//
// 本示例演示 oxcache 的同步 API（sync_mode）：
// - 使用 CacheBuilder::sync_mode(true) 启用 sync API
// - get_sync / set_sync / delete_sync / exists_sync
// - set_with_ttl_sync 设置 per-entry TTL
// - get_or_sync 单飞（single-flight）避免重复计算
//
// 注意：sync API 仅在 multi_thread runtime 或无 runtime 上下文下可用。
// current-thread runtime 会导致 Moka 的 sync_block_on panic。

use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建启用了 sync API 的缓存（默认 Moka 后端）
    let cache: Cache<String, User> = Cache::builder().sync_mode(true).build().await?;

    // === 基本 sync 操作 ===
    println!("=== 基本 sync 操作 ===");
    let alice = User {
        id: 1,
        name: "Alice".to_string(),
    };
    cache.set_sync(&"user:1".to_string(), &alice)?;
    println!("设置 user:1 = {:?}", alice);

    let cached = cache.get_sync(&"user:1".to_string())?;
    println!("获取 user:1 = {:?}", cached);
    assert_eq!(cached, Some(alice));

    println!("exists user:1 = {}", cache.exists_sync(&"user:1".to_string())?);

    cache.delete_sync(&"user:1".to_string())?;
    println!("删除 user:1");
    assert_eq!(cache.get_sync(&"user:1".to_string())?, None);

    // === sync + TTL ===
    println!("\n=== sync + TTL ===");
    cache.set_with_ttl_sync(
        &"temp".to_string(),
        &User {
            id: 2,
            name: "Temp".to_string(),
        },
        Some(std::time::Duration::from_secs(60)),
    )?;
    println!("设置 temp（60s TTL）");
    let cached = cache.get_sync(&"temp".to_string())?;
    println!("获取 temp = {:?}", cached);

    // === get_or_sync（单飞） ===
    println!("\n=== get_or_sync（单飞） ===");
    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let count_for_fallback = call_count.clone();

    // 第一次调用：cache miss，触发 fallback
    let user = cache.get_or_sync(&"user:42".to_string(), || {
        count_for_fallback.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(User {
            id: 42,
            name: "Bob".to_string(),
        })
    })?;
    println!("第一次 get_or_sync = {:?}（fallback 被调用）", user);

    // 第二次调用：cache hit，不触发 fallback
    let user = cache.get_or_sync(&"user:42".to_string(), || {
        Ok(User {
            id: 42,
            name: "Should not be called".to_string(),
        })
    })?;
    println!("第二次 get_or_sync = {:?}（cache hit）", user);

    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // === sync 与 async 混用 ===
    println!("\n=== sync 与 async 混用 ===");
    cache
        .set(
            &"async_key".to_string(),
            &User {
                id: 99,
                name: "Async".to_string(),
            },
        )
        .await?;
    let sync_value = cache.get_sync(&"async_key".to_string())?;
    println!("async set → sync get = {:?}", sync_value);

    println!("\n同步 API 示例完成！");
    Ok(())
}
