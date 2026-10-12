// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存预热示例
//!
//! 本示例演示了 Oxcache 的缓存预热功能：
//! - 应用启动时预热缓存
//! - 预加载热点数据
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_warmup
//! ```

use oxcache::Cache;
use std::sync::Arc;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_WARMUP_1_SIMULATING_APPLICATION_CONFIG, MSG_EXAMPLE_WARMUP_2_SIMULATING_HOT_USER,
    MSG_EXAMPLE_WARMUP_3_VERIFYING_THE_WARMED, MSG_EXAMPLE_WARMUP_4_SIMULATING_A_CACHE,
    MSG_EXAMPLE_WARMUP_5_POST_WARMUP_STATISTICS, MSG_EXAMPLE_WARMUP_CACHE_REBUILD_COMPLETE_TOOK,
    MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE, MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_WARMUP_CLEARING_THE_CACHE, MSG_EXAMPLE_WARMUP_CONFIG_VERIFICATION,
    MSG_EXAMPLE_WARMUP_CONFIG_WARMUP_COMPLETE_ITEMS, MSG_EXAMPLE_WARMUP_EXPECTED,
    MSG_EXAMPLE_WARMUP_HITS, MSG_EXAMPLE_WARMUP_HOT_USER_WARMUP_COMPLETE,
    MSG_EXAMPLE_WARMUP_LOADING_CONFIG, MSG_EXAMPLE_WARMUP_LOADING_CONFIG_FROM_THE,
    MSG_EXAMPLE_WARMUP_NOT_FOUND, MSG_EXAMPLE_WARMUP_NOT_FOUND_2,
    MSG_EXAMPLE_WARMUP_PRELOADING_HOT_USERS, MSG_EXAMPLE_WARMUP_REWARMING,
    MSG_EXAMPLE_WARMUP_TOTAL_ENTRIES, MSG_EXAMPLE_WARMUP_USER_VERIFICATION, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}\n", t(MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE, &[]));

    let cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await?);

    println!(
        "{}",
        t(MSG_EXAMPLE_WARMUP_1_SIMULATING_APPLICATION_CONFIG, &[])
    );
    let configs = vec![
        ("app:theme", "dark", "app theme"),
        ("app:language", "zh-CN", "default language"),
        ("app:timezone", "Asia/Shanghai", "time zone"),
        ("app:max_connections", "100", "max connections"),
        ("app:session_timeout", "3600", "session timeout"),
    ];

    println!("  {}", t(MSG_EXAMPLE_WARMUP_LOADING_CONFIG_FROM_THE, &[]));
    for (key, value, desc) in &configs {
        let k = key.to_string();
        let v = value.to_string();
        cache.set(&k, &v).await?;
        println!(
            "    {}",
            t(
                MSG_EXAMPLE_WARMUP_LOADING_CONFIG,
                &[
                    ("k", k.to_string()),
                    ("v", v.to_string()),
                    ("desc", desc.to_string())
                ]
            )
        );
    }
    println!(
        "   {}\n",
        t(
            MSG_EXAMPLE_WARMUP_CONFIG_WARMUP_COMPLETE_ITEMS,
            &[("configs", configs.len().to_string())]
        )
    );

    println!("{}", t(MSG_EXAMPLE_WARMUP_2_SIMULATING_HOT_USER, &[]));
    let hot_users = vec![1, 2, 3, 4, 5, 10, 100, 101];

    println!("  {}", t(MSG_EXAMPLE_WARMUP_PRELOADING_HOT_USERS, &[]));
    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for user_id in &hot_users {
        let cache = cache.clone();
        let id = *user_id;
        let handle = tokio::spawn(async move {
            // 模拟从数据库查询用户
            let username = format!("user_{}", id);
            let role = if id == 1 { "admin" } else { "user" };
            cache
                .set(&format!("user:{}", id), &format!("{}:{}", username, role))
                .await?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!(
        "   {}\n",
        t(
            MSG_EXAMPLE_WARMUP_HOT_USER_WARMUP_COMPLETE,
            &[
                ("hot_users", hot_users.len().to_string()),
                ("elapsed", format!("{:?}", elapsed))
            ]
        )
    );

    println!("{}", t(MSG_EXAMPLE_WARMUP_3_VERIFYING_THE_WARMED, &[]));
    println!("  {}", t(MSG_EXAMPLE_WARMUP_CONFIG_VERIFICATION, &[]));
    for (key, value, _) in &configs {
        let k = key.to_string();
        let v = value.to_string();
        let retrieved = cache.get(&k).await?;
        match retrieved {
            Some(val) if val == v => println!("     ✓ {} = {}", k, val),
            Some(val) => println!(
                "     {}",
                t(
                    MSG_EXAMPLE_WARMUP_EXPECTED,
                    &[
                        ("k", k.to_string()),
                        ("val", val.to_string()),
                        ("v", v.to_string())
                    ]
                )
            ),
            None => println!(
                "     {}",
                t(MSG_EXAMPLE_WARMUP_NOT_FOUND, &[("k", k.to_string())])
            ),
        }
    }

    println!("   \n  {}", t(MSG_EXAMPLE_WARMUP_USER_VERIFICATION, &[]));
    for user_id in &hot_users {
        let key = format!("user:{}", user_id);
        let retrieved = cache.get(&key).await?;
        match retrieved {
            // CodeQL cleartext-logging：命中展示只报长度，不回显缓存明文
            Some(v) => println!("     ✓ {} = <{} chars>", key, v.len()),
            None => println!(
                "     {}",
                t(MSG_EXAMPLE_WARMUP_NOT_FOUND_2, &[("key", key.to_string())])
            ),
        }
    }
    println!();

    println!("{}", t(MSG_EXAMPLE_WARMUP_4_SIMULATING_A_CACHE, &[]));
    println!("  {}", t(MSG_EXAMPLE_WARMUP_CLEARING_THE_CACHE, &[]));
    cache.clear().await?;

    println!("  {}", t(MSG_EXAMPLE_WARMUP_REWARMING, &[]));
    let start = std::time::Instant::now();

    // 并发重新加载所有配置
    let mut handles = Vec::new();
    for (key, value, _) in &configs {
        let cache = cache.clone();
        let k = key.to_string();
        let v = value.to_string();
        let handle = tokio::spawn(async move {
            cache.set(&k, &v).await?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!(
        "   {}",
        t(
            MSG_EXAMPLE_WARMUP_CACHE_REBUILD_COMPLETE_TOOK,
            &[("elapsed", format!("{:?}", elapsed))]
        )
    );
    println!();

    println!("{}", t(MSG_EXAMPLE_WARMUP_5_POST_WARMUP_STATISTICS, &[]));
    let stats = cache.stats().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_WARMUP_TOTAL_ENTRIES,
            &[(
                "stats",
                stats
                    .get("item_count")
                    .map(String::as_str)
                    .unwrap_or("0")
                    .to_string()
            )]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_WARMUP_HITS,
            &[(
                "stats",
                stats
                    .get("hit_count")
                    .map(String::as_str)
                    .unwrap_or("0")
                    .to_string()
            )]
        )
    );
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE_COMPLETE, &[])
    );
    Ok(())
}
