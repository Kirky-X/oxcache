// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 同步 API 示例：CacheBuilder::sync_mode(true) 启用后可调 get_sync / set_sync /
// delete_sync / exists_sync / set_with_ttl_sync（per-entry TTL）/ get_or_sync（单飞）。
// 注意：sync API 仅在 multi_thread runtime 或 runtime 外可用；在
// current-thread runtime 的异步上下文内调用会显性返回
// Err(NotSupported)（不 panic）。需要运行时无关的同步消费请改用原生同步
// 后端 sync_backend_arc(DashMapMemoryBackend)。

use oxcache::Cache;
use serde::{Deserialize, Serialize};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_SYNC_API_BASIC_SYNC_OPERATIONS, MSG_EXAMPLE_SYNC_API_CACHE_CLEARED,
    MSG_EXAMPLE_SYNC_API_CACHE_METADATA_SYNC, MSG_EXAMPLE_SYNC_API_DELETE_USER_1,
    MSG_EXAMPLE_SYNC_API_ENTRIES_AFTER_CLEAR, MSG_EXAMPLE_SYNC_API_ENTRIES_LEN_SYNC,
    MSG_EXAMPLE_SYNC_API_EXAMPLE_COMPLETE, MSG_EXAMPLE_SYNC_API_FIRST_GET_OR_SYNC,
    MSG_EXAMPLE_SYNC_API_GET_OR_SYNC_SINGLE, MSG_EXAMPLE_SYNC_API_GET_TEMP,
    MSG_EXAMPLE_SYNC_API_GET_USER_1, MSG_EXAMPLE_SYNC_API_MIXING_SYNC_AND_ASYNC,
    MSG_EXAMPLE_SYNC_API_SECOND_GET_OR_SYNC, MSG_EXAMPLE_SYNC_API_SET_TEMP_60S_TTL,
    MSG_EXAMPLE_SYNC_API_SET_USER_1, t,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建启用了 sync API 的缓存（默认 Moka 后端）
    let cache: Cache<String, User> = Cache::builder().sync_mode(true).build().await?;

    // 基本 sync 操作
    println!("{}", t(MSG_EXAMPLE_SYNC_API_BASIC_SYNC_OPERATIONS, &[]));
    let alice = User {
        id: 1,
        name: "Alice".to_string(),
    };
    cache.set_sync(&"user:1".to_string(), &alice)?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_SET_USER_1,
            &[("alice", format!("{:?}", alice))]
        )
    );

    let cached = cache.get_sync(&"user:1".to_string())?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_GET_USER_1,
            &[("cached", format!("{:?}", cached))]
        )
    );
    assert_eq!(cached, Some(alice));

    println!(
        "exists user:1 = {}",
        cache.exists_sync(&"user:1".to_string())?
    );

    cache.delete_sync(&"user:1".to_string())?;
    println!("{}", t(MSG_EXAMPLE_SYNC_API_DELETE_USER_1, &[]));
    assert_eq!(cache.get_sync(&"user:1".to_string())?, None);

    // sync + TTL
    println!("\n=== sync + TTL ===");
    cache.set_with_ttl_sync(
        &"temp".to_string(),
        &User {
            id: 2,
            name: "Temp".to_string(),
        },
        Some(std::time::Duration::from_secs(60)),
    )?;
    println!("{}", t(MSG_EXAMPLE_SYNC_API_SET_TEMP_60S_TTL, &[]));
    let cached = cache.get_sync(&"temp".to_string())?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_GET_TEMP,
            &[("cached", format!("{:?}", cached))]
        )
    );

    // get_or_sync（单飞）
    println!("\n{}", t(MSG_EXAMPLE_SYNC_API_GET_OR_SYNC_SINGLE, &[]));
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
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_FIRST_GET_OR_SYNC,
            &[("user", format!("{:?}", user))]
        )
    );

    // 第二次调用：cache hit，不触发 fallback
    let user = cache.get_or_sync(&"user:42".to_string(), || {
        Ok(User {
            id: 42,
            name: "Should not be called".to_string(),
        })
    })?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_SECOND_GET_OR_SYNC,
            &[("user", format!("{:?}", user))]
        )
    );

    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // sync 与 async 混用
    println!("\n{}", t(MSG_EXAMPLE_SYNC_API_MIXING_SYNC_AND_ASYNC, &[]));
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

    // 缓存元信息 sync
    println!("\n{}", t(MSG_EXAMPLE_SYNC_API_CACHE_METADATA_SYNC, &[]));
    let len = cache.len_sync()?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_ENTRIES_LEN_SYNC,
            &[("len", len.to_string())]
        )
    );

    // clear_sync
    println!("\n=== clear_sync ===");
    cache.clear_sync()?;
    println!("{}", t(MSG_EXAMPLE_SYNC_API_CACHE_CLEARED, &[]));
    let len_after = cache.len_sync()?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_SYNC_API_ENTRIES_AFTER_CLEAR,
            &[("len_after", len_after.to_string())]
        )
    );
    assert_eq!(len_after, 0);

    println!("\n{}", t(MSG_EXAMPLE_SYNC_API_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
