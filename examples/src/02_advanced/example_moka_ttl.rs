// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Moka per-entry TTL 示例
//
// 本示例演示 oxcache 的 Moka per-entry TTL 功能：
// - set(key, value, Some(ttl))：per-entry TTL 真实生效（通过 moka::Expiry trait）
// - ttl(key)：只上报 per-entry TTL（未设置时返回 None，即使该条目按全局 TTL 过期）
// - expire(key, new_ttl)：更新已有 key 的 TTL
// - 全局 TTL vs per-entry TTL：set(None) 沿用全局 TTL；set(Some) 覆盖全局，
//   且可长于全局 TTL（全局 TTL 不是封顶）

use std::time::Duration;

use oxcache::backend::MokaMemoryBackend;
use oxcache::backend::{CacheReader, CacheWriter};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === per-entry TTL ===
    println!("=== per-entry TTL ===");
    let backend = MokaMemoryBackend::new();
    println!("Creating MokaMemoryBackend (no global TTL)");

    // set with 60s per-entry TTL
    backend
        .set(
            "k1".into(),
            b"value1".to_vec().into(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("\nset 'k1' (60s per-entry TTL)");

    // ttl 查询剩余时间
    let ttl = backend.ttl("k1").await?;
    println!(
        "ttl('k1') = {:?} (should be about 60s)",
        ttl.map(|d| d.as_secs())
    );

    // === expire 更新 TTL ===
    println!("\n=== expire updates TTL ===");
    let result = backend.expire("k1", Duration::from_secs(120)).await?;
    println!(
        "expire('k1', 120s) = {} (should return true when the key exists)",
        result
    );

    let ttl = backend.ttl("k1").await?;
    println!(
        "ttl('k1') = {:?} (should be about 120s)",
        ttl.map(|d| d.as_secs())
    );

    // expire 不存在的 key 返回 false
    let result = backend.expire("missing", Duration::from_secs(60)).await?;
    println!(
        "expire('missing', 60s) = {} (should return false when the key does not exist)",
        result
    );

    // === 无 TTL 的 key ===
    println!("\n=== Keys without TTL ===");
    backend
        .set("k2".into(), b"value2".to_vec().into(), None)
        .await?;
    println!("set 'k2' (no TTL, never expires)");

    let ttl = backend.ttl("k2").await?;
    println!(
        "ttl('k2') = {:?} (should be None without a per-entry TTL)",
        ttl
    );

    // === TTL 过期验证 ===
    println!("\n=== TTL expiry verification ===");
    backend
        .set(
            "temp".into(),
            b"temp_value".to_vec().into(),
            Some(Duration::from_millis(50)),
        )
        .await?;
    println!("set 'temp' (50ms TTL)");

    let value = backend.get("temp").await?;
    println!("immediate get('temp') = {:?}", value);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let value = backend.get("temp").await?;
    println!("get('temp') after 100ms = {:?} (should be expired)", value);

    // === 全局 TTL vs per-entry TTL ===
    println!("\n=== Global TTL vs per-entry TTL ===");
    let backend_with_global_ttl = MokaMemoryBackend::builder()
        .capacity(10_000)
        .ttl(Duration::from_millis(300))
        .build();
    println!("Creating MokaMemoryBackend (global TTL=300ms)");

    // 无 per-entry TTL：沿用全局 TTL，300ms 后过期
    backend_with_global_ttl
        .set("a".into(), b"a_val".to_vec().into(), None)
        .await?;
    println!("set 'a' (no per-entry TTL -> inherits the global 300ms)");

    // 有 per-entry TTL：覆盖全局 TTL（600ms > 300ms，全局 TTL 不封顶 per-entry）
    backend_with_global_ttl
        .set(
            "b".into(),
            b"b_val".to_vec().into(),
            Some(Duration::from_millis(600)),
        )
        .await?;
    println!("set 'b' (per-entry TTL=600ms > global 300ms -> per-entry wins)");

    // ttl() 只上报 per-entry TTL
    let ttl_a: Option<Duration> = backend_with_global_ttl.ttl("a").await?;
    let ttl_b: Option<Duration> = backend_with_global_ttl.ttl("b").await?;
    println!(
        "ttl('a') = {:?} (None: ttl() reports only per-entry TTL; 'a' still expires on the global 300ms)",
        ttl_a.map(|d| d.as_millis())
    );
    println!(
        "ttl('b') = {:?} (should be about 600ms: overrides the global, not capped at 300ms)",
        ttl_b.map(|d| d.as_millis())
    );

    tokio::time::sleep(Duration::from_millis(450)).await;
    let a = backend_with_global_ttl.get("a").await?;
    let b = backend_with_global_ttl.get("b").await?;
    println!(
        "get('a') after 450ms = {:?} (global 300ms elapsed -> None)",
        a
    );
    println!(
        "get('b') after 450ms = {:?} (per-entry 600ms not yet elapsed -> Some)",
        b
    );

    println!("\nMoka per-entry TTL example complete!");
    Ok(())
}
