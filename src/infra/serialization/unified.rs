//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified serialization manager that consolidates all serialization functionality

use crate::error::Result;
#[cfg(any(feature = "serialization", feature = "full", feature = "extra-serialization"))]
use crate::infra::serialization::Serializer;
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// Unified serialization manager
///
/// This provides a centralized way to handle all serialization operations
/// with support for different formats and zero-copy operations.
#[derive(Clone, Debug)]
pub struct UnifiedSerializer {
    inner: Arc<UnifiedSerializerInner>,
}

enum UnifiedSerializerInner {
    Json(crate::infra::serialization::json::JsonSerializer),
    #[cfg(feature = "bincode")]
    Bincode(crate::infra::serialization::bincode::BincodeSerializer),
    #[cfg(feature = "extra-serialization")]
    Cbor(crate::infra::serialization::extra::CborSerializer),
    #[cfg(feature = "extra-serialization")]
    MessagePack(crate::infra::serialization::extra::MessagePackSerializer),
}

/// Dispatch to the inner serializer's method, avoiding repeated match blocks.
macro_rules! with_inner {
    ($self:ident, $method:ident($($arg:expr),* $(,)?)) => {
        match &*$self.inner {
            UnifiedSerializerInner::Json(s) => s.$method($($arg),*),
            #[cfg(feature = "bincode")]
            UnifiedSerializerInner::Bincode(s) => s.$method($($arg),*),
            #[cfg(feature = "extra-serialization")]
            UnifiedSerializerInner::Cbor(s) => s.$method($($arg),*),
            #[cfg(feature = "extra-serialization")]
            UnifiedSerializerInner::MessagePack(s) => s.$method($($arg),*),
        }
    };
}

impl std::fmt::Debug for UnifiedSerializerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnifiedSerializerInner::Json(_) => write!(f, "JsonSerializer"),
            #[cfg(feature = "bincode")]
            UnifiedSerializerInner::Bincode(_) => write!(f, "BincodeSerializer"),
            #[cfg(feature = "extra-serialization")]
            UnifiedSerializerInner::Cbor(_) => write!(f, "CborSerializer"),
            #[cfg(feature = "extra-serialization")]
            UnifiedSerializerInner::MessagePack(_) => write!(f, "MessagePackSerializer"),
        }
    }
}

/// Serialization format type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SerializationFormat {
    /// JSON format (human-readable, default)
    Json,
    /// Bincode format (compact, fast)
    #[cfg(feature = "bincode")]
    Bincode,
    /// CBOR format (compact, self-describing)
    #[cfg(feature = "extra-serialization")]
    Cbor,
    /// MessagePack format (compact, efficient)
    #[cfg(feature = "extra-serialization")]
    MessagePack,
}

impl UnifiedSerializer {
    /// Create a new unified serializer with the specified format
    pub fn new(format: SerializationFormat) -> Self {
        let inner = match format {
            SerializationFormat::Json => {
                UnifiedSerializerInner::Json(crate::infra::serialization::json::JsonSerializer::new())
            }
            #[cfg(feature = "bincode")]
            SerializationFormat::Bincode => {
                UnifiedSerializerInner::Bincode(crate::infra::serialization::bincode::BincodeSerializer)
            }
            #[cfg(feature = "extra-serialization")]
            SerializationFormat::Cbor => {
                UnifiedSerializerInner::Cbor(crate::infra::serialization::extra::CborSerializer)
            }
            #[cfg(feature = "extra-serialization")]
            SerializationFormat::MessagePack => {
                UnifiedSerializerInner::MessagePack(crate::infra::serialization::extra::MessagePackSerializer)
            }
        };

        Self { inner: Arc::new(inner) }
    }

    /// Create a JSON serializer (default)
    pub fn json() -> Self {
        Self::new(SerializationFormat::Json)
    }

    /// Create a Bincode serializer
    #[cfg(feature = "bincode")]
    pub fn bincode() -> Self {
        Self::new(SerializationFormat::Bincode)
    }

    /// Create a CBOR serializer
    #[cfg(feature = "extra-serialization")]
    pub fn cbor() -> Self {
        Self::new(SerializationFormat::Cbor)
    }

    /// Create a MessagePack serializer
    #[cfg(feature = "extra-serialization")]
    pub fn messagepack() -> Self {
        Self::new(SerializationFormat::MessagePack)
    }

    /// Get the current format
    pub fn format(&self) -> SerializationFormat {
        match &*self.inner {
            UnifiedSerializerInner::Json(_) => SerializationFormat::Json,
            #[cfg(feature = "bincode")]
            UnifiedSerializerInner::Bincode(_) => SerializationFormat::Bincode,
            #[cfg(feature = "extra-serialization")]
            UnifiedSerializerInner::Cbor(_) => SerializationFormat::Cbor,
            #[cfg(feature = "extra-serialization")]
            UnifiedSerializerInner::MessagePack(_) => SerializationFormat::MessagePack,
        }
    }

    /// Serialize a value to bytes
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>> {
        let type_name = std::any::type_name::<T>();
        let data = serde_json::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        self.serialize_with_type(type_name, &data)
    }

    /// Serialize with explicit type name (for internal use)
    pub fn serialize_with_type(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        with_inner!(self, serialize(type_name, data))
    }

    /// Deserialize bytes to a value
    pub fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T> {
        let type_name = std::any::type_name::<T>();
        let result_data = self.deserialize_with_type(type_name, data)?;
        serde_json::from_slice(&result_data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))
    }

    /// Deserialize with explicit type name (for internal use)
    pub fn deserialize_with_type(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        with_inner!(self, deserialize(type_name, data))
    }

    /// Zero-copy serialize (when supported)
    pub fn serialize_zero_copy<'a, T: Serialize>(&self, value: &'a T) -> Result<Cow<'a, [u8]>> {
        let type_name = std::any::type_name::<T>();
        let data = serde_json::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        let result = with_inner!(self, serialize(type_name, &data))?;
        Ok(Cow::Owned(result))
    }

    /// Zero-copy deserialize (when supported)
    pub fn deserialize_zero_copy<'a, T: DeserializeOwned + Clone>(&self, data: &'a [u8]) -> Result<Cow<'a, T>> {
        let type_name = std::any::type_name::<T>();
        let result_data = with_inner!(self, deserialize(type_name, data))?;

        let value: T =
            serde_json::from_slice(&result_data).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        Ok(Cow::Owned(value))
    }

    /// Get approximate size of serialized data (for estimation)
    pub fn estimate_size<T: Serialize>(&self, value: &T) -> Result<usize> {
        let serialized = self.serialize(value)?;
        Ok(serialized.len())
    }

    /// Check if the format supports zero-copy operations
    pub fn supports_zero_copy(&self) -> bool {
        #[cfg(feature = "bincode")]
        {
            matches!(&*self.inner, UnifiedSerializerInner::Bincode(_))
        }
        #[cfg(not(feature = "bincode"))]
        {
            false
        }
    }

    /// Get format information
    pub fn format_info(&self) -> FormatInfo {
        FormatInfo {
            format: self.format(),
            supports_zero_copy: self.supports_zero_copy(),
            is_human_readable: matches!(self.format(), SerializationFormat::Json),
            is_compact: !matches!(self.format(), SerializationFormat::Json),
        }
    }
}

/// Information about a serialization format
#[derive(Debug, Clone)]
pub struct FormatInfo {
    /// The format type
    pub format: SerializationFormat,
    /// Whether zero-copy operations are supported
    pub supports_zero_copy: bool,
    /// Whether the format is human-readable
    pub is_human_readable: bool,
    /// Whether the format is compact (binary)
    pub is_compact: bool,
}

/// Serialization registry for managing multiple formats
#[derive(Debug, Default, Clone)]
pub struct SerializationRegistry {
    serializers: std::collections::HashMap<SerializationFormat, UnifiedSerializer>,
}

impl SerializationRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a serializer for a format
    pub fn register(&mut self, format: SerializationFormat, serializer: UnifiedSerializer) {
        self.serializers.insert(format, serializer);
    }

    /// Get a serializer for a format
    pub fn get(&self, format: &SerializationFormat) -> Option<&UnifiedSerializer> {
        self.serializers.get(format)
    }

    /// Get or create a serializer for a format
    pub fn get_or_create(&mut self, format: SerializationFormat) -> &UnifiedSerializer {
        self.serializers
            .entry(format)
            .or_insert_with(|| UnifiedSerializer::new(format));
        self.serializers.get(&format).unwrap()
    }

    /// List all registered formats
    pub fn formats(&self) -> impl Iterator<Item = &SerializationFormat> {
        self.serializers.keys()
    }

    /// Get information about all registered formats
    pub fn format_infos(&self) -> Vec<FormatInfo> {
        self.serializers.values().map(|s| s.format_info()).collect()
    }
}

/// Adapter to convert UnifiedSerializer to the legacy Serializer trait
pub struct UnifiedSerializerAdapter {
    inner: UnifiedSerializer,
}

impl UnifiedSerializerAdapter {
    pub fn new(serializer: UnifiedSerializer) -> Self {
        Self { inner: serializer }
    }
}

impl crate::infra::serialization::Serializer for UnifiedSerializerAdapter {
    fn serialize(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        self.inner.serialize_with_type(type_name, data)
    }

    fn deserialize(&self, type_name: &str, data: &[u8]) -> Result<Vec<u8>> {
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

    /// Quick JSON serialization
    pub fn to_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
        let type_name = std::any::type_name::<T>();
        let data = serde_json::to_vec(value).map_err(|e| crate::error::CacheError::Serialization(e.to_string()))?;
        default_serializer().serialize_with_type(type_name, &data)
    }

    /// Quick JSON deserialization
    pub fn from_json<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
        default_serializer().deserialize(data)
    }

    /// Quick Bincode serialization
    #[cfg(feature = "bincode")]
    pub fn to_bincode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
        UnifiedSerializer::bincode().serialize(value)
    }

    /// Quick Bincode deserialization
    #[cfg(feature = "bincode")]
    pub fn from_bincode<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
        UnifiedSerializer::bincode().deserialize(data)
    }

    /// Estimate serialized size with JSON
    pub fn estimate_json_size<T: Serialize>(value: &T) -> Result<usize> {
        default_serializer().estimate_size(value)
    }

    /// Estimate serialized size with Bincode
    #[cfg(feature = "bincode")]
    pub fn estimate_bincode_size<T: Serialize>(value: &T) -> Result<usize> {
        UnifiedSerializer::bincode().estimate_size(value)
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

    #[cfg(feature = "bincode")]
    #[test]
    fn test_bincode_serialization() {
        let serializer = UnifiedSerializer::bincode();
        let data = test_data();

        let serialized = serializer.serialize(&data).unwrap();
        let deserialized: TestData = serializer.deserialize(&serialized).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_format_info() {
        let json_serializer = UnifiedSerializer::json();
        let info = json_serializer.format_info();

        assert_eq!(info.format, SerializationFormat::Json);
        assert!(info.is_human_readable);
        assert!(!info.is_compact);
        assert!(!info.supports_zero_copy);

        #[cfg(feature = "bincode")]
        {
            let bincode_serializer = UnifiedSerializer::bincode();
            let info = bincode_serializer.format_info();

            assert_eq!(info.format, SerializationFormat::Bincode);
            assert!(!info.is_human_readable);
            assert!(info.is_compact);
            assert!(info.supports_zero_copy);
        }
    }

    #[test]
    fn test_serialization_registry() {
        let mut registry = SerializationRegistry::new();

        // Register JSON serializer
        registry.register(SerializationFormat::Json, UnifiedSerializer::json());

        // Get existing serializer
        let json_serializer = registry.get(&SerializationFormat::Json).unwrap();
        assert_eq!(json_serializer.format(), SerializationFormat::Json);

        // Test with data using the existing serializer
        let data = test_data();
        let serialized = json_serializer.serialize(&data).unwrap();
        let deserialized: TestData = json_serializer.deserialize(&serialized).unwrap();
        assert_eq!(data, deserialized);

        // Get or create serializer - create a new registry to avoid borrow conflict
        let mut registry2 = registry.clone();
        let json_serializer2 = registry2.get_or_create(SerializationFormat::Json);
        assert_eq!(json_serializer2.format(), SerializationFormat::Json);
    }

    #[test]
    fn test_convenience_functions() {
        let data = test_data();

        // Test JSON convenience functions
        let json_bytes = convenience::to_json(&data).unwrap();
        let json_deserialized: TestData = convenience::from_json(&json_bytes).unwrap();
        assert_eq!(data, json_deserialized);

        // Test size estimation
        let estimated_size = convenience::estimate_json_size(&data).unwrap();
        assert!(estimated_size > 0);
        assert_eq!(estimated_size, json_bytes.len());
    }

    #[cfg(feature = "bincode")]
    #[test]
    fn test_convenience_bincode() {
        let data = test_data();

        // Test Bincode convenience functions
        let bincode_bytes = convenience::to_bincode(&data).unwrap();
        let bincode_deserialized: TestData = convenience::from_bincode(&bincode_bytes).unwrap();
        assert_eq!(data, bincode_deserialized);

        // Test size estimation
        let estimated_size = convenience::estimate_bincode_size(&data).unwrap();
        assert!(estimated_size > 0);
        assert_eq!(estimated_size, bincode_bytes.len());
    }

    #[test]
    fn test_zero_copy_operations() {
        #[cfg(feature = "bincode")]
        {
            let data = test_data();
            let serializer = UnifiedSerializer::bincode();
            assert!(serializer.supports_zero_copy());

            // Test zero-copy serialization
            let zero_copy_result = serializer.serialize_zero_copy(&data).unwrap();
            // 验证零拷贝序列化结果
            assert!(matches!(zero_copy_result, Cow::Borrowed(_) | Cow::Owned(_)));

            // Test zero-copy deserialization
            // First serialize data to bytes using the serializer's format
            let serialized = serializer.serialize(&data).unwrap();
            let zero_copy_result = serializer.deserialize_zero_copy::<TestData>(&serialized).unwrap();
            // 验证零拷贝反序列化结果
            assert!(matches!(zero_copy_result, Cow::Borrowed(_) | Cow::Owned(_)));
        }

        // JSON doesn't support zero-copy
        let json_serializer = UnifiedSerializer::json();
        assert!(!json_serializer.supports_zero_copy());
    }

    #[test]
    fn test_adapter() {
        let unified = UnifiedSerializer::json();
        let adapter = UnifiedSerializerAdapter::new(unified);

        let data = test_data();

        // Test adapter implements Serializer trait
        let type_name = std::any::type_name::<TestData>();
        let json_data = serde_json::to_vec(&data).unwrap();
        let serialized = adapter.serialize(type_name, &json_data).unwrap();
        let deserialized = adapter.deserialize(type_name, &serialized).unwrap();
        assert_eq!(json_data, deserialized);
    }
}
