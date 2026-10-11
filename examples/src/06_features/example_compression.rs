// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据压缩示例
//!
//! 本示例演示 oxcache 的压缩功能：
//! - 使用 JsonSerializer::with_compression() 启用压缩
//! - 对比压缩前后的数据大小
//! - 展示智能压缩策略（根据数据大小选择压缩级别）
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_compression
//! ```

use oxcache::infra::serialization::JsonSerializer;
use oxcache::infra::serialization::Serializer;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    bio: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LargeData {
    id: u64,
    content: String,
    metadata: Vec<String>,
}

fn create_small_data() -> User {
    User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        bio: "Software developer".to_string(),
    }
}

fn create_medium_data() -> LargeData {
    let content = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100);
    let metadata: Vec<String> = (0..50)
        .map(|i| format!("metadata_{}: value_{}", i, i))
        .collect();

    LargeData {
        id: 1,
        content,
        metadata,
    }
}

fn create_large_data() -> LargeData {
    // 创建约 150KB 的重复数据
    let content = "The quick brown fox jumps over the lazy dog. ".repeat(3000);
    let metadata: Vec<String> = (0..1000)
        .map(|i| {
            format!(
                "item_{}: this is some repetitive content for compression testing",
                i
            )
        })
        .collect();

    LargeData {
        id: 2,
        content,
        metadata,
    }
}

fn measure_serialization(serializer: &dyn Serializer, data: &[u8], label: &str) -> Vec<u8> {
    let start = std::time::Instant::now();
    let serialized = serializer.serialize("test", data).unwrap();
    let serialize_time = start.elapsed();

    let start = std::time::Instant::now();
    let _deserialized = serializer.deserialize("test", &serialized).unwrap();
    let deserialize_time = start.elapsed();

    println!("  {}:", label);
    println!("    original size: {} bytes", data.len());
    println!("    serialized: {} bytes", serialized.len());
    if !data.is_empty() {
        let ratio = serialized.len() as f64 / data.len() as f64;
        println!("    compression ratio: {:.2}x", ratio);
    }
    println!("    serialization took: {:?}", serialize_time);
    println!("    deserialization took: {:?}", deserialize_time);

    serialized
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Data compression example ===\n");

    println!("--- 1. Small data (< 100 bytes) ---");
    let small_data = create_small_data();
    let small_bytes = json_serialize(&small_data);

    let normal_serializer = JsonSerializer::new();
    let compress_serializer = JsonSerializer::with_compression();

    measure_serialization(&normal_serializer, &small_bytes, "no compression");
    measure_serialization(&compress_serializer, &small_bytes, "compression enabled");
    println!();

    println!("--- 2. Medium data (100B - 1KB) ---");
    let medium_data = create_medium_data();
    let medium_bytes = json_serialize(&medium_data);

    measure_serialization(&normal_serializer, &medium_bytes, "no compression");
    measure_serialization(&compress_serializer, &medium_bytes, "compression enabled");
    println!();

    println!("--- 3. Large data (1KB - 100KB) ---");
    let large_data = create_large_data();
    let large_bytes = json_serialize(&large_data);

    measure_serialization(&normal_serializer, &large_bytes, "no compression");
    measure_serialization(&compress_serializer, &large_bytes, "compression enabled");
    println!();

    println!("--- 4. Repeated data (high compression ratio) ---");
    let repeated_content = "This is a test string that will be repeated many times. ".repeat(500);
    let repeated_data = LargeData {
        id: 3,
        content: repeated_content,
        metadata: vec!["key1".to_string(); 200],
    };
    let repeated_bytes = json_serialize(&repeated_data);

    measure_serialization(&normal_serializer, &repeated_bytes, "no compression");
    measure_serialization(&compress_serializer, &repeated_bytes, "compression enabled");
    println!();

    println!("--- 5. Using compression in the cache ---");
    use oxcache::Cache;

    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // 存储压缩数据
    let key = "large_data".to_string();
    let compressed = compress_serializer.serialize("LargeData", &large_bytes)?;
    cache.set(&key, &compressed).await?;
    println!(
        "  storing compressed data: {} bytes -> {} bytes",
        large_bytes.len(),
        compressed.len()
    );

    // 读取并解压
    if let Some(stored) = cache.get(&key).await? {
        let decompressed = compress_serializer.deserialize("LargeData", &stored)?;
        println!(
            "  reading and decompressing: {} bytes -> {} bytes",
            stored.len(),
            decompressed.len()
        );
        assert_eq!(decompressed, large_bytes);
        println!("  ✓ data integrity verified");
    }
    println!();

    println!("--- 6. Smart compression strategy ---");
    println!("  oxcache picks the compression level automatically by data size:");
    println!("  - < 100 bytes: no compression (the overhead outweighs the gain)");
    println!("  - 100B - 1KB: fast compression (Compression::fast())");
    println!("  - 1KB - 100KB: medium compression (Compression::new(6))");
    println!("  - > 100KB: high compression ratio (Compression::best())");
    println!();

    // 清理
    cache.clear().await?;

    println!("=== Data compression example complete ===");
    println!("  key points:");
    println!("  - enable compression with JsonSerializer::with_compression()");
    println!("  - small data is left uncompressed, avoiding compression overhead");
    println!("  - repeated data compresses well");
    println!("  - compression/decompression is transparent and automatic");

    Ok(())
}

fn json_serialize<T: serde::Serialize>(data: &T) -> Vec<u8> {
    serde_json::to_vec(data).unwrap()
}
