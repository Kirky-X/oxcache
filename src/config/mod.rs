//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Configuration management module
//!
//! This module provides configuration structures for the cache library
//! using the confers library for zero-boilerplate configuration management.

#[cfg(feature = "confers")]
pub mod confers_config;

#[cfg(feature = "confers")]
pub use confers_config::*;
