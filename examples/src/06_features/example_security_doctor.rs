// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全配置自审计示例（doctor feature）
//!
//! 打印默认配置与"生产意图"配置的安全报告，按生产就绪度设置退出码。
//! 默认 feature 下 `value-encryption` 为 [CRIT]，退出码为 1。

use oxcache::config::{CacheConfig, CacheConfigBuilder};
use oxcache::{SecurityReport, audit};
use std::time::Duration;

fn print_report(title: &str, report: &SecurityReport) {
    println!("=== {title} ===");
    println!("{}", report.render());
    println!();
}

fn main() {
    let default_report = audit(&CacheConfig::default());
    print_report("default configuration", &default_report);

    let production_intent = CacheConfigBuilder::default()
        .redis_url("rediss://cache.example.com:6379")
        .null_cache_ttl(Duration::from_secs(30))
        .ttl_jitter_factor(0.2)
        .metrics_enabled(true)
        .build();
    let production_report = audit(&production_intent);
    print_report("production-intent configuration", &production_report);

    if default_report.is_production_ready() {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
