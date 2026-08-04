// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! ICU4X-backed internationalization formatting for cache operations.
//!
//! Provides locale-aware number formatting, date formatting, plural rules,
//! string collation, and localized message formatting via the `icu` crate
//! (ICU4X 2.x). Useful for generating locale-sensitive cache keys, formatting
//! cache statistics (e.g. "1 item" vs "2 items"), displaying expiry times,
//! sorting cache entries by locale-specific collation rules, and rendering
//! error messages in the user's preferred locale.
//!
//! This module is always enabled (included in the `minimal` feature tier).
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::i18n::CacheI18nFormatter;
//!
//! let fmt = CacheI18nFormatter::new("en-US")?;
//! let key = fmt.format_cache_key("user", 1234)?;
//! let expiry = fmt.format_expiry(2026, 7, 11)?;
//! let plural = fmt.format_count(1)?; // "One"
//! let msg = fmt.format_message("error.not_found", &[("key", "user:42")])?;
//! ```

use icu::collator::CollatorBorrowed;
use icu::decimal::DecimalFormatter;
use icu::locale::Locale;
use icu::plurals::PluralRules;
use once_cell::sync::Lazy;
use std::fmt;
use std::sync::RwLock;

mod i18n_impl;
pub mod messages;

// ============================================================================
// Global default locale
// ============================================================================

static DEFAULT_LOCALE: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(detect_system_locale()));

/// Detect the system locale from environment variables.
///
/// Reads `LC_ALL`, `LC_MESSAGES`, and `LANG` (in priority order) and returns
/// a normalized locale string. Falls back to `"en"` when:
/// - No environment variable is set
/// - The locale is `C` or `POSIX`
/// - The language is not in the [supported list](messages::is_supported)
///
/// This function is called once during global initialization.
pub fn detect_system_locale() -> String {
    let raw = std::env::var("LC_ALL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LC_MESSAGES").ok().filter(|v| !v.is_empty()))
        .or_else(|| std::env::var("LANG").ok().filter(|v| !v.is_empty()));

    let Some(raw) = raw else {
        return String::from("en");
    };

    let locale = parse_locale_tag(&raw);

    // C / POSIX → English
    if locale == "C" || locale == "POSIX" {
        return String::from("en");
    }

    // Check if the detected language is supported
    let lang = locale.split('-').next().unwrap_or(&locale);
    if messages::is_supported(lang) {
        locale
    } else {
        String::from("en")
    }
}

/// Parse a raw locale environment variable value into a normalized tag.
///
/// Strips encoding (`UTF-8`), modifier (`@collation`), and normalizes
/// separators (`_` → `-`).
///
/// # Examples
///
/// - `"en_US.UTF-8"` → `"en-US"`
/// - `"zh_CN.UTF-8"` → `"zh-CN"`
/// - `"C"` → `"C"`
/// - `"POSIX"` → `"POSIX"`
fn parse_locale_tag(raw: &str) -> String {
    // Strip encoding (e.g. ".UTF-8")
    let s = raw.split('.').next().unwrap_or(raw);
    // Strip modifier (e.g. "@collation")
    let s = s.split('@').next().unwrap_or(s);
    // Normalize separator
    s.replace('_', "-")
}

/// Set the global default locale for all `Display` implementations of error types.
///
/// This affects how [`OxCacheError`](crate::error::OxCacheError),
/// [`OxCacheConfigError`](crate::error::OxCacheConfigError), and
/// [`I18nError`] render their messages via `fmt::Display`.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::i18n;
///
/// i18n::set_default_locale("zh-CN");
/// let err = OxCacheError::NotFound("key".to_string());
/// assert!(err.to_string().contains("键未找到"));
/// ```
pub fn set_default_locale(locale: &str) {
    if let Ok(mut guard) = DEFAULT_LOCALE.write() {
        *guard = locale.to_string();
    }
}

/// Get the current global default locale.
///
/// On first call (before any explicit [`set_default_locale`]), this returns
/// the auto-detected system locale (or `"en"` if detection fails).
pub fn get_default_locale() -> String {
    DEFAULT_LOCALE
        .read()
        .map(|g| g.clone())
        .unwrap_or_else(|_| String::from("en"))
}

/// Errors returned by [`CacheI18nFormatter`] operations.
#[derive(Debug)]
pub enum I18nError {
    /// BCP-47 locale string could not be parsed.
    InvalidLocale { input: String, reason: String },
    /// Number value could not be formatted (e.g. NaN, Infinity, or parse failure).
    InvalidNumber { input: String, reason: String },
    /// Date component out of range or otherwise invalid.
    DateError(String),
    /// Underlying ICU4X data or formatting failure.
    FormatError(String),
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let locale = get_default_locale();
        f.write_str(&self.localized_message(&locale))
    }
}

impl std::error::Error for I18nError {}

impl I18nError {
    /// Return the i18n message ID for this error variant.
    pub fn message_id(&self) -> &'static str {
        match self {
            I18nError::InvalidLocale { .. } => messages::MSG_I18N_INVALID_LOCALE,
            I18nError::InvalidNumber { .. } => messages::MSG_I18N_INVALID_NUMBER,
            I18nError::DateError(_) => messages::MSG_I18N_DATE_ERROR,
            I18nError::FormatError(_) => messages::MSG_I18N_FORMAT_ERROR,
        }
    }

    /// Render a locale-aware error message.
    pub fn localized_message(&self, locale: &str) -> String {
        let params: Vec<(&str, String)> = match self {
            I18nError::InvalidLocale { input, reason } => vec![("input", input.clone()), ("reason", reason.clone())],
            I18nError::InvalidNumber { input, reason } => vec![("input", input.clone()), ("reason", reason.clone())],
            I18nError::DateError(d) => vec![("detail", d.clone())],
            I18nError::FormatError(d) => vec![("detail", d.clone())],
        };
        let template = messages::lookup(locale, self.message_id()).unwrap_or(self.message_id());
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        messages::format_template(template, &borrowed)
    }
}

/// Locale-aware formatter backed by ICU4X compiled data.
///
/// Construct with [`CacheI18nFormatter::new`] using a BCP-47 locale tag
/// (e.g. `"en-US"`, `"zh-CN"`). All formatters are created eagerly so
/// that repeated formatting calls are allocation-light.
pub struct CacheI18nFormatter {
    locale: Locale,
    locale_tag: String,
    decimal_formatter: DecimalFormatter,
    plural_rules: PluralRules,
    collator: CollatorBorrowed<'static>,
}

impl CacheI18nFormatter {
    /// Return a reference to the formatter's BCP-47 locale.
    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    /// Return the original BCP-47 locale tag string (e.g. `"en-US"`, `"zh-CN"`).
    pub fn locale_tag(&self) -> &str {
        &self.locale_tag
    }

    /// Format a message from the catalog using the formatter's locale.
    ///
    /// Looks up `message_id` in the message catalog for the current locale,
    /// then substitutes `{key}` placeholders with values from `params`.
    ///
    /// Falls back to English if the locale is not supported, and returns the
    /// raw `message_id` if the message is not found in any locale.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let fmt = CacheI18nFormatter::new("zh-CN")?;
    /// let msg = fmt.format_message("error.not_found", &[("detail", "user:42")])?;
    /// assert_eq!(msg, "键未找到：user:42。请求的键在缓存中不存在。");
    /// ```
    pub fn format_message(&self, message_id: &str, params: &[(&str, &str)]) -> Result<String, I18nError> {
        let template = messages::lookup(&self.locale_tag, message_id).unwrap_or(message_id);
        Ok(messages::format_template(template, params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_locale_parsing_en() {
        let fmt = CacheI18nFormatter::new("en-US");
        assert!(fmt.is_ok(), "en-US should parse successfully");
    }

    #[test]
    fn test_locale_parsing_zh() {
        let fmt = CacheI18nFormatter::new("zh-CN");
        assert!(fmt.is_ok(), "zh-CN should parse successfully");
    }

    #[test]
    fn test_invalid_locale() {
        let result = CacheI18nFormatter::new("not-a-valid-locale!!!");
        assert!(result.is_err(), "invalid locale should return error");
        match result.err().unwrap() {
            I18nError::InvalidLocale { input, .. } => assert_eq!(input, "not-a-valid-locale!!!"),
            other => panic!("expected InvalidLocale, got {other:?}"),
        }
    }

    #[test]
    fn test_format_count() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        assert_eq!(
            fmt.format_count(1).expect("plural 1"),
            "One",
            "en: count=1 should be One"
        );
        assert_eq!(
            fmt.format_count(2).expect("plural 2"),
            "Other",
            "en: count=2 should be Other"
        );
    }

    #[test]
    fn test_format_number_en() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_number(1_234_567.89_f64).expect("format number");
        // en-US: thousands separator is comma, decimal separator is period
        assert!(
            result.contains(','),
            "en-US number should contain thousands separator: got '{result}'"
        );
        assert!(
            result.contains('.'),
            "en-US number should contain decimal point: got '{result}'"
        );
    }

    #[test]
    fn test_format_number_not_finite() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        assert!(fmt.format_number(f64::NAN).is_err());
        assert!(fmt.format_number(f64::INFINITY).is_err());
    }

    #[test]
    fn test_format_cache_key() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let key = fmt.format_cache_key("user", 1234).expect("cache key");
        assert!(
            key.starts_with("user:"),
            "cache key should start with namespace: got '{key}'"
        );
        assert!(key.contains('1'), "cache key should contain the count: got '{key}'");
    }

    #[test]
    fn test_compare_keys() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        assert_eq!(
            fmt.compare_keys("apple", "banana").expect("compare"),
            Ordering::Less,
            "apple < banana"
        );
        assert_eq!(
            fmt.compare_keys("banana", "apple").expect("compare"),
            Ordering::Greater,
            "banana > apple"
        );
        assert_eq!(
            fmt.compare_keys("apple", "apple").expect("compare"),
            Ordering::Equal,
            "apple == apple"
        );
    }

    #[test]
    fn test_format_expiry() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_expiry(2026, 7, 11).expect("format expiry");
        assert!(result.contains("2026"), "expiry should contain year: got '{result}'");
        assert!(!result.is_empty(), "expiry should be non-empty: got '{result}'");
    }

    // ========================================================================
    // Message catalog tests
    // ========================================================================

    #[test]
    fn test_format_message_en_not_found() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_NOT_FOUND, &[("detail", "user:42")])
            .expect("format message");
        assert!(
            msg.contains("Key not found: user:42"),
            "en message should contain 'Key not found: user:42': got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_zh_not_found() {
        let fmt = CacheI18nFormatter::new("zh-CN").expect("zh-CN locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_NOT_FOUND, &[("detail", "user:42")])
            .expect("format message");
        assert!(
            msg.contains("键未找到：user:42"),
            "zh message should contain '键未找到：user:42': got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_en_key_too_long() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_KEY_TOO_LONG, &[("actual", "600"), ("max", "512")])
            .expect("format message");
        assert!(
            msg.contains("600") && msg.contains("512"),
            "en message should contain actual and max: got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_zh_key_too_long() {
        let fmt = CacheI18nFormatter::new("zh-CN").expect("zh-CN locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_KEY_TOO_LONG, &[("actual", "600"), ("max", "512")])
            .expect("format message");
        assert!(
            msg.contains("键过长") && msg.contains("600") && msg.contains("512"),
            "zh message should contain '键过长' with values: got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_unknown_id_returns_id() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        let msg = fmt.format_message("unknown.message.id", &[]).expect("format message");
        assert_eq!(msg, "unknown.message.id", "unknown ID should return raw ID");
    }

    #[test]
    fn test_format_message_unsupported_locale_falls_back_to_en() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_CONNECTION, &[("detail", "timeout")])
            .expect("format message");
        assert!(
            msg.contains("Connection error: timeout"),
            "unsupported locale should fall back to English: got '{msg}'"
        );
    }

    #[test]
    fn test_locale_getter() {
        let fmt = CacheI18nFormatter::new("zh-CN").expect("zh-CN locale");
        assert!(
            fmt.locale_tag().starts_with("zh"),
            "locale_tag should start with 'zh': got '{}'",
            fmt.locale_tag()
        );
    }

    #[test]
    fn test_template_substitution() {
        let result = messages::format_template("Hello {name}, age {age}", &[("name", "Alice"), ("age", "30")]);
        assert_eq!(result, "Hello Alice, age 30");
    }

    #[test]
    fn test_template_unmatched_placeholder_preserved() {
        let result = messages::format_template("Hello {name}, {unknown}", &[("name", "Alice")]);
        assert_eq!(result, "Hello Alice, {unknown}");
    }

    #[test]
    fn test_i18n_error_message_id() {
        let err = I18nError::InvalidLocale {
            input: "bad".to_string(),
            reason: "parse failed".to_string(),
        };
        assert_eq!(err.message_id(), messages::MSG_I18N_INVALID_LOCALE);
    }

    #[test]
    fn test_i18n_error_localized_message_en() {
        let err = I18nError::DateError("month out of range".to_string());
        let msg = err.localized_message("en");
        assert!(
            msg.contains("date error: month out of range"),
            "en I18nError message: got '{msg}'"
        );
    }

    #[test]
    fn test_i18n_error_localized_message_zh() {
        let err = I18nError::DateError("月份超出范围".to_string());
        let msg = err.localized_message("zh-CN");
        assert!(
            msg.contains("日期错误：月份超出范围"),
            "zh I18nError message: got '{msg}'"
        );
    }

    // ========================================================================
    // Global default locale tests
    // ========================================================================

    #[test]
    fn test_i18n_error_display_en() {
        set_default_locale("en");
        let err = I18nError::DateError("month out of range".to_string());
        let s = err.to_string();
        assert!(
            s.contains("date error: month out of range"),
            "en I18nError Display: got '{s}'"
        );
    }

    #[test]
    fn test_i18n_error_display_zh() {
        set_default_locale("zh-CN");
        let err = I18nError::DateError("月份超出范围".to_string());
        let s = err.to_string();
        assert!(s.contains("日期错误：月份超出范围"), "zh I18nError Display: got '{s}'");
        set_default_locale("en");
    }

    #[test]
    fn test_set_get_default_locale() {
        set_default_locale("en");
        assert_eq!(get_default_locale(), "en");
        set_default_locale("zh-CN");
        assert_eq!(get_default_locale(), "zh-CN");
        set_default_locale("en");
    }

    // ========================================================================
    // System locale detection tests
    // ========================================================================

    #[test]
    fn test_parse_locale_tag_en() {
        assert_eq!(super::parse_locale_tag("en_US.UTF-8"), "en-US");
    }

    #[test]
    fn test_parse_locale_tag_zh() {
        assert_eq!(super::parse_locale_tag("zh_CN.UTF-8"), "zh-CN");
    }

    #[test]
    fn test_parse_locale_tag_c() {
        assert_eq!(super::parse_locale_tag("C"), "C");
    }

    #[test]
    fn test_parse_locale_tag_posix() {
        assert_eq!(super::parse_locale_tag("POSIX"), "POSIX");
    }

    #[test]
    fn test_parse_locale_tag_with_modifier() {
        assert_eq!(super::parse_locale_tag("en_US.UTF-8@collation"), "en-US");
    }

    #[test]
    fn test_parse_locale_tag_simple() {
        assert_eq!(super::parse_locale_tag("fr"), "fr");
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_with_lang_zh() {
        // Save original
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();

        // Clear higher-priority vars
        // SAFETY: test-only; serialised by `--test-threads=1` or env mutex in practice.
        unsafe {
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "zh_CN.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "zh-CN", "should detect zh-CN from LANG");

        // Restore
        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_with_lang_en() {
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();

        unsafe {
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "en_US.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "en-US", "should detect en-US from LANG");

        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_c_fallback_to_en() {
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();

        unsafe {
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "C");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "en", "C locale should fall back to en");

        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_unsupported_fallback_to_en() {
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();

        unsafe {
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "ja_JP.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "en", "unsupported locale (ja) should fall back to en");

        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_lc_all_priority() {
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();

        // LC_ALL should take priority over LC_MESSAGES and LANG
        unsafe {
            std::env::set_var("LC_ALL", "zh_CN.UTF-8");
            std::env::set_var("LC_MESSAGES", "en_US.UTF-8");
            std::env::set_var("LANG", "fr_FR.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "zh-CN", "LC_ALL should take priority: got '{locale}'");

        // Restore
        unsafe {
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            } else {
                std::env::remove_var("LC_ALL");
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            } else {
                std::env::remove_var("LC_MESSAGES");
            }
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            } else {
                std::env::remove_var("LANG");
            }
        }
    }

    #[test]
    fn test_is_supported() {
        assert!(messages::is_supported("en"));
        assert!(messages::is_supported("zh"));
        assert!(!messages::is_supported("fr"));
        assert!(!messages::is_supported("ja"));
        assert!(!messages::is_supported("de"));
    }
}
