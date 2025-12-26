use crate::cli::CleanArgs;
use crate::client::CacheOps;
use crate::manager::get_typed_client;
use anyhow::{Context, Result};

pub async fn execute(args: &CleanArgs) -> Result<()> {
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
