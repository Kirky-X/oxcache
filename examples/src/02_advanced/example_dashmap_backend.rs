// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! DashMap 后端示例
//!
//! 本示例演示 DashMapMemoryBackend 的使用，
//! 并对比 Moka 和 DashMap 后端的差异。

use oxcache::backend::{CacheReader, CacheWriter};
use oxcache::backend::{DashMapMemoryBackend, MokaMemoryBackend};
use std::time::Duration;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_DASHMAP_BACKEND_1_CREATE_A_DASHMAP, MSG_EXAMPLE_DASHMAP_BACKEND_2_BASIC_OPERATIONS,
    MSG_EXAMPLE_DASHMAP_BACKEND_3_BATCH_OPERATIONS,
    MSG_EXAMPLE_DASHMAP_BACKEND_4_COMPARING_MOKA_AND,
    MSG_EXAMPLE_DASHMAP_BACKEND_5_DELETE_OPERATIONS,
    MSG_EXAMPLE_DASHMAP_BACKEND_6_CLEAR_OPERATIONS,
    MSG_EXAMPLE_DASHMAP_BACKEND_7_FEATURE_COMPARISON, MSG_EXAMPLE_DASHMAP_BACKEND_AFTER_CLEAR_KEYS,
    MSG_EXAMPLE_DASHMAP_BACKEND_AUTOMATIC_EXPIRY, MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_READ_RESULTS,
    MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_WRITE_KEY_VALUE,
    MSG_EXAMPLE_DASHMAP_BACKEND_BEFORE_CLEAR_KEYS, MSG_EXAMPLE_DASHMAP_BACKEND_CAPACITY_LIMITS,
    MSG_EXAMPLE_DASHMAP_BACKEND_CREATED, MSG_EXAMPLE_DASHMAP_BACKEND_DELETE_KEY1,
    MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE, MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_CHECK_KEY1, MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_KEY1,
    MSG_EXAMPLE_DASHMAP_BACKEND_KEYS, MSG_EXAMPLE_DASHMAP_BACKEND_LRU_TINYLFU_EVICTION_POLICIES,
    MSG_EXAMPLE_DASHMAP_BACKEND_MANUAL_TTL_MANAGEMENT,
    MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_KEYS, MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_TRAITS,
    MSG_EXAMPLE_DASHMAP_BACKEND_NO_EVICTION_POLICY,
    MSG_EXAMPLE_DASHMAP_BACKEND_PLAIN_CONCURRENT_HASHMAP, MSG_EXAMPLE_DASHMAP_BACKEND_READ_KEY1,
    MSG_EXAMPLE_DASHMAP_BACKEND_SUITABLE_FOR_PRODUCTION,
    MSG_EXAMPLE_DASHMAP_BACKEND_SUITED_TO_SIMPLE_SCENARIOS, MSG_EXAMPLE_DASHMAP_BACKEND_TRAITS,
    MSG_EXAMPLE_DASHMAP_BACKEND_WRITE_KEY1_VALUE1, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE, &[]));

    println!("{}", t(MSG_EXAMPLE_DASHMAP_BACKEND_1_CREATE_A_DASHMAP, &[]));
    let dashmap = DashMapMemoryBackend::new();
    println!("  {}", t(MSG_EXAMPLE_DASHMAP_BACKEND_CREATED, &[]));

    println!(
        "\n{}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_2_BASIC_OPERATIONS, &[])
    );

    dashmap
        .set(
            "key1".into(),
            b"value1".to_vec().into(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_WRITE_KEY1_VALUE1, &[])
    );

    let value = dashmap.get("key1").await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_READ_KEY1,
            &[(
                "value",
                format!(
                    "{:?}",
                    value.map(|v| String::from_utf8_lossy(&v).to_string())
                )
            )]
        )
    );

    let exists = dashmap.exists("key1").await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_KEY1,
            &[("exists", exists.to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_3_BATCH_OPERATIONS, &[])
    );

    let keys_values: Vec<(&str, Vec<u8>)> = vec![
        ("item:1", b"apple".to_vec()),
        ("item:2", b"banana".to_vec()),
        ("item:3", b"cherry".to_vec()),
    ];

    for (key, value) in &keys_values {
        dashmap
            .set(
                (*key).into(),
                value.clone().into(),
                Some(Duration::from_secs(300)),
            )
            .await?;
    }
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_WRITE_KEY_VALUE,
            &[("keys_values", keys_values.len().to_string())]
        )
    );

    let keys: Vec<String> = vec![
        "item:1".to_string(),
        "item:2".to_string(),
        "item:3".to_string(),
    ];
    let results = dashmap.get_many(&keys).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_READ_RESULTS,
            &[("results", results.len().to_string())]
        )
    );
    for (key, value) in keys.iter().zip(results.iter()) {
        println!(
            "    {} = {:?}",
            key,
            value
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string())
        );
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_4_COMPARING_MOKA_AND, &[])
    );

    let moka = MokaMemoryBackend::builder().capacity(1000).build();
    let dashmap2 = DashMapMemoryBackend::new();

    // 写入相同数据
    for i in 0..10 {
        let key = format!("key:{}", i);
        let value = format!("value:{}", i).into_bytes();
        moka.set(key.as_str().into(), value.clone().into(), None)
            .await?;
        dashmap2
            .set(key.as_str().into(), value.into(), None)
            .await?;
    }

    let moka_count = moka.len().await?;
    let dashmap_count = dashmap2.len().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_KEYS,
            &[("moka_count", moka_count.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_KEYS,
            &[("dashmap_count", dashmap_count.to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_5_DELETE_OPERATIONS, &[])
    );

    dashmap.delete("key1").await?;
    println!("  {}", t(MSG_EXAMPLE_DASHMAP_BACKEND_DELETE_KEY1, &[]));

    let exists_after = dashmap.exists("key1").await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_CHECK_KEY1,
            &[("exists_after", exists_after.to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_6_CLEAR_OPERATIONS, &[])
    );

    let count_before = dashmap.len().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_BEFORE_CLEAR_KEYS,
            &[("count_before", count_before.to_string())]
        )
    );

    dashmap.clear().await?;
    let count_after = dashmap.len().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_AFTER_CLEAR_KEYS,
            &[("count_after", count_after.to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_7_FEATURE_COMPARISON, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_TRAITS, &[])
    );
    println!(
        "    {}",
        t(
            MSG_EXAMPLE_DASHMAP_BACKEND_LRU_TINYLFU_EVICTION_POLICIES,
            &[]
        )
    );
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_CAPACITY_LIMITS, &[])
    );
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_AUTOMATIC_EXPIRY, &[])
    );
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_SUITABLE_FOR_PRODUCTION, &[])
    );
    println!();
    println!("  {}", t(MSG_EXAMPLE_DASHMAP_BACKEND_TRAITS, &[]));
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_PLAIN_CONCURRENT_HASHMAP, &[])
    );
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_NO_EVICTION_POLICY, &[])
    );
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_MANUAL_TTL_MANAGEMENT, &[])
    );
    println!(
        "    {}",
        t(MSG_EXAMPLE_DASHMAP_BACKEND_SUITED_TO_SIMPLE_SCENARIOS, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
