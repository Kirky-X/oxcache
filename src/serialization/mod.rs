//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的序列化机制，支持多种序列化格式。

#[cfg(feature = "bincode")]
pub mod bincode;
pub mod json;

use crate::error::Result;
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Cow;

pub use json::JsonSerializer;

#[cfg(feature = "bincode")]
pub use bincode::BincodeSerializer;

/// 序列化器特征
///
/// 定义序列化和反序列化操作的接口
pub trait Serializer: Send + Sync {
    /// 序列化值为字节数组
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>>;

    /// 从字节数组反序列化值
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T>;
}

/// 零拷贝序列化器特征
///
/// 提供零拷贝序列化和反序列化操作，使用 Cow<[u8]> 避免不必要的内存分配
pub trait ZeroCopySerializer: Send + Sync {
    /// 零拷贝序列化（返回 Cow 以避免不必要的克隆）
    ///
    /// 某些序列化格式（如 bincode）可以实现真正的零拷贝序列化
    fn serialize_zero_copy<'a, T: Serialize>(&self, value: &'a T) -> Result<Cow<'a, [u8]>>;

    /// 零拷贝反序列化
    ///
    /// 某些场景下可以避免数据拷贝
    fn deserialize_zero_copy<'a, T: DeserializeOwned + Clone>(
        &self,
        data: &'a [u8],
    ) -> Result<Cow<'a, T>>;
}

/// 序列化器枚举
///
/// 用于支持多态的序列化器
#[derive(Clone, Debug)]
pub enum SerializerEnum {
    Json(JsonSerializer),
    #[cfg(feature = "bincode")]
    Bincode(bincode::BincodeSerializer),
}

impl Serializer for SerializerEnum {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        match self {
            SerializerEnum::Json(s) => s.serialize(value),
            #[cfg(feature = "bincode")]
            SerializerEnum::Bincode(s) => s.serialize(value),
        }
    }

    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        match self {
            SerializerEnum::Json(s) => s.deserialize(data),
            #[cfg(feature = "bincode")]
            SerializerEnum::Bincode(s) => s.deserialize(data),
        }
    }
}
