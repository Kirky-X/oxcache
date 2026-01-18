//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的错误类型和处理机制。

use thiserror::Error;

/// 脱敏连接字符串，隐藏密码等敏感信息
fn sanitize_connection_string(conn_str: &str) -> String {
    // 使用正则表达式隐藏密码
    // 匹配模式: protocol://[:password@]host:port
    if let Some(start) = conn_str.find("://") {
        let protocol = &conn_str[..start];
        let after_protocol = &conn_str[start + 3..];

        // 检查是否包含密码 (@ 符号)
        if let Some(at_pos) = after_protocol.find('@') {
            let user_part = &after_protocol[..at_pos];
            // 隐藏密码部分
            let sanitized_user: String = user_part
                .chars()
                .take_while(|c| *c != ':')
                .chain(std::iter::once('*'))
                .chain(std::iter::once(':').chain(std::iter::once('*')).take(5))
                .collect();
            return format!(
                "{}://{}@{}",
                protocol,
                sanitized_user,
                &after_protocol[at_pos + 1..]
            );
        }
    }
    conn_str.to_string()
}

/// 缓存系统错误类型枚举
///
/// 定义了缓存系统中可能发生的各种错误类型。
/// 所有错误都实现了std::error::Error trait，可以使用?操作符传播。
///
/// # 错误分类
///
/// - **配置错误** ([`CacheError::Configuration`]): 配置问题，如缺少必需字段
/// - **序列化错误** ([`CacheError::Serialization`]): 数据序列化/反序列化失败
/// - **后端错误** ([`CacheError::BackendError`]): L1/L2缓存后端操作失败
/// - **连接错误** ([`CacheError::ConnectionError`]): 网络连接问题
/// - **超时错误** ([`CacheError::TimeoutError`]): 操作超时
/// - **数据库错误** ([`CacheError::DatabaseError`]): 数据库相关错误
///
/// # 示例
///
/// ```rust,ignore
/// use oxcache::error::CacheError;
///
/// async fn safe_cache_operation() -> Result<String, CacheError> {
///     let result = cache.get("key").await?;
///     match result {
///         Some(value) => Ok(value),
///         None => Err(CacheError::BackendError("Key not found".to_string()))
///     }
/// }
/// ```
#[derive(Error, Debug)]
pub enum CacheError {
    /// 序列化错误
    ///
    /// 通常发生在：
    /// - 尝试序列化不支持的数据类型
    /// - 序列化器配置不兼容
    /// - 数据在传输过程中被损坏
    #[error("Serialization error: {0}. Please check the data format and ensure the serializer is compatible."
    )]
    Serialization(String),

    /// L1缓存操作失败
    ///
    /// L1缓存是进程内的内存缓存，可能因以下原因失败：
    /// - 内存不足导致缓存被逐出
    /// - 缓存容量达到上限
    /// - 缓存项过期
    #[error("L1 cache operation failed: {0}. This may indicate memory pressure or configuration issues."
    )]
    L1Error(String),

    /// L2缓存操作失败
    #[error("L2 cache operation failed: {0}. Please check Redis connection and server status.")]
    L2Error(String),

    /// 配置错误
    #[error("Configuration error: {0}. Please review your configuration file and ensure all required settings are provided."
    )]
    ConfigError(String),

    /// 配置错误（别名，为了兼容）
    #[error("Configuration error: {0}. Please review your configuration file.")]
    Configuration(String),

    /// 操作不支持
    #[error("Operation not supported: {0}. This feature may not be available for the current cache type."
    )]
    NotSupported(String),

    /// WAL（预写日志）操作失败
    #[error("WAL (Write-Ahead Log) operation failed: {0}. Check disk space and file permissions.")]
    WalError(String),

    /// 数据库错误
    #[error("Database error: {0}. Please check database connectivity and query syntax.")]
    DatabaseError(String),

    /// Redis错误（脱敏后的错误信息）
    #[cfg(feature = "l2-redis")]
    #[error("Redis connection failed: {}. Please ensure Redis server is running and the connection string is correct.",
        sanitize_connection_string(&0.to_string())
    )]
    RedisError(#[from] redis::RedisError),

    /// Redis错误（占位符，当 l2-redis feature 禁用时）
    #[cfg(not(feature = "l2-redis"))]
    #[error("Redis connection failed: {0}. Please enable l2-redis feature and ensure Redis server is running."
    )]
    RedisError(String),

    /// IO错误
    #[error("I/O error: {0}. Check file permissions and disk space.")]
    IoError(std::io::Error),

    /// 后端错误
    #[error("Backend error: {0}. This may be a transient issue, please retry.")]
    BackendError(String),

    /// 超时错误
    #[error("Operation timed out: {0}. Consider increasing the timeout value or check system performance."
    )]
    Timeout(String),

    /// 关闭错误
    #[error("Shutdown error: {0}. Some resources may not have been properly released.")]
    ShutdownError(String),

    /// 键过长错误
    #[error("Key too long: {0}. Maximum key length is {1} bytes.")]
    KeyTooLong(usize, usize),

    /// 值过大错误
    #[error("Value too large: {0}. Maximum value size is {1} bytes.")]
    ValueTooLarge(usize, usize),

    /// 缓冲区已满错误
    #[error("Buffer full: {0}. The batch write buffer has reached capacity. Please retry later or increase buffer size."
    )]
    BufferFull(String),

    /// 无效输入错误
    #[error(
        "Invalid input: {0}. The provided input does not meet the required format or constraints."
    )]
    InvalidInput(String),

    /// 无效键错误
    #[error("Invalid key: {0}. The provided key does not meet the required format or contains forbidden characters.")]
    InvalidKey(String),
}

/// 缓存操作结果类型别名
///
/// 简化错误处理，所有缓存操作都返回此类型
pub type Result<T> = std::result::Result<T, CacheError>;

#[cfg(feature = "database")]
impl From<sea_orm::DbErr> for CacheError {
    fn from(e: sea_orm::DbErr) -> Self {
        CacheError::DatabaseError(e.to_string())
    }
}

impl From<std::io::Error> for CacheError {
    fn from(e: std::io::Error) -> Self {
        CacheError::IoError(e)
    }
}
