// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Unified serialization manager

use crate::error::OxCacheResult;
use crate::infra::{JsonSerializer, Serializer};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

/// Json-only unified serializer
#[derive(Clone, Debug)]
pub struct UnifiedSerializer {
    inner: Arc<JsonSerializer>,
}

impl UnifiedSerializer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(JsonSerializer::new()),
        }
    }

    pub fn json() -> Self {
        Self::new()
    }

    /// Serialize a value to bytes
    pub fn serialize<T: Serialize>(&self, value: &T) -> OxCacheResult<Vec<u8>> {
        let type_name = std::any::type_name::<T>();
        let data = serde_json::to_vec(value).map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))?;
        self.inner.serialize(type_name, &data)
    }

    /// Serialize with explicit type name (for internal use)
    pub fn serialize_with_type(&self, type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        self.inner.serialize(type_name, data)
    }

    /// Deserialize bytes to a value
    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> OxCacheResult<T> {
        let type_name = std::any::type_name::<T>();
        let result_data = self.inner.deserialize(type_name, data)?;
        serde_json::from_slice(&result_data).map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))
    }

    /// Deserialize with explicit type name (for internal use)
    pub fn deserialize_with_type(&self, type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        self.inner.deserialize(type_name, data)
    }

    /// Get approximate size of serialized data
    pub fn estimate_size<T: Serialize>(&self, value: &T) -> OxCacheResult<usize> {
        let serialized = self.serialize(value)?;
        Ok(serialized.len())
    }
}

impl Default for UnifiedSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter to convert UnifiedSerializer to the Serializer trait
pub struct UnifiedSerializerAdapter {
    inner: UnifiedSerializer,
}

impl UnifiedSerializerAdapter {
    pub fn new(serializer: UnifiedSerializer) -> Self {
        Self { inner: serializer }
    }
}

impl Serializer for UnifiedSerializerAdapter {
    fn serialize(&self, type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        self.inner.serialize_with_type(type_name, data)
    }

    fn deserialize(&self, type_name: &str, data: &[u8]) -> OxCacheResult<Vec<u8>> {
        self.inner.deserialize_with_type(type_name, data)
    }
}

/// Default serializer instance
pub fn default_serializer() -> UnifiedSerializer {
    UnifiedSerializer::json()
}

/// Convenience functions for common operations
pub mod convenience {
    use super::*;

    pub fn to_json<T: Serialize>(value: &T) -> OxCacheResult<Vec<u8>> {
        let type_name = std::any::type_name::<T>();
        let data = serde_json::to_vec(value).map_err(|e| crate::error::OxCacheError::Serialization(e.to_string()))?;
        default_serializer().serialize_with_type(type_name, &data)
    }

    pub fn from_json<T: DeserializeOwned>(data: &[u8]) -> OxCacheResult<T> {
        default_serializer().deserialize(data)
    }

    pub fn estimate_json_size<T: Serialize>(value: &T) -> OxCacheResult<usize> {
        default_serializer().estimate_size(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    struct TestData {
        name: String,
        value: i32,
        items: Vec<String>,
    }

    fn test_data() -> TestData {
        TestData {
            name: "test".to_string(),
            value: 42,
            items: vec!["item1".to_string(), "item2".to_string()],
        }
    }

    #[test]
    fn test_json_serialization() {
        let serializer = UnifiedSerializer::json();
        let data = test_data();

        let serialized = serializer.serialize(&data).unwrap();
        let deserialized: TestData = serializer.deserialize(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_convenience_functions() {
        let data = test_data();

        let json_bytes = convenience::to_json(&data).unwrap();
        let json_deserialized: TestData = convenience::from_json(&json_bytes).unwrap();
        assert_eq!(data, json_deserialized);

        let estimated_size = convenience::estimate_json_size(&data).unwrap();
        assert!(estimated_size > 0);
        assert_eq!(estimated_size, json_bytes.len());
    }

    #[test]
    fn test_adapter() {
        let unified = UnifiedSerializer::json();
        let adapter = UnifiedSerializerAdapter::new(unified);

        let data = test_data();
        let type_name = std::any::type_name::<TestData>();
        let json_data = serde_json::to_vec(&data).unwrap();
        let serialized = adapter.serialize(type_name, &json_data).unwrap();
        let deserialized = adapter.deserialize(type_name, &serialized).unwrap();
        assert_eq!(json_data, deserialized);
    }
}
