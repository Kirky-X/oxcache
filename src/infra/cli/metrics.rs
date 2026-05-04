//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了指标查询命令的实现。

use anyhow::Result;

pub async fn execute(args: &MetricsArgs) -> Result<()> {
    if args.prometheus || args.json {
        println!("Metrics export requires the metrics feature with OpenTelemetry.");
        return Ok(());
    }

    println!("Cache metrics require the new Cache API.");
    println!("Metrics are available via OpenTelemetry when the 'metrics' feature is enabled.");
    println!();
    println!("Example:");
    println!("  use oxcache::backend::CacheBackend;");
    println!("  let stats = cache.stats().await?;");

    Ok(())
}
