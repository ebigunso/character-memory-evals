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
    pub backend: serde_json::Value,
    #[serde(default)]
    pub retrieval: RetrievalConfig,
    #[serde(default)]
    pub ingest: serde_json::Value,
    #[serde(default)]
    pub metrics: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    #[serde(default = "default_top_k_episodes")]
    pub top_k_episodes: usize,
    #[serde(default = "default_top_k_observations")]
    pub top_k_observations: usize,
    #[serde(default)]
    pub include_derived_memories: bool,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            top_k_episodes: default_top_k_episodes(),
            top_k_observations: default_top_k_observations(),
            include_derived_memories: false,
        }
    }
}

fn default_top_k_episodes() -> usize {
    10
}

fn default_top_k_observations() -> usize {
    20
}
