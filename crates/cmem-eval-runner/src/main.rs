mod commands;
mod enrichment;
mod frozen_embeddings;
mod official_exports;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    commands::Cli::parse().run().await
}
