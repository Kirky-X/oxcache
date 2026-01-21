//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 策略特征定义
//!
//! 定义 L2BackendStrategy trait，所有 Redis 后端策略都实现此特征。

use async_trait::async_trait;
use std::time::Duration;

/// 健康检查状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// 服务健康
    Healthy,
    /// 服务不健康
    Unhealthy(String),
    /// 部分健康（如集群中部分节点故障）
    Degraded(String),
}

/// SCAN 操作结果
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// 匹配的键
    pub keys: Vec<String>,
    /// 下次扫描的游标（0 表示扫描完成）
    pub cursor: u64,
}

/// SCAN 迭代器
pub struct ScanIterator<'a> {
    strategy: &'a dyn L2BackendStrategy,
    pattern: String,
    cursor: u64,
    count: usize,
}

impl<'a> ScanIterator<'a> {
    /// 创建新的 SCAN 迭代器
    pub fn new(strategy: &'a dyn L2BackendStrategy, pattern: &str, count: usize) -> Self {
        Self {
            strategy,
            pattern: pattern.to_string(),
            cursor: 0,
            count,
        }
    }
}

#[async_trait]
impl<'a> Iterator for ScanIterator<'a> {
    type Item = Result<String, crate::error::CacheError>;

    fn next(&mut self) -> Option<Self::Item> {
        // 使用同步阻塞的方式进行迭代
        // 注意：这是一个简化的实现，实际使用中可能需要异步迭代器
        if self.cursor == u64::MAX {
            return None;
        }

        // 第一次迭代时 cursor 为 0
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.strategy
                    .scan(&self.pattern, self.count, self.cursor)
                    .await
            })
        });

        match result {
            Ok(scan_result) => {
                self.cursor = scan_result.cursor;
                if scan_result.cursor == 0 {
                    self.cursor = u64::MAX; // 标记结束
                }
                scan_result.keys.into_iter().map(Ok).next()
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// L2 后端策略特征
///
/// 定义所有 L2 后端操作必须实现的方法。
/// 通过策略模式，可以支持不同的 Redis 部署模式。
#[async_trait]
pub trait L2BackendStrategy: Send + Sync {
    /// 获取策略名称
    fn name(&self) -> &str;

    /// 检查是否已连接
    fn is_connected(&self) -> bool;

    // =================================================================
    // 核心操作
    // =================================================================

    /// 获取缓存值
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * `Ok(Some(value))` - 找到的值
    /// * `Ok(None)` - 键不存在
    /// * `Err(...)` - 发生错误
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, crate::error::CacheError>;

    /// 设置缓存值
    ///
    /// # 参数
    /// * `key` - 缓存键
    /// * `value` - 值（字节数组）
    /// * `ttl` - 过期时间（秒），None 表示永不过期
    async fn set(
        &self,
        key: &str,
        value: &[u8],
        ttl: Option<u64>,
    ) -> Result<(), crate::error::CacheError>;

    /// 删除缓存值
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * `Ok(true)` - 键存在并删除
    /// * `Ok(false)` - 键不存在
    async fn delete(&self, key: &str) -> Result<bool, crate::error::CacheError>;

    /// 检查键是否存在
    async fn exists(&self, key: &str) -> Result<bool, crate::error::CacheError>;

    /// 设置过期时间
    ///
    /// # 参数
    /// * `key` - 缓存键
    /// * `ttl` - 过期时间（秒）
    async fn expire(&self, key: &str, ttl: u64) -> Result<bool, crate::error::CacheError>;

    /// 获取剩余过期时间
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * `Ok(Some(ttl))` - 剩余时间（秒）
    /// * `Ok(None)` - 键不存在或永不过期
    async fn ttl(&self, key: &str) -> Result<Option<i64>, crate::error::CacheError>;

    // =================================================================
    // 版本化操作（用于乐观锁）
    // =================================================================

    /// 获取带版本号的缓存值
    ///
    /// # 参数
    /// * `key` - 缓存键
    ///
    /// # 返回值
    /// * `Ok(Some((value, version)))` - 值和版本号
    /// * `Ok(None)` - 键不存在
    async fn get_with_version(
        &self,
        key: &str,
    ) -> Result<Option<(Vec<u8>, u64)>, crate::error::CacheError>;

    /// 带版本号的原子设置
    ///
    /// 仅当当前版本与 expected_version 匹配时才设置新值。
    ///
    /// # 参数
    /// * `key` - 缓存键
    /// * `value` - 值（字节数组）
    /// * `expected_version` - 期望的当前版本
    /// * `new_version` - 设置的新版本号
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    /// * `Ok(true)` - 设置成功
    /// * `Ok(false)` - 版本不匹配，设置失败
    async fn compare_and_set(
        &self,
        key: &str,
        value: &[u8],
        expected_version: u64,
        new_version: u64,
        ttl: Option<u64>,
    ) -> Result<bool, crate::error::CacheError>;

    // =================================================================
    // 锁操作
    // =================================================================

    /// 尝试获取分布式锁
    ///
    /// 使用 SET NX PX 实现，自动生成安全的随机锁值。
    ///
    /// # 参数
    /// * `key` - 锁键
    /// * `ttl` - 锁过期时间（秒）
    ///
    /// # 返回值
    /// * `Ok(Some(value))` - 成功获取锁，value 为生成的锁值
    /// * `Ok(None)` - 锁已被其他进程持有
    async fn lock(&self, key: &str, ttl: u64) -> Result<Option<String>, crate::error::CacheError>;

    /// 释放分布式锁
    ///
    /// 使用 Lua 脚本保证原子性。
    ///
    /// # 参数
    /// * `key` - 锁键
    /// * `value` - 锁值（创建锁时生成的值）
    ///
    /// # 返回值
    /// * `Ok(true)` - 锁已释放
    /// * `Ok(false)` - 锁值不匹配或锁不存在
    async fn unlock(&self, key: &str, value: &str) -> Result<bool, crate::error::CacheError>;

    // =================================================================
    // 批量操作
    // =================================================================

    /// 批量获取
    ///
    /// # 参数
    /// * `keys` - 键列表
    ///
    /// # 返回值
    /// * 值的哈希表，缺失的键不在表中
    async fn mget(
        &self,
        keys: &[&str],
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, crate::error::CacheError>;

    /// 批量设置
    ///
    /// # 参数
    /// * `items` - 键值对列表
    /// * `ttl` - 公共过期时间（秒）
    async fn mset(
        &self,
        items: &[(&str, &[u8])],
        ttl: Option<u64>,
    ) -> Result<(), crate::error::CacheError>;

    // =================================================================
    // SCAN 操作
    // =================================================================

    /// SCAN 操作
    ///
    /// 增量式遍历匹配模式的键。
    ///
    /// # 参数
    /// * `pattern` - 匹配模式（如 "user:*"）
    /// * `count` - 每次返回的最大匹配数
    /// * `cursor` - 游标位置（0 表示开始）
    ///
    /// # 返回值
    /// * 匹配的键列表和下次扫描的游标
    async fn scan(
        &self,
        pattern: &str,
        count: usize,
        cursor: u64,
    ) -> Result<ScanResult, crate::error::CacheError>;

    /// 获取匹配模式的键（有限制）
    ///
    /// # 参数
    /// * `pattern` - 匹配模式
    /// * `limit` - 最大返回数量
    ///
    /// # 返回值
    /// * 匹配的键列表（不超过 limit）
    async fn scan_keys(
        &self,
        pattern: &str,
        limit: usize,
    ) -> Result<Vec<String>, crate::error::CacheError>;

    // =================================================================
    // 管理操作
    // =================================================================

    /// PING 命令（简单健康检查）
    async fn ping(&self) -> Result<(), crate::error::CacheError>;

    /// 详细健康检查
    async fn health_check(&self) -> Result<HealthStatus, crate::error::CacheError>;

    /// 获取命令超时配置
    fn command_timeout(&self) -> Duration;

    /// 关闭连接
    async fn close(&self) -> Result<(), crate::error::CacheError>;
}
