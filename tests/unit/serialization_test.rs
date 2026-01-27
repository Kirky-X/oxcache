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
/// 验证启用压缩后数据大小是否减少
#[test]
fn test_json_serializer_with_compression() {
    let serializer = JsonSerializer::with_compression();

    // Create some data that compresses well
    let original_data = b"aaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffffgggggggghhhhhhhh";

    let serialized = serializer.serialize("test", original_data).unwrap();
    let deserialized = serializer.deserialize("test", &serialized).unwrap();

    assert_eq!(original_data.as_slice(), deserialized.as_slice());

    // Verify compression actually reduced size (compared to uncompressed JSON)
    let uncompressed_serializer = JsonSerializer::new();
    let uncompressed = uncompressed_serializer
        .serialize("test", original_data)
        .unwrap();
    assert!(serialized.len() < uncompressed.len());
}
