// Copyright (c) 2025-2026 Kirky.X
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

use oxcache::infra::serialization::json::JsonSerializer;
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
    let metadata: Vec<String> = (0..50).map(|i| format!("metadata_{}: value_{}", i, i)).collect();

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
        .map(|i| format!("item_{}: this is some repetitive content for compression testing", i))
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
    println!("    原始大小: {} bytes", data.len());
    println!("    序列化后: {} bytes", serialized.len());
    if !data.is_empty() {
        let ratio = serialized.len() as f64 / data.len() as f64;
        println!("    压缩率: {:.2}x", ratio);
    }
    println!("    序列化耗时: {:?}", serialize_time);
    println!("    反序列化耗时: {:?}", deserialize_time);

    serialized
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 数据压缩示例 ===\n");

    // 1. 小数据对比
    println!("--- 1. 小数据 (< 100 bytes) ---");
    let small_data = create_small_data();
    let small_bytes = json_serialize(&small_data);

    let normal_serializer = JsonSerializer::new();
    let compress_serializer = JsonSerializer::with_compression();

    measure_serialization(&normal_serializer, &small_bytes, "不压缩");
    measure_serialization(&compress_serializer, &small_bytes, "启用压缩");
    println!();

    // 2. 中等数据对比 (100B - 1KB)
    println!("--- 2. 中等数据 (100B - 1KB) ---");
    let medium_data = create_medium_data();
    let medium_bytes = json_serialize(&medium_data);

    measure_serialization(&normal_serializer, &medium_bytes, "不压缩");
    measure_serialization(&compress_serializer, &medium_bytes, "启用压缩");
    println!();

    // 3. 大数据对比 (1KB - 100KB)
    println!("--- 3. 大数据 (1KB - 100KB) ---");
    let large_data = create_large_data();
    let large_bytes = json_serialize(&large_data);

    measure_serialization(&normal_serializer, &large_bytes, "不压缩");
    measure_serialization(&compress_serializer, &large_bytes, "启用压缩");
    println!();

    // 4. 重复数据对比
    println!("--- 4. 重复数据 (高压缩率场景) ---");
    let repeated_content = "This is a test string that will be repeated many times. ".repeat(500);
    let repeated_data = LargeData {
        id: 3,
        content: repeated_content,
        metadata: vec!["key1".to_string(); 200],
    };
    let repeated_bytes = json_serialize(&repeated_data);

    measure_serialization(&normal_serializer, &repeated_bytes, "不压缩");
    measure_serialization(&compress_serializer, &repeated_bytes, "启用压缩");
    println!();

    // 5. 在缓存中使用压缩
    println!("--- 5. 在缓存中使用压缩 ---");
    use oxcache::Cache;

    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // 存储压缩数据
    let key = "large_data".to_string();
    let compressed = compress_serializer.serialize("LargeData", &large_bytes)?;
    cache.set(&key, &compressed).await?;
    println!(
        "  存储压缩数据: {} bytes -> {} bytes",
        large_bytes.len(),
        compressed.len()
    );

    // 读取并解压
    if let Some(stored) = cache.get(&key).await? {
        let decompressed = compress_serializer.deserialize("LargeData", &stored)?;
        println!("  读取并解压: {} bytes -> {} bytes", stored.len(), decompressed.len());
        assert_eq!(decompressed, large_bytes);
        println!("  ✓ 数据完整性验证通过");
    }
    println!();

    // 6. 智能压缩策略说明
    println!("--- 6. 智能压缩策略 ---");
    println!("  oxcache 根据数据大小自动选择压缩级别:");
    println!("  - < 100 bytes: 不压缩（压缩开销不划算）");
    println!("  - 100B - 1KB: 快速压缩 (Compression::fast())");
    println!("  - 1KB - 100KB: 中等压缩 (Compression::new(6))");
    println!("  - > 100KB: 高压缩率 (Compression::best())");
    println!();

    // 清理
    cache.clear().await?;

    println!("=== 数据压缩示例完成 ===");
    println!("  关键点:");
    println!("  - 使用 JsonSerializer::with_compression() 启用压缩");
    println!("  - 小数据不压缩，避免压缩开销");
    println!("  - 重复数据压缩率高");
    println!("  - 压缩/解压对用户透明，自动处理");

    Ok(())
}

fn json_serialize<T: serde::Serialize>(data: &T) -> Vec<u8> {
    serde_json::to_vec(data).unwrap()
}
