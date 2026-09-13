use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FROZEN_EMBEDDING_STORE_SCHEMA_VERSION: u32 = 2;
pub const FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrozenEmbeddingSource {
    OpenAiApi,
    TestFixture,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrozenEmbeddingDimensionPolicy {
    ModelNative,
    ExplicitNonstandard,
    TestFixture,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrozenEmbeddingStore {
    pub schema_version: u32,
    pub model: String,
    pub vector_size: usize,
    pub dimension_policy: FrozenEmbeddingDimensionPolicy,
    pub source: FrozenEmbeddingSource,
    pub entries: Vec<FrozenEmbeddingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrozenEmbeddingEntry {
    pub text_sha256: String,
    pub text: String,
    pub embedding: Vec<f32>,
}

impl FrozenEmbeddingStore {
    pub fn new(
        model: impl Into<String>,
        source: FrozenEmbeddingSource,
        embeddings: impl IntoIterator<Item = (String, Vec<f32>)>,
    ) -> Result<Self> {
        let dimension_policy = match source {
            FrozenEmbeddingSource::OpenAiApi => FrozenEmbeddingDimensionPolicy::ModelNative,
            FrozenEmbeddingSource::TestFixture => FrozenEmbeddingDimensionPolicy::TestFixture,
        };
        Self::new_with_dimension_policy(model, source, dimension_policy, embeddings)
    }

    pub fn new_with_dimension_policy(
        model: impl Into<String>,
        source: FrozenEmbeddingSource,
        dimension_policy: FrozenEmbeddingDimensionPolicy,
        embeddings: impl IntoIterator<Item = (String, Vec<f32>)>,
    ) -> Result<Self> {
        let model = model.into();
        let mut entries = embeddings
            .into_iter()
            .map(|(text, embedding)| FrozenEmbeddingEntry {
                text_sha256: text_sha256(&text),
                text,
                embedding,
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.text_sha256.cmp(&right.text_sha256));
        let vector_size = entries
            .first()
            .map(|entry| entry.embedding.len())
            .unwrap_or_default();
        let store = Self {
            schema_version: FROZEN_EMBEDDING_STORE_SCHEMA_VERSION,
            model,
            vector_size,
            dimension_policy,
            source,
            entries,
        };
        store.validate()?;
        Ok(store)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("read frozen embedding store {}", path.display()))?;
        let store: Self = serde_json::from_slice(&bytes).map_err(|error| {
            anyhow::anyhow!("parse frozen embedding store {}: {error}", path.display())
        })?;
        store.validate().map_err(|error| {
            anyhow::anyhow!(
                "validate frozen embedding store {}: {error}",
                path.display()
            )
        })?;
        Ok(store)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != FROZEN_EMBEDDING_STORE_SCHEMA_VERSION {
            bail!(
                "unsupported frozen embedding store schema_version {}; expected {}",
                self.schema_version,
                FROZEN_EMBEDDING_STORE_SCHEMA_VERSION
            );
        }
        if self.model.trim().is_empty() {
            bail!("frozen embedding store model must not be empty");
        }
        if self.vector_size == 0 {
            bail!("frozen embedding store vector_size must be greater than zero");
        }
        match (self.source, self.dimension_policy) {
            (
                FrozenEmbeddingSource::OpenAiApi,
                FrozenEmbeddingDimensionPolicy::ModelNative
                | FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            ) => {
                let expected_policy = classify_frozen_embedding_dimensions(
                    &self.model,
                    self.vector_size,
                    self.dimension_policy == FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
                )?;
                if self.dimension_policy != expected_policy {
                    bail!(
                        "frozen embedding store dimension_policy={:?} is inconsistent with model {:?} and vector_size {}",
                        self.dimension_policy,
                        self.model,
                        self.vector_size
                    );
                }
            }
            (FrozenEmbeddingSource::TestFixture, FrozenEmbeddingDimensionPolicy::TestFixture) => {}
            (source, policy) => {
                bail!(
                    "frozen embedding store source={source:?} is incompatible with dimension_policy={policy:?}"
                );
            }
        }
        if self.entries.is_empty() {
            bail!("frozen embedding store must contain at least one entry");
        }

        let mut previous_hash: Option<&str> = None;
        let mut texts = BTreeSet::new();
        for entry in &self.entries {
            if entry.text.is_empty() {
                bail!("frozen embedding store entries must not contain empty text");
            }
            let actual_hash = text_sha256(&entry.text);
            if entry.text_sha256 != actual_hash {
                bail!(
                    "frozen embedding entry hash mismatch for text {:?}: recorded {}, computed {actual_hash}",
                    entry.text,
                    entry.text_sha256
                );
            }
            if previous_hash.is_some_and(|previous| previous >= entry.text_sha256.as_str()) {
                bail!("frozen embedding store entries must be uniquely sorted by text_sha256");
            }
            previous_hash = Some(&entry.text_sha256);
            if !texts.insert(entry.text.as_str()) {
                bail!("frozen embedding store contains duplicate exact text bytes");
            }
            if entry.embedding.len() != self.vector_size {
                bail!(
                    "frozen embedding entry {} has vector size {}, expected {}",
                    entry.text_sha256,
                    entry.embedding.len(),
                    self.vector_size
                );
            }
            if entry.embedding.iter().any(|value| !value.is_finite()) {
                bail!(
                    "frozen embedding entry {} contains a non-finite vector component",
                    entry.text_sha256
                );
            }
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

pub fn model_native_embedding_vector_size(model: &str) -> Result<usize> {
    match model.trim() {
        "text-embedding-3-small" => Ok(1536),
        "text-embedding-3-large" => Ok(3072),
        _ => bail!(
            "embedding model {model:?} has no known canonical width; expected text-embedding-3-small or text-embedding-3-large"
        ),
    }
}

pub fn classify_frozen_embedding_dimensions(
    model: &str,
    vector_size: usize,
    allow_nonstandard_dimensions: bool,
) -> Result<FrozenEmbeddingDimensionPolicy> {
    let canonical_width = model_native_embedding_vector_size(model)?;
    if vector_size == canonical_width {
        return Ok(FrozenEmbeddingDimensionPolicy::ModelNative);
    }
    if allow_nonstandard_dimensions {
        return Ok(FrozenEmbeddingDimensionPolicy::ExplicitNonstandard);
    }
    bail!(
        "embedding vector_size {vector_size} for model {model:?} differs from canonical width {canonical_width}; pass --allow-nonstandard-dimensions only to generate an explicit nonstandard test fixture; live Character Memory continuity requires canonical width {canonical_width}"
    )
}

#[derive(Debug, Clone)]
pub struct FrozenEmbeddingProvider {
    inner: Arc<FrozenEmbeddingProviderInner>,
}

#[derive(Debug)]
struct FrozenEmbeddingProviderInner {
    store: FrozenEmbeddingStore,
    entries_by_hash: BTreeMap<String, usize>,
    store_path: PathBuf,
}

impl FrozenEmbeddingProvider {
    pub fn load(path: &Path, expected_model: &str, expected_vector_size: usize) -> Result<Self> {
        let store = FrozenEmbeddingStore::load(path)?;
        Self::from_store(store, path, expected_model, expected_vector_size)
    }

    pub fn from_store(
        store: FrozenEmbeddingStore,
        store_path: impl Into<PathBuf>,
        expected_model: &str,
        expected_vector_size: usize,
    ) -> Result<Self> {
        store.validate()?;
        if store.model != expected_model {
            bail!(
                "frozen embedding store model {:?} does not match configured model {expected_model:?}",
                store.model
            );
        }
        if store.vector_size != expected_vector_size {
            bail!(
                "frozen embedding store vector_size {} does not match configured vector_size {expected_vector_size}",
                store.vector_size
            );
        }
        let entries_by_hash = store
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.text_sha256.clone(), index))
            .collect();
        Ok(Self {
            inner: Arc::new(FrozenEmbeddingProviderInner {
                store,
                entries_by_hash,
                store_path: store_path.into(),
            }),
        })
    }

    pub fn model(&self) -> &str {
        &self.inner.store.model
    }

    pub fn vector_size(&self) -> usize {
        self.inner.store.vector_size
    }

    pub fn source(&self) -> FrozenEmbeddingSource {
        self.inner.store.source
    }

    pub fn dimension_policy(&self) -> FrozenEmbeddingDimensionPolicy {
        self.inner.store.dimension_policy
    }

    pub fn store_sha256(&self) -> Result<String> {
        let bytes = self.inner.store.canonical_bytes()?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn vector_for_text(&self, text: &str) -> Result<Vec<f32>> {
        let hash = text_sha256(text);
        let Some(index) = self.inner.entries_by_hash.get(&hash) else {
            bail!(
                "frozen embedding cache miss for model {:?}, exact-text SHA-256 {hash}, store {}; regenerate the store with `cmem-eval embeddings generate --manifest <manifest> --model {} --out {}`",
                self.model(),
                self.inner.store_path.display(),
                self.model(),
                self.inner.store_path.display()
            );
        };
        let entry = &self.inner.store.entries[*index];
        if entry.text.as_bytes() != text.as_bytes() {
            bail!(
                "frozen embedding SHA-256 collision for model {:?} and digest {hash}; refusing to return a vector",
                self.model()
            );
        }
        Ok(entry.embedding.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrozenEmbeddingManifest {
    pub schema_version: u32,
    pub texts: Vec<FrozenEmbeddingText>,
    #[serde(default)]
    pub similarity_orderings: Vec<FrozenSimilarityOrdering>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrozenEmbeddingText {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrozenSimilarityOrdering {
    pub description: String,
    pub anchor_id: String,
    pub descending_ids: Vec<String>,
    #[serde(default)]
    pub min_margin: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrozenSimilarityMeasurement {
    pub description: String,
    pub anchor_id: String,
    pub candidate_id: String,
    pub cosine_similarity: f32,
}

impl FrozenEmbeddingManifest {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("read frozen embedding manifest {}", path.display()))?;
        let manifest: Self = serde_json::from_slice(&bytes).map_err(|error| {
            anyhow::anyhow!(
                "parse frozen embedding manifest {}: {error}",
                path.display()
            )
        })?;
        manifest.validate().map_err(|error| {
            anyhow::anyhow!(
                "validate frozen embedding manifest {}: {error}",
                path.display()
            )
        })?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION {
            bail!(
                "unsupported frozen embedding manifest schema_version {}; expected {}",
                self.schema_version,
                FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION
            );
        }
        if self.texts.is_empty() {
            bail!("frozen embedding manifest must contain at least one text");
        }
        let mut ids = BTreeSet::new();
        for text in &self.texts {
            if text.id.trim().is_empty() {
                bail!("frozen embedding manifest text IDs must not be empty");
            }
            if text.text.is_empty() {
                bail!(
                    "frozen embedding manifest text {:?} must not be empty",
                    text.id
                );
            }
            if !ids.insert(text.id.as_str()) {
                bail!("duplicate frozen embedding manifest text ID {:?}", text.id);
            }
        }
        for ordering in &self.similarity_orderings {
            if ordering.description.trim().is_empty() {
                bail!("frozen embedding similarity ordering description must not be empty");
            }
            if !ids.contains(ordering.anchor_id.as_str()) {
                bail!(
                    "similarity ordering {:?} references unknown anchor ID {:?}",
                    ordering.description,
                    ordering.anchor_id
                );
            }
            if ordering.descending_ids.len() < 2 {
                bail!(
                    "similarity ordering {:?} must contain at least two descending IDs",
                    ordering.description
                );
            }
            if !ordering.min_margin.is_finite() || ordering.min_margin < 0.0 {
                bail!(
                    "similarity ordering {:?} min_margin must be finite and non-negative",
                    ordering.description
                );
            }
            let mut candidates = BTreeSet::new();
            for candidate_id in &ordering.descending_ids {
                if !ids.contains(candidate_id.as_str()) {
                    bail!(
                        "similarity ordering {:?} references unknown candidate ID {candidate_id:?}",
                        ordering.description
                    );
                }
                if candidate_id == &ordering.anchor_id {
                    bail!(
                        "similarity ordering {:?} must not rank its anchor as a candidate",
                        ordering.description
                    );
                }
                if !candidates.insert(candidate_id.as_str()) {
                    bail!(
                        "similarity ordering {:?} contains duplicate candidate ID {candidate_id:?}",
                        ordering.description
                    );
                }
            }
        }
        Ok(())
    }

    pub fn unique_texts(&self) -> Result<Vec<String>> {
        self.validate()?;
        let mut by_hash = BTreeMap::<String, String>::new();
        for item in &self.texts {
            let hash = text_sha256(&item.text);
            if let Some(existing) = by_hash.get(&hash)
                && existing.as_bytes() != item.text.as_bytes()
            {
                bail!("SHA-256 collision while deduplicating embedding manifest texts");
            }
            by_hash.entry(hash).or_insert_with(|| item.text.clone());
        }
        Ok(by_hash.into_values().collect())
    }

    pub fn validate_store(
        &self,
        provider: &FrozenEmbeddingProvider,
    ) -> Result<Vec<FrozenSimilarityMeasurement>> {
        self.validate()?;
        let manifest_hashes = self
            .unique_texts()?
            .into_iter()
            .map(|text| text_sha256(&text))
            .collect::<BTreeSet<_>>();
        let store_hashes = provider
            .inner
            .store
            .entries
            .iter()
            .map(|entry| entry.text_sha256.clone())
            .collect::<BTreeSet<_>>();
        let missing = manifest_hashes
            .difference(&store_hashes)
            .map(String::as_str)
            .collect::<Vec<_>>();
        let extras = store_hashes
            .difference(&manifest_hashes)
            .map(String::as_str)
            .collect::<Vec<_>>();
        if !missing.is_empty() || !extras.is_empty() {
            bail!(
                "frozen embedding manifest/store must be a strict bijection: missing manifest entry count: {} (first SHA-256 keys: [{}]); extra store entry count: {} (first SHA-256 keys: [{}])",
                missing.len(),
                missing
                    .iter()
                    .take(5)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", "),
                extras.len(),
                extras
                    .iter()
                    .take(5)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        let texts_by_id = self
            .texts
            .iter()
            .map(|item| (item.id.as_str(), item.text.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut vectors = BTreeMap::new();
        for item in &self.texts {
            vectors.insert(item.id.as_str(), provider.vector_for_text(&item.text)?);
        }

        let mut measurements = Vec::new();
        for ordering in &self.similarity_orderings {
            let anchor = vectors
                .get(ordering.anchor_id.as_str())
                .expect("manifest validation established the anchor ID");
            let mut scores = Vec::with_capacity(ordering.descending_ids.len());
            for candidate_id in &ordering.descending_ids {
                let candidate = vectors
                    .get(candidate_id.as_str())
                    .expect("manifest validation established the candidate ID");
                let score = cosine_similarity(anchor, candidate).with_context(|| {
                    format!(
                        "measure similarity ordering {:?}: anchor {:?}, candidate {:?}",
                        ordering.description, ordering.anchor_id, candidate_id
                    )
                })?;
                scores.push((candidate_id.as_str(), score));
                measurements.push(FrozenSimilarityMeasurement {
                    description: ordering.description.clone(),
                    anchor_id: ordering.anchor_id.clone(),
                    candidate_id: candidate_id.clone(),
                    cosine_similarity: score,
                });
            }
            for pair in scores.windows(2) {
                let (nearer_id, nearer) = pair[0];
                let (farther_id, farther) = pair[1];
                if nearer <= farther + ordering.min_margin {
                    bail!(
                        "similarity ordering {:?} failed: {:?} cosine {nearer} must exceed {:?} cosine {farther} by more than min_margin {}; anchor text {:?}",
                        ordering.description,
                        nearer_id,
                        farther_id,
                        ordering.min_margin,
                        texts_by_id[ordering.anchor_id.as_str()]
                    );
                }
            }
        }
        Ok(measurements)
    }
}

pub fn text_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32> {
    if left.len() != right.len() {
        bail!(
            "cannot measure cosine similarity for vector sizes {} and {}",
            left.len(),
            right.len()
        );
    }
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        bail!("cannot measure cosine similarity for a zero-magnitude vector");
    }
    let similarity = dot / (left_norm * right_norm);
    if !similarity.is_finite() {
        bail!("cosine similarity produced a non-finite value");
    }
    Ok(similarity)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::process::Command;

    use super::*;

    const PROCESS_STORE_PATH: &str = "CMEM_FROZEN_EMBEDDING_PROCESS_STORE";
    const PROCESS_OUTPUT_PATH: &str = "CMEM_FROZEN_EMBEDDING_PROCESS_OUTPUT";

    #[test]
    fn canonical_store_is_hash_sorted_and_lf_terminated() {
        let store = smoke_store();
        let bytes = store.canonical_bytes().unwrap();
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert!(!bytes.windows(2).any(|window| window == b"\r\n"));
        assert!(
            store
                .entries
                .windows(2)
                .all(|pair| pair[0].text_sha256 < pair[1].text_sha256)
        );
    }

    #[test]
    fn openai_store_dimension_policy_is_explicit_and_self_consistent() {
        let native = FrozenEmbeddingStore::new(
            "text-embedding-3-small",
            FrozenEmbeddingSource::OpenAiApi,
            [("native".to_string(), vec![0.0; 1_536])],
        )
        .unwrap();
        assert_eq!(
            native.dimension_policy,
            FrozenEmbeddingDimensionPolicy::ModelNative
        );

        let error = FrozenEmbeddingStore::new(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            [("reduced".to_string(), vec![0.0; 1_024])],
        )
        .unwrap_err()
        .to_string();
        for token in [
            "text-embedding-3-large",
            "1024",
            "3072",
            "--allow-nonstandard-dimensions",
        ] {
            assert!(error.contains(token), "missing {token:?} in {error}");
        }

        let reduced = FrozenEmbeddingStore::new_with_dimension_policy(
            "text-embedding-3-large",
            FrozenEmbeddingSource::OpenAiApi,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard,
            [("reduced".to_string(), vec![0.0; 1_024])],
        )
        .unwrap();
        assert_eq!(
            reduced.dimension_policy,
            FrozenEmbeddingDimensionPolicy::ExplicitNonstandard
        );
        assert!(
            String::from_utf8(reduced.canonical_bytes().unwrap())
                .unwrap()
                .contains("\"dimension_policy\": \"explicit_nonstandard\"")
        );
    }

    #[test]
    fn cache_miss_fails_loudly_with_regeneration_command() {
        let provider = FrozenEmbeddingProvider::from_store(
            smoke_store(),
            "fixtures/smoke.json",
            "task21-smoke-model",
            3,
        )
        .unwrap();
        let error = provider
            .vector_for_text("not in the store")
            .unwrap_err()
            .to_string();
        assert!(error.contains("frozen embedding cache miss"), "{error}");
        assert!(error.contains(&text_sha256("not in the store")), "{error}");
        assert!(error.contains("cmem-eval embeddings generate"), "{error}");
        assert!(error.contains("fixtures/smoke.json"), "{error}");
    }

    #[test]
    fn semantic_ordering_requires_target_then_near_miss_then_background() {
        let provider = FrozenEmbeddingProvider::from_store(
            smoke_store(),
            "fixtures/smoke.json",
            "task21-smoke-model",
            3,
        )
        .unwrap();
        let manifest = smoke_manifest();
        let measurements = manifest.validate_store(&provider).unwrap();
        assert_eq!(measurements.len(), 3);
        assert!(measurements[0].cosine_similarity > measurements[1].cosine_similarity);
        assert!(measurements[1].cosine_similarity > measurements[2].cosine_similarity);
    }

    #[test]
    fn manifest_validation_rejects_store_supersets() {
        let extra_text = "This stale vector is not part of the runtime lookup set.";
        let store = FrozenEmbeddingStore::new(
            "task21-smoke-model",
            FrozenEmbeddingSource::TestFixture,
            smoke_store()
                .entries
                .into_iter()
                .map(|entry| (entry.text, entry.embedding))
                .chain([(extra_text.to_string(), vec![0.0, 1.0, 0.0])]),
        )
        .unwrap();
        let provider = FrozenEmbeddingProvider::from_store(
            store,
            "fixtures/superset.json",
            "task21-smoke-model",
            3,
        )
        .unwrap();

        let error = smoke_manifest()
            .validate_store(&provider)
            .unwrap_err()
            .to_string();

        assert!(error.contains("strict bijection"), "{error}");
        assert!(error.contains("extra store entry count: 1"), "{error}");
        assert!(error.contains(&text_sha256(extra_text)), "{error}");
    }

    #[test]
    fn store_loader_rejects_corrupt_and_partial_input() {
        let path = temporary_path("corrupt");
        fs::write(&path, b"{\"schema_version\":1,").unwrap();
        let error = FrozenEmbeddingStore::load(&path).unwrap_err().to_string();
        assert!(error.contains("parse frozen embedding store"), "{error}");

        fs::write(
            &path,
            br#"{"schema_version":1,"model":"task21-smoke-model","vector_size":3}"#,
        )
        .unwrap();
        let error = FrozenEmbeddingStore::load(&path).unwrap_err().to_string();
        assert!(error.contains("parse frozen embedding store"), "{error}");
        assert!(error.contains("missing field"), "{error}");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn store_loader_rejects_wrong_version_with_found_and_expected_values() {
        let path = temporary_path("wrong-version");
        let mut value = serde_json::to_value(smoke_store()).unwrap();
        value["schema_version"] = serde_json::json!(FROZEN_EMBEDDING_STORE_SCHEMA_VERSION + 1);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = FrozenEmbeddingStore::load(&path).unwrap_err().to_string();
        assert!(
            error.contains(&(FROZEN_EMBEDDING_STORE_SCHEMA_VERSION + 1).to_string()),
            "{error}"
        );
        assert!(
            error.contains(&FROZEN_EMBEDDING_STORE_SCHEMA_VERSION.to_string()),
            "{error}"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn store_loader_rejects_malformed_embedding_entries() {
        let path = temporary_path("malformed-entry");
        let mut value = serde_json::to_value(smoke_store()).unwrap();
        value["entries"][0]["text_sha256"] = serde_json::json!("not-the-exact-hash");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = FrozenEmbeddingStore::load(&path).unwrap_err().to_string();
        assert!(error.contains("hash mismatch"), "{error}");

        let mut value = serde_json::to_value(smoke_store()).unwrap();
        value["entries"][0]["unexpected"] = serde_json::json!(true);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = FrozenEmbeddingStore::load(&path).unwrap_err().to_string();
        assert!(error.contains("unknown field"), "{error}");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provider_outputs_are_byte_identical_across_processes() {
        let process_id = std::process::id();
        let store_path = env::temp_dir().join(format!("cmem-frozen-store-{process_id}.json"));
        fs::write(&store_path, smoke_store().canonical_bytes().unwrap()).unwrap();
        let current_exe = env::current_exe().unwrap();
        let mut outputs = Vec::new();
        for run in 0..2 {
            let output_path =
                env::temp_dir().join(format!("cmem-frozen-output-{process_id}-{run}.bin"));
            let status = Command::new(&current_exe)
                .args([
                    "--exact",
                    "frozen_embedding::tests::cross_process_probe",
                    "--nocapture",
                ])
                .env(PROCESS_STORE_PATH, &store_path)
                .env(PROCESS_OUTPUT_PATH, &output_path)
                .status()
                .unwrap();
            assert!(status.success());
            outputs.push(fs::read(&output_path).unwrap());
            fs::remove_file(output_path).unwrap();
        }
        fs::remove_file(store_path).unwrap();

        assert_eq!(outputs[0], outputs[1]);
        assert!(!outputs[0].is_empty());
    }

    #[test]
    fn cross_process_probe() {
        let (Ok(store_path), Ok(output_path)) =
            (env::var(PROCESS_STORE_PATH), env::var(PROCESS_OUTPUT_PATH))
        else {
            return;
        };
        let provider =
            FrozenEmbeddingProvider::load(Path::new(&store_path), "task21-smoke-model", 3).unwrap();
        let bytes = provider
            .vector_for_text("Where is the cobalt notebook?")
            .unwrap()
            .iter()
            .flat_map(|component| component.to_le_bytes())
            .collect::<Vec<_>>();
        fs::write(output_path, bytes).unwrap();
    }

    fn temporary_path(label: &str) -> PathBuf {
        env::temp_dir().join(format!("cmem-frozen-{label}-{}.json", std::process::id()))
    }

    fn smoke_store() -> FrozenEmbeddingStore {
        FrozenEmbeddingStore::new(
            "task21-smoke-model",
            FrozenEmbeddingSource::TestFixture,
            [
                (
                    "Where is the cobalt notebook?".to_string(),
                    vec![1.0, 0.0, 0.0],
                ),
                (
                    "The cobalt notebook is in the east cabinet.".to_string(),
                    vec![0.99, 0.1, 0.0],
                ),
                (
                    "The amber notebook is in the west cabinet.".to_string(),
                    vec![0.75, 0.65, 0.0],
                ),
                (
                    "A heron crossed the lake at dawn.".to_string(),
                    vec![0.0, 0.0, 1.0],
                ),
            ],
        )
        .unwrap()
    }

    fn smoke_manifest() -> FrozenEmbeddingManifest {
        FrozenEmbeddingManifest {
            schema_version: FROZEN_EMBEDDING_MANIFEST_SCHEMA_VERSION,
            texts: vec![
                FrozenEmbeddingText {
                    id: "query".to_string(),
                    text: "Where is the cobalt notebook?".to_string(),
                },
                FrozenEmbeddingText {
                    id: "target".to_string(),
                    text: "The cobalt notebook is in the east cabinet.".to_string(),
                },
                FrozenEmbeddingText {
                    id: "near-miss".to_string(),
                    text: "The amber notebook is in the west cabinet.".to_string(),
                },
                FrozenEmbeddingText {
                    id: "background".to_string(),
                    text: "A heron crossed the lake at dawn.".to_string(),
                },
            ],
            similarity_orderings: vec![FrozenSimilarityOrdering {
                description: "target then same-domain near miss then background".to_string(),
                anchor_id: "query".to_string(),
                descending_ids: vec![
                    "target".to_string(),
                    "near-miss".to_string(),
                    "background".to_string(),
                ],
                min_margin: 0.01,
            }],
        }
    }
}
