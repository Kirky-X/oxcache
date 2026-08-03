// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! error 模块单元测试

use super::OxCacheError;
#[cfg(feature = "redis")]
use super::OxCacheConfigError;

// ============================================================================
// OxCacheConfigError Display tests
// ============================================================================

#[cfg(feature = "redis")]
#[test]
fn test_cache_config_error_missing_field_display() {
    let err = OxCacheConfigError::MissingField("host".to_string());
    assert_eq!(err.to_string(), "Missing required field: host.");
}

#[cfg(feature = "redis")]
#[test]
fn test_cache_config_error_invalid_value_display() {
    let err = OxCacheConfigError::InvalidValue {
        field: "capacity".to_string(),
        reason: "must be > 0".to_string(),
    };
    assert_eq!(err.to_string(), "Invalid value for field 'capacity': must be > 0.");
}

#[cfg(feature = "redis")]
#[test]
fn test_cache_config_error_unsupported_backend_display() {
    let err = OxCacheConfigError::UnsupportedBackend("unknown".to_string());
    assert_eq!(err.to_string(), "Unsupported backend combination: unknown.");
}

#[cfg(feature = "redis")]
#[test]
fn test_cache_config_error_connection_failed_display() {
    let err = OxCacheConfigError::ConnectionFailed("timeout".to_string());
    assert_eq!(err.to_string(), "Connection failed during initialization: timeout.");
}

// ============================================================================
// OxCacheError Display tests - all variants
// ============================================================================

#[test]
fn test_cache_error_serialization_display() {
    let err = OxCacheError::Serialization("bad data".to_string());
    let s = err.to_string();
    assert!(s.contains("Serialization error: bad data"));
}

#[test]
fn test_cache_error_operation_display() {
    let err = OxCacheError::Operation("fail".to_string());
    let s = err.to_string();
    assert!(s.contains("Operation failed: fail"));
}

#[test]
fn test_cache_error_connection_display() {
    let err = OxCacheError::Connection("refused".to_string());
    let s = err.to_string();
    assert!(s.contains("Connection error: refused"));
}

#[test]
fn test_cache_error_not_found_display() {
    let err = OxCacheError::NotFound("key1".to_string());
    let s = err.to_string();
    assert!(s.contains("Key not found: key1"));
}

#[test]
fn test_cache_error_degraded_display() {
    let err = OxCacheError::Degraded("L2 down".to_string());
    let s = err.to_string();
    assert!(s.contains("Cache degraded: L2 down"));
}

#[test]
fn test_cache_error_l1_error_display() {
    let err = OxCacheError::L1Error("oom".to_string());
    let s = err.to_string();
    assert!(s.contains("L1 cache operation failed: oom"));
}

#[test]
fn test_cache_error_l2_error_display() {
    let err = OxCacheError::L2Error("redis down".to_string());
    let s = err.to_string();
    assert!(s.contains("L2 cache operation failed: redis down"));
}

#[test]
fn test_cache_error_not_supported_display() {
    let err = OxCacheError::NotSupported("scan".to_string());
    let s = err.to_string();
    assert!(s.contains("Operation not supported: scan"));
}

#[test]
fn test_cache_error_wal_error_display() {
    let err = OxCacheError::WalError("disk full".to_string());
    let s = err.to_string();
    assert!(s.contains("WAL (Write-Ahead Log) operation failed: disk full"));
}

#[test]
fn test_cache_error_database_error_display() {
    let err = OxCacheError::DatabaseError("query failed".to_string());
    let s = err.to_string();
    assert!(s.contains("Database error: query failed"));
}

#[test]
fn test_cache_error_redis_error_display() {
    #[cfg(feature = "redis")]
    {
        let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::other("auth failed")));
        let s = err.to_string();
        assert!(s.contains("Redis connection failed"));
    }
    #[cfg(not(feature = "redis"))]
    {
        let err = OxCacheError::RedisError("auth failed".to_string());
        let s = err.to_string();
        assert!(s.contains("Redis connection failed: auth failed"));
    }
}

#[test]
fn test_cache_error_io_error_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err = OxCacheError::IoError(io_err);
    let s = err.to_string();
    assert!(s.contains("I/O error:"));
}

#[test]
fn test_cache_error_backend_error_display() {
    let err = OxCacheError::BackendError("transient".to_string());
    let s = err.to_string();
    assert!(s.contains("Backend error: transient"));
}

#[test]
fn test_cache_error_timeout_display() {
    let err = OxCacheError::Timeout("5s".to_string());
    let s = err.to_string();
    assert!(s.contains("Operation timed out: 5s"));
}

#[test]
fn test_cache_error_shutdown_error_display() {
    let err = OxCacheError::ShutdownError("leak".to_string());
    let s = err.to_string();
    assert!(s.contains("Shutdown error: leak"));
}

#[test]
fn test_cache_error_key_too_long_display() {
    let err = OxCacheError::KeyTooLong(600, 512);
    let s = err.to_string();
    assert!(s.contains("Key too long: 600. Maximum key length is 512 bytes."));
}

#[test]
fn test_cache_error_value_too_large_display() {
    let err = OxCacheError::ValueTooLarge(2048, 1024);
    let s = err.to_string();
    assert!(s.contains("Value too large: 2048. Maximum value size is 1024 bytes."));
}

#[test]
fn test_cache_error_buffer_full_display() {
    let err = OxCacheError::BufferFull("batch".to_string());
    let s = err.to_string();
    assert!(s.contains("Buffer full: batch"));
}

#[test]
fn test_cache_error_invalid_input_display() {
    let err = OxCacheError::InvalidInput("bad".to_string());
    let s = err.to_string();
    assert!(s.contains("Invalid input: bad"));
}

#[test]
fn test_cache_error_invalid_key_display() {
    let err = OxCacheError::InvalidKey("bad key".to_string());
    let s = err.to_string();
    assert!(s.contains("Invalid key: bad key"));
}

#[test]
fn test_cache_error_lock_error_display() {
    let err = OxCacheError::LockError("poisoned".to_string());
    let s = err.to_string();
    assert!(s.contains("Lock error: poisoned"));
}

#[test]
fn test_cache_error_service_not_found_display() {
    let err = OxCacheError::ServiceNotFound("svc".to_string());
    let s = err.to_string();
    assert!(s.contains("Service not found: svc"));
}

#[test]
fn test_cache_error_internal_display() {
    let err = OxCacheError::Internal("boom".to_string());
    let s = err.to_string();
    assert_eq!(s, "Internal error: boom.");
}

// ============================================================================
// From conversions
// ============================================================================

#[test]
fn test_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let cache_err: OxCacheError = io_err.into();
    assert!(matches!(cache_err, OxCacheError::IoError(_)));
}

#[cfg(any(feature = "serialization", feature = "full"))]
#[test]
fn test_from_serde_json_error() {
    let serde_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let cache_err: OxCacheError = serde_err.into();
    assert!(matches!(cache_err, OxCacheError::Serialization(_)));
}

// ============================================================================
// code() method tests
// ============================================================================

#[test]
fn test_error_code_not_found() {
    assert_eq!(OxCacheError::NotFound("k".to_string()).code(), "OXCACHE_001");
}

#[test]
fn test_error_code_connection() {
    assert_eq!(OxCacheError::Connection("c".to_string()).code(), "OXCACHE_002");
}

#[test]
fn test_error_code_serialization() {
    assert_eq!(OxCacheError::Serialization("s".to_string()).code(), "OXCACHE_003");
}

#[test]
fn test_error_code_operation() {
    assert_eq!(OxCacheError::Operation("o".to_string()).code(), "OXCACHE_004");
}

#[test]
fn test_error_code_degraded() {
    assert_eq!(OxCacheError::Degraded("d".to_string()).code(), "OXCACHE_005");
}

#[test]
fn test_error_code_l1() {
    assert_eq!(OxCacheError::L1Error("l1".to_string()).code(), "OXCACHE_006");
}

#[test]
fn test_error_code_l2() {
    assert_eq!(OxCacheError::L2Error("l2".to_string()).code(), "OXCACHE_007");
}

#[test]
fn test_error_code_not_supported() {
    assert_eq!(OxCacheError::NotSupported("ns".to_string()).code(), "OXCACHE_009");
}

#[test]
fn test_error_code_wal() {
    assert_eq!(OxCacheError::WalError("w".to_string()).code(), "OXCACHE_010");
}

#[test]
fn test_error_code_database() {
    assert_eq!(OxCacheError::DatabaseError("db".to_string()).code(), "OXCACHE_011");
}

#[test]
fn test_error_code_redis() {
    #[cfg(feature = "redis")]
    {
        let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::other("r")));
        assert_eq!(err.code(), "OXCACHE_012");
    }
    #[cfg(not(feature = "redis"))]
    {
        assert_eq!(OxCacheError::RedisError("r".to_string()).code(), "OXCACHE_012");
    }
}

#[test]
fn test_error_code_io() {
    let io_err = std::io::Error::other("x");
    assert_eq!(OxCacheError::IoError(io_err).code(), "OXCACHE_013");
}

#[test]
fn test_error_code_backend() {
    assert_eq!(OxCacheError::BackendError("b".to_string()).code(), "OXCACHE_014");
}

#[test]
fn test_error_code_timeout() {
    assert_eq!(OxCacheError::Timeout("t".to_string()).code(), "OXCACHE_015");
}

#[test]
fn test_error_code_shutdown() {
    assert_eq!(OxCacheError::ShutdownError("s".to_string()).code(), "OXCACHE_016");
}

#[test]
fn test_error_code_key_too_long() {
    assert_eq!(OxCacheError::KeyTooLong(1, 2).code(), "OXCACHE_017");
}

#[test]
fn test_error_code_value_too_large() {
    assert_eq!(OxCacheError::ValueTooLarge(1, 2).code(), "OXCACHE_018");
}

#[test]
fn test_error_code_buffer_full() {
    assert_eq!(OxCacheError::BufferFull("b".to_string()).code(), "OXCACHE_019");
}

#[test]
fn test_error_code_invalid_input() {
    assert_eq!(OxCacheError::InvalidInput("i".to_string()).code(), "OXCACHE_020");
}

#[test]
fn test_error_code_invalid_key() {
    assert_eq!(OxCacheError::InvalidKey("k".to_string()).code(), "OXCACHE_021");
}

#[test]
fn test_error_code_lock_error() {
    assert_eq!(OxCacheError::LockError("l".to_string()).code(), "OXCACHE_022");
}

#[test]
fn test_error_code_service_not_found() {
    assert_eq!(OxCacheError::ServiceNotFound("s".to_string()).code(), "OXCACHE_023");
}

#[test]
fn test_error_code_internal() {
    assert_eq!(OxCacheError::Internal("i".to_string()).code(), "OXCACHE_024");
}

// ============================================================================
// is_recoverable() tests
// ============================================================================

#[test]
fn test_is_recoverable_connection() {
    assert!(OxCacheError::Connection("c".to_string()).is_recoverable());
}

#[test]
fn test_is_recoverable_timeout() {
    assert!(OxCacheError::Timeout("t".to_string()).is_recoverable());
}

#[test]
fn test_is_recoverable_redis() {
    // RedisError is a connection error and should be recoverable
    #[cfg(feature = "redis")]
    {
        let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::other("connection reset")));
        assert!(err.is_recoverable());
    }
}

#[test]
fn test_is_recoverable_l2() {
    assert!(OxCacheError::L2Error("l2".to_string()).is_recoverable());
}

#[test]
fn test_is_recoverable_backend() {
    assert!(OxCacheError::BackendError("b".to_string()).is_recoverable());
}

#[test]
fn test_is_recoverable_buffer_full() {
    assert!(OxCacheError::BufferFull("b".to_string()).is_recoverable());
}

#[test]
fn test_is_not_recoverable_not_found() {
    assert!(!OxCacheError::NotFound("k".to_string()).is_recoverable());
}

#[test]
fn test_is_not_recoverable_internal() {
    assert!(!OxCacheError::Internal("i".to_string()).is_recoverable());
}

#[test]
fn test_is_not_recoverable_serialization() {
    assert!(!OxCacheError::Serialization("s".to_string()).is_recoverable());
}

// ============================================================================
// is_not_found() tests
// ============================================================================

#[test]
fn test_is_not_found_true() {
    assert!(OxCacheError::NotFound("key".to_string()).is_not_found());
}

#[test]
fn test_is_not_found_false() {
    assert!(!OxCacheError::Connection("c".to_string()).is_not_found());
}

// ============================================================================
// is_connection_error() tests
// ============================================================================

#[test]
fn test_is_connection_error_connection() {
    assert!(OxCacheError::Connection("c".to_string()).is_connection_error());
}

#[test]
fn test_is_connection_error_redis() {
    #[cfg(feature = "redis")]
    {
        let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "r",
        )));
        assert!(err.is_connection_error());
    }
    #[cfg(not(feature = "redis"))]
    {
        assert!(OxCacheError::RedisError("r".to_string()).is_connection_error());
    }
}

#[test]
fn test_is_connection_error_l2() {
    assert!(OxCacheError::L2Error("l2".to_string()).is_connection_error());
}

#[test]
fn test_is_connection_error_false() {
    assert!(!OxCacheError::NotFound("k".to_string()).is_connection_error());
}

// ============================================================================
// is_degraded() tests
// ============================================================================

#[test]
fn test_is_degraded_true() {
    assert!(OxCacheError::Degraded("d".to_string()).is_degraded());
}

#[test]
fn test_is_degraded_false() {
    assert!(!OxCacheError::NotFound("k".to_string()).is_degraded());
}

// ============================================================================
// Debug trait test
// ============================================================================

#[test]
fn test_cache_error_debug() {
    let err = OxCacheError::NotFound("key".to_string());
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("NotFound"));
}

#[cfg(feature = "redis")]
#[test]
fn test_cache_config_error_debug() {
    let err = OxCacheConfigError::MissingField("f".to_string());
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("MissingField"));
}

// ============================================================================
// std::error::Error trait test
// ============================================================================

#[test]
fn test_cache_error_is_std_error() {
    let err = OxCacheError::NotFound("key".to_string());
    let _: &dyn std::error::Error = &err;
}

#[cfg(feature = "redis")]
#[test]
fn test_cache_config_error_is_std_error() {
    let err = OxCacheConfigError::MissingField("f".to_string());
    let _: &dyn std::error::Error = &err;
}

// ============================================================================
// message_id() tests
// ============================================================================

#[test]
fn test_error_message_id_not_found() {
    assert_eq!(
        OxCacheError::NotFound("k".to_string()).message_id(),
        "error.not_found"
    );
}

#[test]
fn test_error_message_id_connection() {
    assert_eq!(
        OxCacheError::Connection("c".to_string()).message_id(),
        "error.connection"
    );
}

#[test]
fn test_error_message_id_serialization() {
    assert_eq!(
        OxCacheError::Serialization("s".to_string()).message_id(),
        "error.serialization"
    );
}

#[test]
fn test_error_message_id_key_too_long() {
    assert_eq!(
        OxCacheError::KeyTooLong(1, 2).message_id(),
        "error.key_too_long"
    );
}

#[test]
fn test_error_message_id_internal() {
    assert_eq!(
        OxCacheError::Internal("i".to_string()).message_id(),
        "error.internal"
    );
}

// ============================================================================
// localized_message() tests — English
// ============================================================================

#[test]
fn test_localized_message_en_not_found() {
    let err = OxCacheError::NotFound("user:42".to_string());
    let msg = err.localized_message("en");
    assert!(
        msg.contains("Key not found: user:42"),
        "en localized message should contain 'Key not found: user:42': got '{msg}'"
    );
}

#[test]
fn test_localized_message_en_connection() {
    let err = OxCacheError::Connection("refused".to_string());
    let msg = err.localized_message("en");
    assert!(
        msg.contains("Connection error: refused"),
        "en localized message: got '{msg}'"
    );
}

#[test]
fn test_localized_message_en_key_too_long() {
    let err = OxCacheError::KeyTooLong(600, 512);
    let msg = err.localized_message("en");
    assert!(
        msg.contains("600") && msg.contains("512"),
        "en localized message should contain sizes: got '{msg}'"
    );
}

#[test]
fn test_localized_message_en_value_too_large() {
    let err = OxCacheError::ValueTooLarge(2048, 1024);
    let msg = err.localized_message("en");
    assert!(
        msg.contains("2048") && msg.contains("1024"),
        "en localized message should contain sizes: got '{msg}'"
    );
}

#[test]
fn test_localized_message_en_timeout() {
    let err = OxCacheError::Timeout("5s".to_string());
    let msg = err.localized_message("en");
    assert!(
        msg.contains("Operation timed out: 5s"),
        "en localized message: got '{msg}'"
    );
}

#[test]
fn test_localized_message_en_internal() {
    let err = OxCacheError::Internal("boom".to_string());
    let msg = err.localized_message("en");
    assert!(
        msg.contains("Internal error: boom"),
        "en localized message: got '{msg}'"
    );
}

// ============================================================================
// localized_message() tests — Simplified Chinese
// ============================================================================

#[test]
fn test_localized_message_zh_not_found() {
    let err = OxCacheError::NotFound("user:42".to_string());
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("键未找到：user:42"),
        "zh localized message should contain '键未找到：user:42': got '{msg}'"
    );
}

#[test]
fn test_localized_message_zh_connection() {
    let err = OxCacheError::Connection("连接被拒绝".to_string());
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("连接错误：连接被拒绝"),
        "zh localized message: got '{msg}'"
    );
}

#[test]
fn test_localized_message_zh_key_too_long() {
    let err = OxCacheError::KeyTooLong(600, 512);
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("键过长") && msg.contains("600") && msg.contains("512"),
        "zh localized message should contain '键过长' with values: got '{msg}'"
    );
}

#[test]
fn test_localized_message_zh_timeout() {
    let err = OxCacheError::Timeout("5秒".to_string());
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("操作超时：5秒"),
        "zh localized message: got '{msg}'"
    );
}

#[test]
fn test_localized_message_zh_internal() {
    let err = OxCacheError::Internal("内部异常".to_string());
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("内部错误：内部异常"),
        "zh localized message: got '{msg}'"
    );
}

// ============================================================================
// localized_message() — fallback behavior
// ============================================================================

#[test]
fn test_localized_message_unknown_locale_falls_back_to_en() {
    let err = OxCacheError::NotFound("key".to_string());
    let msg = err.localized_message("fr-FR");
    assert!(
        msg.contains("Key not found: key"),
        "unknown locale should fall back to English: got '{msg}'"
    );
}

// ============================================================================
// OxCacheConfigError localized_message tests (redis feature only)
// ============================================================================

#[cfg(feature = "redis")]
#[test]
fn test_config_error_message_id() {
    let err = OxCacheConfigError::MissingField("host".to_string());
    assert_eq!(err.message_id(), "config.missing_field");
}

#[cfg(feature = "redis")]
#[test]
fn test_config_error_localized_message_en() {
    let err = OxCacheConfigError::MissingField("host".to_string());
    let msg = err.localized_message("en");
    assert!(
        msg.contains("Missing required field: host"),
        "en config error message: got '{msg}'"
    );
}

#[cfg(feature = "redis")]
#[test]
fn test_config_error_localized_message_zh() {
    let err = OxCacheConfigError::MissingField("host".to_string());
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("缺少必需字段：host"),
        "zh config error message: got '{msg}'"
    );
}

#[cfg(feature = "redis")]
#[test]
fn test_config_error_invalid_value_localized_zh() {
    let err = OxCacheConfigError::InvalidValue {
        field: "capacity".to_string(),
        reason: "must be > 0".to_string(),
    };
    let msg = err.localized_message("zh-CN");
    assert!(
        msg.contains("capacity") && msg.contains("must be > 0"),
        "zh config invalid value message: got '{msg}'"
    );
}

// ============================================================================
// Display with global default locale
// ============================================================================

#[test]
fn test_display_uses_default_locale_en() {
    // Default locale is "en"
    crate::i18n::set_default_locale("en");
    let err = OxCacheError::NotFound("key1".to_string());
    let s = err.to_string();
    assert!(
        s.contains("Key not found: key1"),
        "en Display should contain 'Key not found: key1': got '{s}'"
    );
}

#[test]
fn test_display_uses_default_locale_zh() {
    crate::i18n::set_default_locale("zh-CN");
    let err = OxCacheError::NotFound("key1".to_string());
    let s = err.to_string();
    assert!(
        s.contains("键未找到：key1"),
        "zh Display should contain '键未找到：key1': got '{s}'"
    );
    // Restore default
    crate::i18n::set_default_locale("en");
}

#[test]
fn test_display_config_error_uses_default_locale_zh() {
    #[cfg(feature = "redis")]
    {
        crate::i18n::set_default_locale("zh-CN");
        let err = OxCacheConfigError::MissingField("host".to_string());
        let s = err.to_string();
        assert!(
            s.contains("缺少必需字段：host"),
            "zh ConfigError Display should contain '缺少必需字段：host': got '{s}'"
        );
        // Restore default
        crate::i18n::set_default_locale("en");
    }
}

#[test]
fn test_set_and_get_default_locale() {
    assert_eq!(crate::i18n::get_default_locale(), "en");
    crate::i18n::set_default_locale("zh-CN");
    assert_eq!(crate::i18n::get_default_locale(), "zh-CN");
    crate::i18n::set_default_locale("en");
    assert_eq!(crate::i18n::get_default_locale(), "en");
}
