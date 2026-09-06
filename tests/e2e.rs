// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// E2E Tests Module
//
// Contains all end-to-end tests for the cache system.
// These tests verify the complete user workflow.

#![allow(clippy::duplicate_mod)]

// Common modules shared by E2E tests
#[path = "common/mod.rs"]
pub mod common;

#[path = "e2e/advanced_scenarios_test.rs"]
mod advanced_scenarios_test;
#[path = "e2e/cache_e2e_test.rs"]
mod cache_e2e_test;
#[path = "e2e/macro_test.rs"]
mod macro_test;
#[path = "e2e/real_world_scenario_test.rs"]
mod real_world_scenario_test;
// 阶段 2 E2E 查缺补漏：看门狗组合语义（依赖 lock，lock 隐含 redis）
#[cfg(feature = "lock")]
#[path = "e2e/dist_lock_watchdog_e2e.rs"]
mod dist_lock_watchdog_e2e;
// 阶段 2 E2E 查缺补漏：ChainCache event_publisher 失败事件集成链
#[cfg(feature = "redis")]
#[path = "e2e/events_chain_e2e.rs"]
mod events_chain_e2e;
