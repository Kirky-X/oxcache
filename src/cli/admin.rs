//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了管理员操作命令的实现。

use crate::client::CacheOps;
use crate::manager::get_typed_client;
use crate::sync::warmup::WarmupStatus;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

pub async fn execute(args: &AdminArgs) -> Result<()> {
    match &args.command {
        AdminSubcommand::Clean(clean_args) => execute_clean(clean_args).await,
        AdminSubcommand::Warmup(warmup_args) => execute_warmup(warmup_args).await,
    }
}

async fn execute_clean(args: &CleanArgs) -> Result<()> {
    let client = get_typed_client(&args.service)
        .with_context(|| format!("Service '{}' not found", args.service))?;

    if args.confirm {
        println!("Preparing to clean cache for service: {}", args.service);
        if args.l1 {
            println!("  - L1 cache");
        }
        if args.l2 {
            println!("  - L2 cache");
        }
        if args.wal {
            println!("  - WAL logs");
        }
        print!("\nDo you want to continue? [y/N]: ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "y" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    if args.l1 {
        println!("Cleaning L1 cache...");
        client.clear_l1().await?;
        println!("L1 cache cleared.");
    }

    if args.l2 {
        println!("Cleaning L2 cache...");
        client.clear_l2().await?;
        println!("L2 cache cleared.");
    }

    if args.wal {
        println!("Cleaning WAL logs...");
        client.clear_wal().await?;
        println!("WAL logs cleared.");
    }

    println!("\n✅ Cleanup completed for service: {}", args.service);

    Ok(())
}

async fn execute_warmup(args: &WarmupArgs) -> Result<()> {
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

#[derive(Parser, Debug)]
pub struct CleanArgs {
    #[arg(short, long, help = "Service name")]
    pub service: String,

    #[arg(long, help = "Clear L1 cache")]
    pub l1: bool,

    #[arg(long, help = "Clear L2 cache")]
    pub l2: bool,

    #[arg(long, help = "Clear WAL logs")]
    pub wal: bool,

    #[arg(short, long, help = "Skip confirmation")]
    pub confirm: bool,
}

#[derive(Parser, Debug)]
pub struct WarmupArgs {
    #[arg(short, long, help = "Service name")]
    pub service: String,

    #[arg(long, help = "Start warmup")]
    pub start: bool,

    #[arg(long, help = "Check warmup status")]
    pub status: bool,

    #[arg(long, help = "Stop warmup")]
    pub stop: bool,
}

#[derive(Subcommand, Debug)]
pub enum AdminSubcommand {
    #[command(name = "clean", about = "Clear cache data")]
    Clean(CleanArgs),

    #[command(name = "warmup", about = "Control cache warmup")]
    Warmup(WarmupArgs),
}

#[derive(Parser, Debug)]
pub struct AdminArgs {
    #[command(subcommand)]
    pub command: AdminSubcommand,
}
