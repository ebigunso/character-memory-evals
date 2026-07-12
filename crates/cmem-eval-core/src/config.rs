use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatasetKind {
    LongMemEvalS,
    LoCoMo,
    Synthetic,
    Continuity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationMode {
    RetrievalOnly,
    FixedReader,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    #[default]
    Hybrid,
    Bm25Only,
    VectorOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunConfig {
    pub run_id: String,
    pub dataset: String,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub retrieval: RetrievalConfig,
    #[serde(default)]
    pub ingest: IngestConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl BenchmarkRunConfig {
    pub fn validate(&self) -> Result<()> {
        self.metrics.validate()?;
        self.ingest.validate()?;
        self.retrieval.validate()?;
        self.backend.validate()?;
        Ok(())
    }

    pub fn validate_for_dataset_kind(&self, dataset_kind: DatasetKind) -> Result<()> {
        self.validate()?;
        if dataset_kind == DatasetKind::Continuity
            && !matches!(
                self.backend.embedding.provider.as_str(),
                "deterministic" | "controllable_similarity"
            )
        {
            bail!(
                "continuity dataset requires a deterministic embedding provider; got backend.embedding.provider {:?}",
                self.backend.embedding.provider
            );
        }
        if dataset_kind == DatasetKind::Continuity {
            if self.backend.oxigraph_persistence_path.is_none() {
                bail!(
                    "continuity dataset requires backend.oxigraph_persistence_path for restart durability"
                );
            }
            if self.backend.retrieval_stats_path.is_none() {
                bail!(
                    "continuity dataset requires backend.retrieval_stats_path for restart durability"
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendConfig {
    #[serde(default)]
    pub namespace_prefix: Option<String>,
    #[serde(default)]
    pub qdrant_connection_string: Option<String>,
    #[serde(default)]
    pub oxigraph_connection_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oxigraph_persistence_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_stats_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_registry_dir: Option<String>,
    #[serde(default = "default_openai_api_key_env")]
    pub openai_api_key_env: String,
    #[serde(default)]
    pub reset_namespace_before_each_question: bool,
    #[serde(default)]
    pub reset_namespace_before_each_sample: bool,
    #[serde(default)]
    pub cleanup: CleanupConfig,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            namespace_prefix: None,
            qdrant_connection_string: None,
            oxigraph_connection_string: None,
            oxigraph_persistence_path: None,
            retrieval_stats_path: None,
            identity_registry_dir: None,
            openai_api_key_env: default_openai_api_key_env(),
            reset_namespace_before_each_question: false,
            reset_namespace_before_each_sample: false,
            cleanup: CleanupConfig::default(),
            embedding: EmbeddingConfig::default(),
        }
    }
}

impl BackendConfig {
    pub fn validate(&self) -> Result<()> {
        self.cleanup.validate()?;
        if self.embedding.vector_size == Some(0) {
            bail!("backend.embedding.vector_size must be greater than zero");
        }
        if self.embedding.provider == "deterministic" {
            let configured_size = self.embedding.vector_size.unwrap_or(3072);
            let model_size = embedding_model_vector_size(&self.embedding.model)?;
            if configured_size != model_size {
                bail!(
                    "backend.embedding.vector_size {configured_size} does not match backend.embedding.model {:?} dimension {model_size} for deterministic provider",
                    self.embedding.model
                );
            }
        }
        for (field, value) in [
            (
                "backend.oxigraph_persistence_path",
                self.oxigraph_persistence_path.as_deref(),
            ),
            (
                "backend.retrieval_stats_path",
                self.retrieval_stats_path.as_deref(),
            ),
            (
                "backend.identity_registry_dir",
                self.identity_registry_dir.as_deref(),
            ),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                bail!("{field} must not be empty when configured");
            }
        }
        if self.cleanup.enabled {
            let Some(namespace_prefix) = self
                .namespace_prefix
                .as_deref()
                .map(str::trim)
                .filter(|prefix| !prefix.is_empty())
            else {
                bail!("backend.cleanup.enabled=true requires backend.namespace_prefix");
            };
            let cleanup_prefix = self
                .cleanup
                .require_collection_prefix
                .as_deref()
                .expect("cleanup validation already required a prefix");
            if sanitized_collection_prefix(namespace_prefix)
                != sanitized_collection_prefix(cleanup_prefix)
            {
                bail!(
                    "backend.cleanup.require_collection_prefix must match backend.namespace_prefix"
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CleanupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub require_collection_prefix: Option<String>,
}

impl CleanupConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            let Some(prefix) = self
                .require_collection_prefix
                .as_deref()
                .map(str::trim)
                .filter(|prefix| !prefix.is_empty())
            else {
                bail!(
                    "backend.cleanup.enabled=true requires backend.cleanup.require_collection_prefix"
                );
            };
            if sanitized_collection_prefix(prefix).len() < 3 {
                bail!("backend.cleanup.require_collection_prefix is too broad");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_provider")]
    pub provider: String,
    #[serde(default = "default_embedding_model")]
    pub model: String,
    #[serde(default)]
    pub vector_size: Option<usize>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            model: default_embedding_model(),
            vector_size: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    #[serde(default)]
    pub mode: RetrievalMode,
    #[serde(default = "default_top_k_episodes")]
    pub top_k_episodes: usize,
    #[serde(default = "default_top_k_observations")]
    pub top_k_observations: usize,
    #[serde(default)]
    pub include_derived_memories: bool,
    #[serde(default)]
    pub include_threads: bool,
    #[serde(default)]
    pub include_entities: bool,
    #[serde(default)]
    pub include_debug_rationale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_vector_candidates: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_graph_roots: Option<usize>,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            mode: RetrievalMode::default(),
            top_k_episodes: default_top_k_episodes(),
            top_k_observations: default_top_k_observations(),
            include_derived_memories: false,
            include_threads: false,
            include_entities: false,
            include_debug_rationale: false,
            max_vector_candidates: None,
            max_graph_roots: None,
        }
    }
}

impl RetrievalConfig {
    pub fn validate(&self) -> Result<()> {
        if self.top_k_episodes == 0 {
            bail!("retrieval.top_k_episodes must be greater than zero");
        }
        if self.top_k_observations == 0 {
            bail!("retrieval.top_k_observations must be greater than zero");
        }
        if self.max_vector_candidates == Some(0) {
            bail!("retrieval.max_vector_candidates must be greater than zero when set");
        }
        if self.max_graph_roots == Some(0) {
            bail!("retrieval.max_graph_roots must be greater than zero when set");
        }
        if let (Some(max_vector_candidates), Some(max_graph_roots)) =
            (self.max_vector_candidates, self.max_graph_roots)
            && max_graph_roots > max_vector_candidates
        {
            bail!(
                "retrieval.max_graph_roots ({max_graph_roots}) must not exceed retrieval.max_vector_candidates ({max_vector_candidates})"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IngestConfig {
    #[serde(default)]
    pub index_observations: bool,
    #[serde(default)]
    pub index_episode_summaries: bool,
    #[serde(default)]
    pub create_threads: bool,
    #[serde(default)]
    pub store_gold_labels: bool,
    #[serde(default)]
    pub index_session_summaries: bool,
    #[serde(default)]
    pub index_generated_observations: bool,
    #[serde(default)]
    pub include_image_captions: bool,
    #[serde(default)]
    pub enrichment_path: Option<String>,
    #[serde(default)]
    pub enrichment_snapshot_path: Option<String>,
    #[serde(default)]
    pub enrichment_manifest_path: Option<String>,
    #[serde(default)]
    pub require_source_hash_match: bool,
}

impl IngestConfig {
    pub fn validate(&self) -> Result<()> {
        if self.store_gold_labels {
            bail!("ingest.store_gold_labels=true is prohibited; gold labels are scorer-only");
        }
        if !self.index_observations {
            bail!("ingest.index_observations=false is not supported by the current eval runner");
        }
        if !self.index_episode_summaries {
            bail!(
                "ingest.index_episode_summaries=false is not supported by the current eval runner"
            );
        }
        if self.create_threads && self.enrichment_path.is_none() {
            bail!("ingest.create_threads requires ingest.enrichment_path");
        }
        if self.enrichment_path.is_some() && self.enrichment_snapshot_path.is_some() {
            bail!(
                "ingest.enrichment_path and ingest.enrichment_snapshot_path are mutually exclusive"
            );
        }
        if self.enrichment_manifest_path.is_some() && self.enrichment_snapshot_path.is_none() {
            bail!("ingest.enrichment_manifest_path requires ingest.enrichment_snapshot_path");
        }
        if self.require_source_hash_match && self.enrichment_manifest_path.is_none() {
            bail!("ingest.require_source_hash_match requires ingest.enrichment_manifest_path");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsConfig {
    #[serde(default = "default_ks_session")]
    pub ks_session: Vec<usize>,
    #[serde(default = "default_ks_turn")]
    pub ks_turn: Vec<usize>,
    #[serde(default = "default_ks_dialog")]
    pub ks_dialog: Vec<usize>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            ks_session: default_ks_session(),
            ks_turn: default_ks_turn(),
            ks_dialog: default_ks_dialog(),
        }
    }
}

impl MetricsConfig {
    pub fn validate(&self) -> Result<()> {
        validate_ks("metrics.ks_session", &self.ks_session)?;
        validate_ks("metrics.ks_turn", &self.ks_turn)?;
        validate_ks("metrics.ks_dialog", &self.ks_dialog)?;
        Ok(())
    }
}

fn default_top_k_episodes() -> usize {
    8
}

fn default_top_k_observations() -> usize {
    16
}

fn default_embedding_provider() -> String {
    "openai".to_string()
}

fn default_embedding_model() -> String {
    "text-embedding-3-large".to_string()
}

fn embedding_model_vector_size(model: &str) -> Result<usize> {
    match model.trim() {
        "text-embedding-3-small" | "text-embedding-ada-002" => Ok(1536),
        "text-embedding-3-large" => Ok(3072),
        _ => bail!(
            "backend.embedding.model {model:?} is unsupported for deterministic provider; expected text-embedding-3-small, text-embedding-3-large, or text-embedding-ada-002"
        ),
    }
}

fn default_openai_api_key_env() -> String {
    "OPENAI_API_KEY".to_string()
}

fn default_ks_session() -> Vec<usize> {
    vec![5, 10]
}

fn default_ks_turn() -> Vec<usize> {
    vec![10, 50]
}

fn default_ks_dialog() -> Vec<usize> {
    vec![5, 10]
}

fn validate_ks(name: &str, values: &[usize]) -> Result<()> {
    require_non_empty(name, values)?;
    if values.contains(&0) {
        bail!("{name} must contain only values greater than zero");
    }
    Ok(())
}

fn require_non_empty(name: &str, values: &[usize]) -> Result<()> {
    if values.is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(())
}

fn sanitized_collection_prefix(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeterministicEmbeddingProvider;

    #[test]
    fn backend_defaults_to_openai_large_embeddings() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic"
        }))
        .unwrap();

        assert_eq!(config.backend.embedding.provider, "openai");
        assert_eq!(config.backend.embedding.model, "text-embedding-3-large");
    }

    #[test]
    fn rejects_zero_embedding_vector_size() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {"embedding": {"provider": "deterministic", "vector_size": 0}},
            "ingest": {"index_observations": true, "index_episode_summaries": true}
        }))
        .unwrap();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("backend.embedding.vector_size"));
        assert!(error.contains("greater than zero"));
    }

    #[test]
    fn deterministic_embedding_dimension_must_match_model_at_validation_boundary() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {
                "embedding": {
                    "provider": "deterministic",
                    "model": "text-embedding-3-small",
                    "vector_size": 3072
                }
            },
            "ingest": {"index_observations": true, "index_episode_summaries": true}
        }))
        .unwrap();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("backend.embedding.vector_size 3072"));
        assert!(error.contains("text-embedding-3-small"));
        assert!(error.contains("dimension 1536"));

        config.backend.embedding.vector_size = Some(1536);
        config.validate().unwrap();
        DeterministicEmbeddingProvider::new(1536).unwrap();
    }

    #[test]
    fn continuity_dataset_rejects_non_deterministic_embedding_provider_at_validation() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "continuity-run",
            "dataset": "continuity",
            "backend": {
                "embedding": {"provider": "openai"},
                "oxigraph_persistence_path": "runs/continuity/oxigraph",
                "retrieval_stats_path": "runs/continuity/retrieval.sqlite"
            },
            "ingest": {"index_observations": true, "index_episode_summaries": true}
        }))
        .unwrap();

        let error = config
            .validate_for_dataset_kind(DatasetKind::Continuity)
            .unwrap_err()
            .to_string();
        assert!(error.contains("continuity dataset"));
        assert!(error.contains("deterministic embedding provider"));
        assert!(error.contains("openai"));

        config.backend.embedding = EmbeddingConfig {
            provider: "deterministic".into(),
            model: "text-embedding-3-large".into(),
            vector_size: Some(3072),
        };
        config
            .validate_for_dataset_kind(DatasetKind::Continuity)
            .unwrap();

        config.backend.embedding = EmbeddingConfig {
            provider: "controllable_similarity".into(),
            model: "fixture-declared".into(),
            vector_size: Some(8),
        };
        config
            .validate_for_dataset_kind(DatasetKind::Continuity)
            .unwrap();
    }

    #[test]
    fn continuity_dataset_requires_each_persistent_store_path() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "continuity-run",
            "dataset": "continuity",
            "backend": {
                "embedding": {
                    "provider": "controllable_similarity",
                    "model": "fixture-declared",
                    "vector_size": 8
                },
                "oxigraph_persistence_path": "runs/continuity/oxigraph",
                "retrieval_stats_path": "runs/continuity/retrieval.sqlite"
            },
            "ingest": {"index_observations": true, "index_episode_summaries": true}
        }))
        .unwrap();

        let mut missing_oxigraph = config.clone();
        missing_oxigraph.backend.oxigraph_persistence_path = None;
        let error = missing_oxigraph
            .validate_for_dataset_kind(DatasetKind::Continuity)
            .unwrap_err()
            .to_string();
        assert!(error.contains("backend.oxigraph_persistence_path"));
        assert!(error.contains("restart durability"));

        let mut missing_stats = config;
        missing_stats.backend.retrieval_stats_path = None;
        let error = missing_stats
            .validate_for_dataset_kind(DatasetKind::Continuity)
            .unwrap_err()
            .to_string();
        assert!(error.contains("backend.retrieval_stats_path"));
        assert!(error.contains("restart durability"));
    }

    #[test]
    fn parses_restart_persistence_paths_and_rejects_empty_values() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {
                "oxigraph_persistence_path": "runs/r/oxigraph",
                "retrieval_stats_path": "runs/r/retrieval.sqlite",
                "identity_registry_dir": "runs/r/identities"
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();
        config.validate().unwrap();
        assert_eq!(
            config.backend.identity_registry_dir.as_deref(),
            Some("runs/r/identities")
        );

        let mut invalid = config;
        invalid.backend.retrieval_stats_path = Some("  ".to_string());
        assert!(
            invalid
                .validate()
                .unwrap_err()
                .to_string()
                .contains("backend.retrieval_stats_path")
        );
    }

    #[test]
    fn parses_existing_retrieval_flags() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "include_threads": true,
                "include_entities": true,
                "include_debug_rationale": true
            }
        }))
        .unwrap();

        assert_eq!(config.retrieval.mode, RetrievalMode::Hybrid);
        assert!(config.retrieval.include_threads);
        assert!(config.retrieval.include_entities);
        assert!(config.retrieval.include_debug_rationale);
    }

    #[test]
    fn parses_bm25_retrieval_mode() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "mode": "bm25_only",
                "top_k_episodes": 3,
                "top_k_observations": 7
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        assert_eq!(config.retrieval.mode, RetrievalMode::Bm25Only);
        config.validate().unwrap();
    }

    #[test]
    fn parses_vector_only_retrieval_mode() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "mode": "vector_only"
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        assert_eq!(config.retrieval.mode, RetrievalMode::VectorOnly);
        config.validate().unwrap();
    }

    #[test]
    fn rejects_unknown_retrieval_mode() {
        let err = serde_json::from_value::<BenchmarkRunConfig>(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "mode": "not_a_mode"
            }
        }))
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown variant"));
    }

    #[test]
    fn core_validation_leaves_dataset_dispatch_to_the_runner_seam() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "future_dataset",
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        config.validate().unwrap();
    }

    #[test]
    fn parses_typed_metric_ks() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "longmemeval_s",
            "metrics": {
                "ks_session": [1, 3],
                "ks_turn": [7]
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        assert_eq!(config.metrics.ks_session, vec![1, 3]);
        assert_eq!(config.metrics.ks_turn, vec![7]);
        config.validate().unwrap();
    }

    #[test]
    fn rejects_empty_metric_ks() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "longmemeval_s",
            "metrics": {
                "ks_session": [],
                "ks_turn": [10]
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("ks_session")
        );
    }

    #[test]
    fn rejects_gold_label_storage() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true,
                "store_gold_labels": true
            }
        }))
        .unwrap();

        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("store_gold_labels")
        );
    }

    #[test]
    fn accepts_graph_retrieval_flags() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "include_threads": true,
                "include_entities": true,
                "max_vector_candidates": 48,
                "max_graph_roots": 48
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        config.validate().unwrap();
        assert_eq!(config.retrieval.max_vector_candidates, Some(48));
        assert_eq!(config.retrieval.max_graph_roots, Some(48));
    }

    #[test]
    fn rejects_graph_root_limit_above_vector_candidate_limit() {
        let mut retrieval = RetrievalConfig {
            max_vector_candidates: Some(12),
            max_graph_roots: Some(13),
            ..RetrievalConfig::default()
        };
        let error = retrieval.validate().unwrap_err().to_string();
        assert!(error.contains("max_graph_roots (13)"), "{error}");
        assert!(error.contains("max_vector_candidates (12)"), "{error}");

        retrieval.max_graph_roots = Some(0);
        assert!(
            retrieval
                .validate()
                .unwrap_err()
                .to_string()
                .contains("greater than zero")
        );
    }

    #[test]
    fn rejects_cleanup_without_required_prefix() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {
                "cleanup": {
                    "enabled": true
                }
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("require_collection_prefix")
        );
    }

    #[test]
    fn accepts_cleanup_with_required_prefix() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {
                "namespace_prefix": "bench:synthetic",
                "cleanup": {
                    "enabled": true,
                    "require_collection_prefix": "bench:synthetic"
                }
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        config.validate().unwrap();
    }

    #[test]
    fn rejects_cleanup_prefix_that_does_not_match_namespace_prefix() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {
                "namespace_prefix": "bench:synthetic",
                "cleanup": {
                    "enabled": true,
                    "require_collection_prefix": "other"
                }
            },
            "ingest": {
                "index_observations": true,
                "index_episode_summaries": true
            }
        }))
        .unwrap();

        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("namespace_prefix")
        );
    }
}
