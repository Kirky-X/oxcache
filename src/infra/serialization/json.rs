// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了JSON序列化器的实现。

use super::Serializer;
use super::utils::{check_data_size, compress_data, decompress_data_with_limit};
use crate::core::MAX_JSON_SIZE;
use crate::error::OxCacheResult;

/// JSON序列化器
///
/// 直接存储原始字节（不再做 base64 包装），可选启用 gzip 压缩。
/// 数据即存即取：序列化时不解析 JSON 文本，反序列化时也不做深度检查
/// （深度检查由 typed 反序列化路径统一负责），避免双重序列化开销。
///
/// # 命名说明
///
/// 尽管名为 `JsonSerializer`，此序列化器实际上是一个**原始字节透传器**，
/// 不执行任何 JSON 解析或生成。名称保留用于向后兼容。
/// 它仅提供可选的 gzip 压缩/解压功能。
#[derive(Clone, Debug)]
pub struct JsonSerializer {
    /// 是否启用压缩
    compress: bool,
}

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
    /// 序列化值为字节数组（可选压缩，不做 base64/JSON 包装）
    ///
    /// # 参数
    ///
    /// * `type_name` - 类型名称（用于记录）
    /// * `data` - 要序列化的字节数组
    ///
    /// # 返回值
    ///
    /// 返回序列化后的字节数组或错误
    fn serialize(&self, _type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        if self.compress {
            compress_data(data)
        } else {
            Ok(data.to_vec())
        }
    }

    /// 从字节数组反序列化值（可选解压，不做 JSON 解析）
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
    /// 此方法限制反序列化数据的大小和解压输出大小，防止拒绝服务攻击。
    fn deserialize(&self, _type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        check_data_size(data, MAX_JSON_SIZE, "JSON")?;

        if self.compress {
            decompress_data_with_limit(data, super::utils::MAX_DECOMPRESS_SIZE)
        } else {
            Ok(data.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_bytes_round_trip() {
        let serializer = JsonSerializer::new();
        let data = vec![0, 1, 2, 255, 254, 253];

        let serialized = serializer.serialize("test", &data).unwrap();
        assert_eq!(serialized, data); // 原始字节直接存储，无膨胀

        let deserialized = serializer.deserialize("test", &serialized).unwrap();
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_raw_bytes_not_base64_encoded() {
        // 不再经过 base64：序列化结果就是原始字节，不含 JSON 字符串包装
        let serializer = JsonSerializer::new();
        let data = vec![0, 1, 2, 255, 254, 253];

        let serialized = serializer.serialize("test", &data).unwrap();
        assert_eq!(serialized.len(), data.len(), "不应有 base64 膨胀 (4/3)");
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
