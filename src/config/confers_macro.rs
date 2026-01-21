//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! confers 配置模块 - 统一使用 confers 进行配置读取
//!
//! 此模块在 confers feature 启用时可用

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "confers")]
use toml;

/// 完整配置（confers 版本）
#[derive(Debug, Clone, Deserialize)]
pub struct OxcacheConfigFile {
    pub global: Option<GlobalConfigConfers>,
    pub services: Option<Vec<ServiceConfigItem>>,
}

/// 全局配置（confers 版本）
#[derive(Debug, Clone, Deserialize)]
pub struct GlobalConfigConfers {
    pub default_ttl: Option<u64>,
    pub health_check_interval: Option<u64>,
    pub serialization: Option<String>,
    pub enable_metrics: Option<bool>,
}

/// 服务配置项
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
#[cfg(feature = "l1-moka")]
#[derive(Debug, Clone, Deserialize)]
pub struct L1ConfigConfers {
    pub max_capacity: Option<u64>,
    pub max_key_length: Option<usize>,
    pub max_value_size: Option<usize>,
    pub cleanup_interval_secs: Option<u64>,
    pub eviction_policy: Option<String>,
}

/// L2 配置（需要 l2-redis feature）
#[cfg(feature = "l2-redis")]
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
#[cfg(feature = "l2-redis")]
#[derive(Debug, Clone, Deserialize)]
pub struct TwoLevelConfigConfers {
    pub promote_on_hit: Option<bool>,
    pub enable_batch_write: Option<bool>,
    pub batch_size: Option<usize>,
    pub batch_interval_ms: Option<u64>,
}

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

impl OxcacheConfigFile {
    /// 从 TOML 文件加载配置
    pub fn from_toml(path: &str) -> Result<Self, String> {
        let config_path = Self::validate_path(path)?;
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse TOML: {}", e))
    }

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

    /// 转换为 OxcacheConfig
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
                            l.connection_string.unwrap_or_default(),
                        ),
                        connection_timeout_ms: l.connection_timeout_ms.unwrap_or(5000),
                        command_timeout_ms: l.command_timeout_ms.unwrap_or(30000),
                        password: l.password.map(SecretString::new),
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

/// 从 TOML 文件加载配置（统一使用 confers）
#[cfg(feature = "confers")]
pub fn confers_load(path: &str) -> Result<super::OxcacheConfig, String> {
    let config = OxcacheConfigFile::from_toml(path)?;
    Ok(config.to_oxcache_config())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(feature = "confers")]
    #[test]
    fn test_confers_load_valid_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("oxcache.toml");

        let config_content = r#"
[global]
default_ttl = 600
health_check_interval = 30
serialization = "json"
enable_metrics = true

[[services]]
name = "test_service"
cache_type = "two_level"
ttl = 3600
"#;

        std::fs::write(&config_path, config_content).unwrap();

        let config = confers_load(config_path.to_str().unwrap());
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.global.default_ttl, 600);
        assert!(config.services.contains_key("test_service"));
    }

    #[cfg(feature = "confers")]
    #[test]
    fn test_confers_load_with_l2_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("oxcache.toml");

        let config_content = r#"
[global]
default_ttl = 300

[[services]]
name = "redis_service"
cache_type = "L2"

[services.l2]
mode = "standalone"
connection_string = "redis://localhost:6379"
connection_timeout_ms = 5000
default_ttl = 3600
"#;

        std::fs::write(&config_path, config_content).unwrap();

        let config = confers_load(config_path.to_str().unwrap());
        assert!(config.is_ok());
        let config = config.unwrap();

        let service = config.services.get("redis_service").unwrap();
        assert!(service.l2.is_some());
        let l2 = service.l2.as_ref().unwrap();
        // connection_string 应该是 SecretString
        let conn_str = l2.connection_string.expose_secret();
        use secrecy::ExposeSecret;
        assert!(conn_str.contains("localhost"));
    }

    #[cfg(feature = "confers")]
    #[test]
    fn test_confers_load_invalid_path() {
        let result = confers_load("/nonexistent/path/config.toml");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read config file"));
    }

    #[cfg(feature = "confers")]
    #[test]
    fn test_confers_load_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.toml");

        let invalid_content = r#"
[global
this is not valid toml
"#;

        std::fs::write(&config_path, invalid_content).unwrap();

        let result = confers_load(config_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[cfg(feature = "confers")]
    #[test]
    fn test_config_source_set_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("oxcache.toml");

        let config_content = r#"
[global]
default_ttl = 300
"#;

        std::fs::write(&config_path, config_content).unwrap();

        let config = confers_load(config_path.to_str().unwrap()).unwrap();
        use crate::ConfigSource;
        assert_eq!(
            config.source,
            Some(ConfigSource::File("confers".to_string()))
        );
    }
}
