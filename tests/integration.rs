// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Integration Tests Module
//
// Contains all integration tests for the cache system.
// These tests verify the interaction between different components.

// Common modules shared by integration tests - declared as pub so #[path] modules can access
#[path = "common/mod.rs"]
pub mod common;

// --- Core integration tests ---
#[cfg(feature = "redis")]
#[path = "integration/batch_write_test.rs"]
mod batch_write_test;
#[cfg(feature = "redis")]
#[path = "integration/chain_cache_integration_test.rs"]
mod chain_cache_integration_test;
#[path = "integration/comprehensive_test.rs"]
mod comprehensive_test;
#[path = "integration/degradation_tests.rs"]
mod degradation_tests;
#[cfg(feature = "redis")]
#[path = "integration/invalidation_test.rs"]
mod invalidation_test;
#[cfg(feature = "redis")]
#[path = "integration/recovery_test.rs"]
mod recovery_test;
#[path = "integration/sync_api_test.rs"]
mod sync_api_test;
#[cfg(feature = "redis")]
#[path = "integration/two_level_test.rs"]
mod two_level_test;
#[cfg(feature = "redis")]
#[path = "integration/version_test.rs"]
mod version_test;

// --- Redis & lock tests ---
#[cfg(feature = "redis")]
#[path = "integration/redis/dist_lock_test.rs"]
mod dist_lock_test;
#[cfg(feature = "redis")]
#[path = "integration/redis/lock_warmup_test.rs"]
mod lock_warmup_test;
#[cfg(feature = "redis")]
#[path = "integration/redis/redis_client_comprehensive_test.rs"]
mod redis_client_comprehensive_test;
#[cfg(feature = "redis")]
#[path = "integration/redis/redis_cluster_test.rs"]
mod redis_cluster_test;
#[cfg(feature = "redis")]
#[path = "integration/redis/redis_sentinel_test.rs"]
mod redis_sentinel_test;
#[cfg(feature = "redis")]
#[path = "integration/redis/redis_version_compatibility_test.rs"]
mod redis_version_compatibility_test;

// --- TTL tests ---
#[path = "integration/ttl/ttl_consistency_test.rs"]
mod ttl_consistency_test;
#[path = "integration/ttl/ttl_expire_test.rs"]
mod ttl_expire_test;

// --- Alternative backend tests ---
#[cfg(feature = "aerospike")]
#[path = "integration/backend/aerospike_test.rs"]
mod aerospike_test;
#[cfg(feature = "dragonfly")]
#[path = "integration/backend/dragonfly_test.rs"]
mod dragonfly_test;
#[cfg(feature = "redis")]
#[path = "integration/backend/l2_backend_test.rs"]
mod l2_backend_test;
#[cfg(feature = "redis")]
#[path = "integration/backend/valkey_test.rs"]
mod valkey_test;
