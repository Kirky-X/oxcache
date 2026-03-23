//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 类型安全的枚举定义，用于替代硬编码字符串常量

use serde::{Deserialize, Serialize};

/// Redis 连接模式
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisModeType {
    /// 单机模式
    #[default]
    Standalone,
    /// 哨兵模式
    Sentinel,
    /// 集群模式
    Cluster,
}

impl std::fmt::Display for RedisModeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standalone => write!(f, "standalone"),
            Self::Sentinel => write!(f, "sentinel"),
            Self::Cluster => write!(f, "cluster"),
        }
    }
}

/// 缓存后端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
    /// Moka 内存缓存
    Moka,
    /// DashMap 内存缓存
    Dashmap,
    /// Redis 分布式缓存
    Redis,
    /// 分层缓存（L1 + L2）
    Tiered,
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Moka => write!(f, "moka"),
            Self::Dashmap => write!(f, "dashmap"),
            Self::Redis => write!(f, "redis"),
            Self::Tiered => write!(f, "tiered"),
        }
    }
}

/// 缓存层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheLayer {
    /// L1 内存缓存
    L1,
    /// L2 分布式缓存
    L2,
}

impl std::fmt::Display for CacheLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L1 => write!(f, "L1"),
            Self::L2 => write!(f, "L2"),
        }
    }
}

/// 序列化格式类型
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerializationType {
    /// JSON 格式
    #[default]
    Json,
    /// Bincode 格式
    #[cfg(feature = "bincode")]
    Bincode,
    /// CBOR 格式
    #[cfg(feature = "extra-serialization")]
    Cbor,
    /// MessagePack 格式
    #[cfg(feature = "extra-serialization")]
    Messagepack,
}

impl std::fmt::Display for SerializationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            #[cfg(feature = "bincode")]
            Self::Bincode => write!(f, "bincode"),
            #[cfg(feature = "extra-serialization")]
            Self::Cbor => write!(f, "cbor"),
            #[cfg(feature = "extra-serialization")]
            Self::Messagepack => write!(f, "messagepack"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_mode_type_default() {
        assert_eq!(RedisModeType::default(), RedisModeType::Standalone);
    }

    #[test]
    fn test_redis_mode_type_serialize() {
        let mode = RedisModeType::Sentinel;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"sentinel\"");
    }

    #[test]
    fn test_redis_mode_type_deserialize() {
        let mode: RedisModeType = serde_json::from_str("\"cluster\"").unwrap();
        assert_eq!(mode, RedisModeType::Cluster);
    }

    #[test]
    fn test_redis_mode_type_display() {
        assert_eq!(format!("{}", RedisModeType::Standalone), "standalone");
        assert_eq!(format!("{}", RedisModeType::Sentinel), "sentinel");
        assert_eq!(format!("{}", RedisModeType::Cluster), "cluster");
    }

    #[test]
    fn test_backend_type_serialize() {
        let backend = BackendType::Moka;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "\"moka\"");
    }

    #[test]
    fn test_backend_type_deserialize() {
        let backend: BackendType = serde_json::from_str("\"redis\"").unwrap();
        assert_eq!(backend, BackendType::Redis);
    }

    #[test]
    fn test_cache_layer_serialize() {
        let layer = CacheLayer::L1;
        let json = serde_json::to_string(&layer).unwrap();
        assert_eq!(json, "\"L1\"");
    }

    #[test]
    fn test_cache_layer_deserialize() {
        let layer: CacheLayer = serde_json::from_str("\"L2\"").unwrap();
        assert_eq!(layer, CacheLayer::L2);
    }

    #[test]
    fn test_serialization_type_default() {
        assert_eq!(SerializationType::default(), SerializationType::Json);
    }
}
