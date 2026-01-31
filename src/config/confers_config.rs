// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration structures for the cache library.
//
// Note: The confers library has known issues with its Config derive macro
// that prevent proper usage. This module provides compatible structures
// using standard serde derive for now.

use serde::{Deserialize, Serialize};

/// Global configuration settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub default_ttl: u64,
    pub default_tti: u64,
    pub health_check_interval: u32,
}

/// Backend configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendConfig {
    pub l1_type: String,
    pub l1_options: serde_json::Value,
    pub l2_type: String,
    pub l2_options: serde_json::Value,
    pub l1_enabled: bool,
    pub l2_enabled: bool,
}

/// Service-specific configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub cache_type: String,
    pub ttl: Option<u64>,
    pub max_capacity: Option<u64>,
    pub enable_metrics: bool,
}

/// Performance settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_operations: usize,
    pub command_timeout: u64,
    pub enable_prefetching: bool,
}

/// Security settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub connection_string_redaction: bool,
    pub enable_rate_limiting: u64,
    pub rate_limit_max_requests: u64,
    pub rate_limit_window_size: u64,
}

/// Metrics settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub detailed: bool,
    pub export_format: String,
    pub export_endpoint: Option<String>,
}

/// Recovery settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub enable_wal: bool,
    pub wal_directory: String,
    pub enable_auto_recovery: bool,
}

/// Unified configuration combining all sections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnifiedConfig {
    pub global: GlobalConfig,
    pub backend: BackendConfig,
    pub services: std::collections::HashMap<String, ServiceConfig>,
}
