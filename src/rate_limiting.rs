//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块实现了速率限制功能，用于防止缓存滥用和拒绝服务攻击。
//! 通过 `rate-limiting` feature 控制启用/禁用

#[cfg(feature = "rate-limiting")]
use dashmap::DashMap;
#[cfg(feature = "rate-limiting")]
use std::sync::atomic::{AtomicU64, Ordering};
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

/// 令牌桶速率限制器
///
/// 使用令牌桶算法实现精确的速率限制，支持突发流量
#[cfg(feature = "rate-limiting")]
#[derive(Debug)]
pub struct TokenBucket {
    tokens: AtomicU64,
    last_update: AtomicU64,
    capacity: u64,
    refill_rate: u64, // 每秒补充的令牌数
}

#[cfg(feature = "rate-limiting")]
impl TokenBucket {
    /// 创建新的令牌桶
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        let now = Self::now_millis();
        Self {
            tokens: AtomicU64::new(capacity),
            last_update: AtomicU64::new(now),
            capacity,
            refill_rate,
        }
    }

    #[inline]
    fn now_millis() -> u64 {
        // 使用UNIX_EPOCH作为参考点，处理系统时间可能回退的情况
        // 如果时间获取失败，使用一个固定的基准时间
        let since_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|e| {
                // 时间回退，使用0作为后备值
                // 这会导致速率限制器在系统时间异常时返回0，
                // 意味着令牌补充会立即进行（最大速率）
                tracing::warn!("系统时间异常，可能时钟回退: {}", e);
                std::time::Duration::ZERO
            });
        since_epoch.as_millis() as u64
    }

    /// 尝试获取一个令牌
    ///
    /// # 返回值
    ///
    /// * `true` - 成功获取令牌，允许请求
    /// * `false` - 令牌不足，请求被拒绝
    pub fn try_acquire(&self) -> bool {
        self.try_acquire_n(1)
    }

    /// 尝试获取多个令牌
    ///
    /// 使用 compare_exchange 实现原子更新，消除竞争条件
    pub fn try_acquire_n(&self, n: u64) -> bool {
        let now = Self::now_millis();

        loop {
            let last_update = self.last_update.load(Ordering::SeqCst);
            let elapsed = now.saturating_sub(last_update);
            let refill = (elapsed * self.refill_rate) / 1000;

            let current_tokens = self.tokens.load(Ordering::SeqCst);
            let new_tokens = (current_tokens + refill).min(self.capacity);

            if new_tokens < n {
                return false;
            }

            // 尝试原子更新 tokens
            let expected = current_tokens;
            match self
                .tokens
                .compare_exchange(expected, new_tokens - n, Ordering::SeqCst, Ordering::Relaxed)
            {
                Ok(_) => {
                    // 成功更新 tokens，同时尝试更新 last_update
                    // 如果 last_update 没有被其他线程修改，则更新
                    self.last_update
                        .compare_exchange(last_update, now, Ordering::SeqCst, Ordering::Relaxed)
                        .ok();
                    return true;
                }
                Err(_) => {
                    // 竞争发生，重试
                    continue;
                }
            }
        }
    }

    /// 获取当前可用令牌数
    pub fn available_tokens(&self) -> u64 {
        let now = Self::now_millis();
        let current_tokens = self.tokens.load(Ordering::Relaxed);
        let last_update = self.last_update.load(Ordering::Relaxed);
        let elapsed = now.saturating_sub(last_update);
        let refill = (elapsed * self.refill_rate) / 1000;

        (current_tokens + refill).min(self.capacity)
    }
}

/// 客户端级别的速率限制器
///
/// 为每个客户端维护独立的速率限制状态
/// 使用 DashMap 实现无锁并发访问
#[cfg(feature = "rate-limiting")]
#[derive(Debug)]
pub struct ClientRateLimiter {
    per_client: DashMap<String, Arc<TokenBucket>>,
    global_limit: TokenBucket,
    config: RateLimitConfig,
}

#[cfg(feature = "rate-limiting")]
impl ClientRateLimiter {
    /// 创建新的客户端速率限制器
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            per_client: DashMap::new(),
            global_limit: TokenBucket::new(config.burst_capacity, config.max_requests_per_second),
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
        let global_available = self.global_limit.available_tokens();

        // DashMap 无锁并发访问
        let bucket = self.per_client.entry(client_id.to_string()).or_insert_with(|| {
            Arc::new(TokenBucket::new(
                self.config.burst_capacity,
                self.config.max_requests_per_second,
            ))
        });

        let per_client_available = bucket.available_tokens();

        if per_client_available < cost {
            let wait_time =
                Duration::from_millis((cost - per_client_available) * 1000 / self.config.max_requests_per_second);
            return Err(wait_time);
        }

        if global_available < cost {
            let wait_time =
                Duration::from_millis((cost - global_available) * 1000 / self.config.max_requests_per_second);
            return Err(wait_time);
        }

        bucket.try_acquire_n(cost);
        self.global_limit.try_acquire_n(cost);

        Ok(())
    }

    /// 获取客户端的速率限制状态
    pub async fn get_client_status(&self, client_id: &str) -> RateLimitStatus {
        // DashMap 直接读取，无需异步锁
        if let Some(bucket) = self.per_client.get(client_id) {
            RateLimitStatus {
                client_available: bucket.available_tokens(),
                client_capacity: bucket.capacity,
                global_available: self.global_limit.available_tokens(),
                global_capacity: self.global_limit.capacity,
            }
        } else {
            RateLimitStatus {
                client_available: self.config.burst_capacity,
                client_capacity: self.config.burst_capacity,
                global_available: self.global_limit.available_tokens(),
                global_capacity: self.global_limit.capacity,
            }
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

/// 令牌桶速率限制器（空实现）
#[cfg(not(feature = "rate-limiting"))]
#[derive(Debug, Clone, Default)]
pub struct TokenBucket;

#[cfg(not(feature = "rate-limiting"))]
impl TokenBucket {
    pub fn new(_capacity: u64, _refill_rate: u64) -> Self {
        Self
    }

    pub fn try_acquire(&self) -> bool {
        true
    }

    pub fn try_acquire_n(&self, _n: u64) -> bool {
        true
    }

    pub fn available_tokens(&self) -> u64 {
        u64::MAX
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

    pub fn inner(&self) -> &Arc<ClientRateLimiter> {
        // 创建一个静态空实例
        static EMPTY: Arc<ClientRateLimiter> = Arc::new(ClientRateLimiter);
        &EMPTY
    }
}

#[cfg(test)]
#[cfg(feature = "rate-limiting")]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_basic() {
        let bucket = TokenBucket::new(10, 10);

        // 初始状态应该有10个令牌
        assert_eq!(bucket.available_tokens(), 10);

        // 尝试获取5个令牌，应该成功
        assert!(bucket.try_acquire_n(5));
        assert_eq!(bucket.available_tokens(), 5);

        // 尝试获取6个令牌，应该失败（只有5个）
        assert!(!bucket.try_acquire_n(6));

        // 再次尝试获取5个，应该成功
        assert!(bucket.try_acquire_n(5));
        assert_eq!(bucket.available_tokens(), 0);
    }

    #[test]
    fn test_token_bucket_refill() {
        let bucket = TokenBucket::new(10, 100);

        bucket.try_acquire_n(10);
        assert_eq!(bucket.available_tokens(), 0);

        std::thread::sleep(Duration::from_millis(50));
        let tokens = bucket.available_tokens();
        assert!(
            tokens >= 5,
            "Expected at least 5 tokens after refill, but got {}",
            tokens
        );
    }

    #[tokio::test]
    async fn test_client_rate_limiter() {
        let limiter = ClientRateLimiter::new(RateLimitConfig {
            max_requests_per_second: 100,
            burst_capacity: 100,
            block_duration_secs: 10,
        });

        // 初始状态检查
        let status = limiter.get_client_status("test_client").await;
        assert_eq!(status.client_available, 100);
        assert_eq!(status.global_available, 100);

        // 正常请求应该被允许
        assert!(limiter.check_rate_limit("test_client", 1).await.is_ok());

        // 大量请求应该被限制
        for _ in 0..100 {
            let _ = limiter.check_rate_limit("test_client", 1).await;
        }

        // 超过限制后应该被拒绝
        assert!(limiter.check_rate_limit("test_client", 1).await.is_err());
    }
}
