// Copyright (c) 2025-2026 Kirky.X
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 国际化 (i18n) 示例 ===\n");

    // 1. 创建不同 locale 的格式化器
    println!("--- 1. 创建格式化器 ---");
    let fmt_en = CacheI18nFormatter::new("en-US")?;
    let fmt_zh = CacheI18nFormatter::new("zh-CN")?;
    let fmt_de = CacheI18nFormatter::new("de-DE")?;
    println!("  ✓ en-US 格式化器创建成功");
    println!("  ✓ zh-CN 格式化器创建成功");
    println!("  ✓ de-DE 格式化器创建成功");

    // 无效 locale 处理
    match CacheI18nFormatter::new("not-a-locale!!!") {
        Ok(_) => println!("  意外：无效 locale 创建成功"),
        Err(e) => println!("  ✓ 无效 locale 正确报错: {}", e),
    }

    // 2. 数字格式化
    println!("\n--- 2. 数字格式化 ---");
    let value = 1_234_567.89_f64;
    println!("  原始值:   {}", value);
    println!("  en-US:    {}", fmt_en.format_number(value)?);
    println!("  zh-CN:    {}", fmt_zh.format_number(value)?);
    println!("  de-DE:    {}", fmt_de.format_number(value)?);

    // NaN / Infinity 处理
    match fmt_en.format_number(f64::NAN) {
        Ok(_) => {}
        Err(e) => println!("  ✓ NaN 正确报错: {}", e),
    }

    // 3. 复数规则
    println!("\n--- 3. 复数规则 ---");
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

    // 4. 日期格式化（缓存过期时间）
    println!("\n--- 4. 日期格式化 ---");
    let (year, month, day) = (2026, 7, 15);
    println!("  过期日期: {}-{}-{}", year, month, day);
    println!("  en-US:    {}", fmt_en.format_expiry(year, month, day)?);
    println!("  zh-CN:    {}", fmt_zh.format_expiry(year, month, day)?);
    println!("  de-DE:    {}", fmt_de.format_expiry(year, month, day)?);

    // 5. Cache key 生成
    println!("\n--- 5. Cache key 生成 ---");
    let user_counts: [u64; 4] = [1, 1000, 1_000_000, 1_000_000_000];
    println!("  {:<15} {:<20} {:<20}", "namespace:count", "en-US key", "zh-CN key");
    println!("  {}", "-".repeat(55));
    for count in &user_counts {
        println!(
            "  {:<15} {:<20} {:<20}",
            format!("user:{}", count),
            fmt_en.format_cache_key("user", *count)?,
            fmt_zh.format_cache_key("user", *count)?,
        );
    }

    // 6. 键比较（locale 敏感排序）
    println!("\n--- 6. 键比较（排序） ---");
    let keys = vec!["banana", "apple", "cherry", "date"];
    let mut sorted_keys = keys.clone();
    sorted_keys.sort_by(|a, b| fmt_en.compare_keys(a, b).unwrap_or(std::cmp::Ordering::Equal));
    println!("  原始顺序: {:?}", keys);
    println!("  en-US 排序: {:?}", sorted_keys);

    // 7. 实际使用场景：多语言缓存统计
    println!("\n--- 7. 多语言缓存统计 ---");
    let cache_hit_count: u64 = 15_234;
    let cache_miss_count: u64 = 1;
    println!("  缓存命中: {} 次", cache_hit_count);
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

    println!("\n  缓存未命中: {} 次", cache_miss_count);
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

    println!("\n✓ i18n 示例完成");
    Ok(())
}
