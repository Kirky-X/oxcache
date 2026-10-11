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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_COMPRESSION_1_SMALL_DATA_100, MSG_EXAMPLE_COMPRESSION_1KB_100KB_MEDIUM_COMPRESSION,
    MSG_EXAMPLE_COMPRESSION_2_MEDIUM_DATA_100B, MSG_EXAMPLE_COMPRESSION_3_LARGE_DATA_1KB,
    MSG_EXAMPLE_COMPRESSION_4_REPEATED_DATA_HIGH, MSG_EXAMPLE_COMPRESSION_5_USING_COMPRESSION_IN,
    MSG_EXAMPLE_COMPRESSION_6_SMART_COMPRESSION_STRATEGY,
    MSG_EXAMPLE_COMPRESSION_100_BYTES_NO_COMPRESSION,
    MSG_EXAMPLE_COMPRESSION_100B_1KB_FAST_COMPRESSION,
    MSG_EXAMPLE_COMPRESSION_100KB_HIGH_COMPRESSION_RATIO,
    MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE,
    MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_COMPRESSION_DATA_INTEGRITY_VERIFIED,
    MSG_EXAMPLE_COMPRESSION_DECOMPRESSION_IS_TRANSPARENT,
    MSG_EXAMPLE_COMPRESSION_DESERIALIZATION_TOOK,
    MSG_EXAMPLE_COMPRESSION_ENABLE_COMPRESSION_WITH_JSONSERIALIZER,
    MSG_EXAMPLE_COMPRESSION_KEY_POINTS, MSG_EXAMPLE_COMPRESSION_ORIGINAL_SIZE_BYTES,
    MSG_EXAMPLE_COMPRESSION_OXCACHE_PICKS_THE_COMPRESSION, MSG_EXAMPLE_COMPRESSION_RATIO_X,
    MSG_EXAMPLE_COMPRESSION_READING_AND_DECOMPRESSING_BYTES,
    MSG_EXAMPLE_COMPRESSION_REPEATED_DATA_COMPRESSES_WELL,
    MSG_EXAMPLE_COMPRESSION_SERIALIZATION_TOOK, MSG_EXAMPLE_COMPRESSION_SERIALIZED_BYTES,
    MSG_EXAMPLE_COMPRESSION_SMALL_DATA_IS_LEFT,
    MSG_EXAMPLE_COMPRESSION_STORING_COMPRESSED_DATA_BYTES, t,
};

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
    println!(
        "    {}",
        t(
            MSG_EXAMPLE_COMPRESSION_ORIGINAL_SIZE_BYTES,
            &[("data", data.len().to_string())]
        )
    );
    println!(
        "    {}",
        t(
            MSG_EXAMPLE_COMPRESSION_SERIALIZED_BYTES,
            &[("serialized", serialized.len().to_string())]
        )
    );
    if !data.is_empty() {
        let ratio = serialized.len() as f64 / data.len() as f64;
        println!(
            "    {}",
            t(
                MSG_EXAMPLE_COMPRESSION_RATIO_X,
                &[("ratio", format!("{:.2}", ratio))]
            )
        );
    }
    println!(
        "    {}",
        t(
            MSG_EXAMPLE_COMPRESSION_SERIALIZATION_TOOK,
            &[("serialize_time", format!("{:?}", serialize_time))]
        )
    );
    println!(
        "    {}",
        t(
            MSG_EXAMPLE_COMPRESSION_DESERIALIZATION_TOOK,
            &[("deserialize_time", format!("{:?}", deserialize_time))]
        )
    );

    serialized
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE, &[])
    );

    println!("{}", t(MSG_EXAMPLE_COMPRESSION_1_SMALL_DATA_100, &[]));
    let small_data = create_small_data();
    let small_bytes = json_serialize(&small_data);

    let normal_serializer = JsonSerializer::new();
    let compress_serializer = JsonSerializer::with_compression();

    measure_serialization(&normal_serializer, &small_bytes, "no compression");
    measure_serialization(&compress_serializer, &small_bytes, "compression enabled");
    println!();

    println!("{}", t(MSG_EXAMPLE_COMPRESSION_2_MEDIUM_DATA_100B, &[]));
    let medium_data = create_medium_data();
    let medium_bytes = json_serialize(&medium_data);

    measure_serialization(&normal_serializer, &medium_bytes, "no compression");
    measure_serialization(&compress_serializer, &medium_bytes, "compression enabled");
    println!();

    println!("{}", t(MSG_EXAMPLE_COMPRESSION_3_LARGE_DATA_1KB, &[]));
    let large_data = create_large_data();
    let large_bytes = json_serialize(&large_data);

    measure_serialization(&normal_serializer, &large_bytes, "no compression");
    measure_serialization(&compress_serializer, &large_bytes, "compression enabled");
    println!();

    println!("{}", t(MSG_EXAMPLE_COMPRESSION_4_REPEATED_DATA_HIGH, &[]));
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

    println!("{}", t(MSG_EXAMPLE_COMPRESSION_5_USING_COMPRESSION_IN, &[]));
    use oxcache::Cache;

    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // 存储压缩数据
    let key = "large_data".to_string();
    let compressed = compress_serializer.serialize("LargeData", &large_bytes)?;
    cache.set(&key, &compressed).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPRESSION_STORING_COMPRESSED_DATA_BYTES,
            &[
                ("large_bytes", large_bytes.len().to_string()),
                ("compressed", compressed.len().to_string())
            ]
        )
    );

    if let Some(stored) = cache.get(&key).await? {
        let decompressed = compress_serializer.deserialize("LargeData", &stored)?;
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_COMPRESSION_READING_AND_DECOMPRESSING_BYTES,
                &[
                    ("stored", stored.len().to_string()),
                    ("decompressed", decompressed.len().to_string())
                ]
            )
        );
        assert_eq!(decompressed, large_bytes);
        println!(
            "  {}",
            t(MSG_EXAMPLE_COMPRESSION_DATA_INTEGRITY_VERIFIED, &[])
        );
    }
    println!();

    println!(
        "{}",
        t(MSG_EXAMPLE_COMPRESSION_6_SMART_COMPRESSION_STRATEGY, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_OXCACHE_PICKS_THE_COMPRESSION, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_100_BYTES_NO_COMPRESSION, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_100B_1KB_FAST_COMPRESSION, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_1KB_100KB_MEDIUM_COMPRESSION, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_100KB_HIGH_COMPRESSION_RATIO, &[])
    );
    println!();

    cache.clear().await?;

    println!(
        "{}",
        t(
            MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE_COMPLETE,
            &[]
        )
    );
    println!("  {}", t(MSG_EXAMPLE_COMPRESSION_KEY_POINTS, &[]));
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_COMPRESSION_ENABLE_COMPRESSION_WITH_JSONSERIALIZER,
            &[]
        )
    );
    println!("  {}", t(MSG_EXAMPLE_COMPRESSION_SMALL_DATA_IS_LEFT, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_REPEATED_DATA_COMPRESSES_WELL, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_COMPRESSION_DECOMPRESSION_IS_TRANSPARENT, &[])
    );

    Ok(())
}

fn json_serialize<T: serde::Serialize>(data: &T) -> Vec<u8> {
    serde_json::to_vec(data).unwrap()
}
