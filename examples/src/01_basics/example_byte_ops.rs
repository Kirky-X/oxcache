// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 字节级操作示例
//!
//! 本示例演示 Cache 的字节级操作 API（get_bytes / set_bytes），
//! 适用于需要直接控制序列化的场景。

use oxcache::Cache;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Byte-level operations example ===\n");

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    println!("--- 1. Basic byte operations ---");

    let key = "raw_data".to_string();
    let data = b"Hello, oxcache!".to_vec();

    // set_bytes 接受 TTL 参数（秒），None 表示无过期
    cache.set_bytes(&key, data.clone(), Some(60)).await?;
    println!("  write: {} bytes", data.len());

    let retrieved = cache.get_bytes(&key).await?;
    println!(
        "  read: {:?}",
        retrieved.map(|d| String::from_utf8_lossy(&d).to_string())
    );

    println!("\n--- 2. Byte operations without TTL ---");

    let key2 = "persistent_data".to_string();
    let data2 = b"Persistent data without TTL".to_vec();

    cache.set_bytes(&key2, data2.clone(), None).await?;
    println!("  write: {} bytes (no TTL)", data2.len());

    let retrieved2 = cache.get_bytes(&key2).await?;
    println!(
        "  read: {} bytes",
        retrieved2.as_ref().map(|d| d.len()).unwrap_or(0)
    );

    println!("\n--- 3. Binary data ---");

    let key3 = "binary_data".to_string();
    let binary_data: Vec<u8> = (0..=255).collect();

    cache
        .set_bytes(&key3, binary_data.clone(), Some(300))
        .await?;
    println!("  write: {} bytes of binary data", binary_data.len());

    let retrieved3 = cache.get_bytes(&key3).await?;
    if let Some(data) = retrieved3 {
        println!("  read: {} bytes", data.len());
        println!(
            "  checksum: {}",
            if data == binary_data {
                "✓ match"
            } else {
                "✗ mismatch"
            }
        );
    }

    println!("\n--- 4. Cache metadata operations ---");

    let count = cache.len().await?;
    println!("  entries: {}", count);

    let empty = cache.is_empty().await?;
    println!("  is empty: {}", empty);

    let cap = cache.capacity().await?;
    println!("  capacity: {}", cap);

    println!("\n--- 5. Serializer access ---");

    let _serializer = cache.serializer();
    println!(
        "  ✓ serializer instance obtained (dyn Serializer does not implement Debug, cannot print)"
    );

    println!("\n--- 6. Large data operations ---");

    let key4 = "large_data".to_string();
    let large_data = vec![0xABu8; 1024]; // 1KB

    cache.set_bytes(&key4, large_data.clone(), Some(60)).await?;
    println!("  write: {} bytes", large_data.len());

    let retrieved4 = cache.get_bytes(&key4).await?;
    println!(
        "  read: {} bytes",
        retrieved4.as_ref().map(|d| d.len()).unwrap_or(0)
    );

    // 清理
    cache.clear().await?;

    // 验证清理后状态
    let count_after = cache.len().await?;
    let empty_after = cache.is_empty().await?;
    println!(
        "\n  after cleanup: entries={}, is_empty={}",
        count_after, empty_after
    );

    // 关闭缓存，释放资源
    cache.shutdown().await;
    println!("  ✓ cache closed");

    println!("\n✓ Example complete");
    Ok(())
}
