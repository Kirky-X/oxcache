//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的序列化机制，支持多种序列化格式。

pub mod json;

use crate::error::Result;
use serde::{de::DeserializeOwned, Serialize};

pub use json::JsonSerializer;

/// 序列化器特征
///
/// 定义序列化和反序列化操作的接口
pub trait Serializer: Send + Sync {
    /// 序列化值为字节数组
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>>;

    /// 从字节数组反序列化值
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T>;
}

/// 序列化器枚举
///
/// 用于支持 trait object 的序列化器
#[derive(Clone)]
pub enum SerializerEnum {
    Json(JsonSerializer),
}

impl Serializer for SerializerEnum {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        match self {
            SerializerEnum::Json(s) => s.serialize(value),
        }
    }

    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        match self {
            SerializerEnum::Json(s) => s.deserialize(data),
        }
    }
}
