//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! confers 宏模块

#[cfg(feature = "confers")]
use super::ConfigSource;
#[cfg(feature = "confers")]
use crate::OxcacheConfig;
#[cfg(feature = "confers")]
use serde::Deserialize;
#[cfg(feature = "confers")]
use std::collections::HashMap;
#[cfg(feature = "confers")]
use std::fs;
#[cfg(feature = "confers")]
use std::path::{Path, PathBuf};

#[cfg(all(feature = "confers", feature = "config-toml"))]
use toml;

/// 完整配置（confers 版本）
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct OxcacheConfigFile {
    pub global: Option<GlobalConfigConfers>,
    pub services: Option<Vec<ServiceConfigItem>>,
}

/// 全局配置（confers 版本）
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct GlobalConfigConfers {
    pub default_ttl: Option<u64>,
    pub health_check_interval: Option<u64>,
    pub serialization: Option<String>,
    pub enable_metrics: Option<bool>,
}

/// 服务配置项
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfigItem {
    pub name: String,
    pub cache_type: Option<String>,
    pub ttl: Option<u64>,
    pub serialization: Option<String>,
    #[cfg(feature = "l1-moka")]
    pub l1: Option<L1ConfigConfers>,
    #[cfg(feature = "l2-redis")]
    pub l2: Option<L2ConfigConfers>,
    #[cfg(feature = "l2-redis")]
    pub two_level: Option<TwoLevelConfigConfers>,
}

/// L1 配置（需要 l1-moka feature）
#[cfg(all(feature = "confers", feature = "l1-moka"))]
#[derive(Debug, Clone, Deserialize)]
pub struct L1ConfigConfers {
    pub max_capacity: Option<u64>,
    pub max_key_length: Option<usize>,
    pub max_value_size: Option<usize>,
    pub cleanup_interval_secs: Option<u64>,
    pub eviction_policy: Option<String>,
}

/// L2 配置（需要 l2-redis feature）
#[cfg(all(feature = "confers", feature = "l2-redis"))]
#[derive(Debug, Clone, Deserialize)]
pub struct L2ConfigConfers {
    pub mode: Option<String>,
    pub connection_string: Option<String>,
    pub connection_timeout_ms: Option<u64>,
    pub command_timeout_ms: Option<u64>,
    pub password: Option<String>,
    pub enable_tls: Option<bool>,
    pub default_ttl: Option<u64>,
}

/// 双层缓存配置（需要 l2-redis feature）
#[cfg(all(feature = "confers", feature = "l2-redis"))]
#[derive(Debug, Clone, Deserialize)]
pub struct TwoLevelConfigConfers {
    pub promote_on_hit: Option<bool>,
    pub enable_batch_write: Option<bool>,
    pub batch_size: Option<usize>,
    pub batch_interval_ms: Option<u64>,
}

#[cfg(feature = "confers")]
impl Default for GlobalConfigConfers {
    fn default() -> Self {
        Self {
            default_ttl: Some(300),
            health_check_interval: Some(60),
            serialization: Some("json".to_string()),
            enable_metrics: Some(true),
        }
    }
}

#[cfg(feature = "confers")]
impl Default for ServiceConfigItem {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            cache_type: Some("two_level".to_string()),
            ttl: None,
            serialization: None,
            #[cfg(feature = "l1-moka")]
            l1: None,
            #[cfg(feature = "l2-redis")]
            l2: None,
            #[cfg(feature = "l2-redis")]
            two_level: None,
        }
    }
}

#[cfg(feature = "confers")]
impl OxcacheConfigFile {
    /// 从 TOML 文件加载配置
    #[cfg(all(feature = "confers", feature = "config-toml"))]
    pub fn from_toml(path: &str) -> Result<Self, String> {
        let config_path = Self::validate_path(path)?;
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse TOML: {}", e))
    }

    #[cfg(feature = "confers")]
    fn validate_path(path: &str) -> Result<PathBuf, String> {
        let path = Path::new(path);
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .map_err(|e| format!("Failed to get current directory: {}", e))
        }
    }

    #[cfg(feature = "confers")]
    pub fn to_oxcache_config(self) -> super::OxcacheConfig {
        use super::{CacheType, GlobalConfig, ServiceConfig};
        use crate::config::CONFIG_VERSION;

        let global = self.global.unwrap_or_default();
        let services = self.services.unwrap_or_default();

        let global = GlobalConfig {
            default_ttl: global.default_ttl.unwrap_or(300),
            health_check_interval: global.health_check_interval.unwrap_or(60),
            serialization: match global
                .serialization
                .unwrap_or_else(|| "json".to_string())
                .as_str()
            {
                "bincode" => super::SerializationType::Bincode,
                _ => super::SerializationType::Json,
            },
            enable_metrics: global.enable_metrics.unwrap_or(true),
        };

        let mut services_map = HashMap::new();
        for svc in services {
            let cache_type = match svc
                .cache_type
                .unwrap_or_else(|| "two_level".to_string())
                .as_str()
            {
                "l1" | "L1" => CacheType::L1,
                "l2" | "L2" => CacheType::L2,
                _ => CacheType::TwoLevel,
            };

            let service = ServiceConfig {
                cache_type,
                ttl: svc.ttl,
                serialization: svc.serialization.map(|s| match s.as_str() {
                    "bincode" => super::SerializationType::Bincode,
                    _ => super::SerializationType::Json,
                }),
                #[cfg(feature = "l1-moka")]
                l1: svc.l1.map(|l| super::L1Config {
                    max_capacity: l.max_capacity.unwrap_or(10000),
                    max_key_length: l.max_key_length.unwrap_or(512),
                    max_value_size: l.max_value_size.unwrap_or(10485760),
                    cleanup_interval_secs: l.cleanup_interval_secs.unwrap_or(60),
                }),
                #[cfg(feature = "l2-redis")]
                l2: svc.l2.map(|l| {
                    use secrecy::SecretString;
                    super::L2Config {
                        mode: match l.mode.unwrap_or_else(|| "standalone".to_string()).as_str() {
                            "sentinel" => super::RedisMode::Sentinel,
                            "cluster" => super::RedisMode::Cluster,
                            _ => super::RedisMode::Standalone,
                        },
                        connection_string: SecretString::new(
                            l.connection_string.unwrap_or_default().into(),
                        ),
                        connection_timeout_ms: l.connection_timeout_ms.unwrap_or(5000),
                        command_timeout_ms: l.command_timeout_ms.unwrap_or(30000),
                        password: l.password.map(|p| SecretString::new(p.into_boxed_str())),
                        enable_tls: l.enable_tls.unwrap_or(false),
                        sentinel: None,
                        cluster: None,
                        default_ttl: l.default_ttl,
                        max_key_length: 512,
                        max_value_size: 10 * 1024 * 1024,
                    }
                }),
                #[cfg(feature = "l2-redis")]
                two_level: svc.two_level.map(|t| super::TwoLevelConfig {
                    promote_on_hit: t.promote_on_hit.unwrap_or(true),
                    enable_batch_write: t.enable_batch_write.unwrap_or(false),
                    batch_size: t.batch_size.unwrap_or(100),
                    batch_interval_ms: t.batch_interval_ms.unwrap_or(10),
                    max_key_length: None,
                    max_value_size: None,
                    bloom_filter: None,
                    invalidation_channel: None,
                    warmup: None,
                }),
            };

            services_map.insert(svc.name, service);
        }

        super::OxcacheConfig {
            config_version: Some(CONFIG_VERSION),
            global,
            services: services_map,
            #[cfg(feature = "l1-moka")]
            layer: None,
            #[cfg(feature = "confers")]
            extensions: HashMap::new(),
            #[cfg(feature = "confers")]
            source: Some(super::ConfigSource::File("confers".to_string())),
        }
    }
}

/// 从 TOML 文件加载配置
#[cfg(feature = "confers")]
pub fn confers_load(path: &str) -> Result<super::OxcacheConfig, String> {
    #[cfg(feature = "config-toml")]
    {
        let config = OxcacheConfigFile::from_toml(path)?;
        Ok(config.to_oxcache_config())
    }
    #[cfg(not(feature = "config-toml"))]
    {
        Err("TOML configuration loading requires 'config-toml' feature".to_string())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "confers")]
    mod confers_tests {
        #[test]
        fn test_config_source_code() {
            let mut config = super::super::OxcacheConfig::new();
            config.set_source(super::super::ConfigSource::Code);
            assert_eq!(config.source, Some(super::super::ConfigSource::Code));
        }
    }
}
