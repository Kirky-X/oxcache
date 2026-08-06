// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified serialization manager

use crate::core::MAX_JSON_DEPTH;
use crate::error::{OxCacheError, OxCacheResult};
use crate::infra::serialization::depth_limited::deserialize_safe;
use crate::infra::serialization::utils::{compress_data, decompress_data_with_limit};
use serde::{Serialize, de::DeserializeOwned};

/// Json-only unified serializer
///
/// 直接使用 `serde_json` 做单次序列化/反序列化，不再经由
/// `JsonSerializer` 的 base64 包装层，消除双重序列化开销。
#[derive(Clone, Debug)]
pub struct UnifiedSerializer {
    compress: bool,
}

impl UnifiedSerializer {
    pub fn new() -> Self {
        Self { compress: false }
    }

    pub fn json() -> Self {
        Self::new()
    }

    /// 创建启用压缩的统一序列化器
    pub fn with_compression() -> Self {
        Self { compress: true }
    }

    /// Serialize a value to bytes
    pub fn serialize<T: Serialize>(&self, value: &T) -> OxCacheResult<Vec<u8>> {
        let data = serde_json::to_vec(value).map_err(|e| OxCacheError::Serialization(e.to_string()))?;
        if self.compress { compress_data(&data) } else { Ok(data) }
    }

    /// Serialize with explicit type name (for internal use)
    pub fn serialize_with_type(&self, _type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        if self.compress {
            compress_data(data)
        } else {
            Ok(data.to_vec())
        }
    }

    /// Deserialize bytes to a value
    ///
    /// 单次文本解析 + 深度校验（`MAX_JSON_DEPTH`），防止栈溢出攻击。
    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> OxCacheResult<T> {
        let data = if self.compress {
            decompress_data_with_limit(data, crate::infra::serialization::utils::MAX_DECOMPRESS_SIZE)?
        } else {
            data.to_vec()
        };
        deserialize_safe(&data, MAX_JSON_DEPTH).map_err(|e| OxCacheError::Serialization(e.to_string()))
    }

    /// Deserialize with explicit type name (for internal use)
    pub fn deserialize_with_type(&self, _type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        if self.compress {
            decompress_data_with_limit(data, crate::infra::serialization::utils::MAX_DECOMPRESS_SIZE)
        } else {
            Ok(data.to_vec())
        }
    }

    /// Get approximate size of serialized data
    pub fn estimate_size<T: Serialize>(&self, value: &T) -> OxCacheResult<usize> {
        let serialized = self.serialize(value)?;
        Ok(serialized.len())
    }
}

impl Default for UnifiedSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter to convert UnifiedSerializer to the Serializer trait
pub struct UnifiedSerializerAdapter {
    inner: UnifiedSerializer,
}

impl UnifiedSerializerAdapter {
    pub fn new(serializer: UnifiedSerializer) -> Self {
        Self { inner: serializer }
    }
}

impl crate::infra::Serializer for UnifiedSerializerAdapter {
    fn serialize(&self, type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        self.inner.serialize_with_type(type_name, data)
    }

    fn deserialize(&self, type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        self.inner.deserialize_with_type(type_name, data)
    }
}

/// Default serializer instance
pub fn default_serializer() -> UnifiedSerializer {
    UnifiedSerializer::json()
}

/// Convenience functions for common operations
pub mod convenience {
    use super::*;

    pub fn to_json<T: Serialize>(value: &T) -> OxCacheResult<Vec<u8>> {
        default_serializer().serialize(value)
    }

    pub fn from_json<T: DeserializeOwned>(data: &[u8]) -> OxCacheResult<T> {
        default_serializer().deserialize(data)
    }

    pub fn estimate_json_size<T: Serialize>(value: &T) -> OxCacheResult<usize> {
        default_serializer().estimate_size(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::serialization::Serializer;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    struct TestData {
        name: String,
        value: i32,
        items: Vec<String>,
    }

    fn test_data() -> TestData {
        TestData {
            name: "test".to_string(),
            value: 42,
            items: vec!["item1".to_string(), "item2".to_string()],
        }
    }

    #[test]
    fn test_json_serialization() {
        let serializer = UnifiedSerializer::json();
        let data = test_data();

        let serialized = serializer.serialize(&data).unwrap();
        let deserialized: TestData = serializer.deserialize(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_convenience_functions() {
        let data = test_data();

        let json_bytes = convenience::to_json(&data).unwrap();
        let json_deserialized: TestData = convenience::from_json(&json_bytes).unwrap();
        assert_eq!(data, json_deserialized);

        let estimated_size = convenience::estimate_json_size(&data).unwrap();
        assert!(estimated_size > 0);
        assert_eq!(estimated_size, json_bytes.len());
    }

    #[test]
    fn test_adapter() {
        let unified = UnifiedSerializer::json();
        let adapter = UnifiedSerializerAdapter::new(unified);

        let data = test_data();
        let type_name = std::any::type_name::<TestData>();
        let json_data = serde_json::to_vec(&data).unwrap();
        let serialized = adapter.serialize(type_name, &json_data).unwrap();
        let deserialized = adapter.deserialize(type_name, &serialized).unwrap();
        assert_eq!(json_data, deserialized);
    }

    #[cfg(feature = "compression")]
    #[test]
    fn test_compression_round_trip() {
        let serializer = UnifiedSerializer::with_compression();
        let data = TestData {
            name: "x".repeat(500),
            value: 1,
            items: vec!["item".to_string(); 100],
        };

        let serialized = serializer.serialize(&data).unwrap();
        let deserialized: TestData = serializer.deserialize(&serialized).unwrap();
        assert_eq!(data, deserialized);
    }

    #[cfg(feature = "compression")]
    #[test]
    fn test_compression_shrinks_repetitive_data() {
        let serializer = UnifiedSerializer::with_compression();
        let data = TestData {
            name: "x".repeat(1000),
            value: 1,
            items: vec!["same".to_string(); 200],
        };

        let plain = UnifiedSerializer::json().serialize(&data).unwrap();
        let compressed = serializer.serialize(&data).unwrap();
        assert!(compressed.len() < plain.len(), "压缩后应更小");
    }

    #[test]
    fn test_no_double_serialization_single_pass() {
        // 序列化结果应为纯 JSON 字节（首个字节是 '{' 而不是 base64 字符串的 '"'）
        let serializer = UnifiedSerializer::json();
        let data = test_data();
        let serialized = serializer.serialize(&data).unwrap();
        assert_eq!(serialized[0], b'{', "应直接序列化为 JSON 对象而非 base64 字符串");
    }
}
