// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了JSON序列化器的实现。

use super::depth_limited::{would_exceed_depth_limit, MAX_DESERIALIZE_DEPTH};
use super::utils::{check_data_size, compress_data, decompress_data};
use super::Serializer;
use crate::error::{CacheError, Result};
use serde::{Deserialize, Serialize};

/// JSON序列化器
///
/// 实现基于serde_json的序列化和反序列化
#[derive(Clone, Debug)]
pub struct JsonSerializer {
    /// 是否启用压缩
    compress: bool,
}

/// 最大JSON反序列化大小限制（5MB）
const MAX_JSON_SIZE: usize = 5 * 1024 * 1024;

/// JSON 字节数组包装器，用于 base64 编码
mod byte_array {
    use base64::prelude::*;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = BASE64_STANDARD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        BASE64_STANDARD.decode(&encoded).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize)]
struct ByteArrayWrapper(#[serde(with = "byte_array")] Vec<u8>);

impl JsonSerializer {
    /// 创建新的JSON序列化器
    pub fn new() -> Self {
        Self { compress: false }
    }

    /// 创建启用压缩的JSON序列化器
    pub fn with_compression() -> Self {
        Self { compress: true }
    }
}

impl Default for JsonSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl Serializer for JsonSerializer {
    /// 序列化值为JSON字节数组
    ///
    /// # 参数
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要序列化的字节数组
    ///
    /// # 返回值
    ///
    /// 返回序列化后的字节数组或错误
    fn serialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        let wrapper = ByteArrayWrapper(data.to_vec());
        let json_bytes = serde_json::to_vec(&wrapper).map_err(|e| CacheError::Serialization(e.to_string()))?;

        if self.compress {
            compress_data(&json_bytes)
        } else {
            Ok(json_bytes)
        }
    }

    /// 从JSON字节数组反序列化值
    ///
    /// # 参数
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要反序列化的字节数组
    ///
    /// # 返回值
    ///
    /// 返回反序列化后的字节数组或错误
    ///
    /// # 安全
    ///
    /// 此方法限制反序列化数据的大小和深度，防止拒绝服务攻击
    fn deserialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        check_data_size(data, MAX_JSON_SIZE, "JSON")?;

        let json_bytes = if self.compress {
            decompress_data(data)?
        } else {
            data.to_vec()
        };

        if would_exceed_depth_limit(&json_bytes, MAX_DESERIALIZE_DEPTH)
            .map_err(|e| CacheError::Serialization(e.to_string()))?
        {
            return Err(CacheError::InvalidInput(format!(
                "JSON 嵌套深度超过最大限制 {}",
                MAX_DESERIALIZE_DEPTH
            )));
        }

        let wrapper: ByteArrayWrapper =
            serde_json::from_slice(&json_bytes).map_err(|e| CacheError::Serialization(e.to_string()))?;

        Ok(wrapper.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_serialization() {
        let serializer = JsonSerializer::new();
        let data = vec![0, 1, 2, 255, 254, 253];

        let serialized = serializer.serialize("test", &data).unwrap();

        let json_str = String::from_utf8(serialized.clone()).unwrap();
        assert!(json_str.contains("\"")); // 应该是字符串而不是数组

        let deserialized = serializer.deserialize("test", &serialized).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_base64_vs_array_size() {
        let serializer = JsonSerializer::new();
        let data: Vec<u8> = (0..=255).collect();

        let serialized = serializer.serialize("test", &data).unwrap();

        let old_size = data.len() * 4;
        let new_size = serialized.len();

        assert!(new_size < old_size, "base64 编码应该比 JSON 数组更小");
    }

    #[test]
    fn test_compression() {
        let serializer = JsonSerializer::with_compression();
        let data = vec![0u8; 1000];

        let serialized = serializer.serialize("test", &data).unwrap();
        let deserialized = serializer.deserialize("test", &serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_empty_bytes() {
        let serializer = JsonSerializer::new();
        let data: Vec<u8> = vec![];

        let serialized = serializer.serialize("test", &data).unwrap();
        let deserialized = serializer.deserialize("test", &serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_max_size_limit() {
        let serializer = JsonSerializer::new();
        let large_data = vec![0u8; MAX_JSON_SIZE + 1];

        let result = serializer.deserialize("test", &large_data);
        assert!(result.is_err());
    }
}
