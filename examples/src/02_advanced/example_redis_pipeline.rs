// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis Pipeline 批量操作示例
//!
//! 本示例演示使用 Redis Pipeline 进行高效的批量操作：
//! - set_many_pipeline: 批量设置键值对
//! - get_many_pipeline: 批量获取键值
//! - delete_many_pipeline: 批量删除键
//!
//! Pipeline 通过单次网络往返执行多个命令，大幅提升批量操作性能。
//!
//! 运行方式：
//! ```bash
//! # 本地开发需允许非 TLS 连接（生产请使用 rediss://）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --example example_redis_pipeline
//! ```

use oxcache::backend::{CacheWriter, RedisBackend};
use std::time::{Duration, Instant};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_REDIS_PIPELINE_1_BATCH_SET_SET, MSG_EXAMPLE_REDIS_PIPELINE_2_BATCH_GET_GET,
    MSG_EXAMPLE_REDIS_PIPELINE_3_PERFORMANCE_COMPARISON_ONE,
    MSG_EXAMPLE_REDIS_PIPELINE_4_BATCH_DELETE_DELETE,
    MSG_EXAMPLE_REDIS_PIPELINE_BATCH_DELETED_KEYS, MSG_EXAMPLE_REDIS_PIPELINE_BATCH_GET_KEYS_TOOK,
    MSG_EXAMPLE_REDIS_PIPELINE_BATCH_OPERATIONS, MSG_EXAMPLE_REDIS_PIPELINE_BATCH_SET_KEYS_TOOK,
    MSG_EXAMPLE_REDIS_PIPELINE_CONNECTING_TO_REDIS, MSG_EXAMPLE_REDIS_PIPELINE_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_REDIS_PIPELINE_PIPELINE_IS_X_FASTER, MSG_EXAMPLE_REDIS_PIPELINE_REDIS_CONNECTED,
    MSG_EXAMPLE_REDIS_PIPELINE_REMAINING_KEYS_SHOULD_BE, MSG_EXAMPLE_REDIS_PIPELINE_RESULTS,
    MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_ONE, MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_VIA,
    t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_REDIS_PIPELINE_BATCH_OPERATIONS, &[]));

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!(
        "{}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_CONNECTING_TO_REDIS,
            &[("redis_url", redis_url.to_string())]
        )
    );

    let backend = RedisBackend::new(&redis_url).await?;
    println!("{}\n", t(MSG_EXAMPLE_REDIS_PIPELINE_REDIS_CONNECTED, &[]));

    println!("{}", t(MSG_EXAMPLE_REDIS_PIPELINE_1_BATCH_SET_SET, &[]));
    // 先收集 String，再借用为 &str
    let owned_items: Vec<(String, Vec<u8>)> = (0..10)
        .map(|i| {
            (
                format!("pipeline:key:{}", i),
                format!("value_{}", i).into_bytes(),
            )
        })
        .collect();
    let items: Vec<(&str, Vec<u8>)> = owned_items
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();

    let start = Instant::now();
    backend
        .set_many_pipeline(&items, Some(Duration::from_secs(60)))
        .await?;
    let elapsed = start.elapsed();
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_BATCH_SET_KEYS_TOOK,
            &[
                ("items", items.len().to_string()),
                ("elapsed", format!("{:?}", elapsed))
            ]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_REDIS_PIPELINE_2_BATCH_GET_GET, &[]));
    let owned_keys: Vec<String> = (0..10).map(|i| format!("pipeline:key:{}", i)).collect();
    let keys: Vec<&str> = owned_keys.iter().map(|s| s.as_str()).collect();

    let start = Instant::now();
    let results = backend.get_many_pipeline(&keys).await?;
    let elapsed = start.elapsed();

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_BATCH_GET_KEYS_TOOK,
            &[
                ("keys", keys.len().to_string()),
                ("elapsed", format!("{:?}", elapsed))
            ]
        )
    );
    println!("  {}", t(MSG_EXAMPLE_REDIS_PIPELINE_RESULTS, &[]));
    for (i, result) in results.iter().enumerate() {
        match result {
            Some(data) => println!("    key[{}] = {:?}", i, String::from_utf8_lossy(data)),
            None => println!("    key[{}] = (nil)", i),
        }
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_REDIS_PIPELINE_3_PERFORMANCE_COMPARISON_ONE, &[])
    );

    // 逐个设置
    let start = Instant::now();
    for i in 0..10 {
        let key = format!("individual:key:{}", i);
        let value = format!("value_{}", i).into_bytes();
        backend.set(key.as_str().into(), value.into(), None).await?;
    }
    let individual_elapsed = start.elapsed();

    // Pipeline 设置
    let owned_perf: Vec<(String, Vec<u8>)> = (0..10)
        .map(|i| {
            (
                format!("pipeline:perf:{}", i),
                format!("value_{}", i).into_bytes(),
            )
        })
        .collect();
    let perf_items: Vec<(&str, Vec<u8>)> = owned_perf
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();

    let start = Instant::now();
    backend.set_many_pipeline(&perf_items, None).await?;
    let pipeline_elapsed = start.elapsed();

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_ONE,
            &[("individual_elapsed", format!("{:?}", individual_elapsed))]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_VIA,
            &[("pipeline_elapsed", format!("{:?}", pipeline_elapsed))]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_PIPELINE_IS_X_FASTER,
            &[(
                "max",
                format!(
                    "{:.1}",
                    individual_elapsed.as_secs_f64() / pipeline_elapsed.as_secs_f64().max(0.000001)
                )
            )]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_REDIS_PIPELINE_4_BATCH_DELETE_DELETE, &[])
    );
    let owned_perf_keys: Vec<String> = (0..10).map(|i| format!("pipeline:perf:{}", i)).collect();
    let owned_ind_keys: Vec<String> = (0..10).map(|i| format!("individual:key:{}", i)).collect();

    let mut all_owned: Vec<String> = owned_keys.clone();
    all_owned.extend(owned_perf_keys.iter().cloned());
    all_owned.extend(owned_ind_keys.iter().cloned());

    let all_keys: Vec<&str> = all_owned.iter().map(|s| s.as_str()).collect();

    backend.delete_many_pipeline(&all_keys).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_BATCH_DELETED_KEYS,
            &[("all_keys", all_keys.len().to_string())]
        )
    );

    // 验证删除
    let results = backend.get_many_pipeline(&all_keys).await?;
    let remaining = results.iter().filter(|r| r.is_some()).count();
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_PIPELINE_REMAINING_KEYS_SHOULD_BE,
            &[("remaining", remaining.to_string())]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_REDIS_PIPELINE_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
