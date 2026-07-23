use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use cmem_eval_adapter_cmem::fs_util::atomic_replace;
use cmem_eval_adapter_cmem::openai_embedding::{EmbeddingRetryPolicy, OpenAiEmbeddingClient};
use cmem_eval_core::{
    FrozenEmbeddingDimensionPolicy, FrozenEmbeddingManifest, FrozenEmbeddingProvider,
    FrozenEmbeddingSource, FrozenEmbeddingStore, classify_frozen_embedding_dimensions,
    model_native_embedding_vector_size,
};

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
    /// Permit an explicit non-model-native width for a test-only store.
    #[arg(long)]
    allow_nonstandard_dimensions: bool,
    #[arg(long)]
    out: PathBuf,
    /// Reuse exact-text vectors from an existing OpenAI store and request only
    /// manifest texts that are absent from it.
    #[arg(long)]
    reuse_store: Option<PathBuf>,
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
    let reuse_store = args
        .reuse_store
        .as_deref()
        .map(FrozenEmbeddingStore::load)
        .transpose()?;
    let request_dimensions =
        openai_request_dimensions(model, args.dimensions, reuse_store.as_ref())?;
    let effective_vector_size =
        request_dimensions.unwrap_or(model_native_embedding_vector_size(model)?);
    let dimension_policy: FrozenEmbeddingDimensionPolicy = classify_frozen_embedding_dimensions(
        model,
        effective_vector_size,
        args.allow_nonstandard_dimensions,
    )?;
    let ReusableEmbeddingSelection {
        mut embeddings_by_text,
        missing_texts,
    } = select_reusable_embeddings(
        &unique_texts,
        reuse_store.as_ref(),
        model,
        effective_vector_size,
    )?;
    if missing_texts.len() > MAX_EMBEDDING_INPUTS_PER_REQUEST {
        bail!(
            "embedding manifest has {} uncached unique texts; the OpenAI embeddings request accepts at most {MAX_EMBEDDING_INPUTS_PER_REQUEST} inputs",
            missing_texts.len()
        );
    }
    let reused_count = embeddings_by_text.len();
    if !missing_texts.is_empty() {
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

        // One batched request embeds each missing exact text exactly once.
        // There is deliberately no retry: an ambiguous network failure must
        // not create untracked duplicate billable calls.
        let embeddings = OpenAiEmbeddingClient::default()
            .embed_batch(
                &api_key,
                model,
                &missing_texts,
                request_dimensions,
                EmbeddingRetryPolicy::no_retry(),
            )
            .await
            .context("request offline OpenAI embeddings")?;
        for (index, embedding) in embeddings.iter().enumerate() {
            if embedding.len() != effective_vector_size {
                bail!(
                    "OpenAI embedding response index {index} has vector_size {}, expected requested width {effective_vector_size}",
                    embedding.len()
                );
            }
        }
        embeddings_by_text.extend(missing_texts.into_iter().zip(embeddings));
    }
    let store = FrozenEmbeddingStore::new_with_dimension_policy(
        model,
        FrozenEmbeddingSource::OpenAiApi,
        dimension_policy,
        unique_texts.into_iter().map(|text| {
            let embedding = embeddings_by_text
                .remove(&text)
                .expect("reuse selection and generation cover every manifest text");
            (text, embedding)
        }),
    )?;
    let store_bytes = store.canonical_bytes()?;
    let store_entry_count = store.entries.len();
    let store_vector_size = store.vector_size;
    let provider =
        FrozenEmbeddingProvider::from_store(store, args.out.clone(), model, store_vector_size)?;
    let measurements = manifest.validate_store(&provider)?;

    write_store(&args.out, &store_bytes)?;
    println!(
        "wrote {} unique {}-dimension embeddings for model {} to {} with dimension_policy={:?} (reused {}, generated {})",
        store_entry_count,
        store_vector_size,
        model,
        args.out.display(),
        dimension_policy,
        reused_count,
        store_entry_count - reused_count
    );
    print_measurements(&measurements);
    Ok(())
}

#[derive(Debug)]
struct ReusableEmbeddingSelection {
    embeddings_by_text: BTreeMap<String, Vec<f32>>,
    missing_texts: Vec<String>,
}

fn openai_request_dimensions(
    model: &str,
    requested_dimensions: Option<usize>,
    reuse_store: Option<&FrozenEmbeddingStore>,
) -> Result<Option<usize>> {
    model_native_embedding_vector_size(model)?;
    Ok(requested_dimensions.or_else(|| reuse_store.map(|store| store.vector_size)))
}

fn select_reusable_embeddings(
    unique_texts: &[String],
    reuse_store: Option<&FrozenEmbeddingStore>,
    requested_model: &str,
    effective_vector_size: usize,
) -> Result<ReusableEmbeddingSelection> {
    let Some(reuse_store) = reuse_store else {
        return Ok(ReusableEmbeddingSelection {
            embeddings_by_text: BTreeMap::new(),
            missing_texts: unique_texts.to_vec(),
        });
    };
    if reuse_store.source != FrozenEmbeddingSource::OpenAiApi {
        bail!(
            "--reuse-store requires source=open_ai_api; found {:?}",
            reuse_store.source
        );
    }
    if reuse_store.model != requested_model {
        bail!(
            "--reuse-store model {:?} does not match requested model {requested_model:?}",
            reuse_store.model
        );
    }
    if reuse_store.vector_size != effective_vector_size {
        bail!(
            "--reuse-store vector_size {} does not match effective embedding width {effective_vector_size}",
            reuse_store.vector_size,
        );
    }

    let cached = reuse_store
        .entries
        .iter()
        .map(|entry| (entry.text.as_str(), entry.embedding.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let mut reused = BTreeMap::new();
    let mut missing = Vec::new();
    for text in unique_texts {
        if let Some(embedding) = cached.get(text.as_str()) {
            reused.insert(text.clone(), embedding.to_vec());
        } else {
            missing.push(text.clone());
        }
    }
    Ok(ReusableEmbeddingSelection {
        embeddings_by_text: reused,
        missing_texts: missing,
    })
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

fn print_measurements(measurements: &[cmem_eval_core::FrozenSimilarityMeasurement]) {
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
    use cmem_eval_adapter_cmem::fs_util::{atomic_replace_with_before_persist, persist_with_retry};

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
    fn reuse_selects_exact_manifest_keys_and_drops_unused_entries() {
        let store = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            [
                ("kept".to_string(), vec![1.0, 0.0]),
                ("unused".to_string(), vec![0.0, 1.0]),
            ],
        )
        .unwrap();
        let ReusableEmbeddingSelection {
            embeddings_by_text: reused,
            missing_texts: missing,
        } = select_reusable_embeddings(
            &["kept".to_string(), "new".to_string()],
            Some(&store),
            "text-embedding-3-large",
            2,
        )
        .unwrap();

        assert_eq!(
            reused,
            BTreeMap::from([("kept".to_string(), vec![1.0, 0.0])])
        );
        assert_eq!(missing, ["new"]);
        assert!(!reused.contains_key("unused"));
    }

    #[test]
    fn reduced_width_reuse_with_a_miss_inherits_width_before_generation() {
        let store = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            [("kept".to_string(), vec![1.0, 0.0, 0.0])],
        )
        .unwrap();
        let unique_texts = vec!["kept".to_string(), "new".to_string()];
        let request_dimensions =
            openai_request_dimensions("text-embedding-3-large", None, Some(&store)).unwrap();
        let effective_vector_size = request_dimensions
            .unwrap_or(model_native_embedding_vector_size("text-embedding-3-large").unwrap());
        let ReusableEmbeddingSelection {
            mut embeddings_by_text,
            missing_texts,
        } = select_reusable_embeddings(
            &unique_texts,
            Some(&store),
            "text-embedding-3-large",
            effective_vector_size,
        )
        .unwrap();

        assert_eq!(request_dimensions, Some(3));
        assert_eq!(missing_texts, ["new"]);

        // This represents the response shape requested from OpenAI. Combining it
        // with the reused vector must produce a valid store without reaching the
        // former post-request mixed-width failure.
        embeddings_by_text.insert("new".to_string(), vec![0.0, 1.0, 0.0]);
        let output = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            unique_texts.into_iter().map(|text| {
                let embedding = embeddings_by_text.remove(&text).unwrap();
                (text, embedding)
            }),
        )
        .unwrap();
        assert_eq!(output.vector_size, 3);
        assert!(
            output
                .entries
                .iter()
                .all(|entry| entry.embedding.len() == 3)
        );
    }

    #[test]
    fn explicit_dimensions_conflicting_with_reuse_fail_before_generation() {
        let store = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            [("kept".to_string(), vec![1.0, 0.0, 0.0])],
        )
        .unwrap();
        let requested_dimensions =
            openai_request_dimensions("text-embedding-3-large", Some(2), Some(&store)).unwrap();
        let effective_vector_size = requested_dimensions
            .unwrap_or(model_native_embedding_vector_size("text-embedding-3-large").unwrap());

        let error = select_reusable_embeddings(
            &["kept".to_string(), "new".to_string()],
            Some(&store),
            "text-embedding-3-large",
            effective_vector_size,
        )
        .unwrap_err()
        .to_string();

        for token in ["vector_size", "3", "2"] {
            assert!(error.contains(token), "missing {token:?} in {error}");
        }
    }

    #[tokio::test]
    async fn nonstandard_dimensions_require_opt_in_and_persist_the_choice() {
        let directory = tempfile::tempdir().unwrap();
        let manifest_path = directory.path().join("manifest.json");
        let reuse_path = directory.path().join("reuse.json");
        let output_path = directory.path().join("output.json");
        let manifest = FrozenEmbeddingManifest {
            schema_version: cmem_eval_core::FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION,
            texts: vec![cmem_eval_core::FrozenEmbeddingText {
                id: "kept".to_string(),
                text: "kept".to_string(),
            }],
            similarity_orderings: Vec::new(),
        };
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let reuse = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            [("kept".to_string(), vec![1.0, 0.0])],
        )
        .unwrap();
        fs::write(&reuse_path, reuse.canonical_bytes().unwrap()).unwrap();

        let args = |allow_nonstandard_dimensions| GenerateArgs {
            manifest: manifest_path.clone(),
            model: "text-embedding-3-large".to_string(),
            dimensions: Some(2),
            allow_nonstandard_dimensions,
            out: output_path.clone(),
            reuse_store: Some(reuse_path.clone()),
            api_key_env: "CMEM_EVAL_UNUSED_OPENAI_KEY".to_string(),
        };
        let error = generate(args(false)).await.unwrap_err().to_string();
        for token in [
            "text-embedding-3-large",
            "2",
            "3072",
            "--allow-nonstandard-dimensions",
        ] {
            assert!(error.contains(token), "missing {token:?} in {error}");
        }
        assert!(!output_path.exists());

        generate(args(true)).await.unwrap();
        let output = FrozenEmbeddingStore::load(&output_path).unwrap();
        assert_eq!(output.vector_size, 2);
        assert_eq!(
            output.dimension_policy,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard
        );
    }

    #[tokio::test]
    async fn padded_model_is_normalized_before_reuse_and_store_serialization() {
        let directory = tempfile::tempdir().unwrap();
        let manifest_path = directory.path().join("manifest.json");
        let reuse_path = directory.path().join("reuse.json");
        let output_path = directory.path().join("output.json");
        let manifest = FrozenEmbeddingManifest {
            schema_version: cmem_eval_core::FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION,
            texts: vec![cmem_eval_core::FrozenEmbeddingText {
                id: "kept".to_string(),
                text: "kept".to_string(),
            }],
            similarity_orderings: Vec::new(),
        };
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let reuse = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            [("kept".to_string(), vec![1.0, 0.0])],
        )
        .unwrap();
        fs::write(&reuse_path, reuse.canonical_bytes().unwrap()).unwrap();

        generate(GenerateArgs {
            manifest: manifest_path,
            model: "  text-embedding-3-large\t".to_string(),
            dimensions: Some(2),
            allow_nonstandard_dimensions: true,
            out: output_path.clone(),
            reuse_store: Some(reuse_path),
            api_key_env: "CMEM_EVAL_UNUSED_OPENAI_KEY".to_string(),
        })
        .await
        .unwrap();

        let serialized: serde_json::Value =
            serde_json::from_slice(&fs::read(output_path).unwrap()).unwrap();
        assert_eq!(serialized["model"], "text-embedding-3-large");
    }

    #[test]
    fn all_committed_stores_validate_without_a_mock_or_network() {
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
