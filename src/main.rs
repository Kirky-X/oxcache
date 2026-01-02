//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块是缓存服务的入口点。

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    oxcache::cli::run().await
}
