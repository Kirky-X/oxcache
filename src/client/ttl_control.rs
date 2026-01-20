//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! TTL 控制模块
//!
//! 提供获取和刷新 TTL 的能力。

use crate::error::Result;

/// TTL 控制 trait
///
/// 定义 TTL 操作的接口。
#[async_trait::async_trait]
pub trait TtlControl: Send + Sync {
    /// 获取 L1 缓存剩余 TTL
    async fn get_l1_ttl(&self, key: &str) -> Result<Option<u64>>;

    /// 获取 L2 缓存剩余 TTL
    async fn get_l2_ttl(&self, key: &str) -> Result<Option<u64>>;

    /// 获取缓存剩余 TTL（从 L1 或 L2）
    async fn get_ttl(&self, key: &str) -> Result<Option<u64>>;

    /// 刷新 L1 缓存 TTL
    async fn refresh_l1_ttl(&self, key: &str, ttl: u64) -> Result<bool>;

    /// 刷新 L2 缓存 TTL
    async fn refresh_l2_ttl(&self, key: &str, ttl: u64) -> Result<bool>;

    /// 刷新缓存 TTL（同时刷新 L1 和 L2）
    async fn refresh_ttl(&self, key: &str, ttl: u64) -> Result<bool>;

    /// Touch 操作（刷新访问时间但不返回值）
    async fn touch(&self, key: &str) -> Result<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_control_exists() {
        // 验证 trait 存在
        let _ = std::marker::PhantomData::<dyn TtlControl>;
    }
}
