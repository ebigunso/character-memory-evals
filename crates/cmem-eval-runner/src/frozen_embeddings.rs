use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use cmem_eval_core::{
    FrozenEmbeddingManifest, FrozenEmbeddingProvider, FrozenEmbeddingSource, FrozenEmbeddingStore,
};
use serde::{Deserialize, Serialize};

const OPENAI_EMBEDDINGS_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";
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
    if args.model.trim().is_empty() {
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
    let ReusableEmbeddingSelection {
        mut embeddings_by_text,
        missing_texts,
    } = select_reusable_embeddings(
        &unique_texts,
        reuse_store.as_ref(),
        &args.model,
        args.dimensions,
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
        let response = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?
            .post(OPENAI_EMBEDDINGS_ENDPOINT)
            .bearer_auth(api_key)
            .json(&OpenAiEmbeddingRequest {
                model: &args.model,
                input: &missing_texts,
                encoding_format: "float",
                dimensions: args.dimensions,
            })
            .send()
            .await
            .context("request offline OpenAI embeddings")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("offline OpenAI embedding request failed with {status}: {body}");
        }
        let response: OpenAiEmbeddingResponse = response
            .json()
            .await
            .context("parse offline OpenAI embedding response")?;
        let embeddings = ordered_embeddings(&args.model, missing_texts.len(), response)?;
        embeddings_by_text.extend(missing_texts.into_iter().zip(embeddings));
    }
    let store = FrozenEmbeddingStore::new(
        args.model.clone(),
        FrozenEmbeddingSource::OpenAiApi,
        unique_texts.into_iter().map(|text| {
            let embedding = embeddings_by_text
                .remove(&text)
                .expect("reuse selection and generation cover every manifest text");
            (text, embedding)
        }),
    )?;
    let provider = FrozenEmbeddingProvider::from_store(
        store.clone(),
        args.out.clone(),
        &store.model,
        store.vector_size,
    )?;
    let measurements = manifest.validate_store(&provider)?;

    write_store(&args.out, &store)?;
    println!(
        "wrote {} unique {}-dimension embeddings for model {} to {} (reused {}, generated {})",
        store.entries.len(),
        store.vector_size,
        store.model,
        args.out.display(),
        reused_count,
        store.entries.len() - reused_count
    );
    print_measurements(&measurements);
    Ok(())
}

struct ReusableEmbeddingSelection {
    embeddings_by_text: BTreeMap<String, Vec<f32>>,
    missing_texts: Vec<String>,
}

fn select_reusable_embeddings(
    unique_texts: &[String],
    reuse_store: Option<&FrozenEmbeddingStore>,
    requested_model: &str,
    requested_dimensions: Option<usize>,
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
    if requested_dimensions.is_some_and(|dimensions| dimensions != reuse_store.vector_size) {
        bail!(
            "--reuse-store vector_size {} does not match requested dimensions {}",
            reuse_store.vector_size,
            requested_dimensions.expect("checked as some")
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
    let provider = FrozenEmbeddingProvider::from_store(
        store.clone(),
        args.store.clone(),
        &store.model,
        store.vector_size,
    )?;
    let measurements = manifest.validate_store(&provider)?;
    println!(
        "validated {} unique {}-dimension embeddings for model {} from {}",
        store.entries.len(),
        store.vector_size,
        store.model,
        args.store.display()
    );
    print_measurements(&measurements);
    Ok(())
}

fn write_store(path: &Path, store: &FrozenEmbeddingStore) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create frozen embedding directory {}", parent.display()))?;
    }
    fs::write(path, store.canonical_bytes()?)
        .with_context(|| format!("write frozen embedding store {}", path.display()))
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

#[derive(Debug, Serialize)]
struct OpenAiEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
    encoding_format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    model: String,
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

fn ordered_embeddings(
    requested_model: &str,
    expected_count: usize,
    response: OpenAiEmbeddingResponse,
) -> Result<Vec<Vec<f32>>> {
    if response.model != requested_model {
        bail!(
            "OpenAI embedding response model {:?} does not match requested model {requested_model:?}",
            response.model
        );
    }
    if response.data.len() != expected_count {
        bail!(
            "OpenAI embedding response returned {} vectors for {expected_count} unique texts",
            response.data.len()
        );
    }
    let mut ordered = vec![None; expected_count];
    for item in response.data {
        if item.index >= expected_count {
            bail!(
                "OpenAI embedding response index {} is outside expected range 0..{expected_count}",
                item.index
            );
        }
        let index = item.index;
        if ordered[index].replace(item.embedding).is_some() {
            bail!("OpenAI embedding response contains duplicate index {index}");
        }
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| {
            embedding.with_context(|| format!("OpenAI embedding response omitted index {index}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_selects_exact_manifest_keys_and_drops_unused_entries() {
        let store = FrozenEmbeddingStore::new(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
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
            Some(2),
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
    fn response_order_is_index_driven_and_duplicate_indices_fail() {
        let ordered = ordered_embeddings(
            "text-embedding-3-large",
            2,
            OpenAiEmbeddingResponse {
                model: "text-embedding-3-large".to_string(),
                data: vec![
                    OpenAiEmbeddingData {
                        index: 1,
                        embedding: vec![0.0, 1.0],
                    },
                    OpenAiEmbeddingData {
                        index: 0,
                        embedding: vec![1.0, 0.0],
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(ordered, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);

        let error = ordered_embeddings(
            "text-embedding-3-large",
            2,
            OpenAiEmbeddingResponse {
                model: "text-embedding-3-large".to_string(),
                data: vec![
                    OpenAiEmbeddingData {
                        index: 0,
                        embedding: vec![1.0, 0.0],
                    },
                    OpenAiEmbeddingData {
                        index: 0,
                        embedding: vec![0.0, 1.0],
                    },
                ],
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("duplicate index 0"), "{error}");
    }

    #[test]
    fn committed_smoke_store_validates_without_a_mock_or_network() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cmem-eval-continuity/fixtures/embeddings");
        validate(ValidateArgs {
            manifest: fixtures.join("task21_smoke_manifest.json"),
            store: fixtures.join("task21_smoke_store.json"),
        })
        .unwrap();
    }
}
