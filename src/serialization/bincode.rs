//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了Bincode序列化器的实现。

use super::Serializer;
use crate::error::{CacheError, Result};
use serde::{de::DeserializeOwned, Serialize};

/// Bincode序列化器
///
/// 实现基于bincode的序列化和反序列化
#[derive(Clone)]
pub struct BincodeSerializer;

impl Serializer for BincodeSerializer {
    /// 序列化值为Bincode字节数组
    ///
    /// # 参数
    ///
    /// * `value` - 要序列化的值
    ///
    /// # 返回值
    ///
    /// 返回序列化后的字节数组或错误
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        bincode::serialize(value).map_err(|e| CacheError::Serialization(e.to_string()))
    }

    /// 从Bincode字节数组反序列化值
    ///
    /// # 参数
    ///
    /// * `data` - 要反序列化的字节数组
    ///
    /// # 返回值
    ///
    /// 返回反序列化后的值或错误
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        bincode::deserialize(data).map_err(|e| CacheError::Serialization(e.to_string()))
    }
}
