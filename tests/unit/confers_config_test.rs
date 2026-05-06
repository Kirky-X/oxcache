// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration tests extracted from confers_config.rs

use garde::Validate;
use oxcache::core::confers_config::{
    BackendConfig, CacheType, ConfigBackendType, ConfigFormat, ConfigProvider, GlobalConfig, MetricsConfig,
    PerformanceConfig, RecoveryConfig, SecurityConfig, ServiceConfig, UnifiedConfig, UnifiedConfigBuilder,
};

// ========================================================================
// ConfigBackendType tests
// ========================================================================

#[test]
fn test_backend_type_default() {
    assert_eq!(ConfigBackendType::default(), ConfigBackendType::Memory);
}

#[test]
fn test_backend_type_display() {
    assert_eq!(format!("{}", ConfigBackendType::Memory), "Memory");
    assert_eq!(format!("{}", ConfigBackendType::Redis), "Redis");
    assert_eq!(format!("{}", ConfigBackendType::Tiered), "Tiered");
}

#[test]
fn test_backend_type_from_str() {
    assert_eq!(
        "Memory".parse::<ConfigBackendType>().unwrap(),
        ConfigBackendType::Memory
    );
    assert_eq!("Redis".parse::<ConfigBackendType>().unwrap(), ConfigBackendType::Redis);
    assert_eq!(
        "Tiered".parse::<ConfigBackendType>().unwrap(),
        ConfigBackendType::Tiered
    );
}

#[test]
fn test_backend_type_from_str_invalid() {
    let result = "Unknown".parse::<ConfigBackendType>();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown backend type"));
}

#[test]
fn test_backend_type_from_str_case_sensitive() {
    assert!("memory".parse::<ConfigBackendType>().is_err());
    assert!("REDIS".parse::<ConfigBackendType>().is_err());
}

// ========================================================================
// CacheType tests
// ========================================================================

#[test]
fn test_cache_type_default() {
    assert_eq!(CacheType::default(), CacheType::L1);
}

#[test]
fn test_cache_type_display() {
    assert_eq!(format!("{}", CacheType::L1), "L1");
    assert_eq!(format!("{}", CacheType::L2), "L2");
    assert_eq!(format!("{}", CacheType::TwoLevel), "TwoLevel");
}

#[test]
fn test_cache_type_from_str() {
    assert_eq!("L1".parse::<CacheType>().unwrap(), CacheType::L1);
    assert_eq!("L2".parse::<CacheType>().unwrap(), CacheType::L2);
    assert_eq!("TwoLevel".parse::<CacheType>().unwrap(), CacheType::TwoLevel);
}

#[test]
fn test_cache_type_from_str_invalid() {
    let result = "Unknown".parse::<CacheType>();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown cache type"));
}

#[test]
fn test_cache_type_from_str_case_sensitive() {
    assert!("l1".parse::<CacheType>().is_err());
    assert!("L2_ONLY".parse::<CacheType>().is_err());
}

// ========================================================================
// ConfigFormat tests
// ========================================================================

#[test]
fn test_config_format_from_path() {
    assert_eq!(ConfigFormat::from_path("config.toml"), Some(ConfigFormat::Toml));
    assert_eq!(ConfigFormat::from_path("config.json"), Some(ConfigFormat::Json));
    assert_eq!(ConfigFormat::from_path("config.yaml"), None);
}

#[test]
fn test_config_format_extension() {
    assert_eq!(ConfigFormat::Toml.extension(), "toml");
    assert_eq!(ConfigFormat::Json.extension(), "json");
}

#[test]
fn test_config_format_mime_type() {
    assert_eq!(ConfigFormat::Toml.mime_type(), "application/toml");
    assert_eq!(ConfigFormat::Json.mime_type(), "application/json");
}

#[test]
fn test_config_format_from_path_edge_cases() {
    assert_eq!(ConfigFormat::from_path("config.TOML"), None);
    assert_eq!(ConfigFormat::from_path("dir/sub/config.toml"), Some(ConfigFormat::Toml));
    assert_eq!(ConfigFormat::from_path(""), None);
}

// ========================================================================
// GlobalConfig tests
// ========================================================================

#[test]
fn test_global_config_default() {
    let config = GlobalConfig::default();
    assert_eq!(config.default_ttl, 0);
    assert_eq!(config.default_tti, 0);
    assert_eq!(config.health_check_interval, 30);
}

#[test]
fn test_global_config_validation() {
    let config = GlobalConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_global_config_validation_ttl_too_large() {
    let config = GlobalConfig {
        default_ttl: 31_536_001,
        default_tti: 0,
        health_check_interval: 30,
    };
    assert!(config.validate().is_err());
}

// ========================================================================
// BackendConfig tests
// ========================================================================

#[test]
fn test_backend_config_default() {
    let config = BackendConfig::default();
    assert_eq!(config.backend_type, "Memory");
    assert_eq!(config.l1_type, "moka");
    assert_eq!(config.l2_type, "redis");
    assert!(config.l1_enabled);
    assert!(!config.l2_enabled);
}

#[test]
fn test_backend_config_backend_type_enum() {
    let config = BackendConfig::default();
    assert_eq!(config.backend_type_enum(), ConfigBackendType::Memory);
}

#[test]
fn test_backend_config_l1_options_empty() {
    let config = BackendConfig::default();
    assert_eq!(config.l1_options(), serde_json::Value::Null);
}

#[test]
fn test_backend_config_l1_options_valid_json() {
    let mut config = BackendConfig::default();
    config.l1_options_json = r#"{"max_capacity": 5000}"#.to_string();
    let options = config.l1_options();
    assert_eq!(options["max_capacity"].as_u64().unwrap(), 5000);
}

// ========================================================================
// ServiceConfig tests
// ========================================================================

#[test]
fn test_service_config_default() {
    let config = ServiceConfig::default();
    assert_eq!(config.cache_type, "L1");
    assert!(config.ttl.is_none());
    assert!(config.max_capacity.is_none());
    assert!(config.enable_metrics);
}

#[test]
fn test_service_config_l1_only() {
    let config = ServiceConfig::l1_only();
    assert_eq!(config.cache_type_enum(), CacheType::L1);
}

#[test]
fn test_service_config_l2_only() {
    let config = ServiceConfig::l2_only();
    assert_eq!(config.cache_type_enum(), CacheType::L2);
}

#[test]
fn test_service_config_two_level() {
    let config = ServiceConfig::two_level();
    assert_eq!(config.cache_type_enum(), CacheType::TwoLevel);
}

#[test]
fn test_service_config_with_ttl() {
    let config = ServiceConfig::l1_only().with_ttl(3600);
    assert_eq!(config.ttl, Some(3600));
}

// ========================================================================
// PerformanceConfig tests
// ========================================================================

#[test]
fn test_performance_config_default() {
    let config = PerformanceConfig::default();
    assert_eq!(config.max_concurrent_operations, 1000);
    assert_eq!(config.command_timeout, 5000);
    assert!(!config.enable_prefetching);
}

#[test]
fn test_performance_config_validation() {
    let config = PerformanceConfig::default();
    assert!(config.validate().is_ok());
}

// ========================================================================
// SecurityConfig tests
// ========================================================================

#[test]
fn test_security_config_default() {
    let config = SecurityConfig::default();
    assert!(config.connection_string_redaction);
    assert_eq!(config.enable_rate_limiting, 0);
}

// ========================================================================
// MetricsConfig tests
// ========================================================================

#[test]
fn test_metrics_config_default() {
    let config = MetricsConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.export_format, "prometheus");
}

// ========================================================================
// RecoveryConfig tests
// ========================================================================

#[test]
fn test_recovery_config_default() {
    let config = RecoveryConfig::default();
    assert!(!config.enable_wal);
    assert_eq!(config.wal_directory, "./wal");
}

// ========================================================================
// UnifiedConfig tests
// ========================================================================

#[test]
fn test_unified_config_default() {
    let config = UnifiedConfig::default();
    assert_eq!(config.global.default_ttl, 0);
    assert_eq!(config.backend.backend_type, "Memory");
}

#[test]
fn test_unified_config_services_empty() {
    let config = UnifiedConfig::default();
    let services = config.services();
    assert!(services.is_empty());
}

#[test]
fn test_unified_config_validate_config() {
    let config = UnifiedConfig::default();
    assert!(config.validate_config().is_ok());
}

// ========================================================================
// UnifiedConfigBuilder tests
// ========================================================================

#[test]
fn test_unified_config_builder_new() {
    let builder = UnifiedConfigBuilder::new();
    let config = builder.build().unwrap();
    assert_eq!(config.global.default_ttl, 0);
}

#[test]
fn test_unified_config_builder_memory_only() {
    let config = UnifiedConfigBuilder::memory_only().build().unwrap();
    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Memory);
    assert!(config.backend.l1_enabled);
    assert!(!config.backend.l2_enabled);
}

#[test]
fn test_unified_config_builder_redis_only() {
    let config = UnifiedConfigBuilder::redis_only().build().unwrap();
    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Redis);
    assert!(!config.backend.l1_enabled);
    assert!(config.backend.l2_enabled);
}

#[test]
fn test_unified_config_builder_tiered() {
    let config = UnifiedConfigBuilder::tiered().build().unwrap();
    assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Tiered);
    assert!(config.backend.l1_enabled);
    assert!(config.backend.l2_enabled);
}

#[test]
fn test_unified_config_builder_with_ttl() {
    let config = UnifiedConfigBuilder::memory_only().with_ttl(3600).build().unwrap();
    assert_eq!(config.global.default_ttl, 3600);
}

#[test]
fn test_unified_config_builder_with_l1_capacity() {
    let config = UnifiedConfigBuilder::memory_only()
        .with_l1_capacity(10000)
        .build()
        .unwrap();
    let options = config.backend.l1_options();
    assert_eq!(options["max_capacity"].as_u64().unwrap(), 10000);
}

#[test]
fn test_unified_config_builder_with_redis_url() {
    let config = UnifiedConfigBuilder::redis_only()
        .with_redis_url("redis://localhost:6379")
        .build()
        .unwrap();
    let options = config.backend.l2_options();
    assert_eq!(options["connection_string"].as_str().unwrap(), "redis://localhost:6379");
}

#[test]
fn test_unified_config_builder_with_service() {
    let config = UnifiedConfigBuilder::memory_only()
        .with_service("auth", CacheType::L1, 3600)
        .build()
        .unwrap();
    let services = config.services();
    assert!(services.contains_key("auth"));
    assert_eq!(services["auth"].ttl, Some(3600));
}

#[test]
fn test_unified_config_builder_build_json() {
    let json = UnifiedConfigBuilder::memory_only().with_ttl(3600).build_json();
    assert!(json.is_object());
    assert_eq!(json["global"]["default_ttl"], 3600);
}

#[test]
fn test_unified_config_builder_with_dependencies() {
    let config = UnifiedConfig::default();
    let builder = UnifiedConfigBuilder::with_dependencies(config);
    let result = builder.build().unwrap();
    assert_eq!(result.global.default_ttl, 0);
}

// ========================================================================
// ConfigProvider trait tests
// ========================================================================

#[test]
fn test_config_provider_get_string_global() {
    let config = UnifiedConfig::default();
    assert_eq!(config.get_string("global.default_ttl"), Some("0".to_string()));
    assert_eq!(
        config.get_string("global.health_check_interval"),
        Some("30".to_string())
    );
}

#[test]
fn test_config_provider_get_string_backend() {
    let config = UnifiedConfig::default();
    assert_eq!(config.get_string("backend.backend_type"), Some("Memory".to_string()));
    assert_eq!(config.get_string("backend.l1_enabled"), Some("true".to_string()));
}

#[test]
fn test_config_provider_get_int() {
    let config = UnifiedConfig::default();
    assert_eq!(config.get_int("global.default_ttl"), Some(0));
    assert_eq!(config.get_int("performance.max_concurrent_operations"), Some(1000));
}

#[test]
fn test_config_provider_get_bool() {
    let config = UnifiedConfig::default();
    assert_eq!(config.get_bool("backend.l1_enabled"), Some(true));
    assert_eq!(config.get_bool("backend.l2_enabled"), Some(false));
}

#[test]
fn test_config_provider_get_json_backend_options() {
    let config = UnifiedConfig::default();
    assert_eq!(config.get_json("backend.l1_options"), Some(serde_json::Value::Null));
}

#[test]
fn test_config_provider_get_string_invalid_key() {
    let config = UnifiedConfig::default();
    assert_eq!(config.get_string("invalid"), None);
    assert_eq!(config.get_string("global.nonexistent"), None);
}

// ========================================================================
// Serialization tests
// ========================================================================

#[test]
fn test_unified_config_serialize_deserialize() {
    let config = UnifiedConfig::default();
    let serialized = serde_json::to_string(&config).unwrap();
    let deserialized: UnifiedConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.global.default_ttl, config.global.default_ttl);
}

#[test]
fn test_service_config_serialize_deserialize() {
    let config = ServiceConfig::l1_only().with_ttl(3600);
    let serialized = serde_json::to_string(&config).unwrap();
    let deserialized: ServiceConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.ttl, config.ttl);
}

#[test]
fn test_backend_config_serialize_deserialize() {
    let config = BackendConfig::default();
    let serialized = serde_json::to_string(&config).unwrap();
    let deserialized: BackendConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.backend_type, config.backend_type);
}
