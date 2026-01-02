//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了指标查询命令的实现。

use crate::cli::MetricsArgs;
use crate::manager::{get_typed_client, MANAGER};
use crate::metrics::get_metrics_string;
use anyhow::Result;

pub async fn execute(args: &MetricsArgs) -> Result<()> {
    if args.prometheus {
        let output = get_metrics_string();
        println!("{}", output);
        return Ok(());
    }

    if args.json {
        let output = get_metrics_string();
        println!("{}", output);
        return Ok(());
    }

    println!("=== Cache Metrics ===\n");

    let metrics = &crate::metrics::GLOBAL_METRICS;

    if let Some(ref service_name) = args.service {
        let client = get_typed_client(service_name)?;
        let _ = client;

        println!("Service: {}", service_name);

        let mut total_requests = 0;
        let mut hits = 0;

        // DashMap 无锁迭代
        for entry in metrics.requests_total.iter() {
            let key = entry.key();
            let count = entry.value();
            if key.starts_with(service_name) {
                total_requests += count;
                if key.ends_with(":hit") {
                    hits += count;
                }
            }
        }

        println!("\nRequests:");
        println!("  Total:  {}", total_requests);
        if total_requests > 0 {
            let hit_rate = (hits as f64 / total_requests as f64 * 100.0).round();
            println!("  Hits:   {} ({:.1}%)", hits, hit_rate);
            println!("  Misses: {}", total_requests - hits);
        }

        // DashMap 无锁获取
        if let Some(status) = metrics.l2_health_status.get(service_name) {
            let status_str = match *status {
                0 => "Degraded",
                1 => "Healthy",
                2 => "Recovering",
                _ => "Unknown",
            };
            println!("\nHealth: {}", status_str);
        }

        if let Some(wal_count) = metrics.wal_entries.get(service_name) {
            println!("\nWAL Entries: {}", *wal_count);
        }

        if let Some(batch_size) = metrics.batch_buffer_size.get(service_name) {
            println!("Batch Buffer: {}", *batch_size);
        }
    } else {
        println!("All Services:\n");

        for entry in MANAGER.iter() {
            let service_name = entry.key().clone();

            let mut total_requests = 0;
            let mut hits = 0;

            // DashMap 无锁迭代
            for entry in metrics.requests_total.iter() {
                let key = entry.key();
                let count = entry.value();
                if key.starts_with(&service_name) {
                    total_requests += count;
                    if key.ends_with(":hit") {
                        hits += count;
                    }
                }
            }

            print!("  {}: {} reqs", service_name, total_requests);
            if total_requests > 0 {
                let hit_rate = (hits as f64 / total_requests as f64 * 100.0).round();
                print!(", {:.1}% hit rate", hit_rate);
            }
            println!();
        }
    }

    Ok(())
}
