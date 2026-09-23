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
            Command::CompareContinuity { before, after } => {
                let before = cmem_eval_continuity::read_continuity_report(&before)?;
                let after = cmem_eval_continuity::read_continuity_report(&after)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &cmem_eval_continuity::compare_continuity_reports(&before, &after)
                    )?
                );
                Ok(())
            }
            Command::Seal { run_dir } => {
                let evidence_root =
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence");
                let destination = crate::seal::seal(&run_dir, &evidence_root)?;
                println!("sealed {}", destination.display());
                Ok(())
            }
            Command::Verify { evidence_dir } => crate::seal::verify(&evidence_dir),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunCommand),
    Embeddings(crate::frozen_embeddings::EmbeddingsCommand),
    Diff(crate::diff::DiffArgs),
    /// Compare scenario outcomes, assertions, recall, and omission invariants.
    CompareContinuity {
        before: PathBuf,
        after: PathBuf,
    },
    /// Preserve a finished run as immutable evidence in this checkout.
    Seal {
        run_dir: PathBuf,
    },
    /// Recompute the file hashes recorded in an evidence directory's seal.
    Verify {
        evidence_dir: PathBuf,
    },
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
}

#[cfg(test)]
pub(crate) fn read_config(path: &PathBuf) -> Result<BenchmarkRunConfig> {
    read_config_source(path).map(|(config, _)| config)
}

pub(crate) fn read_config_source(path: &PathBuf) -> Result<(BenchmarkRunConfig, String)> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: toml::Value = toml::from_str(&content)?;
    let json = serde_json::to_value(value)?;
    Ok((serde_json::from_value(json)?, content))
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
