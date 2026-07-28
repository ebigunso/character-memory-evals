#[path = "pipeline.rs"]
mod pipeline;

use crate::official_exports;
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use cmem_eval_core::{
    BenchmarkRunConfig, RetrievalMode, RunAdapterMetadata, read_jsonl, summarize_rows,
    validate_summary_row_identity, write_summary,
};
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
            Command::ExportOfficial(args) => export_official(args),
            Command::Summarize(args) => summarize(args),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunCommand),
    Embeddings(crate::frozen_embeddings::EmbeddingsCommand),
    ExportOfficial(ExportOfficialCommand),
    Summarize(SummarizeArgs),
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
            RunDataset::Synthetic(args) => pipeline::run_synthetic(args).await,
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
    Synthetic(RunArgs),
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

#[derive(Debug, Args)]
struct ExportOfficialCommand {
    #[command(subcommand)]
    dataset: ExportOfficialDataset,
}

#[derive(Debug, Subcommand)]
enum ExportOfficialDataset {
    Longmemeval(LongMemEvalExportCommand),
    Locomo(LoCoMoExportArgs),
}

#[derive(Debug, Args)]
struct LongMemEvalExportCommand {
    #[command(subcommand)]
    export: LongMemEvalExportKind,
}

#[derive(Debug, Subcommand)]
enum LongMemEvalExportKind {
    Retrieval(OfficialExportArgs),
    Qa(QaExportArgs),
}

#[derive(Debug, Args)]
struct OfficialExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct QaExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    predictions: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct LoCoMoExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    predictions: Option<PathBuf>,
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

#[derive(Debug, Args)]
struct SummarizeArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    dataset: Option<PathBuf>,
    #[arg(long)]
    scenario: Option<String>,
}

fn export_official(args: ExportOfficialCommand) -> Result<()> {
    match args.dataset {
        ExportOfficialDataset::Longmemeval(args) => match args.export {
            LongMemEvalExportKind::Retrieval(args) => {
                let rows = read_jsonl(&args.input)?;
                official_exports::write_longmemeval_retrieval(&args.out, &rows)
            }
            LongMemEvalExportKind::Qa(args) => {
                let rows = read_jsonl(&args.input)?;
                let predictions = official_exports::read_predictions_jsonl(&args.predictions)?;
                official_exports::write_longmemeval_qa(&args.out, &rows, &predictions)
            }
        },
        ExportOfficialDataset::Locomo(args) => {
            let rows = read_jsonl(&args.input)?;
            let predictions = args
                .predictions
                .as_ref()
                .map(|path| official_exports::read_predictions_jsonl(path))
                .transpose()?;
            official_exports::write_locomo(&args.out, &rows, predictions.as_ref())
        }
    }
}

fn summarize(args: SummarizeArgs) -> Result<()> {
    let rows = read_jsonl(&args.input)?;
    let Some(first) = rows.first() else {
        bail!("cannot summarize empty JSONL: {}", args.input.display());
    };
    let config = read_config(&args.config)?;
    config.validate()?;
    let expected_dataset_kind = pipeline::dataset_kind_for_config(&config)?;
    validate_summary_row_identity(
        &config.run_id,
        &config.dataset,
        expected_dataset_kind,
        &first.adapter,
        &rows,
    )?;
    if config.dataset.as_str() == "continuity" {
        let dataset = args.dataset.as_deref().context(
            "summarizing continuity results requires --dataset with the source fixture path",
        )?;
        pipeline::validate_continuity_summary_rows(&rows, dataset, args.scenario.as_deref())?;
    }
    let metric_family = pipeline::metric_family_for_config(
        &config,
        args.dataset.as_deref(),
        args.scenario.as_deref(),
    )?;
    let summary = summarize_rows(
        config.run_id.clone(),
        config.dataset.clone(),
        expected_dataset_kind,
        first.adapter.clone(),
        serde_json::to_value(&config)?,
        &rows,
        &[metric_family],
    )?;
    write_summary(&args.out, &summary)
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
