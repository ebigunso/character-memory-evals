use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use cmem_eval_core::ControllableSimilarityFixture;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CONTINUITY_FIXTURE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityFixtureSet {
    pub schema_version: u32,
    pub seed: u64,
    pub scenarios: Vec<ContinuityScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityScenario {
    pub fixture_id: String,
    pub namespace: String,
    pub collection_name: String,
    pub pattern: ScenarioPattern,
    pub entities: Vec<EntityDeclaration>,
    pub embedding: ControllableSimilarityFixture,
    pub events: Vec<InteractionEvent>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioPattern {
    LongGapRecall,
    RecurringHubEntity,
    SelectiveEntity,
    CorrectionChains,
    ThreadDrift,
    TemporalStructure,
    MixedSalienceAccumulation,
    CrossStoreStress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityDeclaration {
    pub memory_id: Uuid,
    pub external_id: String,
    pub entity_type: String,
    pub label: String,
    pub is_hub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionEvent {
    Remember {
        event_id: String,
        memory_id: Uuid,
        external_id: String,
        timestamp: DateTime<Utc>,
        text: String,
        entity_external_ids: Vec<String>,
        thread: Option<ThreadMembership>,
        salience: f32,
    },
    Correct {
        event_id: String,
        replacement_memory_id: Uuid,
        target_external_id: String,
        replacement_external_id: String,
        timestamp: DateTime<Utc>,
        replacement_text: String,
    },
    Forget {
        event_id: String,
        target_external_id: String,
        timestamp: DateTime<Utc>,
    },
    Link {
        event_id: String,
        memory_id: Uuid,
        external_id: String,
        timestamp: DateTime<Utc>,
        from_external_id: String,
        relation: String,
        to_external_id: String,
    },
    Restart {
        event_id: String,
        timestamp: DateTime<Utc>,
        reopen_graph: bool,
        reopen_stats: bool,
    },
    Query {
        event_id: String,
        query_id: String,
        timestamp: DateTime<Utc>,
        text: String,
        expected: ExpectedRelevance,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadMembership {
    pub thread_external_id: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpectedRelevance {
    pub relevant_external_ids: Vec<String>,
    pub irrelevant_external_ids: Vec<String>,
}

impl ContinuityFixtureSet {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != CONTINUITY_FIXTURE_SCHEMA_VERSION {
            bail!(
                "unsupported continuity fixture schema_version {}; expected {}",
                self.schema_version,
                CONTINUITY_FIXTURE_SCHEMA_VERSION
            );
        }
        if self.scenarios.is_empty() {
            bail!("continuity fixture set must contain scenarios");
        }

        let mut fixture_ids = BTreeSet::new();
        let mut namespaces = BTreeSet::new();
        let mut collections = BTreeSet::new();
        for scenario in &self.scenarios {
            require_non_empty("fixture_id", &scenario.fixture_id)?;
            require_non_empty("namespace", &scenario.namespace)?;
            require_non_empty("collection_name", &scenario.collection_name)?;
            if !fixture_ids.insert(&scenario.fixture_id) {
                bail!("duplicate continuity fixture_id {:?}", scenario.fixture_id);
            }
            if !namespaces.insert(&scenario.namespace) {
                bail!("duplicate continuity namespace {:?}", scenario.namespace);
            }
            if !collections.insert(&scenario.collection_name) {
                bail!(
                    "duplicate continuity collection_name {:?}",
                    scenario.collection_name
                );
            }
            scenario.validate()?;
        }
        Ok(())
    }
}

impl ContinuityScenario {
    pub fn validate(&self) -> Result<()> {
        cmem_eval_core::ControllableSimilarityEmbeddingProvider::new(self.embedding.clone())?;
        let declared_entities = self
            .entities
            .iter()
            .map(|entity| entity.external_id.as_str())
            .collect::<BTreeSet<_>>();
        if declared_entities.len() != self.entities.len() {
            bail!(
                "scenario {:?} has duplicate entity external IDs",
                self.fixture_id
            );
        }

        let mut event_ids = BTreeSet::new();
        let mut memory_ids = self
            .entities
            .iter()
            .map(|entity| entity.memory_id)
            .collect::<BTreeSet<_>>();
        let mut previous_timestamp = None;
        let assigned_inputs = self
            .embedding
            .concepts
            .values()
            .flat_map(|concept| concept.inputs.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();

        for event in &self.events {
            let event_id = event.event_id();
            require_non_empty("event_id", event_id)?;
            if !event_ids.insert(event_id) {
                bail!(
                    "scenario {:?} has duplicate event_id {event_id:?}",
                    self.fixture_id
                );
            }
            let timestamp = event.timestamp();
            if previous_timestamp.is_some_and(|previous| timestamp < previous) {
                bail!(
                    "scenario {:?} events are not chronological",
                    self.fixture_id
                );
            }
            previous_timestamp = Some(timestamp);

            if let Some(memory_id) = event.created_memory_id()
                && !memory_ids.insert(memory_id)
            {
                bail!(
                    "scenario {:?} reuses memory_id {memory_id}",
                    self.fixture_id
                );
            }
            if let InteractionEvent::Remember {
                entity_external_ids,
                text,
                ..
            } = event
            {
                for entity_id in entity_external_ids {
                    if !declared_entities.contains(entity_id.as_str()) {
                        bail!(
                            "scenario {:?} references undeclared entity {entity_id:?}",
                            self.fixture_id
                        );
                    }
                }
                require_embedding_input(&self.fixture_id, &assigned_inputs, text)?;
            }
            if let InteractionEvent::Correct {
                replacement_text, ..
            } = event
            {
                require_embedding_input(&self.fixture_id, &assigned_inputs, replacement_text)?;
            }
            if let InteractionEvent::Query { text, expected, .. } = event {
                require_embedding_input(&self.fixture_id, &assigned_inputs, text)?;
                if expected.relevant_external_ids.is_empty() {
                    bail!(
                        "scenario {:?} query must declare relevant external IDs",
                        self.fixture_id
                    );
                }
            }
        }
        Ok(())
    }
}

impl InteractionEvent {
    pub fn event_id(&self) -> &str {
        match self {
            Self::Remember { event_id, .. }
            | Self::Correct { event_id, .. }
            | Self::Forget { event_id, .. }
            | Self::Link { event_id, .. }
            | Self::Restart { event_id, .. }
            | Self::Query { event_id, .. } => event_id,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Remember { timestamp, .. }
            | Self::Correct { timestamp, .. }
            | Self::Forget { timestamp, .. }
            | Self::Link { timestamp, .. }
            | Self::Restart { timestamp, .. }
            | Self::Query { timestamp, .. } => *timestamp,
        }
    }

    fn created_memory_id(&self) -> Option<Uuid> {
        match self {
            Self::Remember { memory_id, .. } | Self::Link { memory_id, .. } => Some(*memory_id),
            Self::Correct {
                replacement_memory_id,
                ..
            } => Some(*replacement_memory_id),
            Self::Forget { .. } | Self::Restart { .. } | Self::Query { .. } => None,
        }
    }
}

pub fn canonical_fixture_bytes(fixtures: &ContinuityFixtureSet) -> Result<Vec<u8>> {
    fixtures.validate()?;
    let mut bytes = serde_json::to_vec_pretty(fixtures)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn parse_fixture_bytes(bytes: &[u8]) -> Result<ContinuityFixtureSet> {
    let fixtures: ContinuityFixtureSet = serde_json::from_slice(bytes)?;
    fixtures.validate()?;
    Ok(fixtures)
}

pub fn scenario_patterns(fixtures: &ContinuityFixtureSet) -> BTreeMap<ScenarioPattern, usize> {
    let mut counts = BTreeMap::new();
    for scenario in &fixtures.scenarios {
        *counts.entry(scenario.pattern).or_insert(0) += 1;
    }
    counts
}

fn require_embedding_input(
    fixture_id: &str,
    assigned_inputs: &BTreeSet<&str>,
    text: &str,
) -> Result<()> {
    if !assigned_inputs.contains(text) {
        bail!("scenario {fixture_id:?} has text without an embedding concept assignment");
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("continuity fixture {field} must be non-empty");
    }
    Ok(())
}
