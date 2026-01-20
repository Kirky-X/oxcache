//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 分层缓存细粒度控制模块
//!
//! 提供独立操作 L1/L2 和跨层移动的能力。

use crate::error::Result;

/// 分层缓存细粒度控制 trait
///
/// 提供对分层缓存的细粒度控制能力。
#[async_trait::async_trait]
pub trait TieredCacheControl: Send + Sync {
    /// 仅从 L1 获取（不fallback到L2）
    async fn get_l1_direct(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 仅设置 L1（不同步到 L2）
    async fn set_l1_direct(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()>;

    /// 仅从 L1 删除（不同步到 L2）
    async fn delete_l1_direct(&self, key: &str) -> Result<bool>;

    /// 仅从 L2 获取
    async fn get_l2_direct(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 仅设置 L2（不同步到 L1）
    async fn set_l2_direct(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()>;

    /// 仅从 L2 删除（不同步到 L1）
    async fn delete_l2_direct(&self, key: &str) -> Result<bool>;

    /// 将 L2 数据提升到 L1
    async fn promote_to_l1(&self, key: &str) -> Result<bool>;

    /// 将 L1 数据降级到 L2
    async fn demote_to_l2(&self, key: &str, ttl: Option<u64>) -> Result<bool>;

    /// 从所有层清除数据
    async fn evict_all(&self, key: &str) -> Result<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiered_cache_control_exists() {
        // 验证 trait 存在
        let _ = std::marker::PhantomData::<dyn TieredCacheControl>;
    }
}
