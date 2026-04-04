// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 速率限制集成测试
//
// 注意: 由于 rate_limiting 模块现在是 pub(crate)，
// 详细的单元测试已移至 src/rate_limiting.rs 的内置测试中。
// 此文件保留用于 API 级别的集成测试。

#[cfg(test)]
#[cfg(feature = "rate-limiting")]
mod rate_limiting_integration {
    use oxcache::{ClientRateLimiter, RateLimitConfig};

    #[test]
    fn test_default_config() {
        let config = RateLimitConfig::default();
        assert!(config.max_requests_per_second > 0);
        assert!(config.burst_capacity > 0);
        assert!(config.block_duration_secs > 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let config = RateLimitConfig {
            max_requests_per_second: 100,
            burst_capacity: 200,
            block_duration_secs: 10,
        };
        let _limiter = ClientRateLimiter::new(config);
    }
}
