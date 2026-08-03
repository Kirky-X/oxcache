// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Message catalog for locale-aware error and status messages.
//!
//! Provides a compile-time message catalog mapping `(locale, message_id)` pairs
//! to format templates. Templates use `{name}` placeholders substituted at
//! runtime by [`format_template`].
//!
//! # Supported locales
//!
//! - `en` / `en-US` / `en-*` → English (fallback for any English variant)
//! - `zh` / `zh-CN` / `zh-*` → Simplified Chinese
//!
//! Unknown locales fall back to English.
//!
//! # Adding a new message
//!
//! 1. Add a `MSG_ID_*` constant in the **Message IDs** section.
//! 2. Add the English template in `catalog_en()`.
//! 3. Add the Chinese template in `catalog_zh()`.
//! 4. Use `formatter.format_message(MSG_ID_*, &[("key", "value")])` to render.

// ============================================================================
// Message ID constants
// ============================================================================

// -- OxCacheError messages --
pub const MSG_ERR_SERIALIZATION: &str = "error.serialization";
pub const MSG_ERR_OPERATION: &str = "error.operation";
pub const MSG_ERR_CONNECTION: &str = "error.connection";
pub const MSG_ERR_NOT_FOUND: &str = "error.not_found";
pub const MSG_ERR_DEGRADED: &str = "error.degraded";
pub const MSG_ERR_L1: &str = "error.l1";
pub const MSG_ERR_L2: &str = "error.l2";
pub const MSG_ERR_NOT_SUPPORTED: &str = "error.not_supported";
pub const MSG_ERR_WAL: &str = "error.wal";
pub const MSG_ERR_DATABASE: &str = "error.database";
pub const MSG_ERR_REDIS: &str = "error.redis";
pub const MSG_ERR_IO: &str = "error.io";
pub const MSG_ERR_BACKEND: &str = "error.backend";
pub const MSG_ERR_TIMEOUT: &str = "error.timeout";
pub const MSG_ERR_SHUTDOWN: &str = "error.shutdown";
pub const MSG_ERR_KEY_TOO_LONG: &str = "error.key_too_long";
pub const MSG_ERR_VALUE_TOO_LARGE: &str = "error.value_too_large";
pub const MSG_ERR_BUFFER_FULL: &str = "error.buffer_full";
pub const MSG_ERR_INVALID_INPUT: &str = "error.invalid_input";
pub const MSG_ERR_INVALID_KEY: &str = "error.invalid_key";
pub const MSG_ERR_LOCK: &str = "error.lock";
pub const MSG_ERR_SERVICE_NOT_FOUND: &str = "error.service_not_found";
pub const MSG_ERR_INTERNAL: &str = "error.internal";

// -- OxCacheConfigError messages --
pub const MSG_CFG_MISSING_FIELD: &str = "config.missing_field";
pub const MSG_CFG_INVALID_VALUE: &str = "config.invalid_value";
pub const MSG_CFG_UNSUPPORTED_BACKEND: &str = "config.unsupported_backend";
pub const MSG_CFG_CONNECTION_FAILED: &str = "config.connection_failed";

// -- I18nError messages --
pub const MSG_I18N_INVALID_LOCALE: &str = "i18n.invalid_locale";
pub const MSG_I18N_INVALID_NUMBER: &str = "i18n.invalid_number";
pub const MSG_I18N_DATE_ERROR: &str = "i18n.date_error";
pub const MSG_I18N_FORMAT_ERROR: &str = "i18n.format_error";

// ============================================================================
// Template substitution
// ============================================================================

/// Replace `{name}` placeholders in `template` with values from `params`.
///
/// Unmatched placeholders are left as-is (e.g. `{unknown}` stays in the output).
///
/// # Example
///
/// ```ignore
/// let result = format_template("Hello {name}, age {age}", &[("name", "Alice"), ("age", "30")]);
/// assert_eq!(result, "Hello Alice, age 30");
/// ```
pub(crate) fn format_template(template: &str, params: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        let placeholder = format!("{{{key}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

// ============================================================================
// Catalog lookup
// ============================================================================

/// Check whether a language code has a dedicated message catalog.
///
/// Currently supported: `"en"`, `"zh"`.
pub fn is_supported(lang: &str) -> bool {
    matches!(lang, "en" | "zh")
}

/// Look up a message template by `locale` and `message_id`.
///
/// Locale matching strategy:
/// 1. Language prefix match (e.g. `zh` matches `zh-CN`)
/// 2. Fallback to English (`en`)
///
/// Returns `None` if `message_id` is not found in any locale.
pub(crate) fn lookup(locale: &str, message_id: &str) -> Option<&'static str> {
    let lang = locale.split('-').next().unwrap_or(locale);
    match lang {
        "zh" => catalog_zh(message_id).or_else(|| catalog_en(message_id)),
        _ => catalog_en(message_id),
    }
}

// ============================================================================
// English catalog
// ============================================================================

fn catalog_en(id: &str) -> Option<&'static str> {
    match id {
        // OxCacheError
        MSG_ERR_SERIALIZATION => Some("Serialization error: {detail}. Please check the data format and ensure the serializer is compatible."),
        MSG_ERR_OPERATION => Some("Operation failed: {detail}. Please retry or check your request."),
        MSG_ERR_CONNECTION => Some("Connection error: {detail}. Please check network connectivity and server availability."),
        MSG_ERR_NOT_FOUND => Some("Key not found: {detail}. The requested key does not exist in the cache."),
        MSG_ERR_DEGRADED => Some("Cache degraded: {detail}. The cache is operating in degraded mode with limited functionality."),
        MSG_ERR_L1 => Some("L1 cache operation failed: {detail}. This may indicate memory pressure or configuration issues."),
        MSG_ERR_L2 => Some("L2 cache operation failed: {detail}. Please check Redis connection and server status."),
        MSG_ERR_NOT_SUPPORTED => Some("Operation not supported: {detail}. This feature may not be available for the current cache type."),
        MSG_ERR_WAL => Some("WAL (Write-Ahead Log) operation failed: {detail}. Check disk space and file permissions."),
        MSG_ERR_DATABASE => Some("Database error: {detail}. Please check database connectivity and query syntax."),
        MSG_ERR_REDIS => Some("Redis connection failed: {detail}."),
        MSG_ERR_IO => Some("I/O error: {detail}. Check file permissions and disk space."),
        MSG_ERR_BACKEND => Some("Backend error: {detail}. This may be a transient issue, please retry."),
        MSG_ERR_TIMEOUT => Some("Operation timed out: {detail}. Consider increasing the timeout value or check system performance."),
        MSG_ERR_SHUTDOWN => Some("Shutdown error: {detail}. Some resources may not have been properly released."),
        MSG_ERR_KEY_TOO_LONG => Some("Key too long: {actual}. Maximum key length is {max} bytes."),
        MSG_ERR_VALUE_TOO_LARGE => Some("Value too large: {actual}. Maximum value size is {max} bytes."),
        MSG_ERR_BUFFER_FULL => Some("Buffer full: {detail}. The batch write buffer has reached capacity. Please retry later or increase buffer size."),
        MSG_ERR_INVALID_INPUT => Some("Invalid input: {detail}. The provided input does not meet the required format or constraints."),
        MSG_ERR_INVALID_KEY => Some("Invalid key: {detail}. The provided key does not meet the required format or contains forbidden characters."),
        MSG_ERR_LOCK => Some("Lock error: {detail}. The lock may have been poisoned by a previous panic."),
        MSG_ERR_SERVICE_NOT_FOUND => Some("Service not found: {detail}. The requested service configuration does not exist in the UnifiedConfig."),
        MSG_ERR_INTERNAL => Some("Internal error: {detail}."),

        // OxCacheConfigError
        MSG_CFG_MISSING_FIELD => Some("Missing required field: {field}."),
        MSG_CFG_INVALID_VALUE => Some("Invalid value for field '{field}': {reason}."),
        MSG_CFG_UNSUPPORTED_BACKEND => Some("Unsupported backend combination: {detail}."),
        MSG_CFG_CONNECTION_FAILED => Some("Connection failed during initialization: {detail}."),

        // I18nError
        MSG_I18N_INVALID_LOCALE => Some("invalid locale '{input}': {reason}."),
        MSG_I18N_INVALID_NUMBER => Some("invalid number '{input}': {reason}."),
        MSG_I18N_DATE_ERROR => Some("date error: {detail}."),
        MSG_I18N_FORMAT_ERROR => Some("formatting error: {detail}."),

        _ => None,
    }
}

// ============================================================================
// Simplified Chinese catalog
// ============================================================================

fn catalog_zh(id: &str) -> Option<&'static str> {
    match id {
        // OxCacheError
        MSG_ERR_SERIALIZATION => Some("序列化错误：{detail}。请检查数据格式并确保序列化器兼容。"),
        MSG_ERR_OPERATION => Some("操作失败：{detail}。请重试或检查请求。"),
        MSG_ERR_CONNECTION => Some("连接错误：{detail}。请检查网络连接和服务器可用性。"),
        MSG_ERR_NOT_FOUND => Some("键未找到：{detail}。请求的键在缓存中不存在。"),
        MSG_ERR_DEGRADED => Some("缓存降级：{detail}。缓存正在以受限功能降级模式运行。"),
        MSG_ERR_L1 => Some("L1 缓存操作失败：{detail}。可能存在内存压力或配置问题。"),
        MSG_ERR_L2 => Some("L2 缓存操作失败：{detail}。请检查 Redis 连接和服务器状态。"),
        MSG_ERR_NOT_SUPPORTED => Some("操作不支持：{detail}。此功能在当前缓存类型下可能不可用。"),
        MSG_ERR_WAL => Some("WAL（预写日志）操作失败：{detail}。请检查磁盘空间和文件权限。"),
        MSG_ERR_DATABASE => Some("数据库错误：{detail}。请检查数据库连接和查询语法。"),
        MSG_ERR_REDIS => Some("Redis 连接失败：{detail}。"),
        MSG_ERR_IO => Some("I/O 错误：{detail}。请检查文件权限和磁盘空间。"),
        MSG_ERR_BACKEND => Some("后端错误：{detail}。可能是暂时性问题，请重试。"),
        MSG_ERR_TIMEOUT => Some("操作超时：{detail}。请考虑增加超时值或检查系统性能。"),
        MSG_ERR_SHUTDOWN => Some("关闭错误：{detail}。部分资源可能未正确释放。"),
        MSG_ERR_KEY_TOO_LONG => Some("键过长：{actual}。最大键长度为 {max} 字节。"),
        MSG_ERR_VALUE_TOO_LARGE => Some("值过大：{actual}。最大值大小为 {max} 字节。"),
        MSG_ERR_BUFFER_FULL => Some("缓冲区已满：{detail}。批量写入缓冲区已达容量上限。请稍后重试或增大缓冲区。"),
        MSG_ERR_INVALID_INPUT => Some("无效输入：{detail}。提供的输入不符合所需格式或约束。"),
        MSG_ERR_INVALID_KEY => Some("无效键：{detail}。提供的键不符合所需格式或包含禁止字符。"),
        MSG_ERR_LOCK => Some("锁错误：{detail}。锁可能因之前的 panic 而被毒害。"),
        MSG_ERR_SERVICE_NOT_FOUND => Some("服务未找到：{detail}。请求的服务配置在 UnifiedConfig 中不存在。"),
        MSG_ERR_INTERNAL => Some("内部错误：{detail}。"),

        // OxCacheConfigError
        MSG_CFG_MISSING_FIELD => Some("缺少必需字段：{field}。"),
        MSG_CFG_INVALID_VALUE => Some("字段 '{field}' 的值无效：{reason}。"),
        MSG_CFG_UNSUPPORTED_BACKEND => Some("不支持的后端组合：{detail}。"),
        MSG_CFG_CONNECTION_FAILED => Some("初始化时连接失败：{detail}。"),

        // I18nError
        MSG_I18N_INVALID_LOCALE => Some("无效的区域设置 '{input}'：{reason}。"),
        MSG_I18N_INVALID_NUMBER => Some("无效的数字 '{input}'：{reason}。"),
        MSG_I18N_DATE_ERROR => Some("日期错误：{detail}。"),
        MSG_I18N_FORMAT_ERROR => Some("格式化错误：{detail}。"),

        _ => None,
    }
}
