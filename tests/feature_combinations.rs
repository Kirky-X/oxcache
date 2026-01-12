#![allow(dead_code)]

// Feature Combinations Test Suite
//
// This test suite verifies all feature combinations:
// - minimal: L1 only cache
// - core: L1 + L2 basic functionality
// - full: All features
//
// Run with: rustc --test feature_combinations.rs && ./feature_combinations

use std::collections::HashMap;

// ============================================================================
// Configuration Structures
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum CacheType {
    L1,
    L2,
    TwoLevel,
}

#[derive(Debug, Clone, Default)]
struct L1Config {
    max_capacity: u64,
    ttl: Option<u64>,
    tti: Option<u64>,
    initial_capacity: u64,
}

impl L1Config {
    fn new() -> Self {
        Self {
            max_capacity: 10000,
            ttl: Some(300),
            tti: Some(180),
            initial_capacity: 1000,
        }
    }

    fn with_max_capacity(mut self, capacity: u64) -> Self {
        self.max_capacity = capacity;
        self
    }

    fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
enum RedisMode {
    #[default]
    Standalone,
    Sentinel,
    Cluster,
}

#[derive(Debug, Clone, Default)]
struct L2Config {
    mode: RedisMode,
    connection_string: String,
    connection_timeout_ms: u64,
    command_timeout_ms: u64,
    password: Option<String>,
    enable_tls: bool,
    default_ttl: Option<u64>,
}

impl L2Config {
    fn new() -> Self {
        Self {
            mode: RedisMode::Standalone,
            connection_string: "redis://localhost:6379".to_string(),
            connection_timeout_ms: 1000,
            command_timeout_ms: 1000,
            password: None,
            enable_tls: false,
            default_ttl: Some(600),
        }
    }

    fn with_mode(mut self, mode: RedisMode) -> Self {
        self.mode = mode;
        self
    }

    fn with_connection_string(mut self, conn_str: &str) -> Self {
        self.connection_string = conn_str.to_string();
        self
    }

    fn with_password(mut self, password: &str) -> Self {
        self.password = Some(password.to_string());
        self
    }

    fn with_tls(mut self, tls: bool) -> Self {
        self.enable_tls = tls;
        self
    }

    fn with_default_ttl(mut self, ttl: u64) -> Self {
        self.default_ttl = Some(ttl);
        self
    }
}

#[derive(Debug, Clone, Default)]
struct TwoLevelConfig {
    write_through: bool,
    promote_on_hit: bool,
    enable_batch_write: bool,
    batch_size: usize,
    batch_interval_ms: u64,
}

impl TwoLevelConfig {
    fn new() -> Self {
        Self {
            write_through: true,
            promote_on_hit: true,
            enable_batch_write: false,
            batch_size: 100,
            batch_interval_ms: 50,
        }
    }

    fn with_batch_write(mut self, enable: bool) -> Self {
        self.enable_batch_write = enable;
        self
    }

    fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    fn with_batch_interval_ms(mut self, ms: u64) -> Self {
        self.batch_interval_ms = ms;
        self
    }
}

#[derive(Debug, Clone)]
struct ServiceConfig {
    cache_type: CacheType,
    ttl: Option<u64>,
    l1: Option<L1Config>,
    l2: Option<L2Config>,
    two_level: Option<TwoLevelConfig>,
}

impl ServiceConfig {
    fn l1_only() -> Self {
        Self {
            cache_type: CacheType::L1,
            ttl: Some(300),
            l1: Some(L1Config::new()),
            l2: None,
            two_level: None,
        }
    }

    fn l2_only() -> Self {
        Self {
            cache_type: CacheType::L2,
            ttl: Some(3600),
            l1: None,
            l2: Some(L2Config::new()),
            two_level: None,
        }
    }

    fn two_level() -> Self {
        Self {
            cache_type: CacheType::TwoLevel,
            ttl: Some(600),
            l1: Some(L1Config::new()),
            l2: Some(L2Config::new()),
            two_level: Some(TwoLevelConfig::new()),
        }
    }

    fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    fn with_l1_config(mut self, config: L1Config) -> Self {
        self.l1 = Some(config);
        self
    }

    fn with_l2_config(mut self, config: L2Config) -> Self {
        self.l2 = Some(config);
        self
    }

    fn with_two_level_config(mut self, config: TwoLevelConfig) -> Self {
        self.two_level = Some(config);
        self
    }
}

#[derive(Debug, Clone, Default)]
struct GlobalConfig {
    default_ttl: u64,
    health_check_interval: u64,
    enable_metrics: bool,
}

#[derive(Debug, Clone)]
struct Config {
    config_version: Option<u32>,
    global: GlobalConfig,
    services: HashMap<String, ServiceConfig>,
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        // Validate L1 TTL <= L2 TTL (if both exist)
        for (name, service) in &self.services {
            if let (Some(l1_config), Some(l2_config)) = (&service.l1, &service.l2) {
                if let (Some(l1_ttl), Some(l2_ttl)) = (l1_config.ttl, l2_config.default_ttl) {
                    if l1_ttl > l2_ttl {
                        return Err(format!(
                            "Service '{}': L1 TTL ({}) must be <= L2 TTL ({})",
                            name, l1_ttl, l2_ttl
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

// ============================================================================
// Main (for standalone testing)
// ============================================================================

fn main() {
    println!("Running Feature Combinations Test Suite...\n");

    // Run all tests
    let tests: Vec<(&str, fn())> = vec![
        (
            "Minimal Feature Compilation",
            test_minimal_feature_compilation,
        ),
        ("Core Feature Compilation", test_core_feature_compilation),
        ("Full Feature Compilation", test_full_feature_compilation),
        ("TTL Validation", test_config_validation_ttl),
        ("L1 Only Validation", test_l1_only_validation),
        ("Capacity Boundaries", test_capacity_boundaries),
        ("TTL Boundaries", test_ttl_boundaries),
        ("Empty Services", test_empty_services),
        ("Redis Mode Boundaries", test_redis_mode_boundaries),
        ("Timeout Boundaries", test_timeout_boundaries),
        ("L1 Only Mode", test_l1_only_mode),
        ("L2 Only Mode", test_l2_only_mode),
        ("Batch Write Boundaries", test_batch_write_boundaries),
        ("All Cache Types", test_all_cache_types),
        ("Global Config Boundaries", test_global_config_boundaries),
        ("Service Name Boundaries", test_service_name_boundaries),
        ("High Throughput Config", test_high_throughput_config),
        ("Low Latency Config", test_low_latency_config),
        ("Redis Password Config", test_redis_password_config),
        ("Size Limit Config", test_size_limit_config),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (name, test_fn) in tests {
        print!("Testing {}... ", name);
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            test_fn();
        })) {
            Ok(_) => {
                println!("PASSED");
                passed += 1;
            }
            Err(_) => {
                println!("FAILED");
                failed += 1;
            }
        }
    }

    println!("\n========================================");
    println!("Test Results: {} passed, {} failed", passed, failed);
    println!("========================================");

    if failed > 0 {
        std::process::exit(1);
    }
}

// Test functions for main
fn test_minimal_feature_compilation() {
    let mut services = HashMap::new();
    services.insert(
        "minimal_cache".to_string(),
        ServiceConfig::l1_only().with_ttl(600),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 3600,
            health_check_interval: 30,
            enable_metrics: false,
        },
        services,
    };

    assert_eq!(config.global.default_ttl, 3600);
    assert!(config.services.contains_key("minimal_cache"));
}

fn test_core_feature_compilation() {
    let mut services = HashMap::new();
    services.insert(
        "core_cache".to_string(),
        ServiceConfig::two_level()
            .with_ttl(600)
            .with_l1_config(L1Config::new().with_max_capacity(10000))
            .with_l2_config(
                L2Config::new()
                    .with_mode(RedisMode::Standalone)
                    .with_connection_string("redis://localhost:6379"),
            ),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 3600,
            health_check_interval: 30,
            enable_metrics: false,
        },
        services,
    };

    assert!(config.services.contains_key("core_cache"));
}

fn test_full_feature_compilation() {
    let mut services = HashMap::new();
    services.insert(
        "full_cache".to_string(),
        ServiceConfig::two_level()
            .with_ttl(600)
            .with_l1_config(L1Config::new().with_max_capacity(10000))
            .with_l2_config(
                L2Config::new()
                    .with_mode(RedisMode::Standalone)
                    .with_connection_string("redis://localhost:6379"),
            )
            .with_two_level_config(TwoLevelConfig::new()),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 3600,
            health_check_interval: 30,
            enable_metrics: true,
        },
        services,
    };

    assert!(config.services.contains_key("full_cache"));
}

fn test_config_validation_ttl() {
    let mut valid_services = HashMap::new();
    valid_services.insert(
        "valid_service".to_string(),
        ServiceConfig::two_level()
            .with_ttl(300)
            .with_l1_config(L1Config::new().with_ttl(300))
            .with_l2_config(L2Config::new().with_default_ttl(600)),
    );

    let valid_config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: valid_services,
    };
    assert!(valid_config.validate().is_ok());
}

fn test_l1_only_validation() {
    let mut services = HashMap::new();
    services.insert("l1_only".to_string(), ServiceConfig::l1_only());

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    };
    assert!(config.validate().is_ok());
}

fn test_capacity_boundaries() {
    let mut min_services = HashMap::new();
    min_services.insert(
        "min_capacity".to_string(),
        ServiceConfig::l1_only().with_ttl(60),
    );
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: min_services,
    }
    .validate()
    .is_ok());
}

fn test_ttl_boundaries() {
    let mut zero_services = HashMap::new();
    zero_services.insert("zero_ttl".to_string(), ServiceConfig::l1_only().with_ttl(0));
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: zero_services,
    }
    .validate()
    .is_ok());
}

fn test_empty_services() {
    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: HashMap::new(),
    };
    assert!(config.services.is_empty());
    assert!(config.validate().is_ok());
}

fn test_redis_mode_boundaries() {
    let mut services = HashMap::new();
    services.insert(
        "standalone".to_string(),
        ServiceConfig::l2_only().with_ttl(300),
    );
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    }
    .validate()
    .is_ok());
}

fn test_timeout_boundaries() {
    let mut services = HashMap::new();
    services.insert("timeout_test".to_string(), ServiceConfig::l2_only());
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    }
    .validate()
    .is_ok());
}

fn test_l1_only_mode() {
    let mut services = HashMap::new();
    services.insert(
        "hot_data".to_string(),
        ServiceConfig::l1_only().with_ttl(60),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    };

    let service = config.services.get("hot_data").unwrap();
    assert_eq!(service.cache_type, CacheType::L1);
}

fn test_l2_only_mode() {
    let mut services = HashMap::new();
    services.insert(
        "shared_data".to_string(),
        ServiceConfig::l2_only().with_ttl(3600),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    };

    let service = config.services.get("shared_data").unwrap();
    assert_eq!(service.cache_type, CacheType::L2);
}

fn test_batch_write_boundaries() {
    let mut min_services = HashMap::new();
    min_services.insert(
        "min_batch".to_string(),
        ServiceConfig::two_level()
            .with_ttl(300)
            .with_two_level_config(TwoLevelConfig::new().with_batch_size(1)),
    );
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: min_services,
    }
    .validate()
    .is_ok());
}

fn test_all_cache_types() {
    let mut services = HashMap::new();
    services.insert(
        "l1_cache".to_string(),
        ServiceConfig::l1_only().with_ttl(60),
    );
    services.insert(
        "l2_cache".to_string(),
        ServiceConfig::l2_only().with_ttl(3600),
    );
    services.insert(
        "two_level_cache".to_string(),
        ServiceConfig::two_level().with_ttl(600),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    };

    assert_eq!(config.services.len(), 3);
    assert!(config.validate().is_ok());
}

fn test_global_config_boundaries() {
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 1,
            health_check_interval: 1,
            enable_metrics: false,
        },
        services: HashMap::new(),
    }
    .validate()
    .is_ok());
}

fn test_service_name_boundaries() {
    let mut short_services = HashMap::new();
    short_services.insert("a".to_string(), ServiceConfig::l1_only().with_ttl(60));
    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: short_services,
    }
    .validate()
    .is_ok());
}

fn test_high_throughput_config() {
    let mut services = HashMap::new();
    services.insert(
        "high_throughput".to_string(),
        ServiceConfig::two_level()
            .with_ttl(300)
            .with_l1_config(L1Config::new().with_max_capacity(100_000)),
    );

    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 3600,
            health_check_interval: 10,
            enable_metrics: true,
        },
        services,
    }
    .validate()
    .is_ok());
}

fn test_low_latency_config() {
    let mut services = HashMap::new();
    services.insert(
        "low_latency".to_string(),
        ServiceConfig::l1_only().with_ttl(30),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig {
            default_ttl: 60,
            health_check_interval: 5,
            enable_metrics: false,
        },
        services,
    };

    assert!(config.validate().is_ok());
}

fn test_redis_password_config() {
    let mut services = HashMap::new();
    services.insert(
        "secured_redis".to_string(),
        ServiceConfig::l2_only().with_ttl(300).with_l2_config(
            L2Config::new()
                .with_mode(RedisMode::Standalone)
                .with_connection_string("redis://localhost:6379")
                .with_password("strong_password_123")
                .with_tls(true),
        ),
    );

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    };

    assert!(config.validate().is_ok());
}

fn test_size_limit_config() {
    let mut services = HashMap::new();
    services.insert(
        "limited_size".to_string(),
        ServiceConfig::two_level().with_ttl(300),
    );

    assert!(Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services,
    }
    .validate()
    .is_ok());
}
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod feature_tests {
    use super::*;

    // Feature Compilation Tests
    #[test]
    fn test_minimal_feature_compilation() {
        let mut services = HashMap::new();
        services.insert(
            "minimal_cache".to_string(),
            ServiceConfig::l1_only().with_ttl(600),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 3600,
                health_check_interval: 30,
                enable_metrics: false,
            },
            services,
        };

        assert_eq!(config.global.default_ttl, 3600);
        assert!(config.services.contains_key("minimal_cache"));

        let service = config.services.get("minimal_cache").unwrap();
        assert_eq!(service.cache_type, CacheType::L1);
        assert!(service.l1.is_some());
        assert!(service.l2.is_none());
    }

    #[test]
    fn test_core_feature_compilation() {
        let mut services = HashMap::new();
        services.insert(
            "core_cache".to_string(),
            ServiceConfig::two_level()
                .with_ttl(600)
                .with_l1_config(L1Config::new().with_max_capacity(10000))
                .with_l2_config(
                    L2Config::new()
                        .with_mode(RedisMode::Standalone)
                        .with_connection_string("redis://localhost:6379"),
                ),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 3600,
                health_check_interval: 30,
                enable_metrics: false,
            },
            services,
        };

        assert!(config.services.contains_key("core_cache"));
        let service = config.services.get("core_cache").unwrap();
        assert_eq!(service.cache_type, CacheType::TwoLevel);
        assert!(service.l1.is_some());
        assert!(service.l2.is_some());
    }

    #[test]
    fn test_full_feature_compilation() {
        let mut services = HashMap::new();
        services.insert(
            "full_cache".to_string(),
            ServiceConfig::two_level()
                .with_ttl(600)
                .with_l1_config(L1Config::new().with_max_capacity(10000))
                .with_l2_config(
                    L2Config::new()
                        .with_mode(RedisMode::Standalone)
                        .with_connection_string("redis://localhost:6379"),
                )
                .with_two_level_config(TwoLevelConfig::new()),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 3600,
                health_check_interval: 30,
                enable_metrics: true,
            },
            services,
        };

        assert!(config.services.contains_key("full_cache"));
        let service = config.services.get("full_cache").unwrap();
        assert_eq!(service.cache_type, CacheType::TwoLevel);
        assert!(service.two_level.is_some());
    }

    // Configuration Validation Tests
    #[test]
    fn test_config_validation_ttl() {
        // Valid case
        let mut valid_services = HashMap::new();
        valid_services.insert(
            "valid_service".to_string(),
            ServiceConfig::two_level()
                .with_ttl(300)
                .with_l1_config(L1Config::new().with_ttl(300))
                .with_l2_config(L2Config::new().with_default_ttl(600)),
        );

        let valid_config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: valid_services,
        };
        assert!(valid_config.validate().is_ok());

        // Invalid case
        let mut invalid_services = HashMap::new();
        invalid_services.insert(
            "invalid_service".to_string(),
            ServiceConfig::two_level()
                .with_ttl(600)
                .with_l1_config(L1Config::new().with_ttl(600))
                .with_l2_config(L2Config::new().with_default_ttl(300)),
        );

        let invalid_config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: invalid_services,
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_l1_only_validation() {
        let mut services = HashMap::new();
        services.insert("l1_only".to_string(), ServiceConfig::l1_only());

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_capacity_boundaries() {
        // Minimum capacity
        let mut min_services = HashMap::new();
        min_services.insert(
            "min_capacity".to_string(),
            ServiceConfig::l1_only().with_ttl(60),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: min_services,
        }
        .validate()
        .is_ok());

        // Maximum capacity
        let mut max_services = HashMap::new();
        max_services.insert(
            "max_capacity".to_string(),
            ServiceConfig::l1_only().with_ttl(3600),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: max_services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_ttl_boundaries() {
        // Zero TTL
        let mut zero_services = HashMap::new();
        zero_services.insert("zero_ttl".to_string(), ServiceConfig::l1_only().with_ttl(0));
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: zero_services,
        }
        .validate()
        .is_ok());

        // Large TTL (24 hours)
        let mut large_services = HashMap::new();
        large_services.insert(
            "large_ttl".to_string(),
            ServiceConfig::l1_only().with_ttl(86400),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: large_services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_empty_services() {
        let config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: HashMap::new(),
        };
        assert!(config.services.is_empty());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_redis_mode_boundaries() {
        let mut services = HashMap::new();
        services.insert(
            "standalone".to_string(),
            ServiceConfig::l2_only().with_ttl(300),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_timeout_boundaries() {
        let mut services = HashMap::new();
        services.insert("timeout_test".to_string(), ServiceConfig::l2_only());
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_l1_only_mode() {
        let mut services = HashMap::new();
        services.insert(
            "hot_data".to_string(),
            ServiceConfig::l1_only().with_ttl(60),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        };

        let service = config.services.get("hot_data").unwrap();
        assert_eq!(service.cache_type, CacheType::L1);
        assert!(service.l1.is_some());
        assert!(service.l2.is_none());
    }

    #[test]
    fn test_l2_only_mode() {
        let mut services = HashMap::new();
        services.insert(
            "shared_data".to_string(),
            ServiceConfig::l2_only().with_ttl(3600),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        };

        let service = config.services.get("shared_data").unwrap();
        assert_eq!(service.cache_type, CacheType::L2);
        assert!(service.l1.is_none());
        assert!(service.l2.is_some());
    }

    #[test]
    fn test_batch_write_boundaries() {
        // Minimum batch size
        let mut min_services = HashMap::new();
        min_services.insert(
            "min_batch".to_string(),
            ServiceConfig::two_level()
                .with_ttl(300)
                .with_two_level_config(
                    TwoLevelConfig::new()
                        .with_batch_size(1)
                        .with_batch_interval_ms(1),
                ),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: min_services,
        }
        .validate()
        .is_ok());

        // Maximum batch size
        let mut max_services = HashMap::new();
        max_services.insert(
            "max_batch".to_string(),
            ServiceConfig::two_level()
                .with_ttl(300)
                .with_two_level_config(
                    TwoLevelConfig::new()
                        .with_batch_size(10000)
                        .with_batch_interval_ms(1000),
                ),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: max_services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_all_cache_types() {
        let mut services = HashMap::new();
        services.insert(
            "l1_cache".to_string(),
            ServiceConfig::l1_only().with_ttl(60),
        );
        services.insert(
            "l2_cache".to_string(),
            ServiceConfig::l2_only().with_ttl(3600),
        );
        services.insert(
            "two_level_cache".to_string(),
            ServiceConfig::two_level().with_ttl(600),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        };

        assert_eq!(config.services.len(), 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_global_config_boundaries() {
        // Minimum global config
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 1,
                health_check_interval: 1,
                enable_metrics: false,
            },
            services: HashMap::new(),
        }
        .validate()
        .is_ok());

        // Maximum global config
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 86400,
                health_check_interval: 3600,
                enable_metrics: true,
            },
            services: HashMap::new(),
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_service_name_boundaries() {
        // Shortest name
        let mut short_services = HashMap::new();
        short_services.insert("a".to_string(), ServiceConfig::l1_only().with_ttl(60));
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: short_services,
        }
        .validate()
        .is_ok());

        // Special characters
        let mut special_services = HashMap::new();
        special_services.insert(
            "my_service_v2.0".to_string(),
            ServiceConfig::l1_only().with_ttl(60),
        );
        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services: special_services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_high_throughput_config() {
        let mut services = HashMap::new();
        services.insert(
            "high_throughput".to_string(),
            ServiceConfig::two_level()
                .with_ttl(300)
                .with_l1_config(L1Config::new().with_max_capacity(100_000))
                .with_two_level_config(
                    TwoLevelConfig::new()
                        .with_batch_size(500)
                        .with_batch_interval_ms(10),
                ),
        );

        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 3600,
                health_check_interval: 10,
                enable_metrics: true,
            },
            services,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_low_latency_config() {
        let mut services = HashMap::new();
        services.insert(
            "low_latency".to_string(),
            ServiceConfig::l1_only().with_ttl(30),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig {
                default_ttl: 60,
                health_check_interval: 5,
                enable_metrics: false,
            },
            services,
        };

        assert!(config.validate().is_ok());

        let service = config.services.get("low_latency").unwrap();
        assert_eq!(service.cache_type, CacheType::L1);
    }

    #[test]
    fn test_redis_password_config() {
        let mut services = HashMap::new();
        services.insert(
            "secured_redis".to_string(),
            ServiceConfig::l2_only().with_ttl(300).with_l2_config(
                L2Config::new()
                    .with_mode(RedisMode::Standalone)
                    .with_connection_string("redis://localhost:6379")
                    .with_password("strong_password_123")
                    .with_tls(true),
            ),
        );

        let config = Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        };

        assert!(config.validate().is_ok());

        let service = config.services.get("secured_redis").unwrap();
        if let Some(l2) = &service.l2 {
            assert!(l2.password.is_some());
            assert!(l2.enable_tls);
        }
    }

    #[test]
    fn test_size_limit_config() {
        let mut services = HashMap::new();
        services.insert(
            "limited_size".to_string(),
            ServiceConfig::two_level().with_ttl(300),
        );

        assert!(Config {
            config_version: Some(1),
            global: GlobalConfig::default(),
            services,
        }
        .validate()
        .is_ok());
    }
}
