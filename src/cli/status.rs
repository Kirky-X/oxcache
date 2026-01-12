//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了状态查询命令的实现。

use crate::cli::StatusArgs;
use crate::manager::{get_typed_client, MANAGER};
use crate::recovery::health::HealthState;
use anyhow::{Context, Result};
use dashmap::DashMap;
use std::sync::Arc;

pub async fn execute(args: &StatusArgs) -> Result<()> {
    if let Some(ref service_name) = args.service {
        let client = get_typed_client(service_name)
            .with_context(|| format!("Service '{}' not found", service_name))?;

        let state = client.get_health_state().await;
        print_service_status(service_name, &state, args.verbose);
    } else {
        println!("=== Cache Services Status ===\n");

        if MANAGER.is_empty() {
            println!("No cache services registered.");
            return Ok(());
        }

        let mut services: Vec<String> = Vec::new();
        let manager: &DashMap<String, Arc<dyn crate::CacheOps>> = &MANAGER;
        for entry in manager.iter() {
            let key: &String = entry.key();
            services.push(key.clone());
        }
        services.sort();

        for service_name in services {
            let client = get_typed_client(&service_name)?;
            let state = client.get_health_state().await;
            print_service_status(&service_name, &state, args.verbose);
            println!();
        }
    }

    Ok(())
}

fn print_service_status(service_name: &str, state: &HealthState, verbose: bool) {
    let status = match state {
        HealthState::Healthy => "✅ HEALTHY".to_string(),
        HealthState::Degraded {
            since,
            failure_count,
        } => {
            if verbose {
                let elapsed = since.elapsed().as_secs();
                format!("⚠️ DEGRADED ({} failures, {}s ago)", failure_count, elapsed)
            } else {
                "⚠️ DEGRADED".to_string()
            }
        }
        HealthState::Recovering {
            since,
            success_count,
        } => {
            if verbose {
                let elapsed = since.elapsed().as_secs();
                format!(
                    "🔄 RECOVERING ({} successes, {}s ago)",
                    success_count, elapsed
                )
            } else {
                "🔄 RECOVERING".to_string()
            }
        }
        HealthState::WalReplaying { since } => {
            if verbose {
                let elapsed = since.elapsed().as_secs();
                format!("🔄 WalReplaying ({}s ago)", elapsed)
            } else {
                "🔄 WalReplaying".to_string()
            }
        }
    };

    println!("Service: {}", service_name);
    println!("Status:  {}", status);

    if verbose {
        let metrics = &crate::metrics::GLOBAL_METRICS;

        let mut total_requests = 0;
        let mut l1_hits = 0;
        let mut l2_hits = 0;

        // DashMap 无锁迭代
        let requests: &DashMap<String, u64> = &metrics.requests_total;
        for entry in requests.iter() {
            let key: &String = entry.key();
            let count: &u64 = entry.value();
            if key.starts_with(service_name) {
                total_requests += *count;
                if key.ends_with(":hit") {
                    if key.contains(":L1:") {
                        l1_hits += count;
                    } else if key.contains(":L2:") {
                        l2_hits += count;
                    }
                }
            }
        }

        println!("Total Requests: {}", total_requests);
        if total_requests > 0 {
            let hit_rate = ((l1_hits + l2_hits) as f64 / total_requests as f64 * 100.0).round();
            println!("Hit Rate:      {}%", hit_rate);
        }

        // DashMap 无锁，直接获取并解引用
        if let Some(wal_count) = metrics.wal_entries.get(service_name) {
            println!("WAL Entries:   {}", *wal_count);
        }
    }
}
