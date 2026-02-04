#[path = "common/mod.rs"]
pub mod common;

#[path = "performance/performance_test.rs"]
mod performance_test;

#[path = "performance/memory_tests.rs"]
mod memory_tests;

#[path = "performance/memory_leak_test.rs"]
mod memory_leak_test;

#[path = "performance/miri_memory_test.rs"]
mod miri_memory_test;
