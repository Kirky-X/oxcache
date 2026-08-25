// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 序列化单元测试

use oxcache::infra::serialization::{JsonSerializer, Serializer};

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

/// 大对象 round-trip：验证 5MB 随机字节经 JSON base64 编码往返无截断
#[test]
fn test_json_serializer_large_payload_round_trip() {
    let serializer = JsonSerializer::new();

    // 伪随机但确定性的数据，避免依赖 rand
    let mut original_data = Vec::with_capacity(5 * 1024 * 1024);
    let seed: u64 = 0x9E3779B97F4A7C15;
    let mut x = seed;
    for _ in 0..(5 * 1024 * 1024) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        original_data.push((x & 0xFF) as u8);
    }

    let serialized = serializer.serialize("large", &original_data).unwrap();
    let deserialized = serializer.deserialize("large", &serialized).unwrap();

    assert_eq!(deserialized, original_data, "大对象 round-trip 数据必须逐字节一致");
}

/// 特殊 type_name / 非法输入边界：不 panic，损坏数据返回 Err
#[test]
fn test_json_serializer_special_keys_and_corrupt_data() {
    let serializer = JsonSerializer::new();
    let payload = b"edge case payload";

    // 空 type_name、超长 type_name、含控制字符的 type_name 均须正常往返
    let long_key = "k".repeat(4096);
    let special_keys: [&str; 3] = ["", long_key.as_str(), "key\r\nwith\x00control"];
    for name in special_keys {
        let serialized = serializer.serialize(name, payload).unwrap();
        let deserialized = serializer.deserialize(name, &serialized).unwrap();
        assert_eq!(deserialized, payload, "type_name={name:?} round-trip 失败");
    }

    // 空 payload round-trip
    let serialized = serializer.serialize("empty", b"").unwrap();
    let deserialized = serializer.deserialize("empty", &serialized).unwrap();
    assert!(deserialized.is_empty());

    // 错误路径：超过 MAX_JSON_SIZE（=5MB，见 src/core/constants.rs）的数据必须显性返回
    // Err（规则 12：错误不得被吞掉）；且该 serializer 非压缩模式为字节直通，任意
    // 字节内容（含非 JSON）均按原样往返
    let oversized = vec![0u8; 5 * 1024 * 1024 + 1];
    assert!(serializer.deserialize("oversized", &oversized).is_err());
    let passthrough = b"not-json-at-all!!";
    assert_eq!(
        serializer.deserialize("passthrough", passthrough).unwrap().as_slice(),
        passthrough.as_slice(),
        "非压缩模式须为字节直通"
    );
}
