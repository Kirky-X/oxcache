use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    oxcache::cli::run().await
}
