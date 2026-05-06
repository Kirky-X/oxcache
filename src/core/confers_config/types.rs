// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration types (enums)

use serde::{Deserialize, Serialize};

/// 后端类型枚举
/// Backend type for configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConfigBackendType {
    /// Memory backend only (L1)
    Memory,
    /// Redis backend only (L2)
    Redis,
    /// Tiered backend (L1 + L2)
    Tiered,
}

impl Default for ConfigBackendType {
    #[inline]
    fn default() -> Self {
        ConfigBackendType::Memory
    }
}

impl std::fmt::Display for ConfigBackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigBackendType::Memory => write!(f, "Memory"),
            ConfigBackendType::Redis => write!(f, "Redis"),
            ConfigBackendType::Tiered => write!(f, "Tiered"),
        }
    }
}

impl std::str::FromStr for ConfigBackendType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Memory" => Ok(ConfigBackendType::Memory),
            "Redis" => Ok(ConfigBackendType::Redis),
            "Tiered" => Ok(ConfigBackendType::Tiered),
            _ => Err(format!("Unknown backend type: {}", s)),
        }
    }
}

/// 缓存类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CacheType {
    /// 仅 L1 (内存) 缓存
    L1,
    /// 仅 L2 (Redis) 缓存
    L2,
    /// 两级缓存 (L1 + L2)
    TwoLevel,
}

impl Default for CacheType {
    #[inline]
    fn default() -> Self {
        CacheType::L1
    }
}

impl std::fmt::Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheType::L1 => write!(f, "L1"),
            CacheType::L2 => write!(f, "L2"),
            CacheType::TwoLevel => write!(f, "TwoLevel"),
        }
    }
}

impl std::str::FromStr for CacheType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "L1" => Ok(CacheType::L1),
            "L2" => Ok(CacheType::L2),
            "TwoLevel" => Ok(CacheType::TwoLevel),
            _ => Err(format!("Unknown cache type: {}", s)),
        }
    }
}

/// 配置文件格式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML 格式
    Toml,
    /// JSON 格式
    Json,
}

impl ConfigFormat {
    /// 从文件路径检测格式
    pub fn from_path(path: &str) -> Option<Self> {
        use std::path::Path;
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext {
                "toml" => Some(ConfigFormat::Toml),
                "json" => Some(ConfigFormat::Json),
                _ => None,
            })
    }

    /// 获取文件扩展名
    #[inline]
    pub fn extension(&self) -> &str {
        match self {
            ConfigFormat::Toml => "toml",
            ConfigFormat::Json => "json",
        }
    }

    /// 获取 MIME 类型
    #[inline]
    pub fn mime_type(&self) -> &str {
        match self {
            ConfigFormat::Toml => "application/toml",
            ConfigFormat::Json => "application/json",
        }
    }
}
