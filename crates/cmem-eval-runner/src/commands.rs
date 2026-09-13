#[path = "pipeline.rs"]
mod pipeline;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use cmem_eval::BenchmarkRunConfig;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "cmem-eval")]
#[command(about = "Run Character Memory retrieval benchmarks")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Run(run) => run.run().await,
            Command::Embeddings(args) => args.run().await,
            Command::Diff(args) => crate::diff::run(args),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunCommand),
    Embeddings(crate::frozen_embeddings::EmbeddingsCommand),
    Diff(crate::diff::DiffArgs),
}

#[derive(Debug, Args)]
struct RunCommand {
    #[command(subcommand)]
    dataset: RunDataset,
}

impl RunCommand {
    async fn run(self) -> Result<()> {
        match self.dataset {
            RunDataset::Continuity(args) => pipeline::run_continuity(args).await,
            RunDataset::LongmemevalS(args) => pipeline::run_longmemeval(args).await,
            RunDataset::Locomo(args) => pipeline::run_locomo(args).await,
        }
    }
}

#[derive(Debug, Subcommand)]
enum RunDataset {
    Continuity(ContinuityRunArgs),
    LongmemevalS(RunArgs),
    Locomo(RunArgs),
}

#[derive(Debug, Args, Clone)]
pub(crate) struct ContinuityRunArgs {
    #[command(flatten)]
    pub(crate) run: RunArgs,
    #[arg(long = "trace-out")]
    pub(crate) trace_out: PathBuf,
    #[arg(long = "report-out")]
    pub(crate) report_out: PathBuf,
    #[arg(long)]
    pub(crate) scenario: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct RunArgs {
    #[arg(long)]
    pub(crate) dataset: PathBuf,
    #[arg(long)]
    pub(crate) config: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long = "summary-out")]
    pub(crate) summary_out: PathBuf,
}

pub(crate) fn read_config(path: &PathBuf) -> Result<BenchmarkRunConfig> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: toml::Value = toml::from_str(&content)?;
    let json = serde_json::to_value(value)?;
    Ok(serde_json::from_value(json)?)
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
