//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了CLI命令行接口。

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "oxcache")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(name = "status", about = "Query cache service status")]
    Status(StatusArgs),

    #[command(name = "admin", about = "Admin operations (clean, warmup)")]
    Admin(AdminArgs),

    #[command(name = "metrics", about = "Get cache metrics")]
    Metrics(MetricsArgs),
}

#[derive(Parser, Debug)]
pub struct StatusArgs {
    #[arg(short, long, help = "Service name to query")]
    pub service: Option<String>,

    #[arg(short, long, help = "Show detailed information")]
    pub verbose: bool,
}

#[derive(Parser, Debug)]
pub struct MetricsArgs {
    #[arg(short, long, help = "Service name to query")]
    pub service: Option<String>,

    #[arg(short, long, help = "Output in Prometheus format")]
    pub prometheus: bool,

    #[arg(short, long, help = "Output in JSON format")]
    pub json: bool,
}

mod admin;
mod metrics;
mod status;

pub use admin::{AdminArgs, AdminSubcommand, CleanArgs, WarmupArgs};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Status(args) => status::execute(args).await,
        Commands::Admin(args) => admin::execute(args).await,
        Commands::Metrics(args) => metrics::execute(args).await,
    }
}
