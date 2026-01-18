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
pub struct ConnectionPool<T: DatabaseOperations> {
    /// 连接池
    pool: Arc<Mutex<Vec<Arc<T>>>>,
    /// 配置
    #[allow(dead_code)]
    config: PoolConfig,
    /// 连接创建器（用于动态创建连接）
    creator: Arc<Box<dyn Fn() -> Result<Arc<T>> + Send + Sync>>,
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
        F: Fn() -> Result<Arc<T>> + Send + Sync + 'static,
    {
        let mut connections = Vec::new();
        for _ in 0..config.min_idle {
            connections.push(creator()?);
        }

        Ok(Self {
            pool: Arc::new(Mutex::new(connections)),
            config,
            creator: Arc::new(Box::new(creator)),
            active_count: Arc::new(Mutex::new(0)),
            stats: Arc::new(Mutex::new(PoolStats::new())),
        })
    }

    /// 获取连接
    pub async fn get_connection(&self) -> Result<Arc<T>> {
        let mut pool = self.pool.lock().await;

        // 尝试从池中获取连接
        if let Some(conn) = pool.pop() {
            let mut stats = self.stats.lock().await;

            // 连接从空闲变为活跃
            stats.idle_connections = stats.idle_connections.saturating_sub(1);
            stats.active_connections = stats.active_connections.saturating_add(1);

            return Ok(conn);
        }

        // 池为空，检查是否可以达到最大连接数
        let mut stats = self.stats.lock().await;
        if stats.active_connections >= self.config.max_size {
            return Err(CacheError::DatabaseError(
                "Connection pool exhausted".to_string(),
            ));
        }

        // 创建新连接
        stats.active_connections = stats.active_connections.saturating_add(1);
        drop(stats);

        let new_conn = (self.creator)()?;

        Ok(new_conn)
    }

    /// 归还连接
    pub async fn return_connection(&self, conn: Arc<T>) {
        let mut pool = self.pool.lock().await;
        let mut stats = self.stats.lock().await;

        // 连接从活跃变为空闲
        stats.active_connections = stats.active_connections.saturating_sub(1);

        // 将连接归还到池中
        pool.push(conn);
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> PoolStats {
        let stats = self.stats.lock().await;
        let pool = self.pool.lock().await;
        let pool_size = pool.len() as u32;

        PoolStats {
            active_connections: stats.active_connections,
            idle_connections: pool_size,
            waiting_requests: 0,
            total_connections: stats.active_connections + pool_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 测试用的模拟数据库连接
    #[derive(Debug)]
    struct MockConnection {
        id: usize,
        connection_count: &'static AtomicUsize,
    }

    impl MockConnection {
        fn new(id: usize, connection_count: &'static AtomicUsize) -> Self {
            connection_count.fetch_add(1, Ordering::SeqCst);
            Self {
                id,
                connection_count,
            }
        }
    }

    #[async_trait::async_trait]
    impl DatabaseOperations for MockConnection {
        async fn is_connected(&self) -> bool {
            true
        }

        async fn query(&self, _sql: &str) -> Result<Vec<HashMap<String, String>>> {
            Ok(vec![])
        }

        async fn execute(&self, _sql: &str) -> Result<u64> {
            Ok(0)
        }
    }

    impl Drop for MockConnection {
        fn drop(&mut self) {
            self.connection_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_connection_pool_creates_connection_when_empty() {
        static CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);

        let config = PoolConfig {
            max_size: 5,
            min_idle: 0,
            connection_timeout: 30,
            idle_timeout: 600,
        };

        let pool = ConnectionPool::<MockConnection>::new(config, || {
            Ok(Arc::new(MockConnection::new(
                CONNECTION_COUNT.load(Ordering::SeqCst) + 1,
                &CONNECTION_COUNT,
            )))
        })
        .await
        .unwrap();

        // 池为空，获取连接时应创建新连接
        let conn = pool.get_connection().await.unwrap();
        let stats = pool.get_stats().await;

        // 验证连接已创建
        assert_eq!(CONNECTION_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(stats.active_connections, 1);

        // 归还连接
        pool.return_connection(conn).await;
        let stats = pool.get_stats().await;

        // 验证连接已归还到池中
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.idle_connections, 1);
    }

    #[tokio::test]
    async fn test_connection_pool_exhaustion() {
        static CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);

        let config = PoolConfig {
            max_size: 2,
            min_idle: 0,
            connection_timeout: 30,
            idle_timeout: 600,
        };

        let pool = ConnectionPool::<MockConnection>::new(config, || {
            Ok(Arc::new(MockConnection::new(
                CONNECTION_COUNT.load(Ordering::SeqCst) + 1,
                &CONNECTION_COUNT,
            )))
        })
        .await
        .unwrap();

        // 获取第一个连接
        let conn1 = pool.get_connection().await.unwrap();
        assert_eq!(CONNECTION_COUNT.load(Ordering::SeqCst), 1);

        // 获取第二个连接
        let conn2 = pool.get_connection().await.unwrap();
        assert_eq!(CONNECTION_COUNT.load(Ordering::SeqCst), 2);

        // 不归还连接，尝试获取第三个连接（池已满）
        let result = pool.get_connection().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Connection pool exhausted"));

        // 归还连接让它们正常析构
        drop(conn1);
        drop(conn2);
    }

    #[tokio::test]
    async fn test_connection_pool_return_and_reacquire() {
        static CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);

        let config = PoolConfig {
            max_size: 3,
            min_idle: 0,
            connection_timeout: 30,
            idle_timeout: 600,
        };

        let pool = ConnectionPool::<MockConnection>::new(config, || {
            Ok(Arc::new(MockConnection::new(
                CONNECTION_COUNT.load(Ordering::SeqCst) + 1,
                &CONNECTION_COUNT,
            )))
        })
        .await
        .unwrap();

        // 获取连接并归还
        let conn = pool.get_connection().await.unwrap();
        assert_eq!(CONNECTION_COUNT.load(Ordering::SeqCst), 1);
        pool.return_connection(conn).await;

        // 验证归还后可以再次获取
        let conn2 = pool.get_connection().await.unwrap();
        assert_eq!(CONNECTION_COUNT.load(Ordering::SeqCst), 1);
        pool.return_connection(conn2).await;
    }

    #[tokio::test]
    async fn test_connection_pool_zero_min_idle() {
        static CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);

        let config = PoolConfig {
            max_size: 5,
            min_idle: 0, // 零空闲连接
            connection_timeout: 30,
            idle_timeout: 600,
        };

        let pool = ConnectionPool::<MockConnection>::new(config, || {
            Ok(Arc::new(MockConnection::new(
                CONNECTION_COUNT.load(Ordering::SeqCst) + 1,
                &CONNECTION_COUNT,
            )))
        })
        .await
        .unwrap();

        // 验证池为空
        let stats = pool.get_stats().await;
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.idle_connections, 0);

        // 获取连接时应动态创建
        let conn = pool.get_connection().await.unwrap();
        let stats = pool.get_stats().await;
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.idle_connections, 0);
        pool.return_connection(conn).await;
    }

    #[tokio::test]
    async fn test_connection_pool_stats_accuracy() {
        static CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);

        let config = PoolConfig {
            max_size: 10,
            min_idle: 2,
            connection_timeout: 30,
            idle_timeout: 600,
        };

        let pool = ConnectionPool::<MockConnection>::new(config, || {
            Ok(Arc::new(MockConnection::new(
                CONNECTION_COUNT.load(Ordering::SeqCst) + 1,
                &CONNECTION_COUNT,
            )))
        })
        .await
        .unwrap();

        // 验证初始空闲连接
        let stats = pool.get_stats().await;
        assert_eq!(stats.idle_connections, 2);
        assert_eq!(stats.active_connections, 0);

        // 获取一个连接
        let conn = pool.get_connection().await.unwrap();
        let stats = pool.get_stats().await;
        assert_eq!(stats.idle_connections, 1);
        assert_eq!(stats.active_connections, 1);

        pool.return_connection(conn).await;
        let stats = pool.get_stats().await;
        assert_eq!(stats.idle_connections, 2);
        assert_eq!(stats.active_connections, 0);
    }
}
