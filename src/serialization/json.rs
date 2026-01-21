//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了JSON序列化器的实现。

use super::utils::{check_data_size, compress_data, decompress_data};
use super::Serializer;
use crate::error::{CacheError, Result};
use serde::{de::DeserializeOwned, Serialize};

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
    /// * `value` - 要序列化的值
    ///
    /// # 返回值
    ///
    /// 返回序列化后的字节数组或错误
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        let json_bytes =
            serde_json::to_vec(value).map_err(|e| CacheError::Serialization(e.to_string()))?;

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
    /// * `data` - 要反序列化的字节数组
    ///
    /// # 返回值
    ///
    /// 返回反序列化后的值或错误
    ///
    /// # 安全
    ///
    /// 此方法限制反序列化数据的大小，防止拒绝服务攻击
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        // 安全检查：限制数据大小
        check_data_size(data, MAX_JSON_SIZE, "JSON")?;

        let json_bytes = if self.compress {
            // 解压缩
            decompress_data(data)?
        } else {
            data.to_vec()
        };

        serde_json::from_slice(&json_bytes).map_err(|e| CacheError::Serialization(e.to_string()))
    }
}
