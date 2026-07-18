use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use cmem_eval_core::ControllableSimilarityFixture;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use serde_json::{Map, Value};

pub const CONTINUITY_FIXTURE_SCHEMA_VERSION_V2: u32 = 2;
pub const CONTINUITY_FIXTURE_SCHEMA_VERSION_V3: u32 = 3;
pub const CONTINUITY_FIXTURE_SCHEMA_VERSION: u32 = CONTINUITY_FIXTURE_SCHEMA_VERSION_V2;
pub const LATEST_CONTINUITY_FIXTURE_SCHEMA_VERSION: u32 = CONTINUITY_FIXTURE_SCHEMA_VERSION_V3;

/// Relation names accepted by continuity fixtures and checked against the live
/// CharacterMemory facade by the adapter crate's exhaustive parity test.
pub const CONTINUITY_RELATION_VOCABULARY: &[&str] = &[
    "has_observation",
    "observed_in",
    "mentions",
    "involves",
    "about",
    "derived_from",
    "part_of_thread",
    "supports",
    "contradicts",
    "supersedes",
    "resolves",
    "creates_open_loop",
    "fulfills_commitment",
    "associated_with",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContinuityFixtureSet {
    pub schema_version: u32,
    pub seed: u64,
    pub scenarios: Vec<ContinuityScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContinuityScenario {
    pub fixture_id: String,
    pub namespace: String,
    pub pattern: ScenarioPattern,
    pub entities: Vec<EntityDeclaration>,
    pub embedding: ContinuityScenarioEmbedding,
    pub events: Vec<InteractionEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContinuityScenarioEmbedding {
    LegacyControllableSimilarity(ControllableSimilarityFixture),
    ControllableSimilarity(ControllableSimilarityFixture),
    Frozen,
}

impl ContinuityScenarioEmbedding {
    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::LegacyControllableSimilarity(_) | Self::ControllableSimilarity(_) => {
                "controllable_similarity"
            }
            Self::Frozen => "frozen",
        }
    }

    pub fn controllable_similarity(&self) -> Option<&ControllableSimilarityFixture> {
        match self {
            Self::LegacyControllableSimilarity(fixture) | Self::ControllableSimilarity(fixture) => {
                Some(fixture)
            }
            Self::Frozen => None,
        }
    }

    pub fn controllable_similarity_mut(&mut self) -> Option<&mut ControllableSimilarityFixture> {
        match self {
            Self::LegacyControllableSimilarity(fixture) | Self::ControllableSimilarity(fixture) => {
                Some(fixture)
            }
            Self::Frozen => None,
        }
    }

    pub fn is_legacy_v2(&self) -> bool {
        matches!(self, Self::LegacyControllableSimilarity(_))
    }

    pub fn tagged_controllable_similarity(fixture: ControllableSimilarityFixture) -> Self {
        Self::ControllableSimilarity(fixture)
    }

    pub fn frozen() -> Self {
        Self::Frozen
    }

    pub fn vector_size(&self) -> Option<usize> {
        self.controllable_similarity()
            .map(|fixture| fixture.vector_size)
    }
}

impl From<ControllableSimilarityFixture> for ContinuityScenarioEmbedding {
    fn from(value: ControllableSimilarityFixture) -> Self {
        Self::LegacyControllableSimilarity(value)
    }
}

impl Serialize for ContinuityScenarioEmbedding {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::LegacyControllableSimilarity(fixture) => fixture.serialize(serializer),
            Self::ControllableSimilarity(fixture) => {
                let mut object = serde_json::to_value(fixture)
                    .map_err(serde::ser::Error::custom)?
                    .as_object()
                    .cloned()
                    .expect("controllable similarity fixture serializes as an object");
                object.insert(
                    "provider".to_string(),
                    Value::String("controllable_similarity".to_string()),
                );
                object.serialize(serializer)
            }
            Self::Frozen => {
                Map::from_iter([("provider".to_string(), Value::String("frozen".to_string()))])
                    .serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ContinuityScenarioEmbedding {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("continuity embedding block must be an object"))?;
        match object.get("provider") {
            Some(Value::String(provider)) if provider == "frozen" => {
                if let Some(field) = unknown_embedding_field(object, &["provider"]) {
                    return Err(D::Error::custom(format!(
                        "unknown field {field:?} in frozen embedding block"
                    )));
                }
                Ok(Self::Frozen)
            }
            Some(Value::String(provider)) if provider == "controllable_similarity" => {
                if let Some(field) = unknown_embedding_field(
                    object,
                    &[
                        "provider",
                        "seed",
                        "vector_size",
                        "noise_magnitude",
                        "clusters",
                        "concepts",
                    ],
                ) {
                    return Err(D::Error::custom(format!(
                        "unknown field {field:?} in controllable_similarity embedding block"
                    )));
                }
                let mut fixture = object.clone();
                fixture.remove("provider");
                serde_json::from_value(Value::Object(fixture))
                    .map(Self::ControllableSimilarity)
                    .map_err(|error| {
                        D::Error::custom(format!(
                            "malformed controllable_similarity embedding block: {error}"
                        ))
                    })
            }
            Some(Value::String(provider)) => Err(D::Error::custom(format!(
                "unsupported continuity embedding provider {provider:?}; expected controllable_similarity or frozen"
            ))),
            Some(_) => Err(D::Error::custom(
                "continuity embedding field `provider` must be a string",
            )),
            None => {
                if let Some(field) = unknown_embedding_field(
                    object,
                    &[
                        "seed",
                        "vector_size",
                        "noise_magnitude",
                        "clusters",
                        "concepts",
                    ],
                ) {
                    return Err(D::Error::custom(format!(
                        "unknown field {field:?} in legacy schema-v2 controllable_similarity embedding block; frozen schema-v3 blocks require `provider: frozen`"
                    )));
                }
                serde_json::from_value(value)
                    .map(Self::LegacyControllableSimilarity)
                    .map_err(|error| {
                        D::Error::custom(format!(
                            "malformed legacy schema-v2 controllable_similarity embedding block; frozen schema-v3 blocks require `provider: frozen`: {error}"
                        ))
                    })
            }
        }
    }
}

fn unknown_embedding_field<'a>(
    object: &'a Map<String, Value>,
    allowed: &[&str],
) -> Option<&'a str> {
    object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
        .map(String::as_str)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioPattern {
    LongGapRecall,
    RecurringHubEntity,
    HubScale,
    SelectiveEntity,
    CorrectionChains,
    ThreadDrift,
    TemporalStructure,
    MixedSalienceAccumulation,
    CrossStoreStress,
    SurfaceContribution,
    MultiEvidenceAssembly,
    Abstention,
    GradedSimilarity,
    CombinedLife,
    TemporalPatterns,
    EntrenchedCorrection,
    Autobiographical,
}

impl ScenarioPattern {
    fn requires_schema_v3(self) -> bool {
        matches!(
            self,
            Self::MultiEvidenceAssembly
                | Self::Abstention
                | Self::GradedSimilarity
                | Self::CombinedLife
                | Self::TemporalPatterns
                | Self::EntrenchedCorrection
                | Self::Autobiographical
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityDeclaration {
    pub external_id: String,
    pub entity_type: ContinuityEntityKind,
    /// Embedding input as well as display text. Scenario authors must assign it
    /// exactly once in `embedding.concepts`; the generator uses the concept of
    /// the first `Remember` that explicitly references this entity, or the
    /// deterministic `entity_background` concept when no event references it.
    pub label: String,
    pub is_hub: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityEntityKind {
    Location,
    Person,
    Organization,
}

impl ContinuityEntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Location => "location",
            Self::Person => "person",
            Self::Organization => "organization",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContinuityObjectKind {
    Episode,
    Observation,
    Entity,
    MemoryThread,
    DerivedMemory,
    MemoryLink,
}

impl ContinuityObjectKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Episode => "episode",
            Self::Observation => "observation",
            Self::Entity => "entity",
            Self::MemoryThread => "memory_thread",
            Self::DerivedMemory => "derived_memory",
            Self::MemoryLink => "memory_link",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InteractionEvent {
    Remember {
        event_id: String,
        external_id: String,
        timestamp: DateTime<Utc>,
        /// Default Episode, Observation, and DerivedMemory text. When
        /// `surface_texts` is present, this must equal its Episode text.
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        surface_texts: Option<RememberSurfaceTexts>,
        entity_external_ids: Vec<String>,
        thread: Option<ThreadMembership>,
        salience: f32,
    },
    Correct {
        event_id: String,
        target_external_id: String,
        replacement_external_id: String,
        timestamp: DateTime<Utc>,
        replacement_text: String,
    },
    Forget {
        event_id: String,
        target_external_ids: Vec<String>,
        timestamp: DateTime<Utc>,
        suppress_derived_from_target: bool,
        apply_to_derived_from_target: bool,
    },
    Link {
        event_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RememberSurfaceTexts {
    pub episode: String,
    pub observation: String,
    pub derived: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThreadMembership {
    pub thread_external_id: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpectedRelevance {
    /// Previously admitted external IDs expected to be relevant to this query.
    pub relevant_external_ids: Vec<String>,
    /// Sampled, previously admitted negative IDs used for pollution scoring.
    ///
    /// This is not an exhaustive list of every non-relevant ID in the scenario.
    pub irrelevant_external_ids: Vec<String>,
}

impl ContinuityFixtureSet {
    pub fn into_schema_v3(mut self) -> Result<Self> {
        self.schema_version = CONTINUITY_FIXTURE_SCHEMA_VERSION_V3;
        for scenario in &mut self.scenarios {
            if let ContinuityScenarioEmbedding::LegacyControllableSimilarity(fixture) =
                &scenario.embedding
            {
                scenario.embedding =
                    ContinuityScenarioEmbedding::ControllableSimilarity(fixture.clone());
            }
        }
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        if !matches!(
            self.schema_version,
            CONTINUITY_FIXTURE_SCHEMA_VERSION_V2 | CONTINUITY_FIXTURE_SCHEMA_VERSION_V3
        ) {
            bail!(
                "unsupported continuity fixture schema_version {}; expected {} or {}",
                self.schema_version,
                CONTINUITY_FIXTURE_SCHEMA_VERSION_V2,
                CONTINUITY_FIXTURE_SCHEMA_VERSION_V3
            );
        }
        if self.scenarios.is_empty() {
            bail!("continuity fixture set must contain scenarios");
        }

        let mut fixture_ids = BTreeSet::new();
        let mut namespaces = BTreeSet::new();
        let mut query_ids = BTreeSet::new();
        for scenario in &self.scenarios {
            match self.schema_version {
                CONTINUITY_FIXTURE_SCHEMA_VERSION_V2 if !scenario.embedding.is_legacy_v2() => {
                    bail!(
                        "continuity fixture schema_version 2 requires legacy controllable_similarity embedding blocks; provider-tagged and frozen blocks require schema_version 3"
                    );
                }
                CONTINUITY_FIXTURE_SCHEMA_VERSION_V3 if scenario.embedding.is_legacy_v2() => {
                    bail!(
                        "continuity fixture schema_version 3 requires every embedding block to declare provider=controllable_similarity or provider=frozen"
                    );
                }
                _ => {}
            }
            if self.schema_version == CONTINUITY_FIXTURE_SCHEMA_VERSION_V2
                && scenario.pattern.requires_schema_v3()
            {
                bail!(
                    "continuity scenario pattern {:?} requires schema_version 3",
                    scenario.pattern
                );
            }
            require_non_empty("fixture_id", &scenario.fixture_id)?;
            require_non_empty("namespace", &scenario.namespace)?;
            if !fixture_ids.insert(&scenario.fixture_id) {
                bail!("duplicate continuity fixture_id {:?}", scenario.fixture_id);
            }
            if !namespaces.insert(&scenario.namespace) {
                bail!("duplicate continuity namespace {:?}", scenario.namespace);
            }
            scenario.validate()?;
            for event in &scenario.events {
                if let InteractionEvent::Query { query_id, .. } = event
                    && !query_ids.insert(query_id)
                {
                    bail!("continuity fixture set has duplicate query_id {query_id:?}");
                }
            }
        }
        Ok(())
    }
}

impl ContinuityScenario {
    pub fn embedding_inputs(&self) -> BTreeSet<&str> {
        let mut inputs = self
            .entities
            .iter()
            .map(|entity| entity.label.as_str())
            .collect::<BTreeSet<_>>();
        for event in &self.events {
            match event {
                InteractionEvent::Remember {
                    text,
                    surface_texts,
                    ..
                } => {
                    inputs.insert(text);
                    if let Some(surface_texts) = surface_texts {
                        inputs.insert(&surface_texts.episode);
                        inputs.insert(&surface_texts.observation);
                        inputs.insert(&surface_texts.derived);
                    }
                }
                InteractionEvent::Correct {
                    replacement_text, ..
                } => {
                    inputs.insert(replacement_text);
                }
                InteractionEvent::Query { text, .. } => {
                    inputs.insert(text);
                }
                InteractionEvent::Forget { .. }
                | InteractionEvent::Link { .. }
                | InteractionEvent::Restart { .. } => {}
            }
        }
        inputs
    }

    pub fn validate(&self) -> Result<()> {
        let controllable_embedding = self.embedding.controllable_similarity();
        if let Some(embedding) = controllable_embedding {
            cmem_eval_core::ControllableSimilarityEmbeddingProvider::new(embedding.clone())?;
        }
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
            require_non_empty("entity.label", &entity.label)?;
            if let Some(embedding) = controllable_embedding {
                let assignment_count = embedding
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
        }

        let mut event_ids = BTreeSet::new();
        let mut query_ids = BTreeSet::new();
        let mut admitted_external_ids = self
            .entities
            .iter()
            .map(|entity| (entity.external_id.clone(), ContinuityObjectKind::Entity))
            .collect::<BTreeMap<_, _>>();
        let mut previous_timestamp = None;
        let assigned_inputs = controllable_embedding.map(|embedding| {
            embedding
                .concepts
                .values()
                .flat_map(|concept| concept.inputs.iter().map(String::as_str))
                .collect::<BTreeSet<_>>()
        });

        for (event_index, event) in self.events.iter().enumerate() {
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

            match event {
                InteractionEvent::Remember {
                    external_id,
                    entity_external_ids,
                    thread,
                    salience,
                    text,
                    surface_texts,
                    ..
                } => {
                    require_unit_interval("remember.salience", *salience)?;
                    if let Some(thread) = thread {
                        require_unit_interval("remember.thread.confidence", thread.confidence)?;
                    }
                    for entity_id in entity_external_ids {
                        if !declared_entities.contains(entity_id.as_str()) {
                            bail!(
                                "scenario {:?} references undeclared entity {entity_id:?}",
                                self.fixture_id
                            );
                        }
                    }
                    if let Some(surface_texts) = surface_texts {
                        if text != &surface_texts.episode {
                            bail!(
                                "scenario {:?} remember text must equal surface_texts.episode",
                                self.fixture_id
                            );
                        }
                        require_distinct_surface_texts(&self.fixture_id, surface_texts)?;
                        for surface_text in [
                            &surface_texts.episode,
                            &surface_texts.observation,
                            &surface_texts.derived,
                        ] {
                            require_embedding_input(
                                &self.fixture_id,
                                assigned_inputs.as_ref(),
                                surface_text,
                            )?;
                        }
                    }
                    require_embedding_input(&self.fixture_id, assigned_inputs.as_ref(), text)?;
                    admit_external_id(
                        &self.fixture_id,
                        "remember.external_id",
                        external_id,
                        ContinuityObjectKind::Episode,
                        &mut admitted_external_ids,
                    )?;
                    admit_external_id(
                        &self.fixture_id,
                        "remember.observation_external_id",
                        &observation_external_id(external_id),
                        ContinuityObjectKind::Observation,
                        &mut admitted_external_ids,
                    )?;
                    admit_external_id(
                        &self.fixture_id,
                        "remember.derived_external_id",
                        &derived_external_id(external_id),
                        ContinuityObjectKind::DerivedMemory,
                        &mut admitted_external_ids,
                    )?;
                    if let Some(thread) = thread {
                        admit_thread_external_id(
                            &self.fixture_id,
                            &thread.thread_external_id,
                            &mut admitted_external_ids,
                        )?;
                    }
                }
                InteractionEvent::Correct {
                    target_external_id,
                    replacement_external_id,
                    replacement_text,
                    ..
                } => {
                    require_admitted_kind(
                        &self.fixture_id,
                        "correct.target_external_id",
                        target_external_id,
                        &[
                            ContinuityObjectKind::Episode,
                            ContinuityObjectKind::Observation,
                            ContinuityObjectKind::DerivedMemory,
                        ],
                        &admitted_external_ids,
                    )?;
                    require_embedding_input(
                        &self.fixture_id,
                        assigned_inputs.as_ref(),
                        replacement_text,
                    )?;
                    admit_external_id(
                        &self.fixture_id,
                        "correct.replacement_external_id",
                        replacement_external_id,
                        ContinuityObjectKind::DerivedMemory,
                        &mut admitted_external_ids,
                    )?;
                }
                InteractionEvent::Forget {
                    target_external_ids,
                    ..
                } => {
                    if target_external_ids.is_empty() {
                        bail!(
                            "scenario {:?} forget target_external_ids must not be empty",
                            self.fixture_id
                        );
                    }
                    let unique_targets = target_external_ids.iter().collect::<BTreeSet<_>>();
                    if unique_targets.len() != target_external_ids.len() {
                        bail!(
                            "scenario {:?} forget target_external_ids contains duplicates",
                            self.fixture_id
                        );
                    }
                    for target_external_id in target_external_ids {
                        require_admitted_kind(
                            &self.fixture_id,
                            "forget.target_external_ids",
                            target_external_id,
                            &[
                                ContinuityObjectKind::Episode,
                                ContinuityObjectKind::Observation,
                                ContinuityObjectKind::DerivedMemory,
                                ContinuityObjectKind::MemoryThread,
                            ],
                            &admitted_external_ids,
                        )?;
                    }
                }
                InteractionEvent::Link {
                    external_id,
                    from_external_id,
                    relation,
                    to_external_id,
                    ..
                } => {
                    require_admitted_kind(
                        &self.fixture_id,
                        "link.from_external_id",
                        from_external_id,
                        &[
                            ContinuityObjectKind::Episode,
                            ContinuityObjectKind::Observation,
                            ContinuityObjectKind::Entity,
                            ContinuityObjectKind::MemoryThread,
                            ContinuityObjectKind::DerivedMemory,
                        ],
                        &admitted_external_ids,
                    )?;
                    require_admitted_kind(
                        &self.fixture_id,
                        "link.to_external_id",
                        to_external_id,
                        &[
                            ContinuityObjectKind::Episode,
                            ContinuityObjectKind::Observation,
                            ContinuityObjectKind::Entity,
                            ContinuityObjectKind::MemoryThread,
                            ContinuityObjectKind::DerivedMemory,
                        ],
                        &admitted_external_ids,
                    )?;
                    require_supported_relation(&self.fixture_id, relation)?;
                    admit_external_id(
                        &self.fixture_id,
                        "link.external_id",
                        external_id,
                        ContinuityObjectKind::MemoryLink,
                        &mut admitted_external_ids,
                    )?;
                }
                InteractionEvent::Query {
                    query_id,
                    text,
                    expected,
                    ..
                } => {
                    require_non_empty("query.query_id", query_id)?;
                    if !query_ids.insert(query_id) {
                        bail!(
                            "scenario {:?} has duplicate query_id {query_id:?}",
                            self.fixture_id
                        );
                    }
                    require_embedding_input(&self.fixture_id, assigned_inputs.as_ref(), text)?;
                    validate_expected_relevance(
                        &self.fixture_id,
                        self.pattern,
                        expected,
                        &admitted_external_ids,
                    )?;
                }
                InteractionEvent::Restart {
                    event_id,
                    reopen_graph,
                    reopen_stats,
                    ..
                } => {
                    if !reopen_graph || !reopen_stats {
                        bail!(
                            "scenario {:?} restart event {:?} must set reopen_graph=true and reopen_stats=true because the continuity runtime always reconstructs both stores",
                            self.fixture_id,
                            event_id
                        );
                    }
                    if !self.events[event_index + 1..]
                        .iter()
                        .any(|event| matches!(event, InteractionEvent::Query { .. }))
                    {
                        bail!(
                            "scenario {:?} restart event {:?} must have a following scripted query",
                            self.fixture_id,
                            event_id
                        );
                    }
                }
            }
        }
        if query_ids.is_empty() {
            bail!(
                "scenario {:?} must declare at least one scripted query",
                self.fixture_id
            );
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
}

pub(crate) fn observation_external_id(external_id: &str) -> String {
    format!("{external_id}:observation")
}

pub(crate) fn derived_external_id(external_id: &str) -> String {
    format!("{external_id}:derived")
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
    assigned_inputs: Option<&BTreeSet<&str>>,
    text: &str,
) -> Result<()> {
    if assigned_inputs.is_some_and(|assigned_inputs| !assigned_inputs.contains(text)) {
        bail!("scenario {fixture_id:?} has text without an embedding concept assignment");
    }
    Ok(())
}

fn admit_external_id(
    fixture_id: &str,
    field: &str,
    external_id: &str,
    kind: ContinuityObjectKind,
    admitted_external_ids: &mut BTreeMap<String, ContinuityObjectKind>,
) -> Result<()> {
    require_non_empty(field, external_id)?;
    if admitted_external_ids.contains_key(external_id) {
        bail!("scenario {fixture_id:?} {field} duplicates existing external ID {external_id:?}");
    }
    admitted_external_ids.insert(external_id.to_string(), kind);
    Ok(())
}

fn admit_thread_external_id(
    fixture_id: &str,
    external_id: &str,
    admitted_external_ids: &mut BTreeMap<String, ContinuityObjectKind>,
) -> Result<()> {
    require_non_empty("remember.thread.thread_external_id", external_id)?;
    match admitted_external_ids.get(external_id) {
        Some(ContinuityObjectKind::MemoryThread) => Ok(()),
        Some(kind) => bail!(
            "scenario {fixture_id:?} remember.thread.thread_external_id {external_id:?} collides with admitted {}",
            kind.as_str()
        ),
        None => {
            admitted_external_ids
                .insert(external_id.to_string(), ContinuityObjectKind::MemoryThread);
            Ok(())
        }
    }
}

fn require_admitted_external_id(
    fixture_id: &str,
    field: &str,
    external_id: &str,
    admitted_external_ids: &BTreeMap<String, ContinuityObjectKind>,
) -> Result<()> {
    if !admitted_external_ids.contains_key(external_id) {
        bail!(
            "scenario {fixture_id:?} {field} references external ID {external_id:?} before it is admitted"
        );
    }
    Ok(())
}

fn require_admitted_kind(
    fixture_id: &str,
    field: &str,
    external_id: &str,
    allowed: &[ContinuityObjectKind],
    admitted_external_ids: &BTreeMap<String, ContinuityObjectKind>,
) -> Result<()> {
    require_admitted_external_id(fixture_id, field, external_id, admitted_external_ids)?;
    let kind = admitted_external_ids[external_id];
    if !allowed.contains(&kind) {
        let allowed = allowed
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "scenario {fixture_id:?} {field} references {external_id:?} with unsupported kind {}; expected one of [{allowed}]",
            kind.as_str()
        );
    }
    Ok(())
}

fn require_supported_relation(fixture_id: &str, relation: &str) -> Result<()> {
    require_non_empty("link.relation", relation)?;
    if !CONTINUITY_RELATION_VOCABULARY.contains(&relation) {
        bail!(
            "scenario {fixture_id:?} link.relation {relation:?} is not supported by the CharacterMemory facade"
        );
    }
    Ok(())
}

fn validate_expected_relevance(
    fixture_id: &str,
    pattern: ScenarioPattern,
    expected: &ExpectedRelevance,
    admitted_external_ids: &BTreeMap<String, ContinuityObjectKind>,
) -> Result<()> {
    if pattern == ScenarioPattern::Abstention && !expected.relevant_external_ids.is_empty() {
        bail!(
            "scenario {fixture_id:?} uses pattern=abstention, so every query must have empty relevant_external_ids"
        );
    }
    if pattern != ScenarioPattern::Abstention && expected.relevant_external_ids.is_empty() {
        bail!(
            "scenario {fixture_id:?} is non-abstention and every query must declare relevant_external_ids"
        );
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

fn require_distinct_surface_texts(
    fixture_id: &str,
    surface_texts: &RememberSurfaceTexts,
) -> Result<()> {
    let texts = [
        surface_texts.episode.as_str(),
        surface_texts.observation.as_str(),
        surface_texts.derived.as_str(),
    ];
    for text in texts {
        require_non_empty("remember.surface_texts", text)?;
    }
    if texts.into_iter().collect::<BTreeSet<_>>().len() != texts.len() {
        bail!("scenario {fixture_id:?} remember surface_texts must be pairwise distinct");
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("continuity fixture {field} must be non-empty");
    }
    Ok(())
}

fn require_unit_interval(field: &str, value: f32) -> Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("continuity fixture {field} must be finite and within 0.0..=1.0; got {value}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};

    const EXPLICIT_V2_FIXTURE: &[u8] = br#"{
  "schema_version": 2,
  "seed": 7,
  "scenarios": [
    {
      "fixture_id": "explicit-v2-boundary",
      "namespace": "continuity-explicit-v2-boundary",
      "pattern": "long_gap_recall",
      "entities": [],
      "embedding": {
        "seed": 7,
        "vector_size": 1,
        "noise_magnitude": 0.0,
        "clusters": {"target": [1.0]},
        "concepts": {
          "target": {"cluster": "target", "inputs": ["remembered target", "target query"]}
        }
      },
      "events": [
        {
          "kind": "remember",
          "event_id": "event-001",
          "external_id": "memory-target",
          "timestamp": "2025-01-01T00:00:00Z",
          "text": "remembered target",
          "entity_external_ids": [],
          "salience": 0.8
        },
        {
          "kind": "query",
          "event_id": "event-002",
          "query_id": "query-target",
          "timestamp": "2025-02-01T00:00:00Z",
          "text": "target query",
          "expected": {
            "relevant_external_ids": ["memory-target"],
            "irrelevant_external_ids": []
          }
        }
      ]
    }
  ]
}"#;

    fn explicit_v2_fixture() -> ContinuityFixtureSet {
        parse_fixture_bytes(EXPLICIT_V2_FIXTURE).unwrap()
    }

    fn parse_error(fixtures: &ContinuityFixtureSet) -> String {
        let bytes = serde_json::to_vec(fixtures).unwrap();
        parse_fixture_bytes(&bytes).unwrap_err().to_string()
    }

    fn upgrade_to_v3(fixtures: &mut ContinuityFixtureSet) {
        *fixtures = fixtures.clone().into_schema_v3().unwrap();
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

    fn insert_link_before(
        scenario: &mut ContinuityScenario,
        index: usize,
        external_id: &str,
        from_external_id: &str,
    ) {
        let timestamp = scenario.events[index - 1].timestamp() + chrono::Duration::seconds(1);
        scenario.events.insert(
            index,
            InteractionEvent::Link {
                event_id: format!("event-test-link-{external_id}"),
                external_id: external_id.to_string(),
                timestamp,
                from_external_id: from_external_id.to_string(),
                relation: "mentions".to_string(),
                to_external_id: "entity-person".to_string(),
            },
        );
    }

    #[test]
    fn public_parser_rejects_v1_and_retired_caller_supplied_identity_fields() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut v1 = serde_json::to_value(&fixtures).unwrap();
        v1["schema_version"] = Value::from(1);
        let error = parse_fixture_bytes(&serde_json::to_vec(&v1).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("schema_version 1"), "{error}");
        assert!(
            error.contains(&CONTINUITY_FIXTURE_SCHEMA_VERSION.to_string()),
            "{error}"
        );

        for (pattern, field) in [
            (ScenarioPattern::LongGapRecall, "memory_id"),
            (ScenarioPattern::CorrectionChains, "replacement_memory_id"),
            (ScenarioPattern::CrossStoreStress, "memory_id"),
        ] {
            let mut value = serde_json::to_value(&fixtures).unwrap();
            let scenario = value["scenarios"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|scenario| {
                    serde_json::from_value::<ScenarioPattern>(scenario["pattern"].clone()).unwrap()
                        == pattern
                })
                .unwrap();
            let event = scenario["events"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|event| match field {
                    "replacement_memory_id" => event["kind"] == "correct",
                    _ if pattern == ScenarioPattern::CrossStoreStress => event["kind"] == "link",
                    _ => event["kind"] == "remember",
                })
                .unwrap();
            event
                .as_object_mut()
                .unwrap()
                .insert(field.to_string(), Value::from("retired-id"));
            let error = parse_fixture_bytes(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .to_string();
            assert!(error.contains("unknown field"), "{error}");
            assert!(error.contains(field), "{error}");
        }

        let mut value = serde_json::to_value(&fixtures).unwrap();
        value["scenarios"][0]["entities"][0]
            .as_object_mut()
            .unwrap()
            .insert("memory_id".to_string(), Value::from("retired-id"));
        let error = parse_fixture_bytes(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field"), "{error}");
        assert!(error.contains("memory_id"), "{error}");

        let mut value = serde_json::to_value(&fixtures).unwrap();
        value["scenarios"][0]
            .as_object_mut()
            .unwrap()
            .insert("collection_name".to_string(), Value::from("retired-name"));
        let error = parse_fixture_bytes(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field"), "{error}");
        assert!(error.contains("collection_name"), "{error}");
    }

    #[test]
    fn public_parser_rejects_unknown_entity_kinds_before_validation() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut value = serde_json::to_value(&fixtures).unwrap();
        value["scenarios"][0]["entities"][0]["entity_type"] = Value::from("inferred-from-label");

        let error = parse_fixture_bytes(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("inferred-from-label"), "{error}");
        assert!(error.contains("expected one of"), "{error}");
    }

    #[test]
    fn public_parser_requires_pollution_labels_to_be_present_but_allows_unlabeled_contrasts() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
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
        fixtures.validate().unwrap();
    }

    #[test]
    fn public_parser_rejects_duplicate_or_overlapping_relevance_labels() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let expected = expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall));
        expected
            .relevant_external_ids
            .push(expected.relevant_external_ids[0].clone());
        let error = parse_error(&fixtures);
        assert!(
            error.contains("relevant_external_ids contains duplicates"),
            "{error}"
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let expected = expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall));
        expected
            .irrelevant_external_ids
            .push(expected.irrelevant_external_ids[0].clone());
        let error = parse_error(&fixtures);
        assert!(
            error.contains("irrelevant_external_ids contains duplicates"),
            "{error}"
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let expected = expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall));
        expected
            .irrelevant_external_ids
            .push(expected.relevant_external_ids[0].clone());
        let error = parse_error(&fixtures);
        assert!(error.contains("relevance labels overlap"), "{error}");
    }

    #[test]
    fn public_parser_rejects_relevance_labels_before_external_id_admission() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        scenario.events.swap(1, 2);
        let error = parse_error(&fixtures);
        assert!(error.contains("query relevance label"), "{error}");
        assert!(error.contains("memory-recent"), "{error}");
        assert!(error.contains("before it is admitted"), "{error}");
    }

    #[test]
    fn public_parser_rejects_dangling_correction_and_forget_targets() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
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

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "missing-forget-target".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("forget.target_external_ids"), "{error}");
    }

    #[test]
    fn public_parser_rejects_correction_targets_the_driver_cannot_correct() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Correct {
            target_external_id, ..
        } = &mut scenario.events[1]
        else {
            panic!("expected correction event");
        };
        *target_external_id = "entity-person".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("correct.target_external_id"), "{error}");
        assert!(error.contains("unsupported kind entity"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        insert_link_before(scenario, 1, "correction-link", "delivery-v1");
        let InteractionEvent::Correct {
            target_external_id, ..
        } = &mut scenario.events[2]
        else {
            panic!("expected correction event");
        };
        *target_external_id = "correction-link".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("correct.target_external_id"), "{error}");
        assert!(error.contains("unsupported kind memory_link"), "{error}");
    }

    #[test]
    fn public_parser_rejects_forget_targets_the_driver_cannot_forget() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "entity-person".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("forget.target_external_ids"), "{error}");
        assert!(error.contains("unsupported kind entity"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        insert_link_before(scenario, 3, "forget-link", "delivery-v2");
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[4]
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "forget-link".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("forget.target_external_ids"), "{error}");
        assert!(error.contains("unsupported kind memory_link"), "{error}");
    }

    #[test]
    fn public_parser_rejects_empty_or_duplicate_forget_targets() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids.clear();
        let error = parse_error(&fixtures);
        assert!(error.contains("must not be empty"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids[1] = target_external_ids[0].clone();
        let error = parse_error(&fixtures);
        assert!(error.contains("contains duplicates"), "{error}");
    }

    #[test]
    fn public_parser_rejects_conflicting_default_and_episode_surface_text() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::SurfaceContribution);
        let InteractionEvent::Remember { text, .. } = &mut scenario.events[0] else {
            panic!("expected remember event");
        };
        *text = "Conflicting Episode text".to_string();

        let error = parse_error(&fixtures);

        assert!(
            error.contains("must equal surface_texts.episode"),
            "{error}"
        );
    }

    #[test]
    fn public_parser_rejects_dangling_link_endpoints() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
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

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let InteractionEvent::Link { to_external_id, .. } = &mut scenario.events[1] else {
            panic!("expected link event");
        };
        *to_external_id = "missing-link-target".to_string();
        let error = parse_error(&fixtures);
        assert!(error.contains("link.to_external_id"), "{error}");
    }

    #[test]
    fn public_parser_rejects_memory_links_as_link_endpoints() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        insert_link_before(scenario, 2, "nested-link", "restart-link");

        let error = parse_error(&fixtures);
        assert!(error.contains("link.from_external_id"), "{error}");
        assert!(error.contains("unsupported kind memory_link"), "{error}");
    }

    #[test]
    fn public_parser_admits_persisted_observation_derived_and_thread_ids() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::ThreadDrift);
        let query_index = scenario
            .events
            .iter()
            .position(|event| matches!(event, InteractionEvent::Query { .. }))
            .unwrap();
        let thread_external_id = scenario
            .events
            .iter()
            .find_map(|event| match event {
                InteractionEvent::Remember {
                    thread: Some(thread),
                    ..
                } => Some(thread.thread_external_id.clone()),
                _ => None,
            })
            .unwrap();
        insert_link_before(
            scenario,
            query_index,
            "observation-link",
            "thread-focus:observation",
        );
        insert_link_before(
            scenario,
            query_index + 1,
            "derived-link",
            "thread-focus:derived",
        );
        insert_link_before(
            scenario,
            query_index + 2,
            "thread-link",
            &thread_external_id,
        );

        let bytes = serde_json::to_vec(&fixtures).unwrap();
        assert_eq!(parse_fixture_bytes(&bytes).unwrap(), fixtures);
    }

    #[test]
    fn public_parser_rejects_generated_external_id_collisions() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let InteractionEvent::Remember { external_id, .. } = &mut scenario.events[1] else {
            panic!("expected remember event");
        };
        *external_id = "memory-dormant:observation".to_string();

        let error = parse_error(&fixtures);
        assert!(error.contains("remember.external_id"), "{error}");
        assert!(error.contains("memory-dormant:observation"), "{error}");
        assert!(error.contains("duplicates existing external ID"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::ThreadDrift);
        let InteractionEvent::Remember {
            thread: Some(thread),
            ..
        } = &mut scenario.events[0]
        else {
            panic!("expected threaded remember event");
        };
        thread.thread_external_id = "entity-person".to_string();
        let error = parse_error(&fixtures);
        assert!(
            error.contains("remember.thread.thread_external_id"),
            "{error}"
        );
        assert!(error.contains("collides with admitted entity"), "{error}");
    }

    #[test]
    fn public_parser_rejects_queryless_scenarios() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        scenario
            .events
            .retain(|event| !matches!(event, InteractionEvent::Query { .. }));

        let error = parse_error(&fixtures);
        assert!(error.contains("long-gap-recall"), "{error}");
        assert!(error.contains("at least one scripted query"), "{error}");
    }

    #[test]
    fn public_parser_rejects_relations_outside_the_facade_vocabulary() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let InteractionEvent::Link { relation, .. } = &mut scenario.events[1] else {
            panic!("expected link event");
        };
        *relation = "invented_relation".to_string();

        let error = parse_error(&fixtures);
        assert!(error.contains("cross-store-stress"), "{error}");
        assert!(error.contains("link.relation"), "{error}");
        assert!(error.contains("invented_relation"), "{error}");
    }

    #[test]
    fn public_parser_rejects_duplicate_created_external_ids() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
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
    fn public_parser_rejects_duplicate_query_ids() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let duplicate = scenario
            .events
            .iter()
            .find(|event| matches!(event, InteractionEvent::Query { .. }))
            .cloned()
            .unwrap();
        scenario.events.push(duplicate);

        let error = parse_error(&fixtures);

        assert!(error.contains("duplicate event_id"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let mut duplicate = scenario
            .events
            .iter()
            .find(|event| matches!(event, InteractionEvent::Query { .. }))
            .cloned()
            .unwrap();
        let InteractionEvent::Query { event_id, .. } = &mut duplicate else {
            unreachable!()
        };
        *event_id = "event-duplicate-query-id".to_string();
        scenario.events.push(duplicate);
        let error = parse_error(&fixtures);
        assert!(error.contains("duplicate query_id"), "{error}");
    }

    #[test]
    fn public_parser_rejects_invalid_salience_and_thread_confidence() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let InteractionEvent::Remember { salience, .. } = &mut scenario.events[0] else {
            panic!("expected remember event");
        };
        *salience = 1.1;
        let error = parse_error(&fixtures);
        assert!(error.contains("remember.salience"), "{error}");
        assert!(error.contains("0.0..=1.0"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::ThreadDrift);
        let confidence = scenario
            .events
            .iter_mut()
            .find_map(|event| match event {
                InteractionEvent::Remember {
                    thread: Some(thread),
                    ..
                } => Some(&mut thread.confidence),
                _ => None,
            })
            .expect("thread drift fixture has thread membership");
        *confidence = -0.1;
        let error = parse_error(&fixtures);
        assert!(error.contains("remember.thread.confidence"), "{error}");

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let InteractionEvent::Remember { salience, .. } = &mut scenario.events[0] else {
            panic!("expected remember event");
        };
        *salience = f32::NAN;
        let error = scenario.validate().unwrap_err().to_string();
        assert!(error.contains("must be finite"), "{error}");
    }

    #[test]
    fn public_parser_rejects_restart_without_a_following_query() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let restart_index = scenario
            .events
            .iter()
            .position(|event| matches!(event, InteractionEvent::Restart { .. }))
            .unwrap();
        scenario.events.truncate(restart_index + 1);

        let error = parse_error(&fixtures);
        assert!(
            error.contains("must have a following scripted query"),
            "{error}"
        );
    }

    #[test]
    fn public_parser_rejects_restart_flags_the_runtime_cannot_honor() {
        for unsupported_field in ["reopen_graph", "reopen_stats"] {
            let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
            let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
            let restart = scenario
                .events
                .iter_mut()
                .find(|event| matches!(event, InteractionEvent::Restart { .. }))
                .unwrap();
            let InteractionEvent::Restart {
                reopen_graph,
                reopen_stats,
                ..
            } = restart
            else {
                unreachable!()
            };
            match unsupported_field {
                "reopen_graph" => *reopen_graph = false,
                "reopen_stats" => *reopen_stats = false,
                _ => unreachable!(),
            }

            let error = parse_error(&fixtures);
            assert!(error.contains("reopen_graph=true"), "{error}");
            assert!(error.contains("reopen_stats=true"), "{error}");
        }
    }

    #[test]
    fn checked_fixture_json_shape_remains_publicly_parseable() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let value: Value =
            serde_json::from_slice(&canonical_fixture_bytes(&fixtures).unwrap()).unwrap();
        assert!(value["scenarios"].is_array());
        assert_eq!(
            parse_fixture_bytes(&serde_json::to_vec(&value).unwrap()).unwrap(),
            fixtures
        );
    }

    #[test]
    fn fixture_reader_rejects_corrupt_encoding() {
        let error = parse_fixture_bytes(b"{\"schema_version\":1,\xff")
            .unwrap_err()
            .to_string();
        assert!(!error.is_empty());
    }

    #[test]
    fn fixture_reader_rejects_truncated_json() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut bytes = canonical_fixture_bytes(&fixtures).unwrap();
        bytes.truncate(bytes.len() - 10);
        let error = parse_fixture_bytes(&bytes).unwrap_err().to_string();
        assert!(!error.is_empty());
    }

    #[test]
    fn fixture_reader_rejects_an_incompatible_schema_version() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        fixtures.schema_version = CONTINUITY_FIXTURE_SCHEMA_VERSION_V3 + 1;
        let error = parse_fixture_bytes(&serde_json::to_vec(&fixtures).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported continuity fixture schema_version"));
        assert!(error.contains(&(CONTINUITY_FIXTURE_SCHEMA_VERSION_V3 + 1).to_string()));
        assert!(
            error.contains(&CONTINUITY_FIXTURE_SCHEMA_VERSION_V2.to_string()),
            "{error}"
        );
        assert!(
            error.contains(&CONTINUITY_FIXTURE_SCHEMA_VERSION_V3.to_string()),
            "{error}"
        );
    }

    #[test]
    fn public_parser_accepts_explicit_v3_providers_and_rejects_cross_version_blocks() {
        let fixtures = explicit_v2_fixture();
        let mut v3 = serde_json::to_value(&fixtures).unwrap();
        v3["schema_version"] = Value::from(CONTINUITY_FIXTURE_SCHEMA_VERSION_V3);
        for scenario in v3["scenarios"].as_array_mut().unwrap() {
            scenario["embedding"].as_object_mut().unwrap().insert(
                "provider".to_string(),
                Value::String("controllable_similarity".to_string()),
            );
        }
        v3["scenarios"][0]["embedding"] = serde_json::json!({"provider": "frozen"});
        let parsed = parse_fixture_bytes(&serde_json::to_vec(&v3).unwrap()).unwrap();
        assert_eq!(
            parsed.schema_version,
            LATEST_CONTINUITY_FIXTURE_SCHEMA_VERSION
        );
        assert_eq!(parsed.scenarios[0].embedding.provider_name(), "frozen");
        assert!(
            parsed.scenarios[1..]
                .iter()
                .all(|scenario| !scenario.embedding.is_legacy_v2())
        );

        let mut v2_with_frozen = serde_json::to_value(&fixtures).unwrap();
        v2_with_frozen["scenarios"][0]["embedding"] = serde_json::json!({"provider": "frozen"});
        let error = parse_fixture_bytes(&serde_json::to_vec(&v2_with_frozen).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("schema_version 2"), "{error}");
        assert!(error.contains("require schema_version 3"), "{error}");

        let mut v3_with_legacy = serde_json::to_value(&fixtures).unwrap();
        v3_with_legacy["schema_version"] = Value::from(CONTINUITY_FIXTURE_SCHEMA_VERSION_V3);
        let error = parse_fixture_bytes(&serde_json::to_vec(&v3_with_legacy).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("schema_version 3"), "{error}");
        assert!(error.contains("declare provider"), "{error}");
    }

    #[test]
    fn public_parser_rejects_partial_malformed_and_typoed_v3_embedding_blocks() {
        let fixtures = explicit_v2_fixture();
        let mut base = serde_json::to_value(&fixtures).unwrap();
        base["schema_version"] = Value::from(CONTINUITY_FIXTURE_SCHEMA_VERSION_V3);
        for scenario in base["scenarios"].as_array_mut().unwrap() {
            scenario["embedding"].as_object_mut().unwrap().insert(
                "provider".to_string(),
                Value::String("controllable_similarity".to_string()),
            );
        }

        let mut typoed = base.clone();
        typoed["scenarios"][0]["embedding"] = serde_json::json!({"provder": "frozen"});
        let error = parse_fixture_bytes(&serde_json::to_vec(&typoed).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field \"provder\""), "{error}");
        assert!(error.contains("provider: frozen"), "{error}");

        let mut malformed_frozen = base.clone();
        malformed_frozen["scenarios"][0]["embedding"] =
            serde_json::json!({"provider": "frozen", "vector_size": 8});
        let error = parse_fixture_bytes(&serde_json::to_vec(&malformed_frozen).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field \"vector_size\""), "{error}");
        assert!(error.contains("frozen embedding block"), "{error}");

        let mut partial = base;
        partial["scenarios"][0]["embedding"] = serde_json::json!({
            "provider": "controllable_similarity",
            "seed": 1,
            "vector_size": 8
        });
        let error = parse_fixture_bytes(&serde_json::to_vec(&partial).unwrap())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("malformed controllable_similarity embedding block"),
            "{error}"
        );
        assert!(error.contains("missing field"), "{error}");
    }

    #[test]
    fn schema_v3_admits_downstream_patterns_and_couples_abstention_labels() {
        let mut v2 = explicit_v2_fixture();
        v2.scenarios[0].pattern = ScenarioPattern::GradedSimilarity;
        let error = parse_error(&v2);
        assert!(error.contains("GradedSimilarity"), "{error}");
        assert!(error.contains("requires schema_version 3"), "{error}");

        let mut fixtures = explicit_v2_fixture();
        upgrade_to_v3(&mut fixtures);
        fixtures.scenarios[0].pattern = ScenarioPattern::Abstention;
        let error = parse_error(&fixtures);
        assert!(error.contains("pattern=abstention"), "{error}");
        assert!(error.contains("empty relevant_external_ids"), "{error}");

        expected_mut(&mut fixtures.scenarios[0])
            .relevant_external_ids
            .clear();
        parse_fixture_bytes(&serde_json::to_vec(&fixtures).unwrap()).unwrap();

        fixtures.scenarios[0].pattern = ScenarioPattern::Autobiographical;
        let error = parse_error(&fixtures);
        assert!(error.contains("non-abstention"), "{error}");
        assert!(error.contains("declare relevant_external_ids"), "{error}");
    }
}
