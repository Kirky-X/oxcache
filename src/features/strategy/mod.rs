//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache strategy module - combines bloom filter, rate limiting, and smart caching strategies

// Submodules with feature gates
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub(crate) mod bloom;

#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub(crate) mod rate_limit;

#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub(crate) mod smart;

// Re-exports for public API
#[cfg(any(feature = "bloom-filter", feature = "full"))]
pub use bloom::{BloomFilter, BloomFilterOptions};

#[cfg(any(feature = "rate-limiting", feature = "full"))]
pub use rate_limit::{ClientRateLimiter, GlobalRateLimiter, RateLimitConfig, RateLimitStatus};

#[cfg(any(feature = "smart-strategy", feature = "full"))]
pub use smart::{
    CompressibilityChecker, CompressionDecider, HitRateCollector, HitRateStats, PrefetchDecider, SmartStrategyConfig,
    SmartStrategyManager,
};
