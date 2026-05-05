//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的错误类型和处理机制。

use thiserror::Error;

#[cfg(feature = "redis")]
use crate::features::security::redaction::redact_connection_string;

/// Configuration error type for cache initialization
///
/// This error type is used during the configuration phase (factory functions, builders)
/// and is separate from runtime errors. It represents errors that occur when setting up
/// a cache instance, such as invalid configuration values or missing required fields.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::error::CacheConfigError;
///
/// fn validate_config(config: &CacheConfig) -> ConfigResult<()> {
///     if config.l1.capacity == 0 {
///         return Err(CacheConfigError::InvalidValue {
///             field: "capacity".to_string(),
///             reason: "capacity must be greater than 0".to_string(),
///         });
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug, Error)]
pub enum CacheConfigError {
    /// Missing required configuration field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid value for a configuration field
    #[error("Invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },

    /// Unsupported backend combination
    #[error("Unsupported backend combination: {0}")]
    UnsupportedBackend(String),

    /// Connection failed during initialization
    #[error("Connection failed during initialization: {0}")]
    ConnectionFailed(String),
}

/// Result type for configuration operations
pub type ConfigResult<T> = std::result::Result<T, CacheConfigError>;

/// 缓存系统错误类型枚举
///
/// 定义了缓存系统中可能发生的各种错误类型。
/// 所有错误都实现了std::error::Error trait，可以使用?操作符传播。
///
/// # 错误分类
///
/// - **序列化错误** ([`CacheError::Serialization`]): 数据序列化/反序列化失败
/// - **后端错误** ([`CacheError::BackendError`]): L1/L2缓存后端操作失败
/// - **连接错误** ([`CacheError::ConnectionError`]): �络连接问题
/// - **超时错误** ([`CacheError::TimeoutError`]): 操作超时
/// - **数据库错误** ([`CacheError::DatabaseError`]): 数据库相关错误
/// - **未找到错误** ([`CacheError::NotFound`]): 请求的键不存在
/// - **降级错误** ([`CacheError::Degraded`]): 缓存处于降级模式
/// - **操作错误** ([`CacheError::Operation`]): 一般操作错误
///
/// # 配置阶段错误
///
/// 配置阶段的错误（如缺少必需字段、无效值等）使用 [`CacheConfigError`] 类型，
/// 通过 [`ConfigResult`] 类型别名返回。
///
/// # 示例
///
/// ```rust,ignore
/// use oxcache::error::{CacheError, CacheConfigError, ConfigResult};
///
/// // 运行时错误
/// async fn safe_cache_operation() -> Result<String, CacheError> {
///     let result = cache.get("key").await?;
///     match result {
///         Some(value) => Ok(value),
///         None => Err(CacheError::NotFound("Key not found".to_string()))
///     }
/// }
///
/// // 配置阶段错误
/// fn validate_config(config: &CacheConfig) -> ConfigResult<()> {
///     if config.capacity == 0 {
///         return Err(CacheConfigError::InvalidValue {
///             field: "capacity".to_string(),
///             reason: "must be greater than 0".to_string(),
///         });
///     }
///     Ok(())
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
    #[error("Serialization error: {0}. Please check the data format and ensure the serializer is compatible.")]
    Serialization(String),

    /// 操作错误
    ///
    /// 一般的缓存操作失败
    #[error("Operation failed: {0}. Please retry or check your request.")]
    Operation(String),

    /// 连接错误
    ///
    /// 网络连接失败或断开
    #[error("Connection error: {0}. Please check network connectivity and server availability.")]
    Connection(String),

    /// 未找到错误
    ///
    /// 请求的键在缓存中不存在
    #[error("Key not found: {0}. The requested key does not exist in the cache.")]
    NotFound(String),

    /// 降级错误
    ///
    /// 缓存处于降级模式，某些功能不可用
    #[error("Cache degraded: {0}. The cache is operating in degraded mode with limited functionality.")]
    Degraded(String),

    /// L1缓存操作失败
    ///
    /// L1缓存是进程内的内存缓存，可能因以下原因失败：
    /// - 内存不足导致缓存被逐出
    /// - 缓存容量达到上限
    /// - 缓存项过期
    #[error("L1 cache operation failed: {0}. This may indicate memory pressure or configuration issues.")]
    L1Error(String),

    /// L2缓存操作失败
    #[error("L2 cache operation failed: {0}. Please check Redis connection and server status.")]
    L2Error(String),

    /// 操作不支持
    #[error("Operation not supported: {0}. This feature may not be available for the current cache type.")]
    NotSupported(String),

    /// WAL（预写日志）操作失败
    #[error("WAL (Write-Ahead Log) operation failed: {0}. Check disk space and file permissions.")]
    WalError(String),

    /// 数据库错误
    #[error("Database error: {0}. Please check database connectivity and query syntax.")]
    DatabaseError(String),

    /// Redis错误（脱敏后的错误信息）
    #[cfg(feature = "redis")]
    #[error("Redis connection failed: {}. Please ensure Redis server is running and the connection string is correct.",
        redact_connection_string(&0.to_string())
    )]
    RedisError(#[from] redis::RedisError),

    /// Redis错误（占位符，当 redis feature 禁用时）
    #[cfg(not(feature = "redis"))]
    #[error("Redis connection failed: {0}. Please enable redis feature and ensure Redis server is running.")]
    RedisError(String),

    /// IO错误
    #[error("I/O error: {0}. Check file permissions and disk space.")]
    IoError(std::io::Error),

    /// 后端错误
    #[error("Backend error: {0}. This may be a transient issue, please retry.")]
    BackendError(String),

    /// 超时错误
    #[error("Operation timed out: {0}. Consider increasing the timeout value or check system performance.")]
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
    #[error(
        "Buffer full: {0}. The batch write buffer has reached capacity. Please retry later or increase buffer size."
    )]
    BufferFull(String),

    /// 无效输入错误
    #[error("Invalid input: {0}. The provided input does not meet the required format or constraints.")]
    InvalidInput(String),

    /// 无效键错误
    #[error("Invalid key: {0}. The provided key does not meet the required format or contains forbidden characters.")]
    InvalidKey(String),

    /// 锁错误
    ///
    /// 互斥锁获取失败，通常发生在锁被毒害（之前的持有者 panicked）
    #[error("Lock error: {0}. The lock may have been poisoned by a previous panic.")]
    LockError(String),

    /// 服务配置未找到错误
    ///
    /// 请求的服务配置在 UnifiedConfig 中不存在
    #[error("Service not found: {0}. The requested service configuration does not exist in the UnifiedConfig.")]
    ServiceNotFound(String),

    /// 内部错误
    ///
    /// 内部组件错误，通常表示不可恢复的内部状态异常
    #[error("Internal error: {0}")]
    Internal(String),
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

impl CacheError {
    /// 获取错误码
    ///
    /// 返回一个唯一的错误码字符串，便于日志记录和错误追踪。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::NotFound("key".to_string());
    /// assert_eq!(err.code(), "CACHE_001");
    ///
    /// let err = CacheError::Connection("failed".to_string());
    /// assert_eq!(err.code(), "CACHE_002");
    /// ```
    pub fn code(&self) -> &'static str {
        match self {
            CacheError::NotFound(_) => "CACHE_001",
            CacheError::Connection(_) => "CACHE_002",
            CacheError::Serialization(_) => "CACHE_003",
            CacheError::Operation(_) => "CACHE_004",
            CacheError::Degraded(_) => "CACHE_005",
            CacheError::L1Error(_) => "CACHE_006",
            CacheError::L2Error(_) => "CACHE_007",
            CacheError::NotSupported(_) => "CACHE_009",
            CacheError::WalError(_) => "CACHE_010",
            CacheError::DatabaseError(_) => "CACHE_011",
            CacheError::RedisError(_) => "CACHE_012",
            CacheError::IoError(_) => "CACHE_013",
            CacheError::BackendError(_) => "CACHE_014",
            CacheError::Timeout(_) => "CACHE_015",
            CacheError::ShutdownError(_) => "CACHE_016",
            CacheError::KeyTooLong(_, _) => "CACHE_017",
            CacheError::ValueTooLarge(_, _) => "CACHE_018",
            CacheError::BufferFull(_) => "CACHE_019",
            CacheError::InvalidInput(_) => "CACHE_020",
            CacheError::InvalidKey(_) => "CACHE_021",
            CacheError::LockError(_) => "CACHE_022",
            CacheError::ServiceNotFound(_) => "CACHE_023",
            CacheError::Internal(_) => "CACHE_024",
        }
    }

    /// 检查错误是否可恢复
    ///
    /// 返回 true 表示错误可能是暂时的，可以重试。
    ///
    /// `Internal` 错误代表内部状态损坏（如锁中毒、数据不一致），不可恢复。
    /// 调用者不应重试此类错误，而应停止操作或重新初始化缓存实例。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::Connection("failed".to_string());
    /// assert!(err.is_recoverable());
    ///
    /// let err = CacheError::NotFound("key".to_string());
    /// assert!(!err.is_recoverable());
    /// ```
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CacheError::Connection(_)
                | CacheError::Timeout(_)
                | CacheError::L2Error(_)
                | CacheError::BackendError(_)
                | CacheError::BufferFull(_)
        )
    }

    /// Check if this error is a "not found" error
    ///
    /// Returns true if the error indicates that a requested key was not found.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::NotFound("key".to_string());
    /// assert!(err.is_not_found());
    ///
    /// let other_err = CacheError::Connection("failed".to_string());
    /// assert!(!other_err.is_not_found());
    /// ```
    pub fn is_not_found(&self) -> bool {
        matches!(self, CacheError::NotFound(_))
    }

    /// Check if this error is a connection error
    ///
    /// Returns true if the error indicates a connection-related failure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::Connection("failed".to_string());
    /// assert!(err.is_connection_error());
    ///
    /// let other_err = CacheError::NotFound("key".to_string());
    /// assert!(!other_err.is_connection_error());
    /// ```
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            CacheError::Connection(_) | CacheError::RedisError(_) | CacheError::L2Error(_)
        )
    }

    /// Check if this error is a degraded mode error
    ///
    /// Returns true if the error indicates the cache is operating in degraded mode.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::Degraded("L2 unavailable".to_string());
    /// assert!(err.is_degraded());
    ///
    /// let other_err = CacheError::NotFound("key".to_string());
    /// assert!(!other_err.is_degraded());
    /// ```
    pub fn is_degraded(&self) -> bool {
        matches!(self, CacheError::Degraded(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // sanitize_connection_string tests removed — replaced by
    // redact_connection_string tests in features::security::redaction

    // ========================================================================
    // CacheConfigError tests
    // ========================================================================

    #[test]
    fn test_config_error_missing_field() {
        let err = CacheConfigError::MissingField("capacity".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Missing required field"));
        assert!(msg.contains("capacity"));
    }

    #[test]
    fn test_config_error_invalid_value() {
        let err = CacheConfigError::InvalidValue {
            field: "capacity".to_string(),
            reason: "must be greater than 0".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid value for field 'capacity'"));
        assert!(msg.contains("must be greater than 0"));
    }

    #[test]
    fn test_config_error_unsupported_backend() {
        let err = CacheConfigError::UnsupportedBackend("sqlite".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Unsupported backend combination"));
        assert!(msg.contains("sqlite"));
    }

    #[test]
    fn test_config_error_connection_failed() {
        let err = CacheConfigError::ConnectionFailed("timeout".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Connection failed during initialization"));
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn test_config_error_debug_format() {
        let err = CacheConfigError::MissingField("host".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("MissingField"));
    }

    // ========================================================================
    // CacheError Display tests
    // ========================================================================

    #[test]
    fn test_error_display_serialization() {
        let err = CacheError::Serialization("invalid format".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Serialization error"));
        assert!(msg.contains("invalid format"));
    }

    #[test]
    fn test_error_display_operation() {
        let err = CacheError::Operation("retry failed".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Operation failed"));
    }

    #[test]
    fn test_error_display_connection() {
        let err = CacheError::Connection("refused".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Connection error"));
    }

    #[test]
    fn test_error_display_not_found() {
        let err = CacheError::NotFound("user:123".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Key not found"));
        assert!(msg.contains("user:123"));
    }

    #[test]
    fn test_error_display_degraded() {
        let err = CacheError::Degraded("L2 unavailable".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Cache degraded"));
    }

    #[test]
    fn test_error_display_l1_error() {
        let err = CacheError::L1Error("memory pressure".to_string());
        let msg = err.to_string();
        assert!(msg.contains("L1 cache operation failed"));
    }

    #[test]
    fn test_error_display_l2_error() {
        let err = CacheError::L2Error("redis down".to_string());
        let msg = err.to_string();
        assert!(msg.contains("L2 cache operation failed"));
    }

    #[test]
    fn test_error_display_not_supported() {
        let err = CacheError::NotSupported("batch delete".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Operation not supported"));
    }

    #[test]
    fn test_error_display_wal_error() {
        let err = CacheError::WalError("disk full".to_string());
        let msg = err.to_string();
        assert!(msg.contains("WAL (Write-Ahead Log) operation failed"));
    }

    #[test]
    fn test_error_display_database_error() {
        let err = CacheError::DatabaseError("query failed".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Database error"));
    }

    #[test]
    fn test_error_display_redis_error_placeholder() {
        #[cfg(not(feature = "redis"))]
        {
            let err = CacheError::RedisError("connection refused".to_string());
            let msg = err.to_string();
            assert!(msg.contains("Redis connection failed"));
        }
    }

    #[test]
    fn test_error_display_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = CacheError::IoError(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("access denied"));
    }

    #[test]
    fn test_error_display_backend_error() {
        let err = CacheError::BackendError("unexpected".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Backend error"));
    }

    #[test]
    fn test_error_display_timeout() {
        let err = CacheError::Timeout("30s exceeded".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Operation timed out"));
    }

    #[test]
    fn test_error_display_shutdown_error() {
        let err = CacheError::ShutdownError("incomplete".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Shutdown error"));
    }

    #[test]
    fn test_error_display_key_too_long() {
        let err = CacheError::KeyTooLong(2048, 1024);
        let msg = err.to_string();
        assert!(msg.contains("Key too long"));
        assert!(msg.contains("2048"));
        assert!(msg.contains("1024"));
    }

    #[test]
    fn test_error_display_value_too_large() {
        let err = CacheError::ValueTooLarge(200_000_000, 100_000_000);
        let msg = err.to_string();
        assert!(msg.contains("Value too large"));
        assert!(msg.contains("200000000"));
    }

    #[test]
    fn test_error_display_buffer_full() {
        let err = CacheError::BufferFull("write queue saturated".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Buffer full"));
    }

    #[test]
    fn test_error_display_invalid_input() {
        let err = CacheError::InvalidInput("empty key".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid input"));
    }

    #[test]
    fn test_error_display_invalid_key() {
        let err = CacheError::InvalidKey("contains null byte".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Invalid key"));
    }

    #[test]
    fn test_error_display_lock_error() {
        let err = CacheError::LockError("poisoned".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Lock error"));
        assert!(msg.contains("poisoned"));
    }

    #[test]
    fn test_error_display_service_not_found() {
        let err = CacheError::ServiceNotFound("analytics".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Service not found"));
    }

    #[test]
    fn test_error_display_internal() {
        let err = CacheError::Internal("unexpected state".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Internal error"));
    }

    #[test]
    fn test_error_debug_format() {
        let err = CacheError::NotFound("key".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
    }

    // ========================================================================
    // CacheError::code() tests
    // ========================================================================

    #[test]
    fn test_error_code_not_found() {
        assert_eq!(CacheError::NotFound("k".to_string()).code(), "CACHE_001");
    }

    #[test]
    fn test_error_code_connection() {
        assert_eq!(CacheError::Connection("r".to_string()).code(), "CACHE_002");
    }

    #[test]
    fn test_error_code_serialization() {
        assert_eq!(CacheError::Serialization("s".to_string()).code(), "CACHE_003");
    }

    #[test]
    fn test_error_code_operation() {
        assert_eq!(CacheError::Operation("o".to_string()).code(), "CACHE_004");
    }

    #[test]
    fn test_error_code_degraded() {
        assert_eq!(CacheError::Degraded("d".to_string()).code(), "CACHE_005");
    }

    #[test]
    fn test_error_code_l1_error() {
        assert_eq!(CacheError::L1Error("l".to_string()).code(), "CACHE_006");
    }

    #[test]
    fn test_error_code_l2_error() {
        assert_eq!(CacheError::L2Error("l".to_string()).code(), "CACHE_007");
    }

    #[test]
    fn test_error_code_not_supported() {
        assert_eq!(CacheError::NotSupported("n".to_string()).code(), "CACHE_009");
    }

    #[test]
    fn test_error_code_wal_error() {
        assert_eq!(CacheError::WalError("w".to_string()).code(), "CACHE_010");
    }

    #[test]
    fn test_error_code_database_error() {
        assert_eq!(CacheError::DatabaseError("d".to_string()).code(), "CACHE_011");
    }

    #[test]
    fn test_error_code_redis_error() {
        #[cfg(not(feature = "redis"))]
        {
            assert_eq!(CacheError::RedisError("r".to_string()).code(), "CACHE_012");
        }
    }

    #[test]
    fn test_error_code_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "err");
        assert_eq!(CacheError::IoError(io_err).code(), "CACHE_013");
    }

    #[test]
    fn test_error_code_backend_error() {
        assert_eq!(CacheError::BackendError("b".to_string()).code(), "CACHE_014");
    }

    #[test]
    fn test_error_code_timeout() {
        assert_eq!(CacheError::Timeout("t".to_string()).code(), "CACHE_015");
    }

    #[test]
    fn test_error_code_shutdown_error() {
        assert_eq!(CacheError::ShutdownError("s".to_string()).code(), "CACHE_016");
    }

    #[test]
    fn test_error_code_key_too_long() {
        assert_eq!(CacheError::KeyTooLong(100, 50).code(), "CACHE_017");
    }

    #[test]
    fn test_error_code_value_too_large() {
        assert_eq!(CacheError::ValueTooLarge(100, 50).code(), "CACHE_018");
    }

    #[test]
    fn test_error_code_buffer_full() {
        assert_eq!(CacheError::BufferFull("f".to_string()).code(), "CACHE_019");
    }

    #[test]
    fn test_error_code_invalid_input() {
        assert_eq!(CacheError::InvalidInput("i".to_string()).code(), "CACHE_020");
    }

    #[test]
    fn test_error_code_invalid_key() {
        assert_eq!(CacheError::InvalidKey("i".to_string()).code(), "CACHE_021");
    }

    #[test]
    fn test_error_code_lock_error() {
        assert_eq!(CacheError::LockError("l".to_string()).code(), "CACHE_022");
    }

    #[test]
    fn test_error_code_service_not_found() {
        assert_eq!(CacheError::ServiceNotFound("s".to_string()).code(), "CACHE_023");
    }

    #[test]
    fn test_error_code_internal() {
        assert_eq!(CacheError::Internal("i".to_string()).code(), "CACHE_024");
    }

    // ========================================================================
    // CacheError::is_recoverable() tests
    // ========================================================================

    #[test]
    fn test_is_recoverable_connection() {
        assert!(CacheError::Connection("refused".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_timeout() {
        assert!(CacheError::Timeout("expired".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_l2_error() {
        assert!(CacheError::L2Error("down".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_backend_error() {
        assert!(CacheError::BackendError("err".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_buffer_full() {
        assert!(CacheError::BufferFull("full".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_internal() {
        assert!(!CacheError::Internal("state".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_not_found() {
        assert!(!CacheError::NotFound("k".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_serialization() {
        assert!(!CacheError::Serialization("bad".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_degraded() {
        assert!(!CacheError::Degraded("down".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_invalid_input() {
        assert!(!CacheError::InvalidInput("empty".to_string()).is_recoverable());
    }

    // ========================================================================
    // CacheError::is_not_found() tests
    // ========================================================================

    #[test]
    fn test_is_not_found_true() {
        assert!(CacheError::NotFound("k".to_string()).is_not_found());
    }

    #[test]
    fn test_is_not_found_false_for_other_errors() {
        assert!(!CacheError::Connection("r".to_string()).is_not_found());
        assert!(!CacheError::Serialization("s".to_string()).is_not_found());
        assert!(!CacheError::Operation("o".to_string()).is_not_found());
    }

    // ========================================================================
    // CacheError::is_connection_error() tests
    // ========================================================================

    #[test]
    fn test_is_connection_error_direct() {
        assert!(CacheError::Connection("refused".to_string()).is_connection_error());
    }

    #[test]
    fn test_is_connection_error_l2() {
        assert!(CacheError::L2Error("redis down".to_string()).is_connection_error());
    }

    #[test]
    fn test_is_not_connection_error() {
        assert!(!CacheError::NotFound("k".to_string()).is_connection_error());
        assert!(!CacheError::Timeout("t".to_string()).is_connection_error());
        assert!(!CacheError::Degraded("d".to_string()).is_connection_error());
    }

    // ========================================================================
    // CacheError::is_degraded() tests
    // ========================================================================

    #[test]
    fn test_is_degraded_true() {
        assert!(CacheError::Degraded("L2 unavailable".to_string()).is_degraded());
    }

    #[test]
    fn test_is_degraded_false_for_other_errors() {
        assert!(!CacheError::NotFound("k".to_string()).is_degraded());
        assert!(!CacheError::Connection("r".to_string()).is_degraded());
    }

    // ========================================================================
    // From<std::io::Error> tests
    // ========================================================================

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let cache_err: CacheError = io_err.into();
        match cache_err {
            CacheError::IoError(_) => {}
            other => panic!("Expected IoError, got {:?}", other),
        }
    }

    #[test]
    fn test_io_error_question_mark() {
        fn may_fail() -> std::result::Result<(), CacheError> {
            let _ = std::fs::read("/nonexistent/path/that/does/not/exist")?;
            Ok(())
        }
        let result = may_fail();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "CACHE_013");
    }

    // ========================================================================
    // Result type alias tests
    // ========================================================================

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> std::result::Result<String, CacheError> {
            Ok("success".to_string())
        }
        assert_eq!(returns_result().unwrap(), "success");
    }

    #[test]
    fn test_config_result_type_alias() {
        fn returns_config_result() -> ConfigResult<i32> {
            Err(CacheConfigError::MissingField("port".to_string()))
        }
        let result = returns_config_result();
        assert!(result.is_err());
    }
}
