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

    // One batched request embeds each unique exact text exactly once. There is
    // deliberately no retry: an ambiguous network failure must not create
    // untracked duplicate billable calls.
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?
        .post(OPENAI_EMBEDDINGS_ENDPOINT)
        .bearer_auth(api_key)
        .json(&OpenAiEmbeddingRequest {
            model: &args.model,
            input: &unique_texts,
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
    let embeddings = ordered_embeddings(&args.model, unique_texts.len(), response)?;
    let store = FrozenEmbeddingStore::new(
        args.model.clone(),
        FrozenEmbeddingSource::OpenAiApi,
        unique_texts.into_iter().zip(embeddings),
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
        "wrote {} unique {}-dimension embeddings for model {} to {}",
        store.entries.len(),
        store.vector_size,
        store.model,
        args.out.display()
    );
    print_measurements(&measurements);
    Ok(())
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
