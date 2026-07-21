use crate::{
    ControllableSimilarityFixture, FrozenEmbeddingDimensionPolicy, FrozenEmbeddingProvider,
    ObjectType,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DatasetId(String);

impl DatasetId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
        {
            bail!(
                "dataset ID must use non-empty lowercase ASCII letters, digits, '_' or '-': {value:?}"
            );
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DatasetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for DatasetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DatasetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DatasetKind {
    LongMemEvalS,
    LoCoMo,
    Synthetic,
    Continuity,
}

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
        dimension_policy: FrozenEmbeddingDimensionPolicy,
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
    Controllable {
        fixture_sha256: String,
        vector_size: usize,
        dimension_policy: ControllableDimensionPolicy,
    },
    Frozen {
        store_sha256: String,
        model: String,
        vector_size: usize,
        dimension_policy: FrozenEmbeddingDimensionPolicy,
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
        self.validate()?;
        let unsupported = self
            .object_types
            .iter()
            .copied()
            .filter(|object_type| {
                !matches!(object_type, ObjectType::Episode | ObjectType::Observation)
            })
            .map(|object_type| object_type.to_string())
            .collect::<Vec<_>>();
        if !unsupported.is_empty() {
            bail!(
                "retrieval.mode=vector_only supports only episode and observation object_types; unsupported selections: {}",
                unsupported.join(", ")
            );
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
