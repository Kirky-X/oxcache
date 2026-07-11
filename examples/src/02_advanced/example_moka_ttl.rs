// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Moka per-entry TTL 示例
//
// 本示例演示 oxcache 0.3.0 的 Moka per-entry TTL 功能：
// - set(key, value, Some(ttl))：per-entry TTL 真实生效（通过 moka::Expiry trait）
// - ttl(key)：返回剩余 TTL
// - expire(key, new_ttl)：更新已有 key 的 TTL
// - 全局 TTL vs per-entry TTL 的优先级

use std::time::Duration;

use oxcache::backend::interface::{CacheReader, CacheWriter};
use oxcache::backend::MokaMemoryBackend;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === per-entry TTL ===
    println!("=== per-entry TTL ===");
    let backend = MokaMemoryBackend::new();
    println!("创建 MokaMemoryBackend（无全局 TTL）");

    // set with 60s per-entry TTL
    backend
        .set("k1", b"value1".to_vec(), Some(Duration::from_secs(60)))
        .await?;
    println!("\nset 'k1'（60s per-entry TTL）");

    // ttl 查询剩余时间
    let ttl = backend.ttl("k1").await?;
    println!("ttl('k1') = {:?}（应约为 60s）", ttl.map(|d| d.as_secs()));

    // === expire 更新 TTL ===
    println!("\n=== expire 更新 TTL ===");
    let result = backend.expire("k1", Duration::from_secs(120)).await?;
    println!("expire('k1', 120s) = {}（key 存在应返回 true）", result);

    let ttl = backend.ttl("k1").await?;
    println!("ttl('k1') = {:?}（应约为 120s）", ttl.map(|d| d.as_secs()));

    // expire 不存在的 key 返回 false
    let result = backend.expire("missing", Duration::from_secs(60)).await?;
    println!("expire('missing', 60s) = {}（key 不存在应返回 false）", result);

    // === 无 TTL 的 key ===
    println!("\n=== 无 TTL 的 key ===");
    backend.set("k2", b"value2".to_vec(), None).await?;
    println!("set 'k2'（无 TTL，永不过期）");

    let ttl = backend.ttl("k2").await?;
    println!("ttl('k2') = {:?}（无 per-entry TTL 应返回 None）", ttl);

    // === TTL 过期验证 ===
    println!("\n=== TTL 过期验证 ===");
    backend
        .set("temp", b"temp_value".to_vec(), Some(Duration::from_millis(50)))
        .await?;
    println!("set 'temp'（50ms TTL）");

    let value = backend.get("temp").await?;
    println!("立即 get('temp') = {:?}", value);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let value = backend.get("temp").await?;
    println!("100ms 后 get('temp') = {:?}（应已过期）", value);

    // === 全局 TTL vs per-entry TTL ===
    println!("\n=== 全局 TTL vs per-entry TTL ===");
    let backend_with_global_ttl = MokaMemoryBackend::builder()
        .capacity(10_000)
        .ttl(Duration::from_secs(300))
        .build();
    println!("创建 MokaMemoryBackend（全局 TTL=300s）");

    // 无 per-entry TTL：使用全局 TTL
    backend_with_global_ttl.set("a", b"a_val".to_vec(), None).await?;
    println!("set 'a'（无 per-entry TTL → 使用全局 300s）");

    // 有 per-entry TTL：覆盖全局 TTL
    backend_with_global_ttl
        .set("b", b"b_val".to_vec(), Some(Duration::from_secs(10)))
        .await?;
    println!("set 'b'（per-entry TTL=10s → 覆盖全局 300s）");

    let ttl_a: Option<Duration> = backend_with_global_ttl.ttl("a").await?;
    let ttl_b: Option<Duration> = backend_with_global_ttl.ttl("b").await?;
    println!("ttl('a') = {:?}（应接近 300s）", ttl_a.map(|d| d.as_secs()));
    println!("ttl('b') = {:?}（应接近 10s）", ttl_b.map(|d| d.as_secs()));

    println!("\nMoka per-entry TTL 示例完成！");
    Ok(())
}
