//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了JSON序列化器的实现。

use super::Serializer;
use crate::error::{CacheError, Result};
use serde::{de::DeserializeOwned, Serialize};

/// JSON序列化器
///
/// 实现基于serde_json的序列化和反序列化
#[derive(Clone)]
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
            #[cfg(feature = "flate2")]
            {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;

                let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
                encoder
                    .write_all(&json_bytes)
                    .map_err(|e| CacheError::Serialization(e.to_string()))?;
                encoder
                    .finish()
                    .map_err(|e| CacheError::Serialization(e.to_string()))
            }

            #[cfg(not(feature = "flate2"))]
            {
                // 如果没有启用flate2特性，返回未压缩的数据
                Ok(json_bytes)
            }
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
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        let json_bytes = if self.compress {
            // 解压缩
            #[cfg(feature = "flate2")]
            {
                use flate2::read::GzDecoder;
                use std::io::Read;

                let mut decoder = GzDecoder::new(data);
                let mut decoded = Vec::new();
                decoder
                    .read_to_end(&mut decoded)
                    .map_err(|e| CacheError::Serialization(e.to_string()))?;
                decoded
            }

            #[cfg(not(feature = "flate2"))]
            {
                // 如果没有启用flate2特性，直接使用原始数据
                data.to_vec()
            }
        } else {
            data.to_vec()
        };

        serde_json::from_slice(&json_bytes).map_err(|e| CacheError::Serialization(e.to_string()))
    }
}
