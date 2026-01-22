// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 降级策略和健康状态测试
//
// 合并自:
use oxcache::recovery::health::{
    HealthCheckableBackend, HealthChecker, HealthState, WalReplayableBackendTrait,
};
use oxcache::recovery::wal::{WalEntry, WalManager};
/// - tests/degradation_test.rs
/// - tests/degradation_integration_test.rs
/// - tests/health_state_test.rs
use oxcache::CacheError;
use oxcache::{L2Config, RedisMode};
