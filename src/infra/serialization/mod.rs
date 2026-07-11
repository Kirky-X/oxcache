//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的序列化机制，支持多种序列化格式。

pub mod depth_limited;
pub mod json;

pub mod unified;
pub mod utils;

use crate::error::Result;

pub use json::JsonSerializer;

// Unified serialization exports
pub use unified::{default_serializer, UnifiedSerializer, UnifiedSerializerAdapter};

/// 序列化器特征
///
/// 定义序列化和反序列化操作的接口。
/// 这是公共API,但实现细节应该私有。
pub trait Serializer: Send + Sync {
    /// 序列化值为字节数组
    ///
    /// # Arguments
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要序列化的字节数组
    ///
    /// # Returns
    ///
    /// 返回序列化后的字节数组或错误
    fn serialize(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>>;

    /// 从字节数组反序列化值
    ///
    /// # Arguments
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要反序列化的字节数组
    ///
    /// # Returns
    ///
    /// 返回反序列化后的字节数组或错误
    fn deserialize(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>>;

    /// 零拷贝序列化
    ///
    /// 提供零拷贝序列化操作，默认实现调用普通 serialize 方法。
    /// 某些序列化格式（如 bincode）可以重写此方法以实现真正的零拷贝。
    ///
    /// # Arguments
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要序列化的字节数组
    ///
    /// # Returns
    ///
    /// 返回序列化后的字节数组或错误
    fn serialize_zero_copy(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        self.serialize(type_name, data)
    }

    /// 零拷贝反序列化
    ///
    /// 提供零拷贝反序列化操作，默认实现调用普通 deserialize 方法。
    /// 某些序列化格式可以重写此方法以避免数据拷贝。
    ///
    /// # Arguments
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要反序列化的字节数组
    ///
    /// # Returns
    ///
    /// 返回反序列化后的字节数组或错误
    fn deserialize_zero_copy(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        self.deserialize(type_name, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用的简单序列化器实现
    struct TestSerializer;

    impl Serializer for TestSerializer {
        fn serialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
            Ok(data.to_vec())
        }

        fn deserialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
            Ok(data.to_vec())
        }
    }

    #[test]
    fn test_serializer_serialize() {
        let serializer = TestSerializer;
        let data = b"test data";
        let result = serializer.serialize("TestType", data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data.to_vec());
    }

    #[test]
    fn test_serializer_deserialize() {
        let serializer = TestSerializer;
        let data = b"test data";
        let result = serializer.deserialize("TestType", data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data.to_vec());
    }

    #[test]
    fn test_serializer_serialize_zero_copy_default() {
        // 测试默认的 serialize_zero_copy 方法
        let serializer = TestSerializer;
        let data = b"test data";
        let result = serializer.serialize_zero_copy("TestType", data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data.to_vec());
    }

    #[test]
    fn test_serializer_deserialize_zero_copy_default() {
        // 测试默认的 deserialize_zero_copy 方法
        let serializer = TestSerializer;
        let data = b"test data";
        let result = serializer.deserialize_zero_copy("TestType", data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data.to_vec());
    }

    #[test]
    fn test_serializer_serialize_empty_data() {
        let serializer = TestSerializer;
        let data = b"";
        let result = serializer.serialize("TestType", data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_serializer_deserialize_empty_data() {
        let serializer = TestSerializer;
        let data = b"";
        let result = serializer.deserialize("TestType", data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_serializer_serialize_zero_copy_empty() {
        let serializer = TestSerializer;
        let data = b"";
        let result = serializer.serialize_zero_copy("TestType", data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_serializer_deserialize_zero_copy_empty() {
        let serializer = TestSerializer;
        let data = b"";
        let result = serializer.deserialize_zero_copy("TestType", data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_serializer_trait_object() {
        // 测试 trait 对象动态分发
        let serializer: &dyn Serializer = &TestSerializer;
        let data = b"test data";

        let serialized = serializer.serialize("TestType", data).unwrap();
        assert_eq!(serialized, data.to_vec());

        let deserialized = serializer.deserialize("TestType", data).unwrap();
        assert_eq!(deserialized, data.to_vec());

        let zero_copy_serialized = serializer.serialize_zero_copy("TestType", data).unwrap();
        assert_eq!(zero_copy_serialized, data.to_vec());

        let zero_copy_deserialized = serializer.deserialize_zero_copy("TestType", data).unwrap();
        assert_eq!(zero_copy_deserialized, data.to_vec());
    }

    #[test]
    fn test_serializer_roundtrip() {
        // 测试序列化-反序列化往返
        let serializer = TestSerializer;
        let original_data = b"roundtrip test data";

        let serialized = serializer.serialize("TestType", original_data).unwrap();
        let deserialized = serializer.deserialize("TestType", &serialized).unwrap();

        assert_eq!(deserialized, original_data.to_vec());
    }

    #[test]
    fn test_serializer_zero_copy_roundtrip() {
        // 测试零拷贝序列化-反序列化往返
        let serializer = TestSerializer;
        let original_data = b"zero copy roundtrip test";

        let serialized = serializer.serialize_zero_copy("TestType", original_data).unwrap();
        let deserialized = serializer.deserialize_zero_copy("TestType", &serialized).unwrap();

        assert_eq!(deserialized, original_data.to_vec());
    }
}
