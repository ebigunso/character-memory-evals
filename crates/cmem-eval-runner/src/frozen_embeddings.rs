use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use cmem_eval::fs_util::atomic_replace;
use cmem_eval::openai_embedding::{EmbeddingRetryPolicy, OpenAiEmbeddingClient};
use cmem_eval::{FrozenEmbeddingManifest, FrozenEmbeddingProvider, FrozenEmbeddingStore};

const MAX_EMBEDDING_INPUTS_PER_REQUEST: usize = 2_048;

#[derive(Debug, Args)]
pub(crate) struct EmbeddingsCommand {
    #[command(subcommand)]
    command: EmbeddingsSubcommand,
}

#[derive(Debug, Subcommand)]
enum EmbeddingsSubcommand {
    /// Generate a frozen store in one explicit, network-using offline step.
    Generate(GenerateArgs),
    /// Validate store integrity, coverage, and declared semantic orderings offline.
    Validate(ValidateArgs),
}

#[derive(Debug, Args)]
struct GenerateArgs {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long, default_value = "text-embedding-3-large")]
    model: String,
    #[arg(long)]
    dimensions: Option<usize>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "OPENAI_API_KEY")]
    api_key_env: String,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    store: PathBuf,
}

impl EmbeddingsCommand {
    pub(crate) async fn run(self) -> Result<()> {
        match self.command {
            EmbeddingsSubcommand::Generate(args) => generate(args).await,
            EmbeddingsSubcommand::Validate(args) => validate(args),
        }
    }
}

async fn generate(args: GenerateArgs) -> Result<()> {
    let model = args.model.trim();
    if model.is_empty() {
        bail!("--model must not be empty");
    }
    if args.dimensions == Some(0) {
        bail!("--dimensions must be greater than zero when set");
    }
    if args.api_key_env.trim().is_empty() {
        bail!("--api-key-env must not be empty");
    }
    let manifest = FrozenEmbeddingManifest::load(&args.manifest)?;
    let unique_texts = manifest.unique_texts()?;
    if unique_texts.len() > MAX_EMBEDDING_INPUTS_PER_REQUEST {
        bail!(
            "embedding manifest has {} unique texts; the OpenAI embeddings request accepts at most {MAX_EMBEDDING_INPUTS_PER_REQUEST} inputs",
            unique_texts.len()
        );
    }
    let api_key = env::var(&args.api_key_env).with_context(|| {
        format!(
            "{} is required for offline embedding generation",
            args.api_key_env
        )
    })?;
    if api_key.trim().is_empty() {
        bail!(
            "{} is required for offline embedding generation",
            args.api_key_env
        );
    }
    // One request; an ambiguous failure must not trigger duplicate billable calls.
    let embeddings = OpenAiEmbeddingClient::default()
        .embed_batch(
            &api_key,
            model,
            &unique_texts,
            args.dimensions,
            EmbeddingRetryPolicy::no_retry(),
        )
        .await
        .context("request offline OpenAI embeddings")?;
    let mut store = FrozenEmbeddingStore::new(
        model,
        "open_ai_api",
        unique_texts.into_iter().zip(embeddings),
    )?;
    if let Some(dimensions) = args.dimensions {
        store.dimension_policy = format!("requested_dimensions={dimensions}");
    } else {
        store.dimension_policy = "provider_default".into();
    }
    let store_bytes = store.canonical_bytes()?;
    let store_entry_count = store.entries.len();
    let store_vector_size = store.vector_size;
    let provider =
        FrozenEmbeddingProvider::from_store(store, args.out.clone(), model, store_vector_size)?;
    let measurements = manifest.validate_store(&provider)?;

    write_store(&args.out, &store_bytes)?;
    println!(
        "wrote {} unique {}-dimension embeddings for model {} to {}",
        store_entry_count,
        store_vector_size,
        model,
        args.out.display()
    );
    print_measurements(&measurements);
    Ok(())
}

fn validate(args: ValidateArgs) -> Result<()> {
    let manifest = FrozenEmbeddingManifest::load(&args.manifest)?;
    let store = FrozenEmbeddingStore::load(&args.store)?;
    let store_entry_count = store.entries.len();
    let store_vector_size = store.vector_size;
    let store_model = store.model.clone();
    let provider = FrozenEmbeddingProvider::from_store(
        store,
        args.store.clone(),
        &store_model,
        store_vector_size,
    )?;
    let measurements = manifest.validate_store(&provider)?;
    println!(
        "validated {} unique {}-dimension embeddings for model {} from {}",
        store_entry_count,
        store_vector_size,
        store_model,
        args.store.display()
    );
    print_measurements(&measurements);
    Ok(())
}

fn write_store(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_replace(path, bytes, "frozen embedding store")
}

fn print_measurements(measurements: &[cmem_eval::FrozenSimilarityMeasurement]) {
    for measurement in measurements {
        println!(
            "similarity description={:?} anchor={} candidate={} cosine={:.9}",
            measurement.description,
            measurement.anchor_id,
            measurement.candidate_id,
            measurement.cosine_similarity
        );
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;
    use cmem_eval::fs_util::{atomic_replace_with_before_persist, persist_with_retry};

    #[test]
    fn failed_store_write_preserves_preexisting_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("store.json");
        let previous_bytes = b"previous complete store\n";
        let replacement_bytes = b"replacement complete store\n";
        fs::write(&path, previous_bytes).unwrap();
        let mut staged_path = None;

        let error = atomic_replace_with_before_persist(
            &path,
            replacement_bytes,
            "frozen embedding store",
            |temporary_path| {
                staged_path = Some(temporary_path.to_path_buf());
                assert_eq!(temporary_path.parent(), path.parent());
                assert_eq!(fs::read(temporary_path).unwrap(), replacement_bytes);
                assert_eq!(fs::read(&path).unwrap(), previous_bytes);
                bail!("simulated failure before atomic store replacement")
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("simulated failure"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), previous_bytes);
        assert!(!staged_path.unwrap().exists());
    }

    #[test]
    fn store_persist_retries_permission_denied_with_same_staged_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("store.json");
        fs::write(&path, b"previous complete store\n").unwrap();
        let staged_bytes = b"replacement complete store\n";
        let mut temporary = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        temporary.write_all(staged_bytes).unwrap();
        temporary.as_file().sync_all().unwrap();
        let mut attempts = 0;

        persist_with_retry(
            temporary,
            &path,
            "frozen embedding store",
            |temporary, path| {
                attempts += 1;
                assert_eq!(fs::read(temporary.path()).unwrap(), staged_bytes);
                if attempts == 1 {
                    return Err(tempfile::PersistError {
                        error: std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "injected Windows replace contention",
                        ),
                        file: temporary,
                    });
                }
                temporary.persist(path)
            },
        )
        .unwrap();

        assert_eq!(attempts, 2);
        assert_eq!(fs::read(&path).unwrap(), staged_bytes);
    }

    #[test]
    fn all_committed_stores_validate_without_network() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cmem-eval-continuity/fixtures/embeddings");
        for pair in ["task21_smoke", "task22_real", "continuity_benchmarks_v1"] {
            validate(ValidateArgs {
                manifest: fixtures.join(format!("{pair}_manifest.json")),
                store: fixtures.join(format!("{pair}_store.json")),
            })
            .unwrap_or_else(|error| panic!("committed frozen pair {pair:?} failed: {error:#}"));
        }
    }
}
