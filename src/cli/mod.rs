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

    #[command(name = "clean", about = "Clear cache data")]
    Clean(CleanArgs),

    #[command(name = "warmup", about = "Control cache warmup")]
    Warmup(WarmupArgs),

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

#[derive(Parser, Debug)]
pub struct MetricsArgs {
    #[arg(short, long, help = "Service name to query")]
    pub service: Option<String>,

    #[arg(short, long, help = "Output in Prometheus format")]
    pub prometheus: bool,

    #[arg(short, long, help = "Output in JSON format")]
    pub json: bool,
}

mod clean;
mod metrics;
mod status;
mod warmup;

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Status(args) => status::execute(args).await,
        Commands::Clean(args) => clean::execute(args).await,
        Commands::Warmup(args) => warmup::execute(args).await,
        Commands::Metrics(args) => metrics::execute(args).await,
    }
}
