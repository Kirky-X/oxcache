// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Security Tests Module

// Common modules shared by security tests
#[path = "common/mod.rs"]
pub mod common;

#[cfg(feature = "redis")]
#[path = "security/security_tests.rs"]
mod security_tests;

#[cfg(feature = "redis")]
#[path = "security/security_coverage_test.rs"]
mod security_coverage_test;
