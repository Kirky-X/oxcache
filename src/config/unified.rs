//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Unified configuration system that consolidates all configuration functionality

use crate::backend::{MemoryBackendType, RedisConfig};
use crate::error::{CacheError, Result};
use crate::serialization::SerializationFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Unified cache configuration
///
/// This provides a centralized way to configure all aspects of the cache system
/// including backend selection, performance tuning, and feature flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedConfig {
    /// Backend configuration
    pub backend: BackendConfig,
    /// Performance configuration
    pub performance: PerformanceConfig,
    /// Feature flags
    pub features: FeatureConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    /// Security configuration
    pub security: SecurityConfig,
}

/// Backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Backend type
    pub backend_type: BackendType,
    /// Memory backend configuration
    pub memory: Option<MemoryBackendConfig>,
    /// Redis backend configuration
    pub redis: Option<RedisBackendConfig>,
    /// Tiered backend configuration
    pub tiered: Option<TieredBackendConfig>,
}

/// Backend type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// Memory-only backend
    Memory,
    /// Redis-only backend
    Redis,
    /// Tiered backend (L1 + L2)
    Tiered,
    /// Custom backend
    Custom,
}

/// Memory backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBackendConfig {
    /// Backend type
    pub backend_type: crate::backend::MemoryBackendType,
    /// Maximum capacity (number of entries)
    pub capacity: u64,
    /// Default TTL for entries
    pub default_ttl: Option<Duration>,
    /// Time-to-idle for entries (Moka only)
    pub time_to_idle: Option<Duration>,
    /// Enable statistics collection
    pub enable_stats: bool,
}

/// Redis backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisBackendConfig {
    /// Connection configuration
    pub connection: RedisConnectionConfig,
    /// Pool configuration
    pub pool: RedisPoolConfig,
    /// Default TTL for entries
    pub default_ttl: Option<Duration>,
    /// Enable key prefixing
    pub key_prefix: Option<String>,
    /// Enable compression
    pub enable_compression: bool,
}

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConnectionConfig {
    /// Connection strings
    pub connection_strings: Vec<String>,
    /// Connection mode
    pub mode: String, // "standalone", "sentinel", "cluster"
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Command timeout
    pub command_timeout: Duration,
    /// Authentication password
    pub password: Option<String>,
    /// Database number
    pub database: Option<i64>,
    /// Connection name
    pub connection_name: Option<String>,
}

/// Redis pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisPoolConfig {
    /// Maximum pool size
    pub max_size: usize,
    /// Minimum pool size
    pub min_size: usize,
    /// Connection idle timeout
    pub idle_timeout: Option<Duration>,
    /// Connection max lifetime
    pub max_lifetime: Option<Duration>,
}

/// Tiered backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredBackendConfig {
    /// L1 backend configuration
    pub l1: MemoryBackendConfig,
    /// L2 backend configuration
    pub l2: RedisBackendConfig,
    /// Enable auto-promotion from L2 to L1
    pub auto_promote: bool,
    /// Write-through strategy
    pub write_through: bool,
    /// Write-behind buffer size
    pub write_behind_buffer_size: Option<usize>,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Serialization configuration
    pub serialization: SerializationConfig,
    /// Batch operation configuration
    pub batch: BatchConfig,
    /// Concurrency configuration
    pub concurrency: ConcurrencyConfig,
}

/// Serialization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializationConfig {
    /// Serialization format
    pub format: SerializationFormat,
    /// Enable zero-copy operations
    pub enable_zero_copy: bool,
    /// Enable compression
    pub enable_compression: bool,
    /// Compression threshold (bytes)
    pub compression_threshold: usize,
}

/// Batch operation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Enable batch operations
    pub enabled: bool,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Batch timeout
    pub batch_timeout: Duration,
    /// Enable parallel processing
    pub parallel_processing: bool,
}

/// Concurrency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent operations
    pub max_concurrent_ops: Option<usize>,
    /// Operation timeout
    pub operation_timeout: Option<Duration>,
    /// Enable operation queuing
    pub enable_queuing: bool,
    /// Maximum queue size
    pub max_queue_size: Option<usize>,
}

/// Feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Enable health checks
    pub enable_health_checks: bool,
    /// Enable distributed locking
    pub enable_locking: bool,
    /// Enable prefetching
    pub enable_prefetching: bool,
    /// Enable cache warming
    pub enable_warming: bool,
    /// Enable TTL management
    pub enable_ttl_management: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Metrics collection configuration
    pub metrics: MetricsConfig,
    /// Health check configuration
    pub health_check: HealthCheckConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable detailed metrics
    pub detailed: bool,
    /// Metrics export format
    pub export_format: MetricsExportFormat,
    /// Export interval
    pub export_interval: Duration,
    /// Retention period
    pub retention_period: Option<Duration>,
}

/// Metrics export format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricsExportFormat {
    Prometheus,
    Json,
    InfluxDB,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check interval
    pub interval: Duration,
    /// Health check timeout
    pub timeout: Duration,
    /// Number of consecutive failures before marking as unhealthy
    pub failure_threshold: u32,
    /// Enable automatic recovery
    pub enable_recovery: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: LogLevel,
    /// Enable operation logging
    pub enable_operation_logging: bool,
    /// Enable performance logging
    pub enable_performance_logging: bool,
    /// Log format
    pub format: LogFormat,
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Log format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    Plain,
    Json,
    Compact,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable encryption
    pub enable_encryption: bool,
    /// Encryption configuration
    pub encryption: Option<EncryptionConfig>,
    /// Access control configuration
    pub access_control: Option<AccessControlConfig>,
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Encryption algorithm
    pub algorithm: String,
    /// Key source
    pub key_source: KeySource,
}

/// Key source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeySource {
    /// Static key
    Static(String),
    /// Environment variable
    Environment(String),
    /// Key management service
    Kms(String),
}

/// Access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// Enable authentication
    pub enable_auth: bool,
    /// Enable authorization
    pub enable_authz: bool,
    /// Access control list
    pub acl: HashMap<String, AccessRule>,
}

/// Access rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRule {
    /// Allowed operations
    pub allowed_ops: Vec<String>,
    /// Resource pattern
    pub resource_pattern: String,
    /// Time restrictions
    pub time_restrictions: Option<TimeRestriction>,
}

/// Time restriction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestriction {
    /// Start time (HH:MM)
    pub start_time: String,
    /// End time (HH:MM)
    pub end_time: String,
    /// Days of week
    pub days_of_week: Vec<u8>, // 0 = Sunday, 6 = Saturday
}

impl Default for UnifiedConfig {
    fn default() -> Self {
        Self {
            backend: BackendConfig::default(),
            performance: PerformanceConfig::default(),
            features: FeatureConfig::default(),
            monitoring: MonitoringConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: BackendType::Memory,
            memory: Some(MemoryBackendConfig::default()),
            redis: None,
            tiered: None,
        }
    }
}

impl Default for MemoryBackendConfig {
    fn default() -> Self {
        Self {
            backend_type: crate::backend::MemoryBackendType::Moka,
            capacity: 10_000,
            default_ttl: None,
            time_to_idle: None,
            enable_stats: true,
        }
    }
}

impl Default for RedisBackendConfig {
    fn default() -> Self {
        Self {
            connection: RedisConnectionConfig::default(),
            pool: RedisPoolConfig::default(),
            default_ttl: None,
            key_prefix: None,
            enable_compression: false,
        }
    }
}

impl Default for RedisConnectionConfig {
    fn default() -> Self {
        Self {
            connection_strings: vec!["redis://localhost:6379".to_string()],
            mode: "standalone".to_string(),
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(5),
            password: None,
            database: Some(0),
            connection_name: Some("oxcache".to_string()),
        }
    }
}

impl Default for RedisPoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_size: 1,
            idle_timeout: Some(Duration::from_secs(300)),
            max_lifetime: Some(Duration::from_secs(1800)),
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            serialization: SerializationConfig::default(),
            batch: BatchConfig::default(),
            concurrency: ConcurrencyConfig::default(),
        }
    }
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            format: SerializationFormat::Json,
            enable_zero_copy: false,
            enable_compression: false,
            compression_threshold: 1024,
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_batch_size: 100,
            batch_timeout: Duration::from_millis(100),
            parallel_processing: false,
        }
    }
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_ops: None,
            operation_timeout: Some(Duration::from_secs(30)),
            enable_queuing: true,
            max_queue_size: Some(1000),
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            enable_health_checks: true,
            enable_locking: false,
            enable_prefetching: false,
            enable_warming: false,
            enable_ttl_management: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            health_check: HealthCheckConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            detailed: true,
            export_format: MetricsExportFormat::Prometheus,
            export_interval: Duration::from_secs(60),
            retention_period: Some(Duration::from_secs(3600)),
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            enable_recovery: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            enable_operation_logging: false,
            enable_performance_logging: true,
            format: LogFormat::Plain,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_encryption: false,
            encryption: None,
            access_control: None,
        }
    }
}

/// Configuration builder
#[derive(Debug, Clone)]
pub struct UnifiedConfigBuilder {
    config: UnifiedConfig,
}

impl UnifiedConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self {
            config: UnifiedConfig::default(),
        }
    }

    /// Set the backend type
    pub fn backend_type(mut self, backend_type: BackendType) -> Self {
        self.config.backend.backend_type = backend_type;
        self
    }

    /// Set memory backend configuration
    pub fn memory_backend(mut self, config: MemoryBackendConfig) -> Self {
        self.config.backend.memory = Some(config);
        self
    }

    /// Set Redis backend configuration
    pub fn redis_backend(mut self, config: RedisBackendConfig) -> Self {
        self.config.backend.redis = Some(config);
        self
    }

    /// Set tiered backend configuration
    pub fn tiered_backend(mut self, config: TieredBackendConfig) -> Self {
        self.config.backend.tiered = Some(config);
        self
    }

    /// Set serialization configuration
    pub fn serialization(mut self, config: SerializationConfig) -> Self {
        self.config.performance.serialization = config;
        self
    }

    /// Set batch configuration
    pub fn batch(mut self, config: BatchConfig) -> Self {
        self.config.performance.batch = config;
        self
    }

    /// Set concurrency configuration
    pub fn concurrency(mut self, config: ConcurrencyConfig) -> Self {
        self.config.performance.concurrency = config;
        self
    }

    /// Set feature configuration
    pub fn features(mut self, config: FeatureConfig) -> Self {
        self.config.features = config;
        self
    }

    /// Set monitoring configuration
    pub fn monitoring(mut self, config: MonitoringConfig) -> Self {
        self.config.monitoring = config;
        self
    }

    /// Set security configuration
    pub fn security(mut self, config: SecurityConfig) -> Self {
        self.config.security = config;
        self
    }

    /// Build the configuration
    pub fn build(self) -> UnifiedConfig {
        self.config
    }
}

impl Default for UnifiedConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration validation
impl UnifiedConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate backend configuration
        match self.backend.backend_type {
            BackendType::Memory => {
                if self.backend.memory.is_none() {
                    return Err(CacheError::ConfigError(
                        "Memory backend configuration is required".to_string(),
                    ));
                }
            }
            BackendType::Redis => {
                if self.backend.redis.is_none() {
                    return Err(CacheError::ConfigError(
                        "Redis backend configuration is required".to_string(),
                    ));
                }
            }
            BackendType::Tiered => {
                if self.backend.tiered.is_none() {
                    return Err(CacheError::ConfigError(
                        "Tiered backend configuration is required".to_string(),
                    ));
                }
            }
            BackendType::Custom => {
                // Custom backend validation would go here
            }
        }

        // Validate performance configuration
        if self.performance.batch.enabled {
            if self.performance.batch.max_batch_size == 0 {
                return Err(CacheError::ConfigError(
                    "Batch max size must be greater than 0".to_string(),
                ));
            }
        }

        // Validate monitoring configuration
        if self.monitoring.metrics.export_interval == Duration::ZERO {
            return Err(CacheError::ConfigError(
                "Metrics export interval must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a memory-only configuration
    pub fn memory_only() -> UnifiedConfigBuilder {
        UnifiedConfigBuilder::new()
            .backend_type(BackendType::Memory)
            .memory_backend(MemoryBackendConfig::default())
    }

    /// Create a Redis-only configuration
    pub fn redis_only() -> UnifiedConfigBuilder {
        UnifiedConfigBuilder::new()
            .backend_type(BackendType::Redis)
            .redis_backend(RedisBackendConfig::default())
    }

    /// Create a tiered configuration
    pub fn tiered() -> UnifiedConfigBuilder {
        UnifiedConfigBuilder::new()
            .backend_type(BackendType::Tiered)
            .tiered_backend(TieredBackendConfig {
                l1: MemoryBackendConfig::default(),
                l2: RedisBackendConfig::default(),
                auto_promote: true,
                write_through: false,
                write_behind_buffer_size: None,
            })
    }

    /// Convert to Redis configuration
    pub fn to_redis_config(&self) -> Option<RedisConfig> {
        self.backend.redis.as_ref().map(|redis_config| {
            let mode = match redis_config.connection.mode.as_str() {
                "standalone" => crate::backend::ClientRedisMode::Standalone,
                "sentinel" => crate::backend::ClientRedisMode::Sentinel,
                "cluster" => crate::backend::ClientRedisMode::Cluster,
                _ => crate::backend::ClientRedisMode::Standalone,
            };

            RedisConfig {
                connection_strings: redis_config.connection.connection_strings.clone(),
                mode,
                connect_timeout: redis_config.connection.connect_timeout,
                command_timeout: redis_config.connection.command_timeout,
                max_pool_size: Some(redis_config.pool.max_size),
                min_pool_size: Some(redis_config.pool.min_size),
                connection_name: redis_config.connection.connection_name.clone(),
                password: redis_config.connection.password.clone(),
                database: redis_config.connection.database,
            }
        })
    }
}

/// Configuration convenience functions
pub mod convenience {
    use super::*;

    /// Create a simple memory cache configuration
    pub fn simple_memory() -> UnifiedConfig {
        UnifiedConfig::memory_only().build()
    }

    /// Create a simple Redis cache configuration
    pub fn simple_redis() -> UnifiedConfig {
        UnifiedConfig::redis_only().build()
    }

    /// Create a simple tiered cache configuration
    pub fn simple_tiered() -> UnifiedConfig {
        UnifiedConfig::tiered().build()
    }

    /// Create a high-performance memory configuration
    #[cfg(feature = "bincode")]
    pub fn high_performance_memory() -> UnifiedConfig {
        UnifiedConfig::memory_only()
            .memory_backend(MemoryBackendConfig {
                backend_type: crate::backend::MemoryBackendType::DashMap,
                capacity: 100_000,
                default_ttl: Some(Duration::from_secs(3600)),
                time_to_idle: Some(Duration::from_secs(1800)),
                enable_stats: true,
            })
            .serialization(SerializationConfig {
                format: SerializationFormat::Bincode,
                enable_zero_copy: true,
                enable_compression: false,
                compression_threshold: 1024,
            })
            .batch(BatchConfig {
                enabled: true,
                max_batch_size: 1000,
                batch_timeout: Duration::from_millis(50),
                parallel_processing: true,
            })
            .build()
    }

    #[cfg(not(feature = "bincode"))]
    pub fn high_performance_memory() -> UnifiedConfig {
        UnifiedConfig::memory_only()
            .memory_backend(MemoryBackendConfig {
                backend_type: crate::backend::MemoryBackendType::DashMap,
                capacity: 100_000,
                default_ttl: Some(Duration::from_secs(3600)),
                time_to_idle: Some(Duration::from_secs(1800)),
                enable_stats: true,
            })
            .serialization(SerializationConfig {
                format: SerializationFormat::Json,
                enable_zero_copy: true,
                enable_compression: false,
                compression_threshold: 1024,
            })
            .batch(BatchConfig {
                enabled: true,
                max_batch_size: 1000,
                batch_timeout: Duration::from_millis(50),
                parallel_processing: true,
            })
            .build()
    }

    /// Create a production-ready Redis configuration
    pub fn production_redis() -> UnifiedConfig {
        UnifiedConfig::redis_only()
            .redis_backend(RedisBackendConfig {
                connection: RedisConnectionConfig {
                    connection_strings: vec!["redis://localhost:6379".to_string()],
                    mode: "standalone".to_string(),
                    connect_timeout: Duration::from_secs(10),
                    command_timeout: Duration::from_secs(5),
                    password: None,
                    database: Some(0),
                    connection_name: Some("oxcache-prod".to_string()),
                },
                pool: RedisPoolConfig {
                    max_size: 20,
                    min_size: 5,
                    idle_timeout: Some(Duration::from_secs(300)),
                    max_lifetime: Some(Duration::from_secs(3600)),
                },
                default_ttl: Some(Duration::from_secs(3600)),
                key_prefix: Some("oxcache:".to_string()),
                enable_compression: true,
            })
            .features(FeatureConfig {
                enable_metrics: true,
                enable_health_checks: true,
                enable_locking: true,
                enable_prefetching: false,
                enable_warming: true,
                enable_ttl_management: true,
            })
            .monitoring(MonitoringConfig {
                metrics: MetricsConfig {
                    detailed: true,
                    export_format: MetricsExportFormat::Prometheus,
                    export_interval: Duration::from_secs(30),
                    retention_period: Some(Duration::from_secs(7200)),
                },
                health_check: HealthCheckConfig {
                    interval: Duration::from_secs(15),
                    timeout: Duration::from_secs(3),
                    failure_threshold: 3,
                    enable_recovery: true,
                },
                logging: LoggingConfig {
                    level: LogLevel::Info,
                    enable_operation_logging: false,
                    enable_performance_logging: true,
                    format: LogFormat::Json,
                },
            })
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = UnifiedConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.backend.backend_type, BackendType::Memory);
        assert!(config.backend.memory.is_some());
        assert!(config.features.enable_metrics);
    }

    #[test]
    fn test_memory_config() {
        let config = UnifiedConfig::memory_only()
            .memory_backend(MemoryBackendConfig {
                engine: MemoryEngine::DashMap,
                capacity: 5000,
                default_ttl: Some(Duration::from_secs(1800)),
                time_to_idle: None,
                enable_stats: true,
            })
            .build();

        assert!(config.validate().is_ok());
        assert_eq!(config.backend.backend_type, BackendType::Memory);
        assert_eq!(config.backend.memory.as_ref().unwrap().capacity, 5000);
    }

    #[test]
    fn test_redis_config() {
        let config = UnifiedConfig::redis_only()
            .redis_backend(RedisBackendConfig {
                connection: RedisConnectionConfig {
                    connection_strings: vec!["redis://localhost:6380".to_string()],
                    mode: "standalone".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .build();

        assert!(config.validate().is_ok());
        assert_eq!(config.backend.backend_type, BackendType::Redis);
        assert_eq!(
            config
                .backend
                .redis
                .as_ref()
                .unwrap()
                .connection
                .connection_strings[0],
            "redis://localhost:6380"
        );
    }

    #[test]
    fn test_tiered_config() {
        let config = UnifiedConfig::tiered()
            .tiered_backend(TieredBackendConfig {
                l1: MemoryBackendConfig {
                    capacity: 1000,
                    ..Default::default()
                },
                l2: RedisBackendConfig::default(),
                auto_promote: true,
                write_through: false,
                write_behind_buffer_size: Some(100),
            })
            .build();

        assert!(config.validate().is_ok());
        assert_eq!(config.backend.backend_type, BackendType::Tiered);
        assert_eq!(config.backend.tiered.as_ref().unwrap().l1.capacity, 1000);
        assert!(config.backend.tiered.as_ref().unwrap().auto_promote);
    }

    #[test]
    fn test_invalid_config() {
        let config = UnifiedConfig {
            backend: BackendConfig {
                backend_type: BackendType::Redis,
                memory: None,
                redis: None, // Missing Redis config
                tiered: None,
            },
            ..Default::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    #[cfg(feature = "bincode")]
    fn test_convenience_functions() {
        let memory_config = convenience::simple_memory();
        assert_eq!(memory_config.backend.backend_type, BackendType::Memory);

        let redis_config = convenience::simple_redis();
        assert_eq!(redis_config.backend.backend_type, BackendType::Redis);

        let tiered_config = convenience::simple_tiered();
        assert_eq!(tiered_config.backend.backend_type, BackendType::Tiered);

        let high_perf_config = convenience::high_performance_memory();
        assert_eq!(
            high_perf_config
                .backend
                .memory
                .as_ref()
                .unwrap()
                .backend_type,
            crate::backend::MemoryBackendType::DashMap
        );
        assert_eq!(
            high_perf_config.performance.serialization.format,
            SerializationFormat::Bincode
        );

        let prod_config = convenience::production_redis();
        assert_eq!(
            prod_config.backend.redis.as_ref().unwrap().pool.max_size,
            20
        );
        assert!(prod_config.features.enable_locking);
    }

    #[test]
    #[cfg(not(feature = "bincode"))]
    fn test_convenience_functions() {
        let memory_config = convenience::simple_memory();
        assert_eq!(memory_config.backend.backend_type, BackendType::Memory);

        let redis_config = convenience::simple_redis();
        assert_eq!(redis_config.backend.backend_type, BackendType::Redis);

        let tiered_config = convenience::simple_tiered();
        assert_eq!(tiered_config.backend.backend_type, BackendType::Tiered);

        let high_perf_config = convenience::high_performance_memory();
        assert_eq!(
            high_perf_config
                .backend
                .memory
                .as_ref()
                .unwrap()
                .backend_type,
            crate::backend::MemoryBackendType::DashMap
        );
        assert_eq!(
            high_perf_config.performance.serialization.format,
            SerializationFormat::Json
        );

        let prod_config = convenience::production_redis();
        assert_eq!(
            prod_config.backend.redis.as_ref().unwrap().pool.max_size,
            20
        );
        assert!(prod_config.features.enable_locking);
    }

    #[test]
    fn test_config_serialization() {
        let config = UnifiedConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: UnifiedConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(
            config.backend.backend_type,
            deserialized.backend.backend_type
        );
        assert_eq!(
            config.features.enable_metrics,
            deserialized.features.enable_metrics
        );
    }

    #[test]
    fn test_redis_config_conversion() {
        let config = UnifiedConfig::redis_only()
            .redis_backend(RedisBackendConfig {
                connection: RedisConnectionConfig {
                    connection_strings: vec!["redis://localhost:6379".to_string()],
                    mode: "cluster".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .build();

        let redis_config = config.to_redis_config().unwrap();
        assert_eq!(redis_config.connection_strings[0], "redis://localhost:6379");
        assert!(matches!(
            redis_config.mode,
            crate::backend::UnifiedRedisMode::Cluster
        ));
    }
}
