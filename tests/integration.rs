// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Integration Tests Module
//
// Contains all integration tests for the cache system.
// These tests verify the interaction between different components.

// Common modules shared by integration tests - declared as pub so #[path] modules can access
#[path = "common/mod.rs"]
pub mod common;

#[cfg(feature = "redis")]
#[path = "integration/batch_write_test.rs"]
mod batch_write_test;
#[path = "integration/comprehensive_test.rs"]
mod comprehensive_test;
#[path = "integration/degradation_tests.rs"]
mod degradation_tests;
#[cfg(feature = "redis")]
#[path = "integration/invalidation_test.rs"]
mod invalidation_test;
#[path = "integration/lifecycle_test.rs"]
mod lifecycle_test;
#[cfg(feature = "redis")]
#[path = "integration/lock_warmup_test.rs"]
mod lock_warmup_test;
#[path = "integration/manual_control_test.rs"]
mod manual_control_test;
#[cfg(feature = "redis")]
#[path = "integration/recovery_test.rs"]
mod recovery_test;
#[cfg(feature = "redis")]
#[path = "integration/redis_client_comprehensive_test.rs"]
mod redis_client_comprehensive_test;

#[cfg(feature = "redis")]
#[path = "integration/l2_backend_test.rs"]
mod l2_backend_test;
#[cfg(feature = "redis")]
#[path = "integration/single_flight_test.rs"]
mod single_flight_test;
#[cfg(feature = "redis")]
#[path = "integration/ttl_control_test.rs"]
mod ttl_control_test;
#[cfg(feature = "redis")]
#[path = "integration/two_level_test.rs"]
mod two_level_test;
#[cfg(feature = "redis")]
#[path = "integration/version_test.rs"]
mod version_test;

#[cfg(feature = "redis")]
#[path = "integration/chain_cache_integration_test.rs"]
mod chain_cache_integration_test;
#[cfg(feature = "redis")]
#[path = "integration/redis_version_compatibility_test.rs"]
mod redis_version_compatibility_test;
#[cfg(feature = "redis")]
#[path = "integration/redis_cluster_test.rs"]
mod redis_cluster_test;
#[cfg(feature = "redis")]
#[path = "integration/redis_sentinel_test.rs"]
mod redis_sentinel_test;
#[cfg(feature = "redis")]
#[path = "integration/valkey_test.rs"]
mod valkey_test;
#[cfg(feature = "dragonfly")]
#[path = "integration/dragonfly_test.rs"]
mod dragonfly_test;
#[cfg(feature = "aerospike")]
#[path = "integration/aerospike_test.rs"]
mod aerospike_test;
