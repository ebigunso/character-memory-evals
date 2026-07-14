use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result};
use cmem_eval_continuity::{CHECKED_FIXTURE_SEED, canonical_fixture_bytes, generate_fixture_set};

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("crates/cmem-eval-continuity/fixtures/continuity_v2.json")
    });
    let seed = args
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()
        .context("fixture seed must be an unsigned integer")?
        .unwrap_or(CHECKED_FIXTURE_SEED);
    if args.next().is_some() {
        anyhow::bail!("usage: generate_continuity_fixtures [output_path] [seed]");
    }

    let bytes = canonical_fixture_bytes(&generate_fixture_set(seed)?)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create fixture directory {}", parent.display()))?;
    }
    fs::write(&output, bytes).with_context(|| format!("write fixture {}", output.display()))?;
    println!(
        "wrote continuity fixtures to {} with seed {seed}",
        output.display()
    );
    Ok(())
}
