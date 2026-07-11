// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

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

use std::cmp::Ordering;
use std::str::FromStr;

use icu::collator::options::CollatorOptions;
use icu::collator::{Collator, CollatorBorrowed};
use icu::datetime::fieldsets::YMD;
use icu::datetime::input::{Date, DateTime, Time};
use icu::datetime::DateTimeFormatter;
use icu::decimal::input::Decimal;
use icu::decimal::options::DecimalFormatterOptions;
use icu::decimal::DecimalFormatter;
use icu::locale::Locale;
use icu::plurals::{PluralCategory, PluralRules, PluralRulesOptions};
use thiserror::Error;
use writeable::Writeable;

/// Map a [`PluralCategory`] to its capitalized CLDR name (e.g. `"One"`, `"Other"`).
fn plural_category_name(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "Zero",
        PluralCategory::One => "One",
        PluralCategory::Two => "Two",
        PluralCategory::Few => "Few",
        PluralCategory::Many => "Many",
        PluralCategory::Other => "Other",
    }
}

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

impl CacheI18nFormatter {
    /// Create a new formatter for the given BCP-47 locale tag.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidLocale`] if the tag cannot be parsed,
    /// or [`I18nError::FormatError`] if ICU4X lacks compiled data for it.
    pub fn new(locale: &str) -> Result<Self, I18nError> {
        let parsed = Locale::from_str(locale).map_err(|e| I18nError::InvalidLocale {
            input: locale.to_string(),
            reason: e.to_string(),
        })?;

        let decimal_formatter = DecimalFormatter::try_new(parsed.clone().into(), DecimalFormatterOptions::default())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;

        let plural_rules = PluralRules::try_new(parsed.clone().into(), PluralRulesOptions::default())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;

        let collator = Collator::try_new(parsed.clone().into(), CollatorOptions::default())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;

        Ok(Self {
            locale: parsed,
            decimal_formatter,
            plural_rules,
            collator,
        })
    }

    /// Format a floating-point number with locale-sensitive grouping
    /// and decimal separators.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] for non-finite values or
    /// if the value cannot be parsed into a fixed decimal.
    pub fn format_number(&self, value: f64) -> Result<String, I18nError> {
        if !value.is_finite() {
            return Err(I18nError::InvalidNumber {
                input: value.to_string(),
                reason: "value is not finite (NaN or Infinity)".into(),
            });
        }
        let repr = format!("{value}");
        let decimal = Decimal::from_str(&repr).map_err(|e| I18nError::InvalidNumber {
            input: repr,
            reason: e.to_string(),
        })?;
        let formatted = self.decimal_formatter.format(&decimal);
        Ok(formatted.write_to_string().into_owned())
    }

    /// Build a locale-aware cache key by combining a namespace with a
    /// locale-formatted count (e.g. `"user:1,234"`).
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] if the count cannot be formatted.
    pub fn format_cache_key(&self, namespace: &str, count: u64) -> Result<String, I18nError> {
        let formatted_count = self.format_number(count as f64)?;
        Ok(format!("{namespace}:{formatted_count}"))
    }

    /// Format an ISO calendar date (year / month / day) as a cache
    /// expiry timestamp using a medium-length locale-specific pattern.
    ///
    /// # Errors
    /// Returns [`I18nError::DateError`] if any component is out of range,
    /// or [`I18nError::FormatError`] if the formatter cannot be constructed.
    pub fn format_expiry(&self, year: i32, month: u8, day: u8) -> Result<String, I18nError> {
        let date = Date::try_new_iso(year, month, day).map_err(|e| I18nError::DateError(e.to_string()))?;
        let time = Time::try_new(0, 0, 0, 0).map_err(|e| I18nError::DateError(e.to_string()))?;
        let datetime = DateTime { date, time };

        let dtf = DateTimeFormatter::try_new(self.locale.clone().into(), YMD::medium())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;
        let formatted = dtf.format(&datetime);
        Ok(formatted.write_to_string().into_owned())
    }

    /// Return the plural category name for `count` in the formatter's locale
    /// (e.g. `"One"` for English count=1, `"Other"` for count=2).
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn format_count(&self, count: u64) -> Result<String, I18nError> {
        Ok(plural_category_name(self.plural_rules.category_for(count)).to_string())
    }

    /// Compare two cache keys using locale-sensitive collation rules.
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn compare_keys(&self, a: &str, b: &str) -> Result<Ordering, I18nError> {
        Ok(self.collator.compare(a, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
