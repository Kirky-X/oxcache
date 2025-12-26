//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 序列化单元测试

use oxcache::serialization::{bincode::BincodeSerializer, json::JsonSerializer, Serializer};
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
    let serializer = JsonSerializer;
    let data = TestStruct {
        id: 1,
        name: "test".to_string(),
        tags: vec!["a".into(), "b".into()],
    };

    let bytes = serializer.serialize(&data).unwrap();
    let deserialized: TestStruct = serializer.deserialize(&bytes).unwrap();

    assert_eq!(data, deserialized);
}

/// 测试Bincode序列化器的往返操作
///
/// 验证数据能否被正确序列化为Bincode格式并成功反序列化回原始数据
#[test]
fn test_bincode_serializer_round_trip() {
    let serializer = BincodeSerializer;
    let data = TestStruct {
        id: 1,
        name: "test".to_string(),
        tags: vec!["a".into(), "b".into()],
    };

    let bytes = serializer.serialize(&data).unwrap();
    let deserialized: TestStruct = serializer.deserialize(&bytes).unwrap();

    assert_eq!(data, deserialized);
}

/// 测试Bincode序列化比JSON更小
///
/// 验证对于具有整数和长度前缀字符串的结构体，Bincode序列化通常比JSON更小
#[test]
fn test_bincode_smaller_than_json() {
    let json = JsonSerializer;
    let bincode = BincodeSerializer;
    let data = TestStruct {
        id: 12345,
        name: "optimization_test".to_string(),
        tags: vec!["rust".into(), "cache".into(), "performance".into()],
    };

    let json_bytes = json.serialize(&data).unwrap();
    let bincode_bytes = bincode.serialize(&data).unwrap();

    // 对于具有整数和长度前缀字符串的结构体，Bincode通常比JSON更小
    // 因为JSON有字段名开销。
    assert!(bincode_bytes.len() < json_bytes.len());
}
