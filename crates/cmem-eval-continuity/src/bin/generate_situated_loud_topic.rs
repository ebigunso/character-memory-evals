use std::{env, fs::File, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use cmem_eval_continuity::{
    CHECKED_FIXTURE_SEED, canonical_fixture_bytes, generate_situated_loud_topic_fixture,
};

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let output = args.next().map(PathBuf::from).context(
        "usage: generate_situated_loud_topic <new_output_path>; an existing file is never replaced",
    )?;
    anyhow::ensure!(args.next().is_none(), "expected one output path");
    let bytes =
        canonical_fixture_bytes(&generate_situated_loud_topic_fixture(CHECKED_FIXTURE_SEED)?)?;
    File::create_new(&output)
        .with_context(|| format!("create new fixture {}", output.display()))?
        .write_all(&bytes)
        .with_context(|| format!("write fixture {}", output.display()))?;
    println!("wrote situated loud-topic fixture to {}", output.display());
    Ok(())
}
