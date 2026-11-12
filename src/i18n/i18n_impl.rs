// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! I18n impl — `CacheI18nFormatter` delegating to the trait-kit
//! `I18nFormatter` (ICU4X through the `trait-kit` `i18n` feature).

use std::cmp::Ordering;

// PluralCategory 取自本仓已声明的 ICU4X 元 crate（`icu::plurals`，compiled_data）——
// 不再依赖 Cargo 传递可见的 `icu_plurals` 直路径（resolver 2018+ 下不可直引用）。
// trait-kit 侧将来重导出该类型后，可改为 `trait_kit::i18n::PluralCategory` 并删除本仓 icu 依赖。
use icu::plurals::PluralCategory;
use trait_kit::i18n::I18nFormatter;

use super::I18nError;

/// Map a `trait-kit` i18n error onto the oxcache error type.
///
/// Both enums are structurally identical (the oxcache type predates the
/// trait-kit one and keeps its shape for API stability).
fn map_error(e: trait_kit::i18n::I18nError) -> I18nError {
    match e {
        trait_kit::i18n::I18nError::InvalidLocale { input, reason } => {
            I18nError::InvalidLocale { input, reason }
        }
        trait_kit::i18n::I18nError::InvalidNumber { input, reason } => {
            I18nError::InvalidNumber { input, reason }
        }
        trait_kit::i18n::I18nError::DateError(d) => I18nError::DateError(d),
        trait_kit::i18n::I18nError::FormatError(d) => I18nError::FormatError(d),
    }
}

/// Largest `u64` that converts to `f64` and back without precision loss
/// (2^53). Counts above this threshold skip locale grouping rather than
/// silently rounding (lossless beats localized).
const U64_F64_EXACT_MAX: u64 = 9_007_199_254_740_992;

impl super::CacheI18nFormatter {
    /// Create a new formatter for the given BCP-47 locale tag.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidLocale`] if the tag cannot be parsed,
    /// or [`I18nError::FormatError`] if ICU4X lacks compiled data for it.
    pub fn new(locale: &str) -> Result<Self, I18nError> {
        let inner = I18nFormatter::new(locale).map_err(map_error)?;
        Ok(Self {
            locale_tag: locale.to_string(),
            inner,
        })
    }

    /// Format a floating-point number with locale-sensitive grouping
    /// and decimal separators.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] for non-finite values or
    /// if the value cannot be parsed into a fixed decimal.
    pub fn format_number(&self, value: f64) -> Result<String, I18nError> {
        self.inner.format_number(value).map_err(map_error)
    }

    /// Build a locale-aware cache key by combining a namespace with a
    /// locale-formatted count (e.g. `"user:1,234"`).
    ///
    /// Counts up to 2^53 format through the ICU decimal formatter (exact
    /// in `f64`). Larger counts fall back to the plain decimal form —
    /// lossless rendering wins over locale grouping.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] if the count cannot be formatted.
    pub fn format_cache_key(&self, namespace: &str, count: u64) -> Result<String, I18nError> {
        let formatted = if count <= U64_F64_EXACT_MAX {
            self.inner.format_number(count as f64).map_err(map_error)?
        } else {
            format!("{count}")
        };
        Ok(format!("{namespace}:{formatted}"))
    }

    /// Format an ISO calendar date (year / month / day) as a cache
    /// expiry timestamp using a medium-length locale-specific pattern.
    ///
    /// # Errors
    /// Returns [`I18nError::DateError`] if any component is out of range,
    /// or [`I18nError::FormatError`] if the formatter cannot be constructed.
    pub fn format_expiry(&self, year: i32, month: u8, day: u8) -> Result<String, I18nError> {
        self.inner.format_date(year, month, day).map_err(map_error)
    }

    /// Return the plural category name for `count` in the formatter's locale
    /// (e.g. `"One"` for English count=1, `"Other"` for count=2).
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn format_count(&self, count: u64) -> Result<String, I18nError> {
        let category = self.inner.plural_category(count).map_err(map_error)?;
        Ok(plural_category_name(category).to_string())
    }

    /// Compare two cache keys using locale-sensitive collation rules.
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn compare_keys(&self, a: &str, b: &str) -> Result<Ordering, I18nError> {
        self.inner.compare(a, b).map_err(map_error)
    }
}

/// Map a `PluralCategory` to its capitalized CLDR name (e.g. `"One"`,
/// `"Other"`).
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
