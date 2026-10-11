// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 国际化 (i18n) 示例
//!
//! 本示例演示 oxcache 的 ICU4X 国际化功能：
//! - CacheI18nFormatter 创建与 locale 解析
//! - 数字格式化（locale 敏感的千位分隔符、小数点）
//! - 复数规则（"One" / "Other" 等 CLDR 分类）
//! - 日期格式化（缓存过期时间展示）
//! - Cache key 生成（namespace + 格式化计数）
//! - 键比较（locale 敏感的排序规则）
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_i18n
//! ```

use oxcache::i18n::CacheI18nFormatter;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_I18N_1_CREATING_FORMATTERS, MSG_EXAMPLE_I18N_2_NUMBER_FORMATTING,
    MSG_EXAMPLE_I18N_3_PLURAL_RULES, MSG_EXAMPLE_I18N_4_DATE_FORMATTING,
    MSG_EXAMPLE_I18N_5_CACHE_KEY_GENERATION, MSG_EXAMPLE_I18N_6_KEY_COMPARISON_COLLATION,
    MSG_EXAMPLE_I18N_7_MULTILINGUAL_CACHE_STATISTICS, MSG_EXAMPLE_I18N_CACHE_HITS,
    MSG_EXAMPLE_I18N_CACHE_MISSES, MSG_EXAMPLE_I18N_DE_DE_FORMATTER_CREATED,
    MSG_EXAMPLE_I18N_EN_US_COLLATION, MSG_EXAMPLE_I18N_EN_US_FORMATTER_CREATED,
    MSG_EXAMPLE_I18N_EXAMPLE_COMPLETE, MSG_EXAMPLE_I18N_EXPIRY_DATE,
    MSG_EXAMPLE_I18N_INTERNATIONALIZATION_I18N_EXAMPLE,
    MSG_EXAMPLE_I18N_INVALID_LOCALE_CORRECTLY_ERRORED, MSG_EXAMPLE_I18N_NAN_CORRECTLY_ERRORED,
    MSG_EXAMPLE_I18N_ORIGINAL_ORDER, MSG_EXAMPLE_I18N_RAW_VALUE,
    MSG_EXAMPLE_I18N_UNEXPECTED_AN_INVALID_LOCALE, MSG_EXAMPLE_I18N_ZH_CN_FORMATTER_CREATED, t,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(MSG_EXAMPLE_I18N_INTERNATIONALIZATION_I18N_EXAMPLE, &[])
    );

    println!("{}", t(MSG_EXAMPLE_I18N_1_CREATING_FORMATTERS, &[]));
    let fmt_en = CacheI18nFormatter::new("en-US")?;
    let fmt_zh = CacheI18nFormatter::new("zh-CN")?;
    let fmt_de = CacheI18nFormatter::new("de-DE")?;
    println!("  {}", t(MSG_EXAMPLE_I18N_EN_US_FORMATTER_CREATED, &[]));
    println!("  {}", t(MSG_EXAMPLE_I18N_ZH_CN_FORMATTER_CREATED, &[]));
    println!("  {}", t(MSG_EXAMPLE_I18N_DE_DE_FORMATTER_CREATED, &[]));

    // 无效 locale 处理
    match CacheI18nFormatter::new("not-a-locale!!!") {
        Ok(_) => println!(
            "  {}",
            t(MSG_EXAMPLE_I18N_UNEXPECTED_AN_INVALID_LOCALE, &[])
        ),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_I18N_INVALID_LOCALE_CORRECTLY_ERRORED,
                &[("e", e.to_string())]
            )
        ),
    }

    println!("\n{}", t(MSG_EXAMPLE_I18N_2_NUMBER_FORMATTING, &[]));
    let value = 1_234_567.89_f64;
    println!(
        "  {}",
        t(MSG_EXAMPLE_I18N_RAW_VALUE, &[("value", value.to_string())])
    );
    println!("  en-US:    {}", fmt_en.format_number(value)?);
    println!("  zh-CN:    {}", fmt_zh.format_number(value)?);
    println!("  de-DE:    {}", fmt_de.format_number(value)?);

    // NaN / Infinity 处理
    match fmt_en.format_number(f64::NAN) {
        Ok(_) => {}
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_I18N_NAN_CORRECTLY_ERRORED,
                &[("e", e.to_string())]
            )
        ),
    }

    println!("\n{}", t(MSG_EXAMPLE_I18N_3_PLURAL_RULES, &[]));
    let counts = [0, 1, 2, 5, 100, 1000];
    println!("  {:<8} {:<10} {:<10}", "count", "en-US", "zh-CN");
    println!("  {}", "-".repeat(30));
    for count in &counts {
        println!(
            "  {:<8} {:<10} {:<10}",
            count,
            fmt_en.format_count(*count)?,
            fmt_zh.format_count(*count)?,
        );
    }

    println!("\n{}", t(MSG_EXAMPLE_I18N_4_DATE_FORMATTING, &[]));
    let (year, month, day) = (2026, 7, 15);
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_I18N_EXPIRY_DATE,
            &[
                ("year", year.to_string()),
                ("month", month.to_string()),
                ("day", day.to_string())
            ]
        )
    );
    println!("  en-US:    {}", fmt_en.format_expiry(year, month, day)?);
    println!("  zh-CN:    {}", fmt_zh.format_expiry(year, month, day)?);
    println!("  de-DE:    {}", fmt_de.format_expiry(year, month, day)?);

    println!("\n{}", t(MSG_EXAMPLE_I18N_5_CACHE_KEY_GENERATION, &[]));
    let user_counts: [u64; 4] = [1, 1000, 1_000_000, 1_000_000_000];
    println!(
        "  {:<15} {:<20} {:<20}",
        "namespace:count", "en-US key", "zh-CN key"
    );
    println!("  {}", "-".repeat(55));
    for count in &user_counts {
        println!(
            "  {:<15} {:<20} {:<20}",
            format!("user:{}", count),
            fmt_en.format_cache_key("user", *count)?,
            fmt_zh.format_cache_key("user", *count)?,
        );
    }

    println!("\n{}", t(MSG_EXAMPLE_I18N_6_KEY_COMPARISON_COLLATION, &[]));
    let keys = vec!["banana", "apple", "cherry", "date"];
    let mut sorted_keys = keys.clone();
    sorted_keys.sort_by(|a, b| {
        fmt_en
            .compare_keys(a, b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_I18N_ORIGINAL_ORDER,
            &[("keys", format!("{:?}", keys))]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_I18N_EN_US_COLLATION,
            &[("sorted_keys", format!("{:?}", sorted_keys))]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_I18N_7_MULTILINGUAL_CACHE_STATISTICS, &[])
    );
    let cache_hit_count: u64 = 15_234;
    let cache_miss_count: u64 = 1;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_I18N_CACHE_HITS,
            &[("cache_hit_count", cache_hit_count.to_string())]
        )
    );
    println!(
        "  en-US: {} hits ({})",
        fmt_en.format_number(cache_hit_count as f64)?,
        fmt_en.format_count(cache_hit_count)?
    );
    println!(
        "  zh-CN: {} 次命中 ({})",
        fmt_zh.format_number(cache_hit_count as f64)?,
        fmt_zh.format_count(cache_hit_count)?
    );
    println!(
        "  de-DE: {} Treffer ({})",
        fmt_de.format_number(cache_hit_count as f64)?,
        fmt_de.format_count(cache_hit_count)?
    );

    println!(
        "\n  {}",
        t(
            MSG_EXAMPLE_I18N_CACHE_MISSES,
            &[("cache_miss_count", cache_miss_count.to_string())]
        )
    );
    println!(
        "  en-US: {} miss ({})",
        fmt_en.format_number(cache_miss_count as f64)?,
        fmt_en.format_count(cache_miss_count)?
    );
    println!(
        "  zh-CN: {} 次未命中 ({})",
        fmt_zh.format_number(cache_miss_count as f64)?,
        fmt_zh.format_count(cache_miss_count)?
    );

    println!("\n{}", t(MSG_EXAMPLE_I18N_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
