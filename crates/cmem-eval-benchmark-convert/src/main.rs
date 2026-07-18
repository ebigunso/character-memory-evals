use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use cmem_eval_benchmark_convert::{convert_paths, load_selection_manifest, write_or_check};

#[derive(Debug, Parser)]
#[command(about = "Convert curated LongMemEval-S and LoCoMo rows into continuity fixtures")]
struct Args {
    #[arg(long)]
    selection: PathBuf,
    #[arg(long)]
    longmemeval: PathBuf,
    #[arg(long)]
    locomo: PathBuf,
    #[arg(long)]
    fixture_out: PathBuf,
    #[arg(long)]
    embedding_manifest_out: PathBuf,
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let selection = load_selection_manifest(&args.selection)?;
    let artifacts = convert_paths(&selection, &args.longmemeval, &args.locomo)?;
    write_or_check(
        &artifacts,
        &args.fixture_out,
        &args.embedding_manifest_out,
        args.check,
    )
}
