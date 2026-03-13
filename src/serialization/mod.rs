//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的序列化机制，支持多种序列化格式。

#[cfg(feature = "bincode")]
pub mod bincode;
pub mod depth_limited;
pub mod json;

#[cfg(feature = "extra-serialization")]
pub mod extra;

#[cfg(feature = "serialization-cache")]
pub mod cache;

pub mod unified;
pub mod utils;

use crate::error::Result;

pub use json::JsonSerializer;

#[cfg(feature = "bincode")]
pub use bincode::BincodeSerializer;

#[cfg(feature = "extra-serialization")]
pub use extra::{CborSerializer, MessagePackSerializer, SerializerRegistry};

#[cfg(feature = "serialization-cache")]
pub use cache::{SerializationCache, SerializationCacheConfig, SerializationCacheStats};

// Unified serialization exports
pub use unified::{
    convenience, default_serializer, FormatInfo, SerializationFormat, SerializationRegistry, UnifiedSerializer,
    UnifiedSerializerAdapter,
};

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
    fn serialize(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            SerializerEnum::Json(s) => s.serialize(type_name, data),
            #[cfg(feature = "bincode")]
            SerializerEnum::Bincode(s) => s.serialize(type_name, data),
        }
    }

    fn deserialize(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            SerializerEnum::Json(s) => s.deserialize(type_name, data),
            #[cfg(feature = "bincode")]
            SerializerEnum::Bincode(s) => s.deserialize(type_name, data),
        }
    }

    fn serialize_zero_copy(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            SerializerEnum::Json(s) => s.serialize_zero_copy(type_name, data),
            #[cfg(feature = "bincode")]
            SerializerEnum::Bincode(s) => s.serialize_zero_copy(type_name, data),
        }
    }

    fn deserialize_zero_copy(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            SerializerEnum::Json(s) => s.deserialize_zero_copy(type_name, data),
            #[cfg(feature = "bincode")]
            SerializerEnum::Bincode(s) => s.deserialize_zero_copy(type_name, data),
        }
    }
}
