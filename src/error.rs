// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了缓存系统的错误类型和处理机制。

use thiserror::Error;

#[cfg(feature = "redis")]
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
#[cfg(feature = "redis")]
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
/// - **连接错误** ([`CacheError::ConnectionError`]): 网络连接问题
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

    /// Redis错误
    #[cfg(feature = "redis")]
    #[error("Redis connection failed: {0}")]
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

impl From<std::io::Error> for CacheError {
    fn from(e: std::io::Error) -> Self {
        CacheError::IoError(e)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(e: serde_json::Error) -> Self {
        CacheError::Serialization(e.to_string())
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

    // ============================================================================
    // CacheConfigError Display tests
    // ============================================================================

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_missing_field_display() {
        let err = CacheConfigError::MissingField("host".to_string());
        assert_eq!(err.to_string(), "Missing required field: host");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_invalid_value_display() {
        let err = CacheConfigError::InvalidValue {
            field: "capacity".to_string(),
            reason: "must be > 0".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid value for field 'capacity': must be > 0");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_unsupported_backend_display() {
        let err = CacheConfigError::UnsupportedBackend("unknown".to_string());
        assert_eq!(err.to_string(), "Unsupported backend combination: unknown");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_connection_failed_display() {
        let err = CacheConfigError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Connection failed during initialization: timeout");
    }

    // ============================================================================
    // CacheError Display tests - all variants
    // ============================================================================

    #[test]
    fn test_cache_error_serialization_display() {
        let err = CacheError::Serialization("bad data".to_string());
        let s = err.to_string();
        assert!(s.contains("Serialization error: bad data"));
    }

    #[test]
    fn test_cache_error_operation_display() {
        let err = CacheError::Operation("fail".to_string());
        let s = err.to_string();
        assert!(s.contains("Operation failed: fail"));
    }

    #[test]
    fn test_cache_error_connection_display() {
        let err = CacheError::Connection("refused".to_string());
        let s = err.to_string();
        assert!(s.contains("Connection error: refused"));
    }

    #[test]
    fn test_cache_error_not_found_display() {
        let err = CacheError::NotFound("key1".to_string());
        let s = err.to_string();
        assert!(s.contains("Key not found: key1"));
    }

    #[test]
    fn test_cache_error_degraded_display() {
        let err = CacheError::Degraded("L2 down".to_string());
        let s = err.to_string();
        assert!(s.contains("Cache degraded: L2 down"));
    }

    #[test]
    fn test_cache_error_l1_error_display() {
        let err = CacheError::L1Error("oom".to_string());
        let s = err.to_string();
        assert!(s.contains("L1 cache operation failed: oom"));
    }

    #[test]
    fn test_cache_error_l2_error_display() {
        let err = CacheError::L2Error("redis down".to_string());
        let s = err.to_string();
        assert!(s.contains("L2 cache operation failed: redis down"));
    }

    #[test]
    fn test_cache_error_not_supported_display() {
        let err = CacheError::NotSupported("scan".to_string());
        let s = err.to_string();
        assert!(s.contains("Operation not supported: scan"));
    }

    #[test]
    fn test_cache_error_wal_error_display() {
        let err = CacheError::WalError("disk full".to_string());
        let s = err.to_string();
        assert!(s.contains("WAL (Write-Ahead Log) operation failed: disk full"));
    }

    #[test]
    fn test_cache_error_database_error_display() {
        let err = CacheError::DatabaseError("query failed".to_string());
        let s = err.to_string();
        assert!(s.contains("Database error: query failed"));
    }

    #[test]
    fn test_cache_error_redis_error_display() {
        #[cfg(feature = "redis")]
        {
            let err = CacheError::RedisError(redis::RedisError::from(std::io::Error::other("auth failed")));
            let s = err.to_string();
            assert!(s.contains("Redis connection failed"));
        }
        #[cfg(not(feature = "redis"))]
        {
            let err = CacheError::RedisError("auth failed".to_string());
            let s = err.to_string();
            assert!(s.contains("Redis connection failed: auth failed"));
        }
    }

    #[test]
    fn test_cache_error_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = CacheError::IoError(io_err);
        let s = err.to_string();
        assert!(s.contains("I/O error:"));
    }

    #[test]
    fn test_cache_error_backend_error_display() {
        let err = CacheError::BackendError("transient".to_string());
        let s = err.to_string();
        assert!(s.contains("Backend error: transient"));
    }

    #[test]
    fn test_cache_error_timeout_display() {
        let err = CacheError::Timeout("5s".to_string());
        let s = err.to_string();
        assert!(s.contains("Operation timed out: 5s"));
    }

    #[test]
    fn test_cache_error_shutdown_error_display() {
        let err = CacheError::ShutdownError("leak".to_string());
        let s = err.to_string();
        assert!(s.contains("Shutdown error: leak"));
    }

    #[test]
    fn test_cache_error_key_too_long_display() {
        let err = CacheError::KeyTooLong(600, 512);
        let s = err.to_string();
        assert!(s.contains("Key too long: 600. Maximum key length is 512 bytes."));
    }

    #[test]
    fn test_cache_error_value_too_large_display() {
        let err = CacheError::ValueTooLarge(2048, 1024);
        let s = err.to_string();
        assert!(s.contains("Value too large: 2048. Maximum value size is 1024 bytes."));
    }

    #[test]
    fn test_cache_error_buffer_full_display() {
        let err = CacheError::BufferFull("batch".to_string());
        let s = err.to_string();
        assert!(s.contains("Buffer full: batch"));
    }

    #[test]
    fn test_cache_error_invalid_input_display() {
        let err = CacheError::InvalidInput("bad".to_string());
        let s = err.to_string();
        assert!(s.contains("Invalid input: bad"));
    }

    #[test]
    fn test_cache_error_invalid_key_display() {
        let err = CacheError::InvalidKey("bad key".to_string());
        let s = err.to_string();
        assert!(s.contains("Invalid key: bad key"));
    }

    #[test]
    fn test_cache_error_lock_error_display() {
        let err = CacheError::LockError("poisoned".to_string());
        let s = err.to_string();
        assert!(s.contains("Lock error: poisoned"));
    }

    #[test]
    fn test_cache_error_service_not_found_display() {
        let err = CacheError::ServiceNotFound("svc".to_string());
        let s = err.to_string();
        assert!(s.contains("Service not found: svc"));
    }

    #[test]
    fn test_cache_error_internal_display() {
        let err = CacheError::Internal("boom".to_string());
        let s = err.to_string();
        assert_eq!(s, "Internal error: boom");
    }

    // ============================================================================
    // From conversions
    // ============================================================================

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let cache_err: CacheError = io_err.into();
        assert!(matches!(cache_err, CacheError::IoError(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let cache_err: CacheError = serde_err.into();
        assert!(matches!(cache_err, CacheError::Serialization(_)));
    }

    // ============================================================================
    // code() method tests
    // ============================================================================

    #[test]
    fn test_error_code_not_found() {
        assert_eq!(CacheError::NotFound("k".to_string()).code(), "CACHE_001");
    }

    #[test]
    fn test_error_code_connection() {
        assert_eq!(CacheError::Connection("c".to_string()).code(), "CACHE_002");
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
    fn test_error_code_l1() {
        assert_eq!(CacheError::L1Error("l1".to_string()).code(), "CACHE_006");
    }

    #[test]
    fn test_error_code_l2() {
        assert_eq!(CacheError::L2Error("l2".to_string()).code(), "CACHE_007");
    }

    #[test]
    fn test_error_code_not_supported() {
        assert_eq!(CacheError::NotSupported("ns".to_string()).code(), "CACHE_009");
    }

    #[test]
    fn test_error_code_wal() {
        assert_eq!(CacheError::WalError("w".to_string()).code(), "CACHE_010");
    }

    #[test]
    fn test_error_code_database() {
        assert_eq!(CacheError::DatabaseError("db".to_string()).code(), "CACHE_011");
    }

    #[test]
    fn test_error_code_redis() {
        #[cfg(feature = "redis")]
        {
            let err = CacheError::RedisError(redis::RedisError::from(std::io::Error::other("r")));
            assert_eq!(err.code(), "CACHE_012");
        }
        #[cfg(not(feature = "redis"))]
        {
            assert_eq!(CacheError::RedisError("r".to_string()).code(), "CACHE_012");
        }
    }

    #[test]
    fn test_error_code_io() {
        let io_err = std::io::Error::other("x");
        assert_eq!(CacheError::IoError(io_err).code(), "CACHE_013");
    }

    #[test]
    fn test_error_code_backend() {
        assert_eq!(CacheError::BackendError("b".to_string()).code(), "CACHE_014");
    }

    #[test]
    fn test_error_code_timeout() {
        assert_eq!(CacheError::Timeout("t".to_string()).code(), "CACHE_015");
    }

    #[test]
    fn test_error_code_shutdown() {
        assert_eq!(CacheError::ShutdownError("s".to_string()).code(), "CACHE_016");
    }

    #[test]
    fn test_error_code_key_too_long() {
        assert_eq!(CacheError::KeyTooLong(1, 2).code(), "CACHE_017");
    }

    #[test]
    fn test_error_code_value_too_large() {
        assert_eq!(CacheError::ValueTooLarge(1, 2).code(), "CACHE_018");
    }

    #[test]
    fn test_error_code_buffer_full() {
        assert_eq!(CacheError::BufferFull("b".to_string()).code(), "CACHE_019");
    }

    #[test]
    fn test_error_code_invalid_input() {
        assert_eq!(CacheError::InvalidInput("i".to_string()).code(), "CACHE_020");
    }

    #[test]
    fn test_error_code_invalid_key() {
        assert_eq!(CacheError::InvalidKey("k".to_string()).code(), "CACHE_021");
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

    // ============================================================================
    // is_recoverable() tests
    // ============================================================================

    #[test]
    fn test_is_recoverable_connection() {
        assert!(CacheError::Connection("c".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_timeout() {
        assert!(CacheError::Timeout("t".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_l2() {
        assert!(CacheError::L2Error("l2".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_backend() {
        assert!(CacheError::BackendError("b".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_buffer_full() {
        assert!(CacheError::BufferFull("b".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_not_found() {
        assert!(!CacheError::NotFound("k".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_internal() {
        assert!(!CacheError::Internal("i".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_serialization() {
        assert!(!CacheError::Serialization("s".to_string()).is_recoverable());
    }

    // ============================================================================
    // is_not_found() tests
    // ============================================================================

    #[test]
    fn test_is_not_found_true() {
        assert!(CacheError::NotFound("key".to_string()).is_not_found());
    }

    #[test]
    fn test_is_not_found_false() {
        assert!(!CacheError::Connection("c".to_string()).is_not_found());
    }

    // ============================================================================
    // is_connection_error() tests
    // ============================================================================

    #[test]
    fn test_is_connection_error_connection() {
        assert!(CacheError::Connection("c".to_string()).is_connection_error());
    }

    #[test]
    fn test_is_connection_error_redis() {
        #[cfg(feature = "redis")]
        {
            let err = CacheError::RedisError(redis::RedisError::from(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "r",
            )));
            assert!(err.is_connection_error());
        }
        #[cfg(not(feature = "redis"))]
        {
            assert!(CacheError::RedisError("r".to_string()).is_connection_error());
        }
    }

    #[test]
    fn test_is_connection_error_l2() {
        assert!(CacheError::L2Error("l2".to_string()).is_connection_error());
    }

    #[test]
    fn test_is_connection_error_false() {
        assert!(!CacheError::NotFound("k".to_string()).is_connection_error());
    }

    // ============================================================================
    // is_degraded() tests
    // ============================================================================

    #[test]
    fn test_is_degraded_true() {
        assert!(CacheError::Degraded("d".to_string()).is_degraded());
    }

    #[test]
    fn test_is_degraded_false() {
        assert!(!CacheError::NotFound("k".to_string()).is_degraded());
    }

    // ============================================================================
    // Debug trait test
    // ============================================================================

    #[test]
    fn test_cache_error_debug() {
        let err = CacheError::NotFound("key".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_debug() {
        let err = CacheConfigError::MissingField("f".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("MissingField"));
    }

    // ============================================================================
    // std::error::Error trait test
    // ============================================================================

    #[test]
    fn test_cache_error_is_std_error() {
        let err = CacheError::NotFound("key".to_string());
        let _: &dyn std::error::Error = &err;
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_is_std_error() {
        let err = CacheConfigError::MissingField("f".to_string());
        let _: &dyn std::error::Error = &err;
    }
}
