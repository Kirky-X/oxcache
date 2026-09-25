// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

// 目标内容依赖 backend/Cache 导出，无 memory/redis 时整目标置空
#![cfg(any(feature = "memory", feature = "redis"))]
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
