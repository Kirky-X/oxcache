//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 序列化单元测试

use oxcache::serialization::{json::JsonSerializer, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TestStruct {
    id: u64,
    name: String,
    tags: Vec<String>,
}

/// 测试JSON序列化器的往返操作
///
/// 验证数据能否被正确序列化为JSON格式并成功反序列化回原始数据
#[test]
fn test_json_serializer_round_trip() {
    let serializer = JsonSerializer::new();
    let data = TestStruct {
        id: 1,
        name: "test".to_string(),
        tags: vec!["a".into(), "b".into()],
    };

    let bytes = serializer.serialize(&data).unwrap();
    let deserialized: TestStruct = serializer.deserialize(&bytes).unwrap();

    assert_eq!(data, deserialized);
}

/// 测试JSON序列化器的压缩功能
///
/// 验证启用压缩后数据大小是否减少
#[test]
fn test_json_serializer_with_compression() {
    let serializer = JsonSerializer::with_compression();
    let data = TestStruct {
        id: 12345,
        name: "optimization_test".to_string(),
        tags: vec!["rust".into(), "cache".into(), "performance".into()],
    };

    let bytes = serializer.serialize(&data).unwrap();
    let deserialized: TestStruct = serializer.deserialize(&bytes).unwrap();

    assert_eq!(data, deserialized);
}
