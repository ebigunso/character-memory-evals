use crate::{ControllableSimilarityFixture, FrozenEmbeddingProvider, ObjectType};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControllableDimensionPolicy {
    FixtureDeclared,
    Exact { vector_size: usize },
}

/// Runtime-only binding used identically for initial construction and restart.
/// Serializable resource declarations stay in configuration/fixture DTOs;
/// loaded providers and fixtures do not masquerade as configuration.
#[derive(Debug, Clone)]
pub enum EmbeddingRuntimeBinding {
    Controllable {
        fixture: ControllableSimilarityFixture,
        dimension_policy: ControllableDimensionPolicy,
    },
    Frozen {
        store: FrozenEmbeddingProvider,
    },
    Live {
        provider: LiveEmbeddingProvider,
        model: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveEmbeddingProvider {
    OpenAi,
    Deterministic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EmbeddingBindingRecord {
    Bm25,
    Controllable {
        fixture_sha256: String,
        vector_size: usize,
        dimension_policy: ControllableDimensionPolicy,
    },
    Frozen {
        store_sha256: String,
        source: String,
        model: String,
        vector_size: usize,
        dimension_policy: String,
    },
    Live {
        provider: LiveEmbeddingProvider,
        model: String,
        vector_size: usize,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetrievalSectionBudgets {
    pub active_threads: usize,
    pub relevant_episodes: usize,
    pub salient_observations: usize,
    pub derived_memories: usize,
    pub preferences: usize,
    pub relationship_notes: usize,
    pub open_loops: usize,
    pub commitments: usize,
    pub character_signals: usize,
}

impl Default for RetrievalSectionBudgets {
    fn default() -> Self {
        Self {
            active_threads: 6,
            relevant_episodes: 8,
            salient_observations: 16,
            derived_memories: 12,
            preferences: 8,
            relationship_notes: 8,
            open_loops: 8,
            commitments: 8,
            character_signals: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetrievalSurfacePolicy {
    pub sections: RetrievalSectionBudgets,
    pub object_types: Vec<ObjectType>,
    pub include_debug_rationale: bool,
    pub max_vector_candidates: Option<usize>,
    pub max_graph_roots: Option<usize>,
}

impl RetrievalSurfacePolicy {
    pub fn validate(&self) -> Result<()> {
        if self.object_types.is_empty() {
            bail!("retrieval surface policy must select at least one object type");
        }
        if self.max_vector_candidates == Some(0) || self.max_graph_roots == Some(0) {
            bail!("retrieval candidate limits must be greater than zero when present");
        }
        if let (Some(vector), Some(roots)) = (self.max_vector_candidates, self.max_graph_roots)
            && roots > vector
        {
            bail!("max_graph_roots ({roots}) must not exceed max_vector_candidates ({vector})");
        }
        Ok(())
    }

    pub fn validate_for_vector_only(&self) -> Result<()> {
        self.validate_for_text_baseline("vector_only")
    }

    pub fn validate_for_bm25_only(&self) -> Result<()> {
        self.validate_for_text_baseline("bm25_only")
    }

    fn validate_for_text_baseline(&self, mode: &str) -> Result<()> {
        self.validate()?;
        let unsupported = self
            .object_types
            .iter()
            .copied()
            .filter(|object_type| {
                !matches!(object_type, ObjectType::Episode | ObjectType::Observation)
            })
            .collect::<Vec<_>>();
        if !unsupported.is_empty() {
            bail!(
                "retrieval.mode={mode} supports only episode and observation object_types; unsupported selections: {}",
                unsupported
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        for (object_type, budget) in [
            (ObjectType::Episode, self.sections.relevant_episodes),
            (ObjectType::Observation, self.sections.salient_observations),
        ] {
            if self.object_types.contains(&object_type) && budget == 0 {
                bail!("retrieval.mode={mode} selected {object_type} with a zero section budget");
            }
        }
        Ok(())
    }
}

impl Default for RetrievalSurfacePolicy {
    fn default() -> Self {
        Self {
            sections: RetrievalSectionBudgets::default(),
            object_types: vec![
                ObjectType::Episode,
                ObjectType::Observation,
                ObjectType::DerivedMemory,
                ObjectType::MemoryThread,
                ObjectType::Entity,
            ],
            include_debug_rationale: false,
            max_vector_candidates: None,
            max_graph_roots: None,
        }
    }
}
