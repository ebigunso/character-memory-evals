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
    pub metrics: serde_json::Value,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CleanupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub require_collection_prefix: Option<String>,
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
            top_k_episodes: default_top_k_episodes(),
            top_k_observations: default_top_k_observations(),
            include_derived_memories: false,
            include_threads: false,
            include_entities: false,
            include_debug_rationale: false,
        }
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

        assert!(config.retrieval.include_threads);
        assert!(config.retrieval.include_entities);
        assert!(config.retrieval.include_debug_rationale);
    }
}
