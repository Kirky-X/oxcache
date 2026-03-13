// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Unit Tests Module
//
// Contains all unit tests for the cache system.
// These tests verify individual component functionality.

#[path = "unit/serialization_test.rs"]
mod serialization_test;

#[path = "unit/bloom_filter_test.rs"]
mod bloom_filter_test;

#[path = "unit/smart_strategy_test.rs"]
mod smart_strategy_test;

#[path = "unit/rate_limiting_test.rs"]
mod rate_limiting_test;

#[path = "unit/error_test.rs"]
mod error_test;

#[path = "unit/key_generator_test.rs"]
mod key_generator_test;

#[path = "unit/depth_limited_test.rs"]
mod depth_limited_test;

#[path = "unit/traits_test.rs"]
mod traits_test;

#[path = "unit/events_test.rs"]
mod events_test;

#[path = "unit/dashmap_backend_test.rs"]
mod dashmap_backend_test;

#[path = "unit/db_loader_test.rs"]
mod db_loader_test;

#[path = "unit/metrics_test.rs"]
mod metrics_test;
