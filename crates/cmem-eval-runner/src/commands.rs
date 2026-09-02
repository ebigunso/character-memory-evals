#[path = "pipeline.rs"]
mod pipeline;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use cmem_eval_core::{BenchmarkRunConfig, RetrievalMode, RunAdapterMetadata};
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
    #[arg(long, value_enum)]
    pub(crate) adapter: Option<AdapterKind>,
    #[arg(long)]
    pub(crate) allow_mock_benchmark: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum AdapterKind {
    Mock,
    Real,
}

impl RunArgs {
    pub(crate) fn selected_adapter(&self) -> AdapterKind {
        self.adapter.unwrap_or(AdapterKind::Real)
    }

    pub(crate) fn validate_adapter_selection(&self, config: &BenchmarkRunConfig) -> Result<()> {
        if self.selected_adapter() == AdapterKind::Mock && !self.allow_mock_benchmark {
            bail!(
                "mock adapter is test/smoke-only; pass `--allow-mock-benchmark` to make mock output explicit, or omit `--adapter` for the default live Character Memory run"
            );
        }
        if config.retrieval.mode == RetrievalMode::Bm25Only
            && self.selected_adapter() != AdapterKind::Mock
        {
            bail!(
                "retrieval.mode=bm25_only is service-free and requires `--adapter mock --allow-mock-benchmark`; refusing to create a live adapter"
            );
        }
        if config.retrieval.mode == RetrievalMode::VectorOnly
            && self.selected_adapter() == AdapterKind::Mock
        {
            bail!(
                "retrieval.mode=vector_only is a live Qdrant baseline and cannot run with `--adapter mock`; omit `--adapter` or pass `--adapter real`"
            );
        }
        Ok(())
    }
}

impl AdapterKind {
    pub(crate) fn metadata(self) -> RunAdapterMetadata {
        match self {
            AdapterKind::Mock => RunAdapterMetadata::mock_smoke(),
            AdapterKind::Real => RunAdapterMetadata::live(),
        }
    }
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
