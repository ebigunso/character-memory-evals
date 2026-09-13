use crate::{DatasetId, RetrievalSurfacePolicy};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMode {
    #[default]
    Hybrid,
    Bm25Only,
    VectorOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreMode {
    #[default]
    Embedded,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkRunConfig {
    pub run_id: String,
    pub dataset: DatasetId,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackendConfig {
    #[serde(default)]
    pub vector_store_mode: VectorStoreMode,
    #[serde(default)]
    pub qdrant_connection_string: Option<String>,
    #[serde(default = "default_openai_api_key_env")]
    pub openai_api_key_env: String,
    #[serde(default)]
    pub retain_stores: bool,
    #[serde(default)]
    pub retain_reason: Option<String>,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_memory: Option<CharacterMemoryConfig>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            vector_store_mode: VectorStoreMode::default(),
            qdrant_connection_string: None,
            openai_api_key_env: default_openai_api_key_env(),
            retain_stores: false,
            retain_reason: None,
            embedding: EmbeddingConfig::default(),
            character_memory: None,
        }
    }
}

impl BackendConfig {
    pub fn validate(&self) -> Result<()> {
        match (self.retain_stores, self.retain_reason.as_deref()) {
            (true, Some(reason)) if !reason.trim().is_empty() => {}
            (true, _) => {
                bail!("backend.retain_stores=true requires a nonempty backend.retain_reason")
            }
            (false, Some(_)) => bail!("backend.retain_reason requires backend.retain_stores=true"),
            (false, None) => {}
        }
        if self.embedding.vector_size == Some(0) {
            bail!("backend.embedding.vector_size must be greater than zero");
        }
        if self.embedding.provider == EmbeddingProviderConfig::Deterministic {
            let configured_size = self.embedding.vector_size.unwrap_or(3072);
            let model_size = embedding_model_vector_size(&self.embedding.model)?;
            if configured_size != model_size {
                bail!(
                    "backend.embedding.vector_size {configured_size} does not match backend.embedding.model {:?} dimension {model_size} for deterministic provider",
                    self.embedding.model
                );
            }
        }
        if self.embedding.uses_frozen_store() {
            if self.embedding.vector_size.is_none() {
                bail!(
                    "backend.embedding.provider={} requires backend.embedding.vector_size",
                    self.embedding.provider
                );
            }
            if self.embedding.store_path.is_none() {
                bail!(
                    "backend.embedding.provider={} requires backend.embedding.store_path",
                    self.embedding.provider
                );
            }
        }
        if self
            .embedding
            .store_path
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            bail!("backend.embedding.store_path must not be empty when configured");
        }
        if let Some(character_memory) = &self.character_memory {
            character_memory.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMemoryConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selectivity_smoothing_alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selectivity_gamma: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<CharacterMemoryRetrievalConfig>,
}

impl CharacterMemoryConfig {
    fn validate(&self) -> Result<()> {
        validate_optional_positive_f64(
            "backend.character_memory.selectivity_smoothing_alpha",
            self.selectivity_smoothing_alpha,
        )?;
        validate_optional_positive_f64(
            "backend.character_memory.selectivity_gamma",
            self.selectivity_gamma,
        )?;
        if let Some(retrieval) = &self.retrieval {
            retrieval.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMemoryRetrievalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanout: Option<CharacterMemoryFanoutConfig>,
}

impl CharacterMemoryRetrievalConfig {
    fn validate(&self) -> Result<()> {
        if let Some(fanout) = &self.fanout {
            fanout.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMemoryFanoutConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about_entity: Option<CharacterMemoryAboutEntityFanoutConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participant_entity: Option<CharacterMemoryParticipantEntityFanoutConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_of_thread: Option<CharacterMemoryPartOfThreadFanoutConfig>,
}

impl CharacterMemoryFanoutConfig {
    fn validate(&self) -> Result<()> {
        if let Some(budget) = self
            .about_entity
            .as_ref()
            .and_then(|fanout| fanout.derived_memory.as_ref())
        {
            budget.validate(
                "backend.character_memory.retrieval.fanout.about_entity.derived_memory",
            )?;
        }
        if let Some(budget) = self
            .participant_entity
            .as_ref()
            .and_then(|fanout| fanout.episode.as_ref())
        {
            budget
                .validate("backend.character_memory.retrieval.fanout.participant_entity.episode")?;
        }
        if let Some(budget) = self
            .part_of_thread
            .as_ref()
            .and_then(|fanout| fanout.derived_memory.as_ref())
        {
            budget.validate(
                "backend.character_memory.retrieval.fanout.part_of_thread.derived_memory",
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMemoryAboutEntityFanoutConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_about_entity_derived_memory_budget"
    )]
    pub derived_memory: Option<FanoutBudgetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMemoryParticipantEntityFanoutConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_participant_entity_episode_budget"
    )]
    pub episode: Option<FanoutBudgetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterMemoryPartOfThreadFanoutConfig {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_part_of_thread_derived_memory_budget"
    )]
    pub derived_memory: Option<FanoutBudgetConfig>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FanoutBudgetConfig {
    pub min: usize,
    pub max: usize,
}

fn deserialize_fanout_budget_at_path<'de, D>(
    deserializer: D,
    path: &str,
) -> std::result::Result<Option<FanoutBudgetConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<FanoutBudgetConfig>::deserialize(deserializer)
        .map_err(|error| serde::de::Error::custom(format!("{path}: {error}")))
}

fn deserialize_about_entity_derived_memory_budget<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<FanoutBudgetConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_fanout_budget_at_path(
        deserializer,
        "backend.character_memory.retrieval.fanout.about_entity.derived_memory",
    )
}

fn deserialize_participant_entity_episode_budget<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<FanoutBudgetConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_fanout_budget_at_path(
        deserializer,
        "backend.character_memory.retrieval.fanout.participant_entity.episode",
    )
}

fn deserialize_part_of_thread_derived_memory_budget<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<FanoutBudgetConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_fanout_budget_at_path(
        deserializer,
        "backend.character_memory.retrieval.fanout.part_of_thread.derived_memory",
    )
}

impl FanoutBudgetConfig {
    fn validate(self, field: &str) -> Result<()> {
        if self.min > self.max {
            bail!(
                "{field}.min must be less than or equal to {field}.max, got min={} max={}",
                self.min,
                self.max
            );
        }
        Ok(())
    }
}

fn validate_optional_positive_f64(field: &str, value: Option<f64>) -> Result<()> {
    if let Some(value) = value
        && (!value.is_finite() || value <= 0.0)
    {
        bail!("{field} must be a finite positive number, got {value}");
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_provider")]
    pub provider: EmbeddingProviderConfig,
    #[serde(default = "default_embedding_model")]
    pub model: String,
    #[serde(default)]
    pub vector_size: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_path: Option<String>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            model: default_embedding_model(),
            vector_size: None,
            store_path: None,
        }
    }
}

impl EmbeddingConfig {
    pub fn uses_frozen_store(&self) -> bool {
        self.provider == EmbeddingProviderConfig::Frozen
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderConfig {
    Deterministic,
    #[serde(rename = "openai")]
    OpenAi,
    ControllableSimilarity,
    Frozen,
}

impl EmbeddingProviderConfig {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::OpenAi => "openai",
            Self::ControllableSimilarity => "controllable_similarity",
            Self::Frozen => "frozen",
        }
    }
}

impl std::fmt::Display for EmbeddingProviderConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RetrievalConfig {
    #[serde(default)]
    pub mode: RetrievalMode,
    #[serde(default)]
    pub surface_policy: RetrievalSurfacePolicy,
}

impl RetrievalConfig {
    pub fn validate(&self) -> Result<()> {
        match self.mode {
            RetrievalMode::VectorOnly => self.surface_policy.validate_for_vector_only(),
            RetrievalMode::Bm25Only => self.surface_policy.validate_for_bm25_only(),
            RetrievalMode::Hybrid => self.surface_policy.validate(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct IngestConfig {
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
}

impl IngestConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enrichment_path.is_some() && self.enrichment_snapshot_path.is_some() {
            bail!(
                "ingest.enrichment_path and ingest.enrichment_snapshot_path are mutually exclusive"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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

fn default_embedding_provider() -> EmbeddingProviderConfig {
    EmbeddingProviderConfig::OpenAi
}

fn default_embedding_model() -> String {
    "text-embedding-3-large".to_string()
}

fn embedding_model_vector_size(model: &str) -> Result<usize> {
    crate::frozen_embedding::model_native_embedding_vector_size(model)
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

        assert_eq!(
            config.backend.embedding.provider,
            EmbeddingProviderConfig::OpenAi
        );
        assert_eq!(
            serde_json::to_value(config.backend.embedding.provider).unwrap(),
            serde_json::json!("openai")
        );
        assert_eq!(config.backend.embedding.model, "text-embedding-3-large");
    }

    #[test]
    fn run_config_rejects_unknown_keys_at_every_container_boundary() {
        let cases = [
            (serde_json::json!({"run_typo": true}), "run_typo"),
            (
                serde_json::json!({"backend": {"backend_typo": true}}),
                "backend_typo",
            ),
            (
                serde_json::json!({"backend": {"oxigraph_path": "legacy"}}),
                "oxigraph_path",
            ),
            (
                serde_json::json!({"backend": {"retain_store": true}}),
                "retain_store",
            ),
            (
                serde_json::json!({"backend": {"embedding": {"embedding_typo": true}}}),
                "embedding_typo",
            ),
            (
                serde_json::json!({"retrieval": {"retrieval_typo": true}}),
                "retrieval_typo",
            ),
            (
                serde_json::json!({"retrieval": {"surface_policy": {"surface_typo": true}}}),
                "surface_typo",
            ),
            (
                serde_json::json!({"retrieval": {"surface_policy": {"sections": {"section_typo": true}}}}),
                "section_typo",
            ),
            (
                serde_json::json!({"ingest": {"ingest_typo": true}}),
                "ingest_typo",
            ),
            (
                serde_json::json!({"ingest": {"enrichment_manifest_path": "manifest.json"}}),
                "enrichment_manifest_path",
            ),
            (
                serde_json::json!({"ingest": {"require_source_hash_match": true}}),
                "require_source_hash_match",
            ),
            (
                serde_json::json!({"metrics": {"metrics_typo": true}}),
                "metrics_typo",
            ),
        ];

        for (extra, unknown_key) in cases {
            let mut value = serde_json::json!({
                "run_id": "r",
                "dataset": "synthetic"
            });
            merge_json_object(&mut value, extra);
            let error = serde_json::from_value::<BenchmarkRunConfig>(value)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(&format!("unknown field `{unknown_key}`")),
                "{error}"
            );
        }
    }

    fn merge_json_object(target: &mut serde_json::Value, source: serde_json::Value) {
        let target = target.as_object_mut().unwrap();
        for (key, value) in source.as_object().unwrap() {
            target.insert(key.clone(), value.clone());
        }
    }

    #[test]
    fn rejects_zero_embedding_vector_size() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {"embedding": {"provider": "deterministic", "vector_size": 0}}
        }))
        .unwrap();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("backend.embedding.vector_size"));
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
            }
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
    fn continuity_dataset_accepts_supported_embedding_resource_kinds() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "continuity-run",
            "dataset": "continuity",
            "backend": {
                "embedding": {"provider": "openai"},
            }
        }))
        .unwrap();

        config.validate().unwrap();

        config.backend.embedding = EmbeddingConfig {
            provider: EmbeddingProviderConfig::Deterministic,
            model: "text-embedding-3-large".into(),
            vector_size: Some(3072),
            store_path: None,
        };
        config.validate().unwrap();

        config.backend.embedding = EmbeddingConfig {
            provider: EmbeddingProviderConfig::ControllableSimilarity,
            model: "fixture-declared".into(),
            vector_size: Some(8),
            store_path: None,
        };
        config.validate().unwrap();

        config.backend.embedding = EmbeddingConfig {
            provider: EmbeddingProviderConfig::Frozen,
            model: "text-embedding-3-large".into(),
            vector_size: Some(3072),
            store_path: Some("fixtures/embeddings/continuity.json".into()),
        };
        config.validate().unwrap();
    }

    #[test]
    fn frozen_embedding_config_requires_store_path_and_vector_size() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "continuity-run",
            "dataset": "continuity",
            "backend": {
                "embedding": {
                    "provider": "frozen",
                    "model": "text-embedding-3-large"
                },
            }
        }))
        .unwrap();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("backend.embedding.vector_size"), "{error}");

        config.backend.embedding.vector_size = Some(256);
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("backend.embedding.store_path"), "{error}");

        config.backend.embedding.store_path = Some("  ".to_string());
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("backend.embedding.store_path"), "{error}");
        assert!(error.contains("must not be empty"), "{error}");
    }

    #[test]
    fn character_memory_overrides_parse_validate_and_survive_config_snapshot() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "continuity",
            "backend": {
                "character_memory": {
                    "selectivity_smoothing_alpha": 2.0,
                    "selectivity_gamma": 0.5,
                    "retrieval": {
                        "fanout": {
                            "about_entity": {"derived_memory": {"min": 2, "max": 8}},
                            "participant_entity": {"episode": {"min": 1, "max": 3}},
                            "part_of_thread": {"derived_memory": {"min": 4, "max": 9}}
                        }
                    }
                }
            }
        }))
        .unwrap();

        config.validate().unwrap();
        let snapshot = serde_json::to_value(&config).unwrap();
        assert_eq!(
            snapshot["backend"]["character_memory"]["selectivity_smoothing_alpha"],
            2.0
        );
        assert_eq!(
            snapshot["backend"]["character_memory"]["selectivity_gamma"],
            0.5
        );
        assert_eq!(
            snapshot["backend"]["character_memory"]["retrieval"]["fanout"]["about_entity"]["derived_memory"]
                ["min"],
            2
        );
        assert_eq!(
            snapshot["backend"]["character_memory"]["retrieval"]["fanout"]["participant_entity"]["episode"]
                ["max"],
            3
        );
        assert_eq!(
            snapshot["backend"]["character_memory"]["retrieval"]["fanout"]["part_of_thread"]["derived_memory"]
                ["max"],
            9
        );
    }

    #[test]
    fn character_memory_overrides_are_absent_from_legacy_config_snapshots() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic"
        }))
        .unwrap();

        config.validate().unwrap();
        let snapshot = serde_json::to_value(&config).unwrap();
        assert!(snapshot["backend"].get("character_memory").is_none());
    }

    #[test]
    fn character_memory_overrides_reject_invalid_selectivity_and_fanout() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "backend": {
                "character_memory": {
                    "selectivity_smoothing_alpha": -1.0
                }
            }
        }))
        .unwrap();

        let error = config.validate().unwrap_err().to_string();
        assert!(
            error.contains("backend.character_memory.selectivity_smoothing_alpha"),
            "{error}"
        );
        assert!(error.contains("finite positive number"), "{error}");

        config.backend.character_memory = Some(
            serde_json::from_value(serde_json::json!({
                "retrieval": {
                    "fanout": {
                        "about_entity": {"derived_memory": {"min": 9, "max": 8}}
                    }
                }
            }))
            .unwrap(),
        );
        let error = config.validate().unwrap_err().to_string();
        assert!(
            error.contains("backend.character_memory.retrieval.fanout.about_entity.derived_memory"),
            "{error}"
        );
        assert!(error.contains("min=9 max=8"), "{error}");
    }

    #[test]
    fn character_memory_fanout_budget_tables_require_min_and_max() {
        for (value, missing_key, table_path) in [
            (
                serde_json::json!({
                    "retrieval": {
                        "fanout": {
                            "about_entity": {"derived_memory": {"min": 2}}
                        }
                    }
                }),
                "max",
                "backend.character_memory.retrieval.fanout.about_entity.derived_memory",
            ),
            (
                serde_json::json!({
                    "retrieval": {
                        "fanout": {
                            "participant_entity": {"episode": {"max": 3}}
                        }
                    }
                }),
                "min",
                "backend.character_memory.retrieval.fanout.participant_entity.episode",
            ),
        ] {
            let error = serde_json::from_value::<CharacterMemoryConfig>(value)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(&format!("missing field `{missing_key}`")),
                "{error}"
            );
            assert!(error.contains(table_path), "{error}");
        }
    }

    #[test]
    fn character_memory_overrides_reject_unknown_keys_and_fanout_targets() {
        let error = serde_json::from_value::<CharacterMemoryConfig>(serde_json::json!({
            "selectivity_gamme": 0.5
        }))
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("unknown field `selectivity_gamme`"),
            "{error}"
        );

        for (value, unknown_key) in [
            (
                serde_json::json!({
                    "retrieval": {
                        "fanout": {
                            "unsupported_relation": {"derived_memory": {"min": 0, "max": 1}}
                        }
                    }
                }),
                "unsupported_relation",
            ),
            (
                serde_json::json!({
                    "retrieval": {
                        "fanout": {
                            "about_entity": {"episode": {"min": 0, "max": 1}}
                        }
                    }
                }),
                "episode",
            ),
        ] {
            let error = serde_json::from_value::<CharacterMemoryConfig>(value)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(&format!("unknown field `{unknown_key}`")),
                "{error}"
            );
        }
    }

    #[test]
    fn parses_typed_retrieval_surface_policy() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "surface_policy": {
                    "sections": {
                        "active_threads": 6,
                        "relevant_episodes": 8,
                        "salient_observations": 16,
                        "derived_memories": 12,
                        "preferences": 8,
                        "relationship_notes": 8,
                        "open_loops": 8,
                        "commitments": 8,
                        "character_signals": 8
                    },
                    "object_types": ["episode", "observation", "memory_thread", "entity"],
                    "include_debug_rationale": true,
                    "max_vector_candidates": null,
                    "max_graph_roots": null
                }
            }
        }))
        .unwrap();

        assert_eq!(config.retrieval.mode, RetrievalMode::Hybrid);
        assert_eq!(
            config.retrieval.surface_policy.object_types,
            vec![
                crate::ObjectType::Episode,
                crate::ObjectType::Observation,
                crate::ObjectType::MemoryThread,
                crate::ObjectType::Entity,
            ]
        );
        assert!(config.retrieval.surface_policy.include_debug_rationale);
    }

    #[test]
    fn vector_only_retrieval_accepts_both_backends_at_admission() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "mode": "vector_only"
            }
        }))
        .unwrap();

        assert_eq!(config.retrieval.mode, RetrievalMode::VectorOnly);
        config.retrieval.surface_policy.object_types =
            vec![crate::ObjectType::Episode, crate::ObjectType::Observation];
        assert_eq!(config.backend.vector_store_mode, VectorStoreMode::Embedded);
        config.validate().unwrap();
        config.backend.vector_store_mode = VectorStoreMode::Service;
        config.validate().unwrap();
    }

    #[test]
    fn parses_bm25_retrieval_mode() {
        let mut config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "locomo",
            "retrieval": {"mode": "bm25_only"}
        }))
        .unwrap();

        assert_eq!(config.retrieval.mode, RetrievalMode::Bm25Only);
        config.retrieval.surface_policy.object_types =
            vec![crate::ObjectType::Episode, crate::ObjectType::Observation];
        config.validate().unwrap();
    }

    #[test]
    fn text_baselines_rejects_object_types_outside_its_episode_observation_capability() {
        for mode in [RetrievalMode::VectorOnly, RetrievalMode::Bm25Only] {
            let mut retrieval = RetrievalConfig {
                mode,
                surface_policy: RetrievalSurfacePolicy {
                    object_types: vec![
                        crate::ObjectType::Episode,
                        crate::ObjectType::DerivedMemory,
                        crate::ObjectType::Entity,
                        crate::ObjectType::MemoryThread,
                    ],
                    ..RetrievalSurfacePolicy::default()
                },
            };

            let error = retrieval.validate().unwrap_err();
            assert_eq!(
                error.downcast_ref::<crate::BaselineSurfacePolicyError>(),
                Some(&crate::BaselineSurfacePolicyError::UnsupportedObjectTypes {
                    mode,
                    object_types: vec![
                        crate::ObjectType::DerivedMemory,
                        crate::ObjectType::Entity,
                        crate::ObjectType::MemoryThread
                    ],
                })
            );

            retrieval.surface_policy.object_types = vec![crate::ObjectType::Observation];
            retrieval.validate().unwrap();
        }
    }

    #[test]
    fn text_baselines_rejects_zero_budget_for_a_selected_episode_surface() {
        for mode in [RetrievalMode::VectorOnly, RetrievalMode::Bm25Only] {
            let mut retrieval = RetrievalConfig {
                mode,
                surface_policy: RetrievalSurfacePolicy {
                    object_types: vec![crate::ObjectType::Episode],
                    ..RetrievalSurfacePolicy::default()
                },
            };
            retrieval.surface_policy.sections.relevant_episodes = 0;

            let error = retrieval.validate().unwrap_err();
            assert_eq!(
                error.downcast_ref::<crate::BaselineSurfacePolicyError>(),
                Some(
                    &crate::BaselineSurfacePolicyError::ZeroSelectedSurfaceBudget {
                        mode,
                        object_type: crate::ObjectType::Episode,
                    }
                )
            );

            retrieval.surface_policy.object_types = vec![crate::ObjectType::Observation];
            retrieval.validate().unwrap();
        }
    }

    #[test]
    fn text_baselines_rejects_zero_budget_for_a_selected_observation_surface() {
        for mode in [RetrievalMode::VectorOnly, RetrievalMode::Bm25Only] {
            let mut retrieval = RetrievalConfig {
                mode,
                surface_policy: RetrievalSurfacePolicy {
                    object_types: vec![crate::ObjectType::Observation],
                    ..RetrievalSurfacePolicy::default()
                },
            };
            retrieval.surface_policy.sections.salient_observations = 0;

            let error = retrieval.validate().unwrap_err();
            assert_eq!(
                error.downcast_ref::<crate::BaselineSurfacePolicyError>(),
                Some(
                    &crate::BaselineSurfacePolicyError::ZeroSelectedSurfaceBudget {
                        mode,
                        object_type: crate::ObjectType::Observation,
                    }
                )
            );

            retrieval.surface_policy.object_types = vec![crate::ObjectType::Episode];
            retrieval.validate().unwrap();
        }
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
            "dataset": "future_dataset"
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
    fn retired_ingest_keys_are_rejected() {
        for key in [
            "index_observations",
            "index_episode_summaries",
            "store_gold_labels",
            "create_threads",
        ] {
            let raw = format!(r#"{{"run_id":"r","dataset":"locomo","ingest":{{"{key}":true}}}}"#);
            let error = serde_json::from_str::<BenchmarkRunConfig>(&raw)
                .unwrap_err()
                .to_string();
            assert!(error.contains("unknown field"), "{key}: {error}");
        }
    }

    #[test]
    fn accepts_graph_retrieval_flags() {
        let config: BenchmarkRunConfig = serde_json::from_value(serde_json::json!({
            "run_id": "r",
            "dataset": "synthetic",
            "retrieval": {
                "surface_policy": {
                    "sections": {
                        "active_threads": 6,
                        "relevant_episodes": 8,
                        "salient_observations": 16,
                        "derived_memories": 12,
                        "preferences": 8,
                        "relationship_notes": 8,
                        "open_loops": 8,
                        "commitments": 8,
                        "character_signals": 8
                    },
                    "object_types": ["episode", "observation", "memory_thread", "entity"],
                    "include_debug_rationale": false,
                    "max_vector_candidates": 48,
                    "max_graph_roots": 48
                }
            }
        }))
        .unwrap();

        config.validate().unwrap();
        assert_eq!(
            config.retrieval.surface_policy.max_vector_candidates,
            Some(48)
        );
        assert_eq!(config.retrieval.surface_policy.max_graph_roots, Some(48));
    }

    #[test]
    fn rejects_graph_root_limit_above_vector_candidate_limit() {
        let mut retrieval = RetrievalConfig {
            surface_policy: RetrievalSurfacePolicy {
                max_vector_candidates: Some(12),
                max_graph_roots: Some(13),
                ..RetrievalSurfacePolicy::default()
            },
            ..RetrievalConfig::default()
        };
        let error = retrieval.validate().unwrap_err().to_string();
        assert!(error.contains("max_graph_roots (13)"), "{error}");
        assert!(error.contains("max_vector_candidates (12)"), "{error}");

        retrieval.surface_policy.max_graph_roots = Some(0);
        assert!(
            retrieval
                .validate()
                .unwrap_err()
                .to_string()
                .contains("greater than zero")
        );
    }

    #[test]
    fn store_retention_requires_a_reason_and_rejects_retired_path_policy() {
        let mut backend = BackendConfig::default();
        assert!(!backend.retain_stores);
        backend.validate().unwrap();
        backend.retain_stores = true;
        for reason in [None, Some("".into()), Some("  ".into())] {
            backend.retain_reason = reason;
            assert!(
                backend
                    .validate()
                    .unwrap_err()
                    .to_string()
                    .contains("retain_reason")
            );
        }
        backend.retain_reason = Some("inspect failed correction".into());
        backend.validate().unwrap();
        backend.retain_stores = false;
        assert!(
            backend
                .validate()
                .unwrap_err()
                .to_string()
                .contains("retain_stores")
        );
        for key in [
            "namespace_prefix",
            "cleanup",
            "oxigraph_persistence_path",
            "retrieval_stats_path",
            "identity_registry_dir",
        ] {
            let value = serde_json::json!({key: "retired"});
            assert!(
                serde_json::from_value::<BackendConfig>(value)
                    .unwrap_err()
                    .to_string()
                    .contains(key)
            );
        }
    }
}
