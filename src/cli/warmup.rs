use crate::cli::WarmupArgs;
use crate::manager::get_typed_client;
use crate::sync::warmup::WarmupStatus;
use anyhow::{Context, Result};

pub async fn execute(args: &WarmupArgs) -> Result<()> {
    let client = get_typed_client(&args.service)
        .with_context(|| format!("Service '{}' not found", args.service))?;

    if args.status {
        let warmup_mgr = client.warmup_manager();
        if let Some(mgr) = warmup_mgr {
            let status = mgr.get_status("all").await;
            println!("=== Warmup Status for '{}' ===\n", args.service);
            display_warmup_status(&status);
        } else {
            println!("No warmup configured for service '{}'", args.service);
        }
        return Ok(());
    }

    if args.start {
        println!("Starting warmup for service: {}...", args.service);
        client.run_warmup().await?;
        println!("✅ Warmup started successfully.");
        return Ok(());
    }

    if args.stop {
        println!("Stop command received for service: {}", args.service);
        println!("Note: Warmup is running asynchronously, cannot be stopped mid-execution.");
        return Ok(());
    }

    println!("No action specified. Use --start, --stop, or --status.");
    Ok(())
}

fn display_warmup_status(status: &WarmupStatus) {
    match status {
        WarmupStatus::Pending => {
            println!("Status:          ⏳ PENDING");
            println!("Progress:        0%");
        }
        WarmupStatus::InProgress { progress, total } => {
            let pct = if *total > 0 {
                (*progress as f64 / *total as f64 * 100.0).round()
            } else {
                0.0
            };
            println!("Status:          🔄 IN PROGRESS");
            println!("Progress:        {}%", pct);
            println!("Items Processed: {}/{}", progress, total);
        }
        WarmupStatus::Completed { loaded, failed } => {
            println!("Status:          ✅ COMPLETED");
            println!("Loaded Items:    {}", loaded);
            println!("Failed Items:    {}", failed);
        }
        WarmupStatus::Failed { error } => {
            println!("Status:          ❌ FAILED");
            println!("Error:           {}", error);
        }
    }
}
