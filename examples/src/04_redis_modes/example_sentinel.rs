// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis Sentinel模式示例
//
// 本示例演示使用Redis Sentinel实现
// 高可用性和自动故障转移。
//
// 注意: 此示例使用仅L1模式进行演示。
// 要使用Redis Sentinel，请配置:
// - cache_type: TwoLevel
// - l2.mode: Sentinel
// - l2.sentinel.master_name: "mymaster"
// - l2.sentinel.nodes: [...]

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct SentinelData {
    id: u64,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 演示使用仅L1模式 (不需要Redis)
    // 实际Sentinel使用，请配置TwoLevel + Sentinel
    let config = OxcacheConfig::builder()
        .with_service(
            "sentinel_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("sentinel_cache")?;

    println!("Redis Sentinel模式示例");
    println!("=======================\n");
    println!("注意: 演示使用仅L1模式");
    println!("对于实际Sentinel，请配置:");
    println!("  - cache_type: TwoLevel");
    println!("  - l2.mode: Sentinel");
    println!("  - l2.sentinel.master_name: mymaster");
    println!("  - l2.sentinel.nodes: [host1:26379, host2:26379, ...]\n");

    // 测试基本操作
    let data = SentinelData {
        id: 1,
        content: "High availability data".to_string(),
    };

    println!("写入数据...");
    client.set("sentinel:test", &data, None).await?;
    println!("  写入: {}", data.content);

    println!("\n读取数据...");
    if let Some(cached) = client.get::<SentinelData>("sentinel:test").await? {
        println!("  读取: {}", cached.content);
    }

    println!("\nSentinel优势:");
    println!("  - 自动故障转移");
    println!("  - 高可用性");
    println!("  - 主副本同步");

    println!("\n✓ Sentinel模式示例完成!");
    Ok(())
}