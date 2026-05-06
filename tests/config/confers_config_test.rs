// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// confers_config 模块的覆盖率测试
// 测试所有配置结构体的 Default、验证、构建和访问方法

#[cfg(test)]
#[cfg(feature = "confers")]
mod confers_config_tests {
    use garde::Validate;
    use oxcache::config::confers_config::*;
    use std::collections::HashMap;

    // ============================================================================
    // ConfigBackendType 测试
    // ============================================================================

    #[test]
    fn test_backend_type_default() {
        assert_eq!(ConfigBackendType::default(), ConfigBackendType::Memory);
    }

    #[test]
    fn test_backend_type_display() {
        assert_eq!(ConfigBackendType::Memory.to_string(), "Memory");
        assert_eq!(ConfigBackendType::Redis.to_string(), "Redis");
        assert_eq!(ConfigBackendType::Tiered.to_string(), "Tiered");
    }

    #[test]
    fn test_backend_type_from_str_valid() {
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
        let result = "InvalidType".parse::<ConfigBackendType>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown backend type"));
    }

    // ============================================================================
    // CacheType 测试
    // ============================================================================

    #[test]
    fn test_cache_type_default() {
        assert_eq!(CacheType::default(), CacheType::L1);
    }

    #[test]
    fn test_cache_type_display() {
        assert_eq!(CacheType::L1.to_string(), "L1");
        assert_eq!(CacheType::L2.to_string(), "L2");
        assert_eq!(CacheType::TwoLevel.to_string(), "TwoLevel");
    }

    #[test]
    fn test_cache_type_from_str_valid() {
        assert_eq!("L1".parse::<CacheType>().unwrap(), CacheType::L1);
        assert_eq!("L2".parse::<CacheType>().unwrap(), CacheType::L2);
        assert_eq!("TwoLevel".parse::<CacheType>().unwrap(), CacheType::TwoLevel);
    }

    #[test]
    fn test_cache_type_from_str_invalid() {
        let result = "InvalidType".parse::<CacheType>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown cache type"));
    }

    // ============================================================================
    // GlobalConfig 测试
    // ============================================================================

    #[test]
    fn test_global_config_default() {
        let config = GlobalConfig::default();
        assert_eq!(config.default_ttl, 0);
        assert_eq!(config.default_tti, 0);
        assert_eq!(config.health_check_interval, 30);
    }

    #[test]
    fn test_global_config_validation_valid() {
        let config = GlobalConfig {
            default_ttl: 3600,
            default_tti: 1800,
            health_check_interval: 60,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_global_config_validation_ttl_exceeds_max() {
        let config = GlobalConfig {
            default_ttl: 31_536_001, // 超过最大值 31_536_000
            default_tti: 1800,
            health_check_interval: 60,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_global_config_validation_health_check_min() {
        let config = GlobalConfig {
            default_ttl: 3600,
            default_tti: 1800,
            health_check_interval: 0, // 低于最小值 1
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_global_config_validation_health_check_max() {
        let config = GlobalConfig {
            default_ttl: 3600,
            default_tti: 1800,
            health_check_interval: 3601, // 超过最大值 3600
        };
        assert!(config.validate().is_err());
    }

    // ============================================================================
    // BackendConfig 测试
    // ============================================================================

    #[test]
    fn test_backend_config_default() {
        let config = BackendConfig::default();
        assert_eq!(config.backend_type, "Memory");
        assert_eq!(config.l1_type, "moka");
        assert_eq!(config.l2_type, "redis");
        assert!(config.l1_enabled);
        assert!(!config.l2_enabled);
        assert!(config.l1_options_json.is_empty());
        assert!(config.l2_options_json.is_empty());
    }

    #[test]
    fn test_backend_config_backend_type_enum() {
        let config = BackendConfig {
            backend_type: "Redis".to_string(),
            l1_type: "moka".to_string(),
            l1_options_json: String::new(),
            l2_type: "redis".to_string(),
            l2_options_json: String::new(),
            l1_enabled: false,
            l2_enabled: true,
        };
        assert_eq!(config.backend_type_enum(), ConfigBackendType::Redis);
    }

    #[test]
    fn test_backend_config_backend_type_enum_invalid() {
        let config = BackendConfig {
            backend_type: "InvalidType".to_string(),
            ..Default::default()
        };
        // 应返回默认值 Memory
        assert_eq!(config.backend_type_enum(), ConfigBackendType::Memory);
    }

    #[test]
    fn test_backend_config_l1_options_empty() {
        let config = BackendConfig::default();
        assert!(config.l1_options_json.is_empty());
        assert_eq!(config.l1_options(), serde_json::Value::Null);
    }

    #[test]
    fn test_backend_config_l1_options_valid_json() {
        let config = BackendConfig {
            l1_options_json: "{\"max_capacity\":10000}".to_string(),
            ..Default::default()
        };
        let options = config.l1_options();
        assert_eq!(options.get("max_capacity").unwrap().as_u64().unwrap(), 10000);
    }

    #[test]
    fn test_backend_config_l1_options_invalid_json() {
        let config = BackendConfig {
            l1_options_json: "invalid json".to_string(),
            ..Default::default()
        };
        assert_eq!(config.l1_options(), serde_json::Value::Null);
    }

    #[test]
    fn test_backend_config_l2_options_empty() {
        let config = BackendConfig::default();
        assert!(config.l2_options_json.is_empty());
        assert_eq!(config.l2_options(), serde_json::Value::Null);
    }

    #[test]
    fn test_backend_config_l2_options_valid_json() {
        let config = BackendConfig {
            l2_options_json: "{\"connection_string\":\"redis://localhost\"}".to_string(),
            ..Default::default()
        };
        let options = config.l2_options();
        assert_eq!(
            options.get("connection_string").unwrap().as_str().unwrap(),
            "redis://localhost"
        );
    }

    // ============================================================================
    // ServiceConfig 测试
    // ============================================================================

    #[test]
    fn test_service_config_l1_only() {
        let config = ServiceConfig::l1_only();
        assert_eq!(config.cache_type_enum(), CacheType::L1);
        assert_eq!(config.cache_type, "L1");
        assert!(config.ttl.is_none());
        assert!(config.max_capacity.is_none());
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_service_config_l2_only() {
        let config = ServiceConfig::l2_only();
        assert_eq!(config.cache_type_enum(), CacheType::L2);
        assert_eq!(config.cache_type, "L2");
        assert!(config.ttl.is_none());
        assert!(config.max_capacity.is_none());
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_service_config_two_level() {
        let config = ServiceConfig::two_level();
        assert_eq!(config.cache_type_enum(), CacheType::TwoLevel);
        assert_eq!(config.cache_type, "TwoLevel");
        assert!(config.ttl.is_none());
        assert!(config.max_capacity.is_none());
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_service_config_with_ttl() {
        let config = ServiceConfig::l1_only().with_ttl(3600);
        assert_eq!(config.ttl, Some(3600));
    }

    #[test]
    fn test_service_config_with_ttl_zero() {
        let config = ServiceConfig::l1_only().with_ttl(0);
        assert_eq!(config.ttl, Some(0));
    }

    #[test]
    fn test_service_config_cache_type_enum_invalid() {
        let config = ServiceConfig {
            cache_type: "InvalidType".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        };
        // 应返回默认值 L1
        assert_eq!(config.cache_type_enum(), CacheType::L1);
    }

    #[test]
    fn test_service_config_validation_valid() {
        let config = ServiceConfig {
            cache_type: "L1".to_string(),
            ttl: Some(3600),
            max_capacity: Some(10000),
            enable_metrics: true,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_service_config_validation_ttl_exceeds_max() {
        let config = ServiceConfig {
            cache_type: "L1".to_string(),
            ttl: Some(31_536_001),
            max_capacity: Some(10000),
            enable_metrics: true,
        };
        assert!(config.validate().is_err());
    }

    // ============================================================================
    // PerformanceConfig 测试
    // ============================================================================

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.max_concurrent_operations, 1000);
        assert_eq!(config.command_timeout, 5000);
        assert!(!config.enable_prefetching);
    }

    #[test]
    fn test_performance_config_validation_valid() {
        let config = PerformanceConfig {
            max_concurrent_operations: 5000,
            command_timeout: 10000,
            enable_prefetching: true,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_performance_config_validation_max_ops_min() {
        let config = PerformanceConfig {
            max_concurrent_operations: 0, // 低于最小值 1
            command_timeout: 5000,
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_config_validation_max_ops_max() {
        let config = PerformanceConfig {
            max_concurrent_operations: 100_001, // 超过最大值 100_000
            command_timeout: 5000,
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_config_validation_timeout_min() {
        let config = PerformanceConfig {
            max_concurrent_operations: 1000,
            command_timeout: 0, // 低于最小值 1
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_config_validation_timeout_max() {
        let config = PerformanceConfig {
            max_concurrent_operations: 1000,
            command_timeout: 300_001, // 超过最大值 300_000
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    // ============================================================================
    // SecurityConfig 测试
    // ============================================================================

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.connection_string_redaction);
        assert_eq!(config.enable_rate_limiting, 0);
        assert_eq!(config.rate_limit_max_requests, 1000);
        assert_eq!(config.rate_limit_window_size, 60);
    }

    #[test]
    fn test_security_config_validation_valid() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 100,
            rate_limit_max_requests: 5000,
            rate_limit_window_size: 120,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_security_config_validation_rate_limit_max_requests_min() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 0, // 低于最小值 1
            rate_limit_window_size: 60,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_security_config_validation_rate_limit_max_requests_max() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 1_000_001, // 超过最大值 1_000_000
            rate_limit_window_size: 60,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_security_config_validation_window_size_min() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 1000,
            rate_limit_window_size: 0, // 低于最小值 1
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_security_config_validation_window_size_max() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 1000,
            rate_limit_window_size: 3601, // 超过最大值 3600
        };
        assert!(config.validate().is_err());
    }

    // ============================================================================
    // MetricsConfig 测试
    // ============================================================================

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert!(!config.enabled);
        assert!(!config.detailed);
        assert_eq!(config.export_format, "prometheus");
        assert!(config.export_endpoint.is_none());
    }

    #[test]
    fn test_metrics_config_with_endpoint() {
        let config = MetricsConfig {
            enabled: true,
            detailed: true,
            export_format: "json".to_string(),
            export_endpoint: Some("/metrics".to_string()),
        };
        assert!(config.enabled);
        assert!(config.detailed);
        assert_eq!(config.export_format, "json");
        assert_eq!(config.export_endpoint, Some("/metrics".to_string()));
    }

    // ============================================================================
    // RecoveryConfig 测试
    // ============================================================================

    #[test]
    fn test_recovery_config_default() {
        let config = RecoveryConfig::default();
        assert!(!config.enable_wal);
        assert_eq!(config.wal_directory, "./wal");
        assert!(config.enable_auto_recovery);
    }

    #[test]
    fn test_recovery_config_custom() {
        let config = RecoveryConfig {
            enable_wal: true,
            wal_directory: "/var/lib/oxcache/wal".to_string(),
            enable_auto_recovery: false,
        };
        assert!(config.enable_wal);
        assert_eq!(config.wal_directory, "/var/lib/oxcache/wal");
        assert!(!config.enable_auto_recovery);
    }

    // ============================================================================
    // UnifiedConfig 测试
    // ============================================================================

    #[test]
    fn test_unified_config_default() {
        let config = UnifiedConfig::default();
        assert_eq!(config.global.default_ttl, 0);
        assert_eq!(config.backend.backend_type, "Memory");
        assert_eq!(config.performance.max_concurrent_operations, 1000);
        assert!(!config.metrics.enabled);
        assert!(!config.recovery.enable_wal);
    }

    #[test]
    fn test_unified_config_services_empty() {
        let config = UnifiedConfig::default();
        assert!(config.services_json.is_empty());
        let services = config.services();
        assert!(services.is_empty());
    }

    #[test]
    fn test_unified_config_services_valid_json() {
        let services_json = serde_json::to_string(&HashMap::from([(
            "user_cache".to_string(),
            ServiceConfig::l1_only().with_ttl(3600),
        )]))
        .unwrap();

        let config = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        let services = config.services();
        assert_eq!(services.len(), 1);
        assert!(services.contains_key("user_cache"));
        assert_eq!(services.get("user_cache").unwrap().ttl, Some(3600));
    }

    #[test]
    fn test_unified_config_services_invalid_json() {
        let config = UnifiedConfig {
            services_json: "invalid json".to_string(),
            ..Default::default()
        };

        // 应返回空 HashMap
        let services = config.services();
        assert!(services.is_empty());
    }

    #[test]
    fn test_unified_config_validate_config_valid() {
        let config = UnifiedConfig::default();
        assert!(config.validate_config().is_ok());
    }

    #[test]
    fn test_unified_config_validate_config_with_services() {
        let services_json = serde_json::to_string(&HashMap::from([(
            "valid_service".to_string(),
            ServiceConfig::l1_only(),
        )]))
        .unwrap();

        let config = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        assert!(config.validate_config().is_ok());
    }

    #[test]
    fn test_unified_config_validate_config_invalid_service() {
        let invalid_service = ServiceConfig {
            cache_type: "L1".to_string(),
            ttl: None,
            max_capacity: Some(0), // 无效容量
            enable_metrics: true,
        };

        let services_json =
            serde_json::to_string(&HashMap::from([("invalid_service".to_string(), invalid_service)])).unwrap();

        let config = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        assert!(config.validate_config().is_err());
    }

    // ============================================================================
    // ConfigProvider trait 测试
    // ============================================================================

    #[test]
    fn test_config_provider_get_string_global() {
        let config = UnifiedConfig::default();

        assert_eq!(config.get_string("global.default_ttl"), Some("0".to_string()));
        assert_eq!(config.get_string("global.default_tti"), Some("0".to_string()));
        assert_eq!(
            config.get_string("global.health_check_interval"),
            Some("30".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_backend() {
        let config = UnifiedConfig::default();

        assert_eq!(config.get_string("backend.backend_type"), Some("Memory".to_string()));
        assert_eq!(config.get_string("backend.l1_type"), Some("moka".to_string()));
        assert_eq!(config.get_string("backend.l1_enabled"), Some("true".to_string()));
        assert_eq!(config.get_string("backend.l2_enabled"), Some("false".to_string()));
    }

    #[test]
    fn test_config_provider_get_string_performance() {
        let config = UnifiedConfig::default();

        assert_eq!(
            config.get_string("performance.max_concurrent_operations"),
            Some("1000".to_string())
        );
        assert_eq!(
            config.get_string("performance.command_timeout"),
            Some("5000".to_string())
        );
        assert_eq!(
            config.get_string("performance.enable_prefetching"),
            Some("false".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_metrics() {
        let config = UnifiedConfig::default();

        assert_eq!(config.get_string("metrics.enabled"), Some("false".to_string()));
        assert_eq!(config.get_string("metrics.detailed"), Some("false".to_string()));
        assert_eq!(
            config.get_string("metrics.export_format"),
            Some("prometheus".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_recovery() {
        let config = UnifiedConfig::default();

        assert_eq!(config.get_string("recovery.enable_wal"), Some("false".to_string()));
        assert_eq!(config.get_string("recovery.wal_directory"), Some("./wal".to_string()));
        assert_eq!(
            config.get_string("recovery.enable_auto_recovery"),
            Some("true".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_invalid_path() {
        let config = UnifiedConfig::default();

        // 路径太短
        assert!(config.get_string("invalid").is_none());
        assert!(config.get_string("").is_none());

        // 不存在的模块
        assert!(config.get_string("invalid_module.field").is_none());

        // 不存在的字段
        assert!(config.get_string("global.invalid_field").is_none());
    }

    #[test]
    fn test_config_provider_get_int() {
        let config = UnifiedConfig::default();

        assert_eq!(config.get_int("global.default_ttl"), Some(0));
        assert_eq!(config.get_int("global.health_check_interval"), Some(30));
        assert_eq!(config.get_int("performance.max_concurrent_operations"), Some(1000));
    }

    #[test]
    fn test_config_provider_get_int_invalid() {
        let config = UnifiedConfig::default();

        // 字符串字段无法解析为整数
        assert!(config.get_int("backend.backend_type").is_none());
        assert!(config.get_int("invalid.path").is_none());
    }

    #[test]
    fn test_config_provider_get_bool() {
        let config = UnifiedConfig::default();

        assert_eq!(config.get_bool("backend.l1_enabled"), Some(true));
        assert_eq!(config.get_bool("backend.l2_enabled"), Some(false));
        assert_eq!(config.get_bool("metrics.enabled"), Some(false));
        assert_eq!(config.get_bool("recovery.enable_auto_recovery"), Some(true));
    }

    #[test]
    fn test_config_provider_get_bool_invalid() {
        let config = UnifiedConfig::default();

        // 整数字段无法解析为布尔值
        assert!(config.get_bool("global.default_ttl").is_none());
        assert!(config.get_bool("invalid.path").is_none());
    }

    #[test]
    fn test_config_provider_get_json_l1_options_empty() {
        let config = UnifiedConfig::default();

        let result = config.get_json("backend.l1_options");
        assert_eq!(result, Some(serde_json::Value::Null));
    }

    #[test]
    fn test_config_provider_get_json_l1_options_valid() {
        let config = UnifiedConfig {
            backend: BackendConfig {
                l1_options_json: "{\"max_capacity\":10000}".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = config.get_json("backend.l1_options");
        assert!(result.is_some());
        let json = result.unwrap();
        assert_eq!(json.get("max_capacity").unwrap().as_u64().unwrap(), 10000);
    }

    #[test]
    fn test_config_provider_get_json_l2_options_empty() {
        let config = UnifiedConfig::default();

        let result = config.get_json("backend.l2_options");
        assert_eq!(result, Some(serde_json::Value::Null));
    }

    #[test]
    fn test_config_provider_get_json_l2_options_valid() {
        let config = UnifiedConfig {
            backend: BackendConfig {
                l2_options_json: "{\"connection_string\":\"redis://localhost\"}".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = config.get_json("backend.l2_options");
        assert!(result.is_some());
        let json = result.unwrap();
        assert_eq!(
            json.get("connection_string").unwrap().as_str().unwrap(),
            "redis://localhost"
        );
    }

    #[test]
    fn test_config_provider_get_json_services_all() {
        let services_json =
            serde_json::to_string(&HashMap::from([("user_cache".to_string(), ServiceConfig::l1_only())])).unwrap();

        let config = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        let result = config.get_json("services.all");
        assert!(result.is_some());
        let json = result.unwrap();
        assert!(json.is_object());
    }

    #[test]
    fn test_config_provider_get_string_services_json() {
        let services_json = "{\"user_cache\":{\"cache_type\":\"L1\"}}".to_string();
        let config = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        assert_eq!(
            config.get_string("services.json"),
            Some("{\"user_cache\":{\"cache_type\":\"L1\"}}".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_services_specific() {
        let services_json = serde_json::to_string(&HashMap::from([(
            "user_cache".to_string(),
            ServiceConfig::l1_only().with_ttl(3600),
        )]))
        .unwrap();

        let config = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        assert_eq!(
            config.get_string("services.user_cache.cache_type"),
            Some("L1".to_string())
        );
        assert_eq!(config.get_string("services.user_cache.ttl"), Some("3600".to_string()));
        assert_eq!(
            config.get_string("services.user_cache.enable_metrics"),
            Some("true".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_services_not_found() {
        let config = UnifiedConfig::default();

        // 不存在的服务名
        assert!(config.get_string("services.nonexistent.field").is_none());
    }

    // ============================================================================
    // UnifiedConfigBuilder 测试
    // ============================================================================

    #[test]
    fn test_unified_config_builder_new() {
        let builder = UnifiedConfigBuilder::new();
        let config = builder.build().unwrap();

        assert_eq!(config.global.default_ttl, 0);
        assert_eq!(config.global.default_tti, 0);
        assert_eq!(config.global.health_check_interval, 30);
        assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Memory);
    }

    #[test]
    fn test_unified_config_builder_memory_only() {
        let config = UnifiedConfigBuilder::memory_only().build().unwrap();

        assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Memory);
        assert!(config.backend.l1_enabled);
        assert!(!config.backend.l2_enabled);
        assert_eq!(config.backend.l1_type, "moka");
    }

    #[test]
    fn test_unified_config_builder_redis_only() {
        let config = UnifiedConfigBuilder::redis_only().build().unwrap();

        assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Redis);
        assert!(!config.backend.l1_enabled);
        assert!(config.backend.l2_enabled);
        assert_eq!(config.backend.l2_type, "redis");
    }

    #[test]
    fn test_unified_config_builder_tiered() {
        let config = UnifiedConfigBuilder::tiered().build().unwrap();

        assert_eq!(config.backend.backend_type_enum(), ConfigBackendType::Tiered);
        assert!(config.backend.l1_enabled);
        assert!(config.backend.l2_enabled);
        assert_eq!(config.backend.l1_type, "moka");
        assert_eq!(config.backend.l2_type, "redis");
    }

    #[test]
    fn test_unified_config_builder_with_ttl() {
        let config = UnifiedConfigBuilder::memory_only().with_ttl(7200).build().unwrap();

        assert_eq!(config.global.default_ttl, 7200);
    }

    #[test]
    fn test_unified_config_builder_with_tti() {
        let config = UnifiedConfigBuilder::memory_only().with_tti(3600).build().unwrap();

        assert_eq!(config.global.default_tti, 3600);
    }

    #[test]
    fn test_unified_config_builder_with_health_check_interval() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_health_check_interval(120)
            .build()
            .unwrap();

        assert_eq!(config.global.health_check_interval, 120);
    }

    #[test]
    fn test_unified_config_builder_with_l1_capacity() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_l1_capacity(50000)
            .build()
            .unwrap();

        let l1_options = config.backend.l1_options();
        assert_eq!(l1_options.get("max_capacity").unwrap().as_u64().unwrap(), 50000);
    }

    #[test]
    fn test_unified_config_builder_with_redis_url() {
        let config = UnifiedConfigBuilder::redis_only()
            .with_redis_url("redis://127.0.0.1:6379")
            .build()
            .unwrap();

        let l2_options = config.backend.l2_options();
        assert_eq!(
            l2_options.get("connection_string").unwrap().as_str().unwrap(),
            "redis://127.0.0.1:6379"
        );
    }

    #[test]
    fn test_unified_config_builder_with_redis_mode() {
        let config = UnifiedConfigBuilder::redis_only()
            .with_redis_mode("cluster")
            .build()
            .unwrap();

        let l2_options = config.backend.l2_options();
        assert_eq!(l2_options.get("mode").unwrap().as_str().unwrap(), "cluster");
    }

    #[test]
    fn test_unified_config_builder_with_max_concurrent_operations() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_max_concurrent_operations(5000)
            .build()
            .unwrap();

        assert_eq!(config.performance.max_concurrent_operations, 5000);
    }

    #[test]
    fn test_unified_config_builder_with_command_timeout() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_command_timeout(10000)
            .build()
            .unwrap();

        assert_eq!(config.performance.command_timeout, 10000);
    }

    #[test]
    fn test_unified_config_builder_with_metrics() {
        let config = UnifiedConfigBuilder::memory_only().with_metrics(true).build().unwrap();

        assert!(config.metrics.enabled);
    }

    #[test]
    fn test_unified_config_builder_with_wal() {
        let config = UnifiedConfigBuilder::memory_only().with_wal(true).build().unwrap();

        assert!(config.recovery.enable_wal);
    }

    #[test]
    fn test_unified_config_builder_with_wal_directory() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_wal_directory("/var/wal")
            .build()
            .unwrap();

        assert_eq!(config.recovery.wal_directory, "/var/wal");
    }

    #[test]
    fn test_unified_config_builder_with_auto_recovery() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_auto_recovery(false)
            .build()
            .unwrap();

        assert!(!config.recovery.enable_auto_recovery);
    }

    #[test]
    fn test_unified_config_builder_with_service() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_service("user_cache", CacheType::L1, 3600)
            .build()
            .unwrap();

        let services = config.services();
        assert!(services.contains_key("user_cache"));
        assert_eq!(services.get("user_cache").unwrap().cache_type_enum(), CacheType::L1);
        assert_eq!(services.get("user_cache").unwrap().ttl, Some(3600));
    }

    #[test]
    fn test_unified_config_builder_with_multiple_services() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_service("user_cache", CacheType::L1, 3600)
            .with_service("session_cache", CacheType::TwoLevel, 1800)
            .build()
            .unwrap();

        let services = config.services();
        assert_eq!(services.len(), 2);
        assert!(services.contains_key("user_cache"));
        assert!(services.contains_key("session_cache"));
    }

    #[test]
    fn test_unified_config_builder_with_service_zero_ttl() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_service("cache", CacheType::L1, 0)
            .build()
            .unwrap();

        let services = config.services();
        assert_eq!(services.get("cache").unwrap().ttl, None);
    }

    #[test]
    fn test_unified_config_builder_with_dependencies() {
        let original = UnifiedConfig::default();
        let builder = UnifiedConfigBuilder::with_dependencies(original.clone());
        let rebuilt = builder.build().unwrap();

        assert_eq!(rebuilt.global.default_ttl, original.global.default_ttl);
        assert_eq!(rebuilt.backend.backend_type, original.backend.backend_type);
        assert_eq!(
            rebuilt.performance.max_concurrent_operations,
            original.performance.max_concurrent_operations
        );
    }

    #[test]
    fn test_unified_config_builder_with_dependencies_preserves_services() {
        let services_json = serde_json::to_string(&HashMap::from([(
            "user_cache".to_string(),
            ServiceConfig::l1_only().with_ttl(3600),
        )]))
        .unwrap();

        let original = UnifiedConfig {
            services_json,
            ..Default::default()
        };

        let builder = UnifiedConfigBuilder::with_dependencies(original);
        let rebuilt = builder.build().unwrap();

        let services = rebuilt.services();
        assert!(services.contains_key("user_cache"));
        assert_eq!(services.get("user_cache").unwrap().ttl, Some(3600));
    }

    #[test]
    fn test_unified_config_builder_with_dependencies_with_endpoint() {
        let original = UnifiedConfig {
            metrics: MetricsConfig {
                enabled: true,
                detailed: true,
                export_format: "json".to_string(),
                export_endpoint: Some("/metrics".to_string()),
            },
            ..Default::default()
        };

        let builder = UnifiedConfigBuilder::with_dependencies(original);
        let rebuilt = builder.build().unwrap();

        assert_eq!(rebuilt.metrics.export_endpoint, Some("/metrics".to_string()));
    }

    #[test]
    fn test_unified_config_builder_build_json() {
        let json = UnifiedConfigBuilder::memory_only().with_ttl(3600).build_json();

        assert!(json.is_object());
        assert_eq!(
            json.get("global")
                .unwrap()
                .get("default_ttl")
                .unwrap()
                .as_u64()
                .unwrap(),
            3600
        );
    }

    #[test]
    fn test_unified_config_builder_default_trait() {
        let builder = UnifiedConfigBuilder::default();
        let config = builder.build().unwrap();

        assert_eq!(config.global.default_ttl, 0);
    }

    // ============================================================================
    // ConfigFormat 测试
    // ============================================================================

    #[test]
    fn test_config_format_from_path_toml() {
        assert_eq!(ConfigFormat::from_path("config.toml"), Some(ConfigFormat::Toml));
        assert_eq!(
            ConfigFormat::from_path("/path/to/config.toml"),
            Some(ConfigFormat::Toml)
        );
    }

    #[test]
    fn test_config_format_from_path_json() {
        assert_eq!(ConfigFormat::from_path("config.json"), Some(ConfigFormat::Json));
        assert_eq!(
            ConfigFormat::from_path("/path/to/config.json"),
            Some(ConfigFormat::Json)
        );
    }

    #[test]
    fn test_config_format_from_path_invalid() {
        assert_eq!(ConfigFormat::from_path("config.yaml"), None);
        assert_eq!(ConfigFormat::from_path("config.yml"), None);
        assert_eq!(ConfigFormat::from_path("config.txt"), None);
        assert_eq!(ConfigFormat::from_path("config"), None);
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
    fn test_config_format_equality() {
        assert_eq!(ConfigFormat::Toml, ConfigFormat::Toml);
        assert_eq!(ConfigFormat::Json, ConfigFormat::Json);
        assert_ne!(ConfigFormat::Toml, ConfigFormat::Json);
    }

    // ============================================================================
    // 文件加载测试（使用临时文件）
    // ============================================================================

    #[test]
    fn test_unified_config_from_json_file_invalid_content() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"invalid json content").unwrap();

        let path = file.path().to_str().unwrap();
        let result = UnifiedConfig::from_json_file(path);

        assert!(result.is_err());
    }

    #[test]
    fn test_unified_config_from_file_not_found() {
        let result = UnifiedConfig::from_toml_file("/nonexistent/path/config.toml");
        assert!(result.is_err());

        let result = UnifiedConfig::from_json_file("/nonexistent/path/config.json");
        assert!(result.is_err());
    }

    // ============================================================================
    // 边界条件和错误路径测试
    // ============================================================================

    #[test]
    fn test_backend_type_all_variants() {
        // 确保测试所有枚举变体
        let variants = [
            ConfigBackendType::Memory,
            ConfigBackendType::Redis,
            ConfigBackendType::Tiered,
        ];
        for variant in variants.iter() {
            let display = variant.to_string();
            let parsed: ConfigBackendType = display.parse().unwrap();
            assert_eq!(*variant, parsed);
        }
    }

    #[test]
    fn test_cache_type_all_variants() {
        let variants = [CacheType::L1, CacheType::L2, CacheType::TwoLevel];
        for variant in variants.iter() {
            let display = variant.to_string();
            let parsed: CacheType = display.parse().unwrap();
            assert_eq!(*variant, parsed);
        }
    }

    #[test]
    fn test_config_format_all_variants() {
        let variants = [ConfigFormat::Toml, ConfigFormat::Json];
        for variant in variants.iter() {
            let ext = variant.extension();
            let mime = variant.mime_type();
            assert!(!ext.is_empty());
            assert!(!mime.is_empty());
        }
    }

    #[test]
    fn test_global_config_boundary_values() {
        // TTL 最大边界
        let config = GlobalConfig {
            default_ttl: 31_536_000,
            default_tti: 0,
            health_check_interval: 1,
        };
        assert!(config.validate().is_ok());

        // health_check_interval 最大边界
        let config = GlobalConfig {
            default_ttl: 0,
            default_tti: 0,
            health_check_interval: 3600,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_performance_config_boundary_values() {
        // max_concurrent_operations 边界
        let config = PerformanceConfig {
            max_concurrent_operations: 1,
            command_timeout: 1,
            enable_prefetching: false,
        };
        assert!(config.validate().is_ok());

        let config = PerformanceConfig {
            max_concurrent_operations: 100_000,
            command_timeout: 300_000,
            enable_prefetching: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_security_config_boundary_values() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1_000_000,
            rate_limit_max_requests: 1,
            rate_limit_window_size: 1,
        };
        assert!(config.validate().is_ok());

        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1_000_000,
            rate_limit_max_requests: 1_000_000,
            rate_limit_window_size: 3600,
        };
        assert!(config.validate().is_ok());
    }
}
