//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 额外序列化模块
//!
//! 提供 MessagePack 和 CBOR 序列化支持。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::core::types::SerializationType;
use crate::error::Result;
use rmp_serde::{decode, encode};

/// MessagePack 序列化器
#[derive(Debug, Clone, Default)]
pub struct MessagePackSerializer;

impl MessagePackSerializer {
    pub fn new() -> Self {
        Self
    }
}

impl crate::infra::serialization::Serializer for MessagePackSerializer {
    fn serialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        encode::to_vec(data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }

    fn deserialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        decode::from_read(data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }
}

/// CBOR 序列化器
#[derive(Debug, Clone, Default)]
pub struct CborSerializer;

impl CborSerializer {
    pub fn new() -> Self {
        Self
    }
}

impl crate::infra::serialization::Serializer for CborSerializer {
    fn serialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(data, &mut buf).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        Ok(buf)
    }

    fn deserialize(&self, _type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        ciborium::from_reader(data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }
}

/// 序列化器注册表
///
/// 使用 serde_json::Value 作为中间类型来实现动态序列化器注册。
/// 使用 SerializationType 作为键类型，避免字符串键名不一致的问题。
#[derive(Default)]
pub struct SerializerRegistry {
    serializers: RwLock<HashMap<SerializationType, Arc<dyn ErasedSerializer>>>,
}

/// 擦除类型的序列化器 Trait
///
/// 使用 serde_json::Value 作为通用数据类型，支持动态分发。
pub trait ErasedSerializer: Send + Sync {
    /// 获取序列化器类型名称
    fn name(&self) -> &'static str;
    /// 序列化 JSON 值
    fn serialize(&self, value: &serde_json::Value) -> Result<Vec<u8>>;
    /// 反序列化为 JSON 值
    fn deserialize(&self, data: &[u8]) -> Result<serde_json::Value>;
}

impl ErasedSerializer for MessagePackSerializer {
    fn name(&self) -> &'static str {
        "messagepack"
    }

    fn serialize(&self, value: &serde_json::Value) -> Result<Vec<u8>> {
        encode::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }

    fn deserialize(&self, data: &[u8]) -> Result<serde_json::Value> {
        decode::from_read(data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }
}

impl ErasedSerializer for CborSerializer {
    fn name(&self) -> &'static str {
        "cbor"
    }

    fn serialize(&self, value: &serde_json::Value) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(value, &mut buf).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        Ok(buf)
    }

    fn deserialize(&self, data: &[u8]) -> Result<serde_json::Value> {
        ciborium::from_reader(data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }
}

impl SerializerRegistry {
    pub fn new() -> Self {
        Self {
            serializers: RwLock::new(HashMap::new()),
        }
    }

    /// 注册自定义序列化器
    pub async fn register(&self, format: SerializationType, serializer: Arc<dyn ErasedSerializer>) {
        let mut cache = self.serializers.write().await;
        cache.insert(format, serializer);
    }

    /// 获取序列化器
    pub async fn get(&self, format: &SerializationType) -> Option<Arc<dyn ErasedSerializer>> {
        let cache = self.serializers.read().await;
        cache.get(format).cloned()
    }

    /// 检查是否存在
    pub async fn contains(&self, format: &SerializationType) -> bool {
        let cache = self.serializers.read().await;
        cache.contains_key(format)
    }

    /// 移除序列化器
    pub async fn remove(&self, format: &SerializationType) -> bool {
        let mut cache = self.serializers.write().await;
        cache.remove(format).is_some()
    }

    /// 清空所有
    pub async fn clear(&self) {
        let mut cache = self.serializers.write().await;
        cache.clear();
    }

    /// 注册 MessagePack 序列化器
    pub async fn register_msgpack(&self) {
        self.register(SerializationType::Messagepack, Arc::new(MessagePackSerializer))
            .await;
    }

    /// 注册 CBOR 序列化器
    pub async fn register_cbor(&self) {
        self.register(SerializationType::Cbor, Arc::new(CborSerializer)).await;
    }

    /// 注册所有可用的额外序列化器
    pub async fn register_all(&self) {
        self.register_msgpack().await;
        self.register_cbor().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SerializationType;

    #[test]
    fn test_messagepack_serializer() {
        let serializer = MessagePackSerializer::new();

        // 测试序列化/反序列化
        let value = serde_json::json!("test_value");
        let bytes = serializer.serialize(&value).unwrap();
        let decoded: serde_json::Value = serializer.deserialize(&bytes).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_cbor_serializer() {
        let serializer = CborSerializer::new();

        // 测试序列化/反序列化
        let value = serde_json::json!("test_value");
        let bytes = serializer.serialize(&value).unwrap();
        let decoded: serde_json::Value = serializer.deserialize(&bytes).unwrap();
        assert_eq!(decoded, value);
    }

    #[tokio::test]
    async fn test_serializer_registry() {
        let registry = SerializerRegistry::new();

        // 初始为空
        assert!(!registry.contains(&SerializationType::Messagepack).await);

        // 注册
        registry
            .register(SerializationType::Messagepack, Arc::new(MessagePackSerializer))
            .await;
        assert!(registry.contains(&SerializationType::Messagepack).await);

        // 获取
        let serializer = registry.get(&SerializationType::Messagepack).await;
        assert!(serializer.is_some());
        assert_eq!(serializer.unwrap().name(), "messagepack");

        // 移除
        assert!(registry.remove(&SerializationType::Messagepack).await);
        assert!(!registry.contains(&SerializationType::Messagepack).await);

        // 清空
        registry
            .register(SerializationType::Cbor, Arc::new(CborSerializer))
            .await;
        registry.clear().await;
        assert!(!registry.contains(&SerializationType::Cbor).await);
    }

    #[tokio::test]
    async fn test_erased_serializer() {
        let registry = SerializerRegistry::new();
        registry.register_all().await;

        // 使用 MessagePack
        let serializer = registry.get(&SerializationType::Messagepack).await.unwrap();
        let json_value = serde_json::json!({"key": "value"});
        let bytes = serializer.serialize(&json_value).unwrap();
        let decoded: serde_json::Value = serializer.deserialize(&bytes).unwrap();
        assert_eq!(decoded, json_value);
    }
}
