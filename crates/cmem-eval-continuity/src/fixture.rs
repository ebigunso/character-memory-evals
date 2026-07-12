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
    /// Embedding input as well as display text. Scenario authors must assign it
    /// exactly once in `embedding.concepts`; the generator uses the concept of
    /// the first `Remember` that explicitly references this entity, or the
    /// deterministic `entity_background` concept when no event references it.
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
    /// Previously admitted external IDs expected to be relevant to this query.
    pub relevant_external_ids: Vec<String>,
    /// Sampled, previously admitted negative IDs used for pollution scoring.
    ///
    /// This is not an exhaustive list of every non-relevant ID in the scenario.
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
        for entity in &self.entities {
            require_non_empty("entity.external_id", &entity.external_id)?;
            require_non_empty("entity.entity_type", &entity.entity_type)?;
            require_non_empty("entity.label", &entity.label)?;
            let assignment_count = self
                .embedding
                .concepts
                .values()
                .flat_map(|concept| &concept.inputs)
                .filter(|input| *input == &entity.label)
                .count();
            if assignment_count != 1 {
                bail!(
                    "scenario {:?} entity label {:?} must have exactly one embedding concept assignment; found {assignment_count}",
                    self.fixture_id,
                    entity.label
                );
            }
        }

        let mut event_ids = BTreeSet::new();
        let mut admitted_external_ids = self
            .entities
            .iter()
            .map(|entity| entity.external_id.clone())
            .collect::<BTreeSet<_>>();
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
            match event {
                InteractionEvent::Remember {
                    external_id,
                    entity_external_ids,
                    text,
                    ..
                } => {
                    for entity_id in entity_external_ids {
                        if !declared_entities.contains(entity_id.as_str()) {
                            bail!(
                                "scenario {:?} references undeclared entity {entity_id:?}",
                                self.fixture_id
                            );
                        }
                    }
                    require_embedding_input(&self.fixture_id, &assigned_inputs, text)?;
                    admit_external_id(
                        &self.fixture_id,
                        "remember.external_id",
                        external_id,
                        &mut admitted_external_ids,
                    )?;
                }
                InteractionEvent::Correct {
                    target_external_id,
                    replacement_external_id,
                    replacement_text,
                    ..
                } => {
                    require_admitted_external_id(
                        &self.fixture_id,
                        "correct.target_external_id",
                        target_external_id,
                        &admitted_external_ids,
                    )?;
                    require_embedding_input(&self.fixture_id, &assigned_inputs, replacement_text)?;
                    admit_external_id(
                        &self.fixture_id,
                        "correct.replacement_external_id",
                        replacement_external_id,
                        &mut admitted_external_ids,
                    )?;
                }
                InteractionEvent::Forget {
                    target_external_id, ..
                } => {
                    require_admitted_external_id(
                        &self.fixture_id,
                        "forget.target_external_id",
                        target_external_id,
                        &admitted_external_ids,
                    )?;
                }
                InteractionEvent::Link {
                    external_id,
                    from_external_id,
                    to_external_id,
                    ..
                } => {
                    require_admitted_external_id(
                        &self.fixture_id,
                        "link.from_external_id",
                        from_external_id,
                        &admitted_external_ids,
                    )?;
                    require_admitted_external_id(
                        &self.fixture_id,
                        "link.to_external_id",
                        to_external_id,
                        &admitted_external_ids,
                    )?;
                    admit_external_id(
                        &self.fixture_id,
                        "link.external_id",
                        external_id,
                        &mut admitted_external_ids,
                    )?;
                }
                InteractionEvent::Query { text, expected, .. } => {
                    require_embedding_input(&self.fixture_id, &assigned_inputs, text)?;
                    validate_expected_relevance(
                        &self.fixture_id,
                        expected,
                        &admitted_external_ids,
                    )?;
                }
                InteractionEvent::Restart { .. } => {}
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

fn admit_external_id(
    fixture_id: &str,
    field: &str,
    external_id: &str,
    admitted_external_ids: &mut BTreeSet<String>,
) -> Result<()> {
    require_non_empty(field, external_id)?;
    if !admitted_external_ids.insert(external_id.to_string()) {
        bail!("scenario {fixture_id:?} {field} duplicates existing external ID {external_id:?}");
    }
    Ok(())
}

fn require_admitted_external_id(
    fixture_id: &str,
    field: &str,
    external_id: &str,
    admitted_external_ids: &BTreeSet<String>,
) -> Result<()> {
    if !admitted_external_ids.contains(external_id) {
        bail!(
            "scenario {fixture_id:?} {field} references external ID {external_id:?} before it is admitted"
        );
    }
    Ok(())
}

fn validate_expected_relevance(
    fixture_id: &str,
    expected: &ExpectedRelevance,
    admitted_external_ids: &BTreeSet<String>,
) -> Result<()> {
    if expected.relevant_external_ids.is_empty() {
        bail!("scenario {fixture_id:?} query must declare relevant_external_ids");
    }
    if expected.irrelevant_external_ids.is_empty() {
        bail!("scenario {fixture_id:?} query must declare irrelevant_external_ids");
    }

    let relevant = expected
        .relevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if relevant.len() != expected.relevant_external_ids.len() {
        bail!("scenario {fixture_id:?} query relevant_external_ids contains duplicates");
    }
    let irrelevant = expected
        .irrelevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if irrelevant.len() != expected.irrelevant_external_ids.len() {
        bail!("scenario {fixture_id:?} query irrelevant_external_ids contains duplicates");
    }
    if let Some(overlap) = relevant.intersection(&irrelevant).next() {
        bail!("scenario {fixture_id:?} query relevance labels overlap at external ID {overlap:?}");
    }
    for external_id in relevant.iter().chain(irrelevant.iter()) {
        require_admitted_external_id(
            fixture_id,
            "query relevance label",
            external_id,
            admitted_external_ids,
        )?;
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("continuity fixture {field} must be non-empty");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};

    fn parse_error(fixtures: &ContinuityFixtureSet) -> String {
        let bytes = serde_json::to_vec(fixtures).unwrap();
        parse_fixture_bytes(&bytes).unwrap_err().to_string()
    }

    fn scenario_mut(
        fixtures: &mut ContinuityFixtureSet,
        pattern: ScenarioPattern,
    ) -> &mut ContinuityScenario {
        fixtures
            .scenarios
            .iter_mut()
            .find(|scenario| scenario.pattern == pattern)
            .unwrap()
    }

    fn expected_mut(scenario: &mut ContinuityScenario) -> &mut ExpectedRelevance {
        scenario
            .events
            .iter_mut()
            .find_map(|event| match event {
                InteractionEvent::Query { expected, .. } => Some(expected),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn public_parser_requires_pollution_labels_to_be_present_and_non_empty() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let mut value = serde_json::to_value(&fixtures).unwrap();
        let scenarios = value["scenarios"].as_array_mut().unwrap();
        let events = scenarios[0]["events"].as_array_mut().unwrap();
        let expected = events
            .iter_mut()
            .find_map(|event| event.get_mut("expected"))
            .unwrap()
            .as_object_mut()
            .unwrap();
        expected.remove("irrelevant_external_ids");
        let error = parse_fixture_bytes(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("irrelevant_external_ids"), "{error}");

        let mut fixtures = fixtures;
        expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall))
            .irrelevant_external_ids
            .clear();
        let error = parse_error(&fixtures);
        assert!(error.contains("declare irrelevant_external_ids"), "{error}");
    }

    #[test]
    fn public_parser_rejects_duplicate_or_overlapping_relevance_labels() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let expected = expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall));
        expected
            .relevant_external_ids
            .push(expected.relevant_external_ids[0].clone());
        let error = parse_error(&fixtures);
        assert!(
            error.contains("relevant_external_ids contains duplicates"),
            "{error}"
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let expected = expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall));
        expected
            .irrelevant_external_ids
            .push(expected.irrelevant_external_ids[0].clone());
        let error = parse_error(&fixtures);
        assert!(
            error.contains("irrelevant_external_ids contains duplicates"),
            "{error}"
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let expected = expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall));
        expected
            .irrelevant_external_ids
            .push(expected.relevant_external_ids[0].clone());
        let error = parse_error(&fixtures);
        assert!(error.contains("relevance labels overlap"), "{error}");
    }

    #[test]
    fn public_parser_rejects_relevance_labels_before_external_id_admission() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        scenario.events.swap(1, 2);
        let error = parse_error(&fixtures);
        assert!(error.contains("query relevance label"), "{error}");
        assert!(error.contains("memory-recent"), "{error}");
        assert!(error.contains("before it is admitted"), "{error}");
    }

    #[test]
    fn public_parser_rejects_dangling_correction_and_forget_targets() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Correct {
            target_external_id, ..
        } = &mut scenario.events[1]
        else {
            panic!("expected correction event");
        };
        *target_external_id = "missing-correction-target".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("correct.target_external_id"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Forget {
            target_external_id, ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        *target_external_id = "missing-forget-target".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("forget.target_external_id"), "{error}");
    }

    #[test]
    fn public_parser_rejects_dangling_link_endpoints() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let InteractionEvent::Link {
            from_external_id, ..
        } = &mut scenario.events[1]
        else {
            panic!("expected link event");
        };
        *from_external_id = "missing-link-source".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("link.from_external_id"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let InteractionEvent::Link { to_external_id, .. } = &mut scenario.events[1] else {
            panic!("expected link event");
        };
        *to_external_id = "missing-link-target".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("link.to_external_id"), "{error}");
    }

    #[test]
    fn public_parser_rejects_duplicate_created_external_ids() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let InteractionEvent::Remember { external_id, .. } = &mut scenario.events[1] else {
            panic!("expected remember event");
        };
        *external_id = "memory-dormant".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("remember.external_id"), "{error}");
        assert!(error.contains("duplicates existing external ID"), "{error}");
    }

    #[test]
    fn checked_fixture_json_shape_remains_publicly_parseable() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let value: Value =
            serde_json::from_slice(&canonical_fixture_bytes(&fixtures).unwrap()).unwrap();
        assert!(value["scenarios"].is_array());
        assert_eq!(
            parse_fixture_bytes(&serde_json::to_vec(&value).unwrap()).unwrap(),
            fixtures
        );
    }
}
