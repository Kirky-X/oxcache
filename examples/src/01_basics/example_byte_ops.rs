// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 字节级操作示例
//!
//! 本示例演示 Cache 的字节级操作 API（get_bytes / set_bytes），
//! 适用于需要直接控制序列化的场景。

use oxcache::Cache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 字节级操作示例 ===\n");

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    // 1. 基本字节操作
    println!("--- 1. 基本字节操作 ---");

    let key = "raw_data".to_string();
    let data = b"Hello, oxcache!".to_vec();

    // set_bytes 接受 TTL 参数（秒），None 表示无过期
    cache.set_bytes(&key, data.clone(), Some(60)).await?;
    println!("  写入: {} 字节", data.len());

    let retrieved = cache.get_bytes(&key).await?;
    println!(
        "  读取: {:?}",
        retrieved.map(|d| String::from_utf8_lossy(&d).to_string())
    );

    // 2. 无 TTL 的字节操作
    println!("\n--- 2. 无 TTL 的字节操作 ---");

    let key2 = "persistent_data".to_string();
    let data2 = b"Persistent data without TTL".to_vec();

    cache.set_bytes(&key2, data2.clone(), None).await?;
    println!("  写入: {} 字节（无 TTL）", data2.len());

    let retrieved2 = cache.get_bytes(&key2).await?;
    println!("  读取: {} 字节", retrieved2.as_ref().map(|d| d.len()).unwrap_or(0));

    // 3. 二进制数据
    println!("\n--- 3. 二进制数据 ---");

    let key3 = "binary_data".to_string();
    let binary_data: Vec<u8> = (0..=255).collect();

    cache.set_bytes(&key3, binary_data.clone(), Some(300)).await?;
    println!("  写入: {} 字节二进制数据", binary_data.len());

    let retrieved3 = cache.get_bytes(&key3).await?;
    if let Some(data) = retrieved3 {
        println!("  读取: {} 字节", data.len());
        println!(
            "  校验: {}",
            if data == binary_data {
                "✓ 一致"
            } else {
                "✗ 不一致"
            }
        );
    }

    // 4. 序列化器访问
    println!("\n--- 4. 序列化器访问 ---");

    let _serializer = cache.serializer();
    println!("  ✓ 已获取序列化器实例（dyn Serializer 不实现 Debug，无法打印）");

    // 5. 大数据操作
    println!("\n--- 5. 大数据操作 ---");

    let key4 = "large_data".to_string();
    let large_data = vec![0xABu8; 1024]; // 1KB

    cache.set_bytes(&key4, large_data.clone(), Some(60)).await?;
    println!("  写入: {} 字节", large_data.len());

    let retrieved4 = cache.get_bytes(&key4).await?;
    println!("  读取: {} 字节", retrieved4.as_ref().map(|d| d.len()).unwrap_or(0));

    // 清理
    cache.clear().await?;

    println!("\n✓ 示例完成");
    Ok(())
}
