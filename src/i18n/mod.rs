// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! ICU4X-backed internationalization formatting for cache operations.
//!
//! Provides locale-aware number formatting, date formatting, plural rules,
//! and string collation via the `icu` crate (ICU4X 2.x). Useful for
//! generating locale-sensitive cache keys, formatting cache statistics
//! (e.g. "1 item" vs "2 items"), displaying expiry times, and sorting
//! cache entries by locale-specific collation rules.
//!
//! Enable with the `i18n` cargo feature:
//! ```toml
//! [dependencies]
//! oxcache = { version = "...", features = ["i18n"] }
//! ```
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
//! ```

use icu::collator::CollatorBorrowed;
use icu::decimal::DecimalFormatter;
use icu::locale::Locale;
use icu::plurals::PluralRules;
use thiserror::Error;

mod i18n_impl;

/// Errors returned by [`CacheI18nFormatter`] operations.
#[derive(Debug, Error)]
pub enum I18nError {
    /// BCP-47 locale string could not be parsed.
    #[error("invalid locale '{input}': {reason}")]
    InvalidLocale { input: String, reason: String },
    /// Number value could not be formatted (e.g. NaN, Infinity, or parse failure).
    #[error("invalid number '{input}': {reason}")]
    InvalidNumber { input: String, reason: String },
    /// Date component out of range or otherwise invalid.
    #[error("date error: {0}")]
    DateError(String),
    /// Underlying ICU4X data or formatting failure.
    #[error("formatting error: {0}")]
    FormatError(String),
}

/// Locale-aware formatter backed by ICU4X compiled data.
///
/// Construct with [`CacheI18nFormatter::new`] using a BCP-47 locale tag
/// (e.g. `"en-US"`, `"zh-CN"`). All formatters are created eagerly so
/// that repeated formatting calls are allocation-light.
pub struct CacheI18nFormatter {
    locale: Locale,
    decimal_formatter: DecimalFormatter,
    plural_rules: PluralRules,
    collator: CollatorBorrowed<'static>,
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
}
