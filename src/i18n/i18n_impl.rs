// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! I18n impl - functions and impl blocks extracted from mod.rs

use super::*;
use std::cmp::Ordering;
use std::str::FromStr;

use icu::collator::Collator;
use icu::collator::options::CollatorOptions;
use icu::datetime::DateTimeFormatter;
use icu::datetime::fieldsets::YMD;
use icu::datetime::input::{Date, DateTime, Time};
use icu::decimal::DecimalFormatter;
use icu::decimal::input::Decimal;
use icu::decimal::options::DecimalFormatterOptions;
use icu::locale::Locale;
use icu::plurals::{PluralCategory, PluralRules, PluralRulesOptions};
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
    /// Uses integer formatting to avoid precision loss that would occur
    /// from converting `u64` to `f64` (which loses precision above 2^53).
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] if the count cannot be formatted.
    pub fn format_cache_key(&self, namespace: &str, count: u64) -> Result<String, I18nError> {
        // Format u64 directly as a decimal string to avoid u64→f64 precision loss
        let repr = format!("{count}");
        let decimal = Decimal::from_str(&repr).map_err(|e| I18nError::InvalidNumber {
            input: repr,
            reason: e.to_string(),
        })?;
        let formatted = self.decimal_formatter.format(&decimal);
        let formatted_str = formatted.write_to_string().into_owned();
        Ok(format!("{namespace}:{formatted_str}"))
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
