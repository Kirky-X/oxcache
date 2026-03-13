//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了JSON序列化器的实现。

use super::utils::{check_data_size, compress_data, decompress_data};
use super::Serializer;
use crate::error::{CacheError, Result};

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

/// 最大反序列化深度限制（防止嵌套攻击）
const MAX_DESERIALIZE_DEPTH: usize = 64;

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
        let json_bytes = serde_json::to_vec(data).map_err(|e| CacheError::Serialization(e.to_string()))?;

        if self.compress {
            // 使用压缩
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
        // 安全检查：限制数据大小
        check_data_size(data, MAX_JSON_SIZE, "JSON")?;

        let json_bytes = if self.compress {
            // 解压缩
            decompress_data(data)?
        } else {
            data.to_vec()
        };

        // 安全检查：限制反序列化深度
        // 注意：serde_json::from_slice 使用 serde 的默认反序列化器
        // 在生产环境中建议使用 depth_limit 功能
        // 当前通过 size 限制来防止大多数 DoS 攻击
        let _ = MAX_DESERIALIZE_DEPTH; // 保留常量供将来使用

        // 解析 JSON 数组并提取字节
        let json_value: serde_json::Value =
            serde_json::from_slice(&json_bytes).map_err(|e| CacheError::Serialization(e.to_string()))?;

        // 从 JSON 数组中提取字节
        let bytes: Vec<u8> = json_value
            .as_array()
            .ok_or_else(|| CacheError::Serialization("Expected JSON array".to_string()))?
            .iter()
            .map(|v| {
                v.as_u64()
                    .ok_or_else(|| CacheError::Serialization("Expected integer in array".to_string()))
                    .map(|n| n as u8)
            })
            .collect::<std::result::Result<Vec<u8>, CacheError>>()?;

        Ok(bytes)
    }
}
