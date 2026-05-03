use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatasetKind {
    LongMemEvalS,
    LoCoMo,
    Synthetic,
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
        match self.dataset.as_str() {
            "longmemeval_s" => {
                require_non_empty("metrics.ks_session", &self.metrics.ks_session)?;
                require_non_empty("metrics.ks_turn", &self.metrics.ks_turn)?;
            }
            "locomo" => {
                require_non_empty("metrics.ks_dialog", &self.metrics.ks_dialog)?;
                require_non_empty("metrics.ks_session", &self.metrics.ks_session)?;
            }
            "synthetic" => {}
            other => bail!("unsupported dataset in config: {other}"),
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
    10
}

fn default_top_k_observations() -> usize {
    20
}

fn default_embedding_provider() -> String {
    "openai".to_string()
}

fn default_embedding_model() -> String {
    "text-embedding-3-large".to_string()
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
    fn rejects_unknown_retrieval_mode() {
        let err = serde_json::from_value::<BenchmarkRunConfig>(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "mode": "vector_only"
            }
        }))
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown variant"));
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
                "include_entities": true
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
