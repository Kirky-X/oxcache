use crate::cli::StatusArgs;
use crate::manager::{get_typed_client, MANAGER};
use crate::recovery::health::HealthState;
use anyhow::{Context, Result};

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

        for entry in MANAGER.iter() {
            let service_name = entry.key().clone();
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
    };

    println!("Service: {}", service_name);
    println!("Status:  {}", status);

    if verbose {
        let metrics = &crate::metrics::GLOBAL_METRICS;
        let reqs = metrics.requests_total.lock().unwrap();

        let mut total_requests = 0;
        let mut l1_hits = 0;
        let mut l2_hits = 0;

        for (key, count) in reqs.iter() {
            if key.starts_with(service_name) {
                total_requests += count;
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

        let wal = metrics.wal_entries.lock().unwrap();
        if let Some(wal_count) = wal.get(service_name) {
            println!("WAL Entries:   {}", wal_count);
        }
    }
}
