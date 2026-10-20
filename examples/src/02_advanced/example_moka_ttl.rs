// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Moka per-entry TTL 示例
//
// 本示例演示 oxcache 的 Moka per-entry TTL 功能：
// - set(key, value, Some(ttl))：per-entry TTL 真实生效（通过 moka::Expiry trait）
// - ttl(key)：只上报 per-entry TTL（未设置时返回 None，即使该条目按全局 TTL 过期）
// - expire(key, new_ttl)：更新已有 key 的 TTL
// - 全局 TTL vs per-entry TTL：set(None) 沿用全局 TTL；set(Some) 覆盖全局，
// 且可长于全局 TTL（全局 TTL 不是封顶）

use std::time::Duration;

use oxcache::backend::MokaMemoryBackend;
use oxcache::backend::{CacheReader, CacheWriter};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_GLOBAL_TTL,
    MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_NO_GLOBAL,
    MSG_EXAMPLE_MOKA_TTL_EXPIRE_K1_120S_SHOULD, MSG_EXAMPLE_MOKA_TTL_EXPIRE_MISSING_60S_SHOULD,
    MSG_EXAMPLE_MOKA_TTL_EXPIRE_UPDATES_TTL, MSG_EXAMPLE_MOKA_TTL_GET_A_AFTER_450MS,
    MSG_EXAMPLE_MOKA_TTL_GET_B_AFTER_450MS, MSG_EXAMPLE_MOKA_TTL_GET_TEMP_AFTER_100MS,
    MSG_EXAMPLE_MOKA_TTL_GLOBAL_TTL_VS_PER, MSG_EXAMPLE_MOKA_TTL_IMMEDIATE_GET_TEMP,
    MSG_EXAMPLE_MOKA_TTL_KEYS_WITHOUT_TTL, MSG_EXAMPLE_MOKA_TTL_MOKA_PER_ENTRY_TTL,
    MSG_EXAMPLE_MOKA_TTL_SET_A_NO_PER, MSG_EXAMPLE_MOKA_TTL_SET_B_PER_ENTRY,
    MSG_EXAMPLE_MOKA_TTL_SET_K1_60S_PER, MSG_EXAMPLE_MOKA_TTL_SET_K2_NO_TTL,
    MSG_EXAMPLE_MOKA_TTL_SET_TEMP_50MS_TTL, MSG_EXAMPLE_MOKA_TTL_TTL_A_NONE_TTL,
    MSG_EXAMPLE_MOKA_TTL_TTL_B_SHOULD_BE, MSG_EXAMPLE_MOKA_TTL_TTL_EXPIRY_VERIFICATION,
    MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE, MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE_2,
    MSG_EXAMPLE_MOKA_TTL_TTL_K2_SHOULD_BE, t,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // per-entry TTL
    println!("=== per-entry TTL ===");
    let backend = MokaMemoryBackend::new();
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_NO_GLOBAL,
            &[]
        )
    );

    // set with 60s per-entry TTL
    backend
        .set(
            "k1".into(),
            b"value1".to_vec().into(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("\n{}", t(MSG_EXAMPLE_MOKA_TTL_SET_K1_60S_PER, &[]));

    // ttl 查询剩余时间
    let ttl = backend.ttl("k1").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE,
            &[("as_secs", format!("{:?}", ttl.map(|d| d.as_secs())))]
        )
    );

    // expire 更新 TTL
    println!("\n{}", t(MSG_EXAMPLE_MOKA_TTL_EXPIRE_UPDATES_TTL, &[]));
    let result = backend.expire("k1", Duration::from_secs(120)).await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_EXPIRE_K1_120S_SHOULD,
            &[("result", result.to_string())]
        )
    );

    let ttl = backend.ttl("k1").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE_2,
            &[("as_secs", format!("{:?}", ttl.map(|d| d.as_secs())))]
        )
    );

    // expire 不存在的 key 返回 false
    let result = backend.expire("missing", Duration::from_secs(60)).await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_EXPIRE_MISSING_60S_SHOULD,
            &[("result", result.to_string())]
        )
    );

    // 无 TTL 的 key
    println!("\n{}", t(MSG_EXAMPLE_MOKA_TTL_KEYS_WITHOUT_TTL, &[]));
    backend
        .set("k2".into(), b"value2".to_vec().into(), None)
        .await?;
    println!("{}", t(MSG_EXAMPLE_MOKA_TTL_SET_K2_NO_TTL, &[]));

    let ttl = backend.ttl("k2").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_TTL_K2_SHOULD_BE,
            &[("ttl", format!("{:?}", ttl))]
        )
    );

    // TTL 过期验证
    println!("\n{}", t(MSG_EXAMPLE_MOKA_TTL_TTL_EXPIRY_VERIFICATION, &[]));
    backend
        .set(
            "temp".into(),
            b"temp_value".to_vec().into(),
            Some(Duration::from_millis(50)),
        )
        .await?;
    println!("{}", t(MSG_EXAMPLE_MOKA_TTL_SET_TEMP_50MS_TTL, &[]));

    let value = backend.get("temp").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_IMMEDIATE_GET_TEMP,
            &[("value", format!("{:?}", value))]
        )
    );

    tokio::time::sleep(Duration::from_millis(100)).await;
    let value = backend.get("temp").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_GET_TEMP_AFTER_100MS,
            &[("value", format!("{:?}", value))]
        )
    );

    // 全局 TTL vs per-entry TTL
    println!("\n{}", t(MSG_EXAMPLE_MOKA_TTL_GLOBAL_TTL_VS_PER, &[]));
    let backend_with_global_ttl = MokaMemoryBackend::builder()
        .capacity(10_000)
        .ttl(Duration::from_millis(300))
        .build();
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_GLOBAL_TTL,
            &[]
        )
    );

    // 无 per-entry TTL：沿用全局 TTL，300ms 后过期
    backend_with_global_ttl
        .set("a".into(), b"a_val".to_vec().into(), None)
        .await?;
    println!("{}", t(MSG_EXAMPLE_MOKA_TTL_SET_A_NO_PER, &[]));

    // 有 per-entry TTL：覆盖全局 TTL（600ms > 300ms，全局 TTL 不封顶 per-entry）
    backend_with_global_ttl
        .set(
            "b".into(),
            b"b_val".to_vec().into(),
            Some(Duration::from_millis(600)),
        )
        .await?;
    println!("{}", t(MSG_EXAMPLE_MOKA_TTL_SET_B_PER_ENTRY, &[]));

    // ttl() 只上报 per-entry TTL
    let ttl_a: Option<Duration> = backend_with_global_ttl.ttl("a").await?;
    let ttl_b: Option<Duration> = backend_with_global_ttl.ttl("b").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_TTL_A_NONE_TTL,
            &[("as_millis", format!("{:?}", ttl_a.map(|d| d.as_millis())))]
        )
    );
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_TTL_B_SHOULD_BE,
            &[("as_millis", format!("{:?}", ttl_b.map(|d| d.as_millis())))]
        )
    );

    tokio::time::sleep(Duration::from_millis(450)).await;
    let a = backend_with_global_ttl.get("a").await?;
    let b = backend_with_global_ttl.get("b").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_GET_A_AFTER_450MS,
            &[("a", format!("{:?}", a))]
        )
    );
    println!(
        "{}",
        t(
            MSG_EXAMPLE_MOKA_TTL_GET_B_AFTER_450MS,
            &[("b", format!("{:?}", b))]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_MOKA_TTL_MOKA_PER_ENTRY_TTL, &[]));
    Ok(())
}
