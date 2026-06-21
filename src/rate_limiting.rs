//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块实现了速率限制功能，用于防止缓存滥用和拒绝服务攻击。
//! 通过 `rate-limiting` feature 控制启用/禁用
//!
//! 使用 `governor` crate 提供生产级的速率限制实现。

#[cfg(feature = "rate-limiting")]
use dashmap::DashMap;
#[cfg(feature = "rate-limiting")]
use governor::{clock::Clock, DefaultDirectRateLimiter, Quota, RateLimiter};
#[cfg(feature = "rate-limiting")]
use std::num::NonZeroU32;
#[cfg(feature = "rate-limiting")]
use std::sync::Arc;
#[cfg(feature = "rate-limiting")]
use std::time::Duration;

/// 速率限制配置
#[cfg(feature = "rate-limiting")]
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 每秒允许的最大请求数
    pub max_requests_per_second: u64,
    /// 令牌桶容量（突发流量处理能力）
    pub burst_capacity: u64,
    /// 封锁时间（秒）- 当超过限制时的临时封锁时间
    pub block_duration_secs: u64,
}

#[cfg(feature = "rate-limiting")]
impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_second: 1000,
            burst_capacity: 2000,
            block_duration_secs: 10,
        }
    }
}

/// 客户端级别的速率限制器
///
/// 为每个客户端维护独立的速率限制状态
/// 使用 governor 提供生产级的令牌桶实现
#[cfg(feature = "rate-limiting")]
#[derive(Debug)]
pub struct ClientRateLimiter {
    per_client: DashMap<String, Arc<DefaultDirectRateLimiter>>,
    global_limit: Arc<DefaultDirectRateLimiter>,
    config: RateLimitConfig,
}

#[cfg(feature = "rate-limiting")]
impl ClientRateLimiter {
    /// 创建新的客户端速率限制器
    pub fn new(config: RateLimitConfig) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(config.max_requests_per_second as u32).unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
        )
        .allow_burst(NonZeroU32::new(config.burst_capacity as u32).unwrap_or_else(|| NonZeroU32::new(1).unwrap()));

        Self {
            per_client: DashMap::new(),
            global_limit: Arc::new(RateLimiter::direct(quota)),
            config,
        }
    }

    /// 检查是否允许请求
    ///
    /// # 参数
    ///
    /// * `client_id` - 客户端标识符
    /// * `cost` - 请求消耗的令牌数
    ///
    /// # 返回值
    ///
    /// * `Ok(())` - 请求被允许
    /// * `Err(Duration)` - 请求被拒绝，返回建议的重试时间
    pub async fn check_rate_limit(&self, client_id: &str, cost: u64) -> Result<(), Duration> {
        let limiter = self.per_client.entry(client_id.to_string()).or_insert_with(|| {
            let quota = Quota::per_second(
                NonZeroU32::new(self.config.max_requests_per_second as u32)
                    .unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            )
            .allow_burst(
                NonZeroU32::new(self.config.burst_capacity as u32).unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            );
            Arc::new(RateLimiter::direct(quota))
        });

        for _ in 0..cost {
            if let Err(not_until) = limiter.check() {
                return Err(not_until.wait_time_from(limiter.clock().now()));
            }
        }

        for _ in 0..cost {
            if let Err(not_until) = self.global_limit.check() {
                return Err(not_until.wait_time_from(self.global_limit.clock().now()));
            }
        }

        Ok(())
    }

    /// 获取客户端的速率限制状态
    pub async fn get_client_status(&self, _client_id: &str) -> RateLimitStatus {
        // governor 不直接暴露剩余令牌数，使用配置值作为容量指示
        RateLimitStatus {
            client_available: self.config.burst_capacity,
            client_capacity: self.config.burst_capacity,
            global_available: self.config.burst_capacity,
            global_capacity: self.config.burst_capacity,
        }
    }
}

/// 速率限制状态
#[cfg(feature = "rate-limiting")]
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// 客户端可用令牌数
    pub client_available: u64,
    /// 客户端令牌桶容量
    pub client_capacity: u64,
    /// 全局可用令牌数
    pub global_available: u64,
    /// 全局令牌桶容量
    pub global_capacity: u64,
}

/// 全局速率限制器单例
#[cfg(feature = "rate-limiting")]
#[derive(Debug, Clone)]
pub struct GlobalRateLimiter(Arc<ClientRateLimiter>);

#[cfg(feature = "rate-limiting")]
impl GlobalRateLimiter {
    /// 创建新的全局速率限制器
    pub fn new(config: Option<RateLimitConfig>) -> Self {
        Self(Arc::new(ClientRateLimiter::new(config.unwrap_or_default())))
    }

    /// 获取内部引用
    pub fn inner(&self) -> &Arc<ClientRateLimiter> {
        &self.0
    }
}

#[cfg(feature = "rate-limiting")]
impl Default for GlobalRateLimiter {
    fn default() -> Self {
        Self::new(None)
    }
}

// ============================================================================
// 当 rate-limiting 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "rate-limiting"))]
/// 速率限制配置（空实现）
#[derive(Debug, Clone, Default)]
pub struct RateLimitConfig;

#[cfg(not(feature = "rate-limiting"))]
impl RateLimitConfig {
    pub fn new(_max_requests_per_second: u64, _burst_capacity: u64, _block_duration_secs: u64) -> Self {
        Self
    }
}

/// 客户端速率限制器（空实现）
#[cfg(not(feature = "rate-limiting"))]
#[derive(Debug, Clone, Default)]
pub struct ClientRateLimiter;

#[cfg(not(feature = "rate-limiting"))]
impl ClientRateLimiter {
    pub fn new(_config: RateLimitConfig) -> Self {
        Self
    }

    pub async fn check_rate_limit(&self, _client_id: &str, _cost: u64) -> Result<(), std::time::Duration> {
        Ok(())
    }

    pub async fn get_client_status(&self, _client_id: &str) -> RateLimitStatus {
        RateLimitStatus::default()
    }
}

/// 速率限制状态（空实现）
#[cfg(not(feature = "rate-limiting"))]
#[derive(Debug, Clone, Default)]
pub struct RateLimitStatus {
    pub client_available: u64,
    pub client_capacity: u64,
    pub global_available: u64,
    pub global_capacity: u64,
}

/// 全局速率限制器（空实现）
#[cfg(not(feature = "rate-limiting"))]
#[derive(Debug, Clone, Default)]
pub struct GlobalRateLimiter;

#[cfg(not(feature = "rate-limiting"))]
impl GlobalRateLimiter {
    pub fn new(_config: Option<RateLimitConfig>) -> Self {
        Self
    }

    pub fn inner(&self) -> &std::sync::Arc<ClientRateLimiter> {
        static EMPTY: std::sync::Arc<ClientRateLimiter> = std::sync::Arc::new(ClientRateLimiter);
        &EMPTY
    }
}

#[cfg(test)]
#[cfg(feature = "rate-limiting")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let config = RateLimitConfig {
            max_requests_per_second: 10,
            burst_capacity: 20,
            block_duration_secs: 10,
        };
        let limiter = ClientRateLimiter::new(config);

        for _ in 0..20 {
            assert!(
                limiter.check_rate_limit("test-client", 1).await.is_ok(),
                "Should allow within burst capacity"
            );
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let config = RateLimitConfig {
            max_requests_per_second: 5,
            burst_capacity: 5,
            block_duration_secs: 10,
        };
        let limiter = ClientRateLimiter::new(config);

        for _ in 0..5 {
            let _ = limiter.check_rate_limit("test-client", 1).await;
        }

        let result = limiter.check_rate_limit("test-client", 1).await;
        assert!(result.is_err(), "Should block when over limit");
    }

    #[tokio::test]
    async fn test_client_status() {
        let config = RateLimitConfig {
            max_requests_per_second: 100,
            burst_capacity: 100,
            block_duration_secs: 10,
        };
        let limiter = ClientRateLimiter::new(config);

        let status = limiter.get_client_status("test_client").await;
        assert_eq!(status.client_available, 100);
        assert_eq!(status.global_available, 100);
    }

    #[tokio::test]
    async fn test_multiple_clients() {
        let config = RateLimitConfig {
            max_requests_per_second: 10,
            burst_capacity: 10,
            block_duration_secs: 10,
        };
        let limiter = ClientRateLimiter::new(config);

        for _ in 0..5 {
            let _ = limiter.check_rate_limit("client-a", 1).await;
        }

        assert!(limiter.check_rate_limit("client-a", 1).await.is_ok());

        assert!(
            limiter.check_rate_limit("client-b", 1).await.is_ok(),
            "Different clients should have independent limits"
        );
    }
}
