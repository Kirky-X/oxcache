// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Integration Tests Module
//
// Contains all integration tests for the cache system.
// These tests verify the interaction between different components.

// Common modules shared by integration tests - declared as pub so #[path] modules can access
#[path = "common/mod.rs"]
pub mod common;

#[path = "integration/batch_write_test.rs"]
mod batch_write_test;
#[path = "integration/cli_test.rs"]
mod cli_test;
#[path = "integration/comprehensive_test.rs"]
mod comprehensive_test;
#[path = "integration/degradation_tests.rs"]
mod degradation_tests;
#[path = "integration/http_cache_test.rs"]
mod http_cache_test;
#[path = "integration/invalidation_test.rs"]
mod invalidation_test;
#[path = "integration/lifecycle_test.rs"]
mod lifecycle_test;
#[path = "integration/lock_warmup_test.rs"]
mod lock_warmup_test;
#[path = "integration/manual_control_test.rs"]
mod manual_control_test;
#[path = "integration/recovery_test.rs"]
mod recovery_test;
#[path = "integration/redis_client_comprehensive_test.rs"]
mod redis_client_comprehensive_test;

#[path = "integration/single_flight_test.rs"]
mod single_flight_test;
#[path = "integration/ttl_control_test.rs"]
mod ttl_control_test;
#[path = "integration/two_level_test.rs"]
mod two_level_test;
#[path = "integration/version_test.rs"]
mod version_test;
#[path = "integration/wal_test.rs"]
mod wal_test;

#[path = "integration/l2_backend_test.rs"]
mod l2_backend_test;

#[path = "integration/chain_cache_integration_test.rs"]
mod chain_cache_integration_test;

#[path = "integration/oxcache_builder_test.rs"]
mod oxcache_builder_test;

#[path = "integration/redis_client_test.rs"]
mod redis_client_test;
