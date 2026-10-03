// Copyright (c) 2025-2026 Kirky.X🌠
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
/// Detection chain (priority order): `OXCACHE_LANG` (project override),
/// `LC_ALL`, `LC_MESSAGES`, `LANG`. Returns a normalized locale string.
/// Falls back to `"en"` when:
/// - No environment variable is set
/// - The locale is `C` or `POSIX`
/// - The language is not in the [supported list](messages::is_supported)
///
/// This function is called once during global initialization.
pub fn detect_system_locale() -> String {
    let raw = std::env::var("OXCACHE_LANG")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LC_ALL").ok().filter(|v| !v.is_empty()))
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
/// `OxCacheConfigError`, and
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
            I18nError::InvalidLocale { input, reason } => {
                vec![("input", input.clone()), ("reason", reason.clone())]
            }
            I18nError::InvalidNumber { input, reason } => {
                vec![("input", input.clone()), ("reason", reason.clone())]
            }
            I18nError::DateError(d) => vec![("detail", d.clone())],
            I18nError::FormatError(d) => vec![("detail", d.clone())],
        };
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        messages::lookup(locale, self.message_id(), &borrowed)
            .unwrap_or_else(|| self.message_id().to_string())
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
    pub fn format_message(
        &self,
        message_id: &str,
        params: &[(&str, &str)],
    ) -> Result<String, I18nError> {
        Ok(messages::lookup(&self.locale_tag, message_id, params)
            .unwrap_or_else(|| message_id.to_string()))
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
