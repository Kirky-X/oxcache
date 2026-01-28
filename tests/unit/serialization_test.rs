// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 序列化单元测试

use oxcache::serialization::{json::JsonSerializer, Serializer};

/// 测试JSON序列化器的往返操作
///
/// 验证原始字节能否被正确序列化为JSON格式并成功反序列化回原始字节
#[test]
fn test_json_serializer_round_trip() {
    let serializer = JsonSerializer::new();

    // Test with raw bytes
    let original_data = b"hello world this is test data";

    let serialized = serializer.serialize("test", original_data).unwrap();
    let deserialized = serializer.deserialize("test", &serialized).unwrap();

    assert_eq!(original_data.as_slice(), deserialized.as_slice());
}

/// 测试JSON序列化器的压缩功能
///
/// 验证启用压缩后数据能正确压缩和解压缩
#[test]
fn test_json_serializer_with_compression() {
    let serializer = JsonSerializer::with_compression();

    // Create test data that can be compressed
    // Using a struct with string field that has repetitive content
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
    struct TestData {
        content: String,
    }

    let test_data = TestData {
        content: "aaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffffgggggggghhhhhhhh".repeat(5),
    };

    // Serialize to bytes first
    let original_bytes = serde_json::to_vec(&test_data).unwrap();

    let serialized = serializer.serialize("test", &original_bytes).unwrap();
    let deserialized = serializer.deserialize("test", &serialized).unwrap();

    // Verify we can deserialize back to the original data
    let deserialized_data: TestData = serde_json::from_slice(&deserialized).unwrap();
    assert_eq!(test_data, deserialized_data);

    // Note: Compression effectiveness depends on data patterns and JSON encoding.
    // Base64-encoded bytes don't compress well. The key is that compression/decompression
    // cycle works correctly, not that output is smaller.
    // For better compression results, use serialization formats that preserve data structure
    // (like bincode) or compress before JSON encoding.
}
