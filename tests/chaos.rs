// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Chaos Tests Module
//
// Contains chaos and fault tolerance tests.

// Common modules shared by chaos tests
#[path = "common/mod.rs"]
pub mod common;

#[path = "chaos/chaos_test.rs"]
mod chaos_test;

#[path = "chaos/random_failure_test.rs"]
mod random_failure_test;

#[path = "chaos/network_failure_test.rs"]
mod network_failure_test;
