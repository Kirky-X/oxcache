//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 数据库连接和常用工具模块
//!

use crate::error::{CacheError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 通用数据库操作trait
#[async_trait]
pub trait DatabaseOperations: Debug + Send + Sync {
    /// 检查连接是否有效
    async fn is_connected(&self) -> bool;

    /// 执行查询
    async fn query(&self, sql: &str) -> Result<Vec<HashMap<String, String>>>;

    /// 执行更新
    async fn execute(&self, sql: &str) -> Result<u64>;
}

/// 连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// 最大连接数
    pub max_size: u32,
    /// 最小空闲连接数
    pub min_idle: u32,
    /// 连接超时（秒）
    pub connection_timeout: u64,
    /// 空闲连接超时（秒）
    pub idle_timeout: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_idle: 1,
            connection_timeout: 30,
            idle_timeout: 600,
        }
    }
}

/// 连接池统计信息
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// 活跃连接数
    pub active_connections: u32,
    /// 空闲连接数
    pub idle_connections: u32,
    /// 等待获取连接的请求数
    pub waiting_requests: u32,
    /// 总连接数
    pub total_connections: u32,
}

impl PoolStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }
}

/// 通用连接池
#[derive(Debug)]
pub struct ConnectionPool<T: DatabaseOperations> {
    /// 连接池
    pool: Arc<Mutex<Vec<Arc<T>>>>,
    /// 配置
    #[allow(dead_code)]
    config: PoolConfig,
    /// 活跃连接数
    #[allow(dead_code)]
    active_count: Arc<Mutex<u32>>,
    /// 统计信息
    stats: Arc<Mutex<PoolStats>>,
}

impl<T: DatabaseOperations> ConnectionPool<T> {
    /// 创建新的连接池
    pub async fn new<F>(config: PoolConfig, creator: F) -> Result<Self>
    where
        F: Fn() -> Result<Arc<T>>,
    {
        let mut connections = Vec::new();
        for _ in 0..config.min_idle {
            connections.push(creator()?);
        }

        Ok(Self {
            pool: Arc::new(Mutex::new(connections)),
            config,
            active_count: Arc::new(Mutex::new(0)),
            stats: Arc::new(Mutex::new(PoolStats::new())),
        })
    }

    /// 获取连接
    pub async fn get_connection(&self) -> Result<Arc<T>> {
        let mut pool = self.pool.lock().await;

        if let Some(conn) = pool.pop() {
            let mut stats = self.stats.lock().await;
            stats.idle_connections = stats.idle_connections.saturating_sub(1);
            stats.active_connections = stats.active_connections.saturating_add(1);

            Ok(conn)
        } else {
            // 没有可用连接，尝试创建新连接
            // 这里应该实现连接创建逻辑
            Err(CacheError::DatabaseError(
                "No connection available".to_string(),
            ))
        }
    }

    /// 归还连接
    pub async fn return_connection(&self, _conn: Arc<T>) {
        let mut stats = self.stats.lock().await;
        stats.active_connections = stats.active_connections.saturating_sub(1);
        stats.idle_connections = stats.idle_connections.saturating_add(1);
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> PoolStats {
        let stats = self.stats.lock().await;
        let pool = self.pool.lock().await;
        PoolStats {
            active_connections: stats.active_connections,
            idle_connections: pool.len() as u32 + stats.idle_connections,
            waiting_requests: 0,
            total_connections: stats.active_connections + stats.idle_connections,
        }
    }
}
