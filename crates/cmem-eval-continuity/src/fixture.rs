use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use chrono::{DateTime, Utc};
use cmem_eval::ControllableSimilarityFixture;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, Unexpected},
};
use serde_json::{Map, Value};

pub const CONTINUITY_FIXTURE_SCHEMA_VERSION: u32 = 3;

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
    ControllableSimilarity(ControllableSimilarityFixture),
    Frozen,
}

impl ContinuityScenarioEmbedding {
    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::ControllableSimilarity(_) => "controllable_similarity",
            Self::Frozen => "frozen",
        }
    }

    pub fn controllable_similarity(&self) -> Option<&ControllableSimilarityFixture> {
        match self {
            Self::ControllableSimilarity(fixture) => Some(fixture),
            Self::Frozen => None,
        }
    }

    pub fn controllable_similarity_mut(&mut self) -> Option<&mut ControllableSimilarityFixture> {
        match self {
            Self::ControllableSimilarity(fixture) => Some(fixture),
            Self::Frozen => None,
        }
    }

    pub fn controllable_similarity_provider(fixture: ControllableSimilarityFixture) -> Self {
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

impl Serialize for ContinuityScenarioEmbedding {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ControllableSimilarity(fixture) => {
                let mut value = serde_json::to_value(fixture).map_err(serde::ser::Error::custom)?;
                value
                    .as_object_mut()
                    .expect("controllable similarity fixture serializes as an object")
                    .insert(
                        "provider".to_string(),
                        Value::String("controllable_similarity".to_string()),
                    );
                value.sort_all_objects();
                value.serialize(serializer)
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
        const PROVIDERS: &[&str] = &["controllable_similarity", "frozen"];
        const FROZEN_FIELDS: &[&str] = &["provider"];
        const CONTROLLABLE_FIELDS: &[&str] = &[
            "provider",
            "seed",
            "vector_size",
            "noise_magnitude",
            "clusters",
            "concepts",
        ];
        let value = Value::deserialize(deserializer)?;
        let object = value.as_object().ok_or_else(|| {
            D::Error::invalid_type(Unexpected::Other("non-object"), &"an embedding block")
        })?;
        match object.get("provider") {
            Some(Value::String(provider)) if provider == "frozen" => {
                reject_unknown_embedding_field::<D>(object, FROZEN_FIELDS)?;
                Ok(Self::Frozen)
            }
            Some(Value::String(provider)) if provider == "controllable_similarity" => {
                reject_unknown_embedding_field::<D>(object, CONTROLLABLE_FIELDS)?;
                let mut fixture = object.clone();
                fixture.remove("provider");
                serde_json::from_value(Value::Object(fixture))
                    .map(Self::ControllableSimilarity)
                    .map_err(D::Error::custom)
            }
            Some(Value::String(provider)) => Err(D::Error::unknown_variant(provider, PROVIDERS)),
            Some(_) => Err(D::Error::invalid_type(
                Unexpected::Other("non-string provider"),
                &"a provider name",
            )),
            None => {
                reject_unknown_embedding_field::<D>(object, CONTROLLABLE_FIELDS)?;
                Err(D::Error::missing_field("provider"))
            }
        }
    }
}

fn reject_unknown_embedding_field<'de, D: Deserializer<'de>>(
    object: &Map<String, Value>,
    allowed: &'static [&'static str],
) -> std::result::Result<(), D::Error> {
    match object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        Some(field) => Err(D::Error::unknown_field(field, allowed)),
        None => Ok(()),
    }
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

/// Object kind an admitted external ID resolves to inside a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityObjectKind {
    Episode,
    Observation,
    Entity,
    MemoryThread,
    DerivedMemory,
    MemoryLink,
}

impl ContinuityObjectKind {
    pub fn as_str(self) -> &'static str {
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

/// Where a continuity fixture rejection was detected: the file root or a
/// scenario, narrowed to the event under inspection when one exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureLocation {
    Root,
    Scenario {
        fixture_id: String,
        event_id: Option<String>,
    },
}

/// What the fixture parser found wrong with `field` at a `FixtureLocation`.
#[derive(Debug, Clone, PartialEq)]
pub enum FixtureAdmissionKind {
    UnsupportedSchemaVersion {
        found: u64,
    },
    Empty,
    Duplicate(String),
    NotChronological,
    Undeclared(String),
    DiffersFrom(&'static str),
    UnassignedEmbeddingInput(String),
    ConceptAssignments {
        label: String,
        found: usize,
    },
    Collision {
        admitted_kind: ContinuityObjectKind,
    },
    NotAdmitted(String),
    UnsupportedKind {
        external_id: String,
        found: ContinuityObjectKind,
        allowed: &'static [ContinuityObjectKind],
    },
    UnsupportedRelation(String),
    Overlap(String),
    ForbiddenForPattern(ScenarioPattern),
    OutOfUnitInterval(f32),
    RestartMustReopen,
    RestartWithoutFollowingQuery,
    MissingQuery,
    MustEndWithQuery,
}

#[derive(Debug)]
pub enum FixtureError {
    /// The bytes are not JSON.
    Json(serde_json::Error),
    /// The JSON does not have the fixture shape; `field` is the field serde
    /// named (unknown, missing or duplicate), when it named one.
    Shape {
        location: FixtureLocation,
        field: Option<String>,
        source: serde_json::Error,
    },
    /// The controllable-similarity embedding block was refused by its provider.
    Embedding {
        location: FixtureLocation,
        source: anyhow::Error,
    },
    Admission {
        location: FixtureLocation,
        field: &'static str,
        kind: FixtureAdmissionKind,
    },
}

impl FixtureLocation {
    fn scenario(fixture_id: &str) -> Self {
        Self::Scenario {
            fixture_id: fixture_id.to_string(),
            event_id: None,
        }
    }

    fn event(fixture_id: &str, event_id: &str) -> Self {
        Self::Scenario {
            fixture_id: fixture_id.to_string(),
            event_id: Some(event_id.to_string()),
        }
    }

    fn error(&self, field: &'static str, kind: FixtureAdmissionKind) -> FixtureError {
        FixtureError::Admission {
            location: self.clone(),
            field,
            kind,
        }
    }
}

impl fmt::Display for FixtureLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "root"),
            Self::Scenario {
                fixture_id,
                event_id,
            } => {
                write!(f, "scenario {fixture_id:?}")?;
                if let Some(event_id) = event_id {
                    write!(f, " event {event_id:?}")?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for FixtureAdmissionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found } => write!(
                f,
                "unsupported continuity fixture schema_version {found}; expected {CONTINUITY_FIXTURE_SCHEMA_VERSION}"
            ),
            Self::Empty => write!(f, "must be non-empty"),
            Self::Duplicate(value) => write!(f, "duplicates {value:?}"),
            Self::NotChronological => write!(f, "precedes the previous event"),
            Self::Undeclared(entity_id) => write!(f, "references undeclared entity {entity_id:?}"),
            Self::DiffersFrom(other) => write!(f, "must equal {other}"),
            Self::UnassignedEmbeddingInput(text) => {
                write!(f, "text {text:?} has no embedding concept assignment")
            }
            Self::ConceptAssignments { label, found } => write!(
                f,
                "label {label:?} must have exactly one embedding concept assignment; found {found}"
            ),
            Self::Collision { admitted_kind } => {
                write!(f, "collides with admitted {}", admitted_kind.as_str())
            }
            Self::NotAdmitted(external_id) => write!(
                f,
                "references external ID {external_id:?} before it is admitted"
            ),
            Self::UnsupportedKind {
                external_id,
                found,
                allowed,
            } => {
                let allowed = allowed
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(
                    f,
                    "references {external_id:?} with unsupported kind {}; expected one of [{allowed}]",
                    found.as_str()
                )
            }
            Self::UnsupportedRelation(relation) => write!(
                f,
                "relation {relation:?} is not supported by the CharacterMemory facade"
            ),
            Self::Overlap(external_id) => {
                write!(f, "relevance labels overlap at external ID {external_id:?}")
            }
            Self::ForbiddenForPattern(pattern) => {
                write!(f, "must be empty for pattern {pattern:?}")
            }
            Self::OutOfUnitInterval(value) => {
                write!(f, "must be finite and within 0.0..=1.0; got {value}")
            }
            Self::RestartMustReopen => write!(
                f,
                "must be true because the continuity runtime always reconstructs both stores"
            ),
            Self::RestartWithoutFollowingQuery => {
                write!(f, "restart must have a following scripted query")
            }
            Self::MissingQuery => write!(f, "must declare at least one scripted query"),
            Self::MustEndWithQuery => write!(
                f,
                "must end with a scripted query so every cumulative operation outcome is emitted"
            ),
        }
    }
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(source) => write!(f, "continuity fixture JSON syntax: {source}"),
            Self::Shape {
                location, source, ..
            } => write!(f, "continuity fixture {location}: {source}"),
            Self::Embedding { location, source } => {
                write!(
                    f,
                    "continuity fixture {location}, field embedding: {source:#}"
                )
            }
            Self::Admission {
                location,
                field,
                kind,
            } => write!(f, "continuity fixture {location}, field {field}: {kind}"),
        }
    }
}

impl std::error::Error for FixtureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) | Self::Shape { source, .. } => Some(source),
            Self::Embedding { source, .. } => Some(source.as_ref()),
            Self::Admission { .. } => None,
        }
    }
}

impl ContinuityFixtureSet {
    pub fn validate(&self) -> Result<(), FixtureError> {
        let root = FixtureLocation::Root;
        if self.schema_version != CONTINUITY_FIXTURE_SCHEMA_VERSION {
            return Err(root.error(
                "schema_version",
                FixtureAdmissionKind::UnsupportedSchemaVersion {
                    found: u64::from(self.schema_version),
                },
            ));
        }
        if self.scenarios.is_empty() {
            return Err(root.error("scenarios", FixtureAdmissionKind::Empty));
        }

        let mut fixture_ids = BTreeSet::new();
        let mut namespaces = BTreeSet::new();
        let mut query_ids = BTreeSet::new();
        for scenario in &self.scenarios {
            let location = FixtureLocation::scenario(&scenario.fixture_id);
            require_non_empty(&location, "fixture_id", &scenario.fixture_id)?;
            require_non_empty(&location, "namespace", &scenario.namespace)?;
            if !fixture_ids.insert(&scenario.fixture_id) {
                return Err(root.error(
                    "fixture_id",
                    FixtureAdmissionKind::Duplicate(scenario.fixture_id.clone()),
                ));
            }
            if !namespaces.insert(&scenario.namespace) {
                return Err(location.error(
                    "namespace",
                    FixtureAdmissionKind::Duplicate(scenario.namespace.clone()),
                ));
            }
            scenario.validate()?;
            for event in &scenario.events {
                if let InteractionEvent::Query {
                    event_id, query_id, ..
                } = event
                    && !query_ids.insert(query_id)
                {
                    return Err(
                        FixtureLocation::event(&scenario.fixture_id, event_id).error(
                            "query.query_id",
                            FixtureAdmissionKind::Duplicate(query_id.clone()),
                        ),
                    );
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

    /// Exact texts requested from the frozen provider after CharacterMemory
    /// composes and normalizes write surfaces. Queries bypass that write-time
    /// normalization and therefore remain byte-exact fixture text.
    pub fn runtime_embedding_inputs(&self) -> BTreeSet<String> {
        let mut inputs = self
            .entities
            .iter()
            .map(|entity| runtime_memory_embedding_text(&entity.label))
            .collect::<BTreeSet<_>>();
        for event in &self.events {
            match event {
                InteractionEvent::Remember {
                    text,
                    surface_texts,
                    ..
                } => {
                    if let Some(surface_texts) = surface_texts {
                        inputs.insert(runtime_memory_embedding_text(&surface_texts.episode));
                        inputs.insert(runtime_memory_embedding_text(&surface_texts.observation));
                        inputs.insert(runtime_memory_embedding_text(&surface_texts.derived));
                    } else {
                        inputs.insert(runtime_memory_embedding_text(text));
                    }
                }
                InteractionEvent::Correct {
                    replacement_text, ..
                } => {
                    inputs.insert(runtime_memory_embedding_text(replacement_text));
                }
                InteractionEvent::Query { text, .. } => {
                    inputs.insert(text.clone());
                }
                InteractionEvent::Forget { .. }
                | InteractionEvent::Link { .. }
                | InteractionEvent::Restart { .. } => {}
            }
        }
        inputs
    }

    pub fn validate(&self) -> Result<(), FixtureError> {
        let scenario = FixtureLocation::scenario(&self.fixture_id);
        let controllable_embedding = self.embedding.controllable_similarity();
        if let Some(embedding) = controllable_embedding {
            cmem_eval::ControllableSimilarityEmbeddingProvider::new(embedding.clone()).map_err(
                |source| FixtureError::Embedding {
                    location: scenario.clone(),
                    source,
                },
            )?;
        }
        let mut declared_entities = BTreeSet::new();
        for entity in &self.entities {
            require_non_empty(&scenario, "entity.external_id", &entity.external_id)?;
            require_non_empty(&scenario, "entity.label", &entity.label)?;
            if !declared_entities.insert(entity.external_id.as_str()) {
                return Err(scenario.error(
                    "entity.external_id",
                    FixtureAdmissionKind::Duplicate(entity.external_id.clone()),
                ));
            }
            if let Some(embedding) = controllable_embedding {
                let found = embedding
                    .concepts
                    .values()
                    .flat_map(|concept| &concept.inputs)
                    .filter(|input| *input == &entity.label)
                    .count();
                if found != 1 {
                    return Err(scenario.error(
                        "entity.label",
                        FixtureAdmissionKind::ConceptAssignments {
                            label: entity.label.clone(),
                            found,
                        },
                    ));
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
            let location = FixtureLocation::event(&self.fixture_id, event_id);
            require_non_empty(&location, "event_id", event_id)?;
            if !event_ids.insert(event_id) {
                return Err(location.error(
                    "event_id",
                    FixtureAdmissionKind::Duplicate(event_id.to_string()),
                ));
            }
            let timestamp = event.timestamp();
            if previous_timestamp.is_some_and(|previous| timestamp < previous) {
                return Err(location.error("timestamp", FixtureAdmissionKind::NotChronological));
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
                    require_unit_interval(&location, "remember.salience", *salience)?;
                    if let Some(thread) = thread {
                        require_unit_interval(
                            &location,
                            "remember.thread.confidence",
                            thread.confidence,
                        )?;
                    }
                    for entity_id in entity_external_ids {
                        if !declared_entities.contains(entity_id.as_str()) {
                            return Err(location.error(
                                "remember.entity_external_ids",
                                FixtureAdmissionKind::Undeclared(entity_id.clone()),
                            ));
                        }
                    }
                    if let Some(surface_texts) = surface_texts {
                        if text != &surface_texts.episode {
                            return Err(location.error(
                                "remember.text",
                                FixtureAdmissionKind::DiffersFrom("remember.surface_texts.episode"),
                            ));
                        }
                        require_distinct_surface_texts(&location, surface_texts)?;
                        for (field, surface_text) in [
                            ("remember.surface_texts.episode", &surface_texts.episode),
                            (
                                "remember.surface_texts.observation",
                                &surface_texts.observation,
                            ),
                            ("remember.surface_texts.derived", &surface_texts.derived),
                        ] {
                            require_embedding_input(
                                &location,
                                field,
                                assigned_inputs.as_ref(),
                                surface_text,
                            )?;
                        }
                    }
                    require_embedding_input(
                        &location,
                        "remember.text",
                        assigned_inputs.as_ref(),
                        text,
                    )?;
                    admit_external_id(
                        &location,
                        "remember.external_id",
                        external_id,
                        ContinuityObjectKind::Episode,
                        &mut admitted_external_ids,
                    )?;
                    admit_external_id(
                        &location,
                        "remember.observation_external_id",
                        &observation_external_id(external_id),
                        ContinuityObjectKind::Observation,
                        &mut admitted_external_ids,
                    )?;
                    admit_external_id(
                        &location,
                        "remember.derived_external_id",
                        &derived_external_id(external_id),
                        ContinuityObjectKind::DerivedMemory,
                        &mut admitted_external_ids,
                    )?;
                    if let Some(thread) = thread {
                        admit_thread_external_id(
                            &location,
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
                        &location,
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
                        &location,
                        "correct.replacement_text",
                        assigned_inputs.as_ref(),
                        replacement_text,
                    )?;
                    admit_external_id(
                        &location,
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
                        return Err(location
                            .error("forget.target_external_ids", FixtureAdmissionKind::Empty));
                    }
                    require_distinct(&location, "forget.target_external_ids", target_external_ids)?;
                    for target_external_id in target_external_ids {
                        require_admitted_kind(
                            &location,
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
                        &location,
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
                        &location,
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
                    require_supported_relation(&location, relation)?;
                    admit_external_id(
                        &location,
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
                    require_non_empty(&location, "query.query_id", query_id)?;
                    if !query_ids.insert(query_id) {
                        return Err(location.error(
                            "query.query_id",
                            FixtureAdmissionKind::Duplicate(query_id.clone()),
                        ));
                    }
                    require_embedding_input(
                        &location,
                        "query.text",
                        assigned_inputs.as_ref(),
                        text,
                    )?;
                    validate_expected_relevance(
                        &location,
                        self.pattern,
                        expected,
                        &admitted_external_ids,
                    )?;
                }
                InteractionEvent::Restart {
                    reopen_graph,
                    reopen_stats,
                    ..
                } => {
                    for (field, reopen) in [
                        ("restart.reopen_graph", *reopen_graph),
                        ("restart.reopen_stats", *reopen_stats),
                    ] {
                        if !reopen {
                            return Err(
                                location.error(field, FixtureAdmissionKind::RestartMustReopen)
                            );
                        }
                    }
                    if !self.events[event_index + 1..]
                        .iter()
                        .any(|event| matches!(event, InteractionEvent::Query { .. }))
                    {
                        return Err(location.error(
                            "restart",
                            FixtureAdmissionKind::RestartWithoutFollowingQuery,
                        ));
                    }
                }
            }
        }
        if query_ids.is_empty() {
            return Err(scenario.error("events", FixtureAdmissionKind::MissingQuery));
        }
        if !matches!(self.events.last(), Some(InteractionEvent::Query { .. })) {
            return Err(scenario.error("events", FixtureAdmissionKind::MustEndWithQuery));
        }
        Ok(())
    }
}

/// Mirrors CharacterMemory's write-surface `clean_text` in
/// `src/policy/embedding_surface.rs`. The live adapter removes the object-type
/// prefix before frozen lookup, leaving this normalized suffix as the exact
/// cache key. Keep this mirror paired with the cross-repository drift guard
/// `embedded_frozen_write_surface_matches_continuity_runtime_normalization` in
/// this module's tests; that test must fail if the upstream policy
/// changes without a corresponding fixture-contract update.
pub fn runtime_memory_embedding_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

pub fn canonical_fixture_bytes(fixtures: &ContinuityFixtureSet) -> anyhow::Result<Vec<u8>> {
    fixtures.validate()?;
    let mut bytes = serde_json::to_vec_pretty(fixtures)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn parse_fixture_bytes(bytes: &[u8]) -> Result<ContinuityFixtureSet, FixtureError> {
    let value: Value = serde_json::from_slice(bytes).map_err(FixtureError::Json)?;
    let fixtures: ContinuityFixtureSet =
        serde_json::from_value(value).map_err(|source| shape_error(bytes, source))?;
    fixtures.validate()?;
    Ok(fixtures)
}

/// Locates a serde rejection: a schema_version mismatch is reported as such
/// before any shape complaint. Check the root without its scenarios first, then
/// retain the location and cause from the same failing deserialization.
fn shape_error(bytes: &[u8], source: serde_json::Error) -> FixtureError {
    if source.classify() != serde_json::error::Category::Data {
        return FixtureError::Json(source);
    }
    let mut value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(source) => return FixtureError::Json(source),
    };
    if let Some(found) = value.get("schema_version").and_then(Value::as_u64)
        && found != u64::from(CONTINUITY_FIXTURE_SCHEMA_VERSION)
    {
        return FixtureLocation::Root.error(
            "schema_version",
            FixtureAdmissionKind::UnsupportedSchemaVersion { found },
        );
    }
    let scenarios = value
        .get_mut("scenarios")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();
    let (location, source) = match serde_json::from_value::<ContinuityFixtureSet>(value) {
        Err(source) => (FixtureLocation::Root, source),
        Ok(_) => scenarios
            .into_iter()
            .find_map(|scenario| {
                let location = FixtureLocation::scenario(
                    scenario
                        .get("fixture_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                );
                serde_json::from_value::<ContinuityScenario>(scenario)
                    .err()
                    .map(|source| (location, source))
            })
            .unwrap_or((FixtureLocation::Root, source)),
    };
    FixtureError::Shape {
        location,
        field: shape_field(&source),
        source,
    }
}

/// serde names the offending field only in its message, in the fixed
/// `unknown field \`x\``, `missing field \`x\`` and `duplicate field \`x\``
/// formats of `serde::de::Error`; other shape complaints name no field.
fn shape_field(source: &serde_json::Error) -> Option<String> {
    let message = source.to_string();
    ["unknown field `", "missing field `", "duplicate field `"]
        .into_iter()
        .find_map(|prefix| message.strip_prefix(prefix))
        .and_then(|rest| rest.split('`').next())
        .map(str::to_string)
}

pub fn scenario_patterns(fixtures: &ContinuityFixtureSet) -> BTreeMap<ScenarioPattern, usize> {
    let mut counts = BTreeMap::new();
    for scenario in &fixtures.scenarios {
        *counts.entry(scenario.pattern).or_insert(0) += 1;
    }
    counts
}

fn require_embedding_input(
    location: &FixtureLocation,
    field: &'static str,
    assigned_inputs: Option<&BTreeSet<&str>>,
    text: &str,
) -> Result<(), FixtureError> {
    if assigned_inputs.is_some_and(|assigned_inputs| !assigned_inputs.contains(text)) {
        return Err(location.error(
            field,
            FixtureAdmissionKind::UnassignedEmbeddingInput(text.to_string()),
        ));
    }
    Ok(())
}

fn admit_external_id(
    location: &FixtureLocation,
    field: &'static str,
    external_id: &str,
    kind: ContinuityObjectKind,
    admitted_external_ids: &mut BTreeMap<String, ContinuityObjectKind>,
) -> Result<(), FixtureError> {
    require_non_empty(location, field, external_id)?;
    if admitted_external_ids.contains_key(external_id) {
        return Err(location.error(
            field,
            FixtureAdmissionKind::Duplicate(external_id.to_string()),
        ));
    }
    admitted_external_ids.insert(external_id.to_string(), kind);
    Ok(())
}

fn admit_thread_external_id(
    location: &FixtureLocation,
    external_id: &str,
    admitted_external_ids: &mut BTreeMap<String, ContinuityObjectKind>,
) -> Result<(), FixtureError> {
    let field = "remember.thread.thread_external_id";
    require_non_empty(location, field, external_id)?;
    match admitted_external_ids.get(external_id) {
        Some(ContinuityObjectKind::MemoryThread) => Ok(()),
        Some(kind) => Err(location.error(
            field,
            FixtureAdmissionKind::Collision {
                admitted_kind: *kind,
            },
        )),
        None => {
            admitted_external_ids
                .insert(external_id.to_string(), ContinuityObjectKind::MemoryThread);
            Ok(())
        }
    }
}

fn require_admitted_external_id(
    location: &FixtureLocation,
    field: &'static str,
    external_id: &str,
    admitted_external_ids: &BTreeMap<String, ContinuityObjectKind>,
) -> Result<(), FixtureError> {
    if !admitted_external_ids.contains_key(external_id) {
        return Err(location.error(
            field,
            FixtureAdmissionKind::NotAdmitted(external_id.to_string()),
        ));
    }
    Ok(())
}

fn require_admitted_kind(
    location: &FixtureLocation,
    field: &'static str,
    external_id: &str,
    allowed: &'static [ContinuityObjectKind],
    admitted_external_ids: &BTreeMap<String, ContinuityObjectKind>,
) -> Result<(), FixtureError> {
    require_admitted_external_id(location, field, external_id, admitted_external_ids)?;
    let found = admitted_external_ids[external_id];
    if !allowed.contains(&found) {
        return Err(location.error(
            field,
            FixtureAdmissionKind::UnsupportedKind {
                external_id: external_id.to_string(),
                found,
                allowed,
            },
        ));
    }
    Ok(())
}

fn require_supported_relation(
    location: &FixtureLocation,
    relation: &str,
) -> Result<(), FixtureError> {
    require_non_empty(location, "link.relation", relation)?;
    if !CONTINUITY_RELATION_VOCABULARY.contains(&relation) {
        return Err(location.error(
            "link.relation",
            FixtureAdmissionKind::UnsupportedRelation(relation.to_string()),
        ));
    }
    Ok(())
}

fn validate_expected_relevance(
    location: &FixtureLocation,
    pattern: ScenarioPattern,
    expected: &ExpectedRelevance,
    admitted_external_ids: &BTreeMap<String, ContinuityObjectKind>,
) -> Result<(), FixtureError> {
    let relevant_field = "query.expected.relevant_external_ids";
    let irrelevant_field = "query.expected.irrelevant_external_ids";
    if pattern == ScenarioPattern::Abstention && !expected.relevant_external_ids.is_empty() {
        return Err(location.error(
            relevant_field,
            FixtureAdmissionKind::ForbiddenForPattern(pattern),
        ));
    }
    if pattern != ScenarioPattern::Abstention && expected.relevant_external_ids.is_empty() {
        return Err(location.error(relevant_field, FixtureAdmissionKind::Empty));
    }
    require_distinct(location, relevant_field, &expected.relevant_external_ids)?;
    require_distinct(
        location,
        irrelevant_field,
        &expected.irrelevant_external_ids,
    )?;
    let relevant = expected
        .relevant_external_ids
        .iter()
        .collect::<BTreeSet<_>>();
    if let Some(overlap) = expected
        .irrelevant_external_ids
        .iter()
        .find(|external_id| relevant.contains(external_id))
    {
        return Err(location.error(
            irrelevant_field,
            FixtureAdmissionKind::Overlap(overlap.clone()),
        ));
    }
    for (field, external_ids) in [
        (relevant_field, &expected.relevant_external_ids),
        (irrelevant_field, &expected.irrelevant_external_ids),
    ] {
        for external_id in external_ids {
            require_admitted_external_id(location, field, external_id, admitted_external_ids)?;
        }
    }
    Ok(())
}

fn require_distinct_surface_texts(
    location: &FixtureLocation,
    surface_texts: &RememberSurfaceTexts,
) -> Result<(), FixtureError> {
    let texts = [
        surface_texts.episode.clone(),
        surface_texts.observation.clone(),
        surface_texts.derived.clone(),
    ];
    for text in &texts {
        require_non_empty(location, "remember.surface_texts", text)?;
    }
    require_distinct(location, "remember.surface_texts", &texts)
}

fn require_distinct(
    location: &FixtureLocation,
    field: &'static str,
    values: &[String],
) -> Result<(), FixtureError> {
    let mut seen = BTreeSet::new();
    match values.iter().find(|value| !seen.insert(value.as_str())) {
        Some(duplicate) => {
            Err(location.error(field, FixtureAdmissionKind::Duplicate(duplicate.clone())))
        }
        None => Ok(()),
    }
}

fn require_non_empty(
    location: &FixtureLocation,
    field: &'static str,
    value: &str,
) -> Result<(), FixtureError> {
    if value.trim().is_empty() {
        return Err(location.error(field, FixtureAdmissionKind::Empty));
    }
    Ok(())
}

fn require_unit_interval(
    location: &FixtureLocation,
    field: &'static str,
    value: f32,
) -> Result<(), FixtureError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(location.error(field, FixtureAdmissionKind::OutOfUnitInterval(value)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use cmem_eval::{
        BackendConfig, BenchmarkRunConfig, CharacterMemoryAdapter, CommitWriteOptions, DatasetId,
        EmbeddingConfig, EmbeddingProviderConfig, FrozenEmbeddingStore, PrepareWriteInput,
        VectorStoreMode,
    };
    use serde_json::Value;

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};

    #[tokio::test]
    async fn embedded_frozen_write_surface_matches_continuity_runtime_normalization() {
        let run_directory = tempfile::tempdir().unwrap();
        let run_root = run_directory.path();
        let directory = tempfile::tempdir().unwrap();
        let token = uuid::Uuid::new_v4();
        let namespace = "frozen-runtime-normalization";
        let content = "  The  cobalt\tnotebook\nis in   the east cabinet.  ";
        let runtime_lookup_text = runtime_memory_embedding_text(content);
        let store_path = directory.path().join("strict-runtime-store.json");
        let store = FrozenEmbeddingStore::new(
            "text-embedding-3-small",
            "test_fixture",
            [(runtime_lookup_text, vec![1.0; 1_536])],
        )
        .unwrap();
        std::fs::write(&store_path, store.canonical_bytes().unwrap()).unwrap();

        let config = BenchmarkRunConfig {
            run_id: format!("frozen-drift-{token}"),
            dataset: DatasetId::new("locomo").unwrap(),
            backend: BackendConfig {
                vector_store_mode: VectorStoreMode::Embedded,
                embedding: EmbeddingConfig {
                    provider: EmbeddingProviderConfig::Frozen,
                    model: "text-embedding-3-small".to_string(),
                    vector_size: Some(1_536),
                    store_path: Some(store_path.display().to_string()),
                },
                ..BackendConfig::default()
            },
            retrieval: Default::default(),
            ingest: Default::default(),
            metrics: Default::default(),
        };
        let adapter = (CharacterMemoryAdapter::new_with_frozen_embeddings(run_root, &config).await)
            .expect("frozen drift-guard adapter construction");
        (adapter.open_namespace(namespace).await).expect("frozen drift-guard namespace open");
        let plan = (adapter
            .prepare(PrepareWriteInput {
                namespace: namespace.to_string(),
                content: content.to_string(),
                episode_external_id: "whitespace-episode".to_string(),
                observation_external_id: "whitespace-observation".to_string(),
                episode_started_at: None,
                observation_observed_at: None,
                raw_refs: Vec::new(),
                idempotency_key: Some("whitespace-drift-guard".to_string()),
                include_vector_index_candidates: true,
                include_stats_update_candidates: true,
            })
            .await)
            .expect("frozen drift-guard write preparation");
        let outcome = (adapter.commit(plan, CommitWriteOptions::default()).await)
            .expect("frozen drift-guard write commit");
        assert_eq!(outcome.vector_indexed_object_refs.len(), 2);
        (adapter.reset_namespace(namespace).await).expect("frozen drift-guard namespace cleanup");
    }

    #[test]
    fn runtime_embedding_inputs_normalize_writes_but_preserve_queries() {
        let mut fixture = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = &mut fixture.scenarios[0];
        let remember = scenario
            .events
            .iter()
            .find(|event| matches!(event, InteractionEvent::Remember { .. }))
            .unwrap()
            .clone();
        let query = scenario
            .events
            .iter()
            .find(|event| matches!(event, InteractionEvent::Query { .. }))
            .unwrap()
            .clone();
        scenario.entities.clear();
        scenario.events = vec![remember, query];
        let InteractionEvent::Remember { text, .. } = &mut scenario.events[0] else {
            unreachable!()
        };
        *text = "  remembered\n\ttarget  ".to_string();
        let InteractionEvent::Query { text, .. } = &mut scenario.events[1] else {
            unreachable!()
        };
        *text = "  target\nquery  ".to_string();

        assert_eq!(
            scenario.runtime_embedding_inputs(),
            BTreeSet::from([
                "  target\nquery  ".to_string(),
                "remembered target".to_string(),
            ])
        );
    }

    #[test]
    fn runtime_embedding_inputs_ignore_unused_top_level_surface_text() {
        let fixture = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut scenario = fixture
            .scenarios
            .into_iter()
            .find(|scenario| scenario.pattern == ScenarioPattern::SurfaceContribution)
            .unwrap();
        let mut remember = scenario
            .events
            .into_iter()
            .find(|event| {
                matches!(
                    event,
                    InteractionEvent::Remember {
                        surface_texts: Some(_),
                        ..
                    }
                )
            })
            .unwrap();
        let InteractionEvent::Remember {
            text,
            surface_texts: Some(surface_texts),
            ..
        } = &mut remember
        else {
            unreachable!()
        };
        *text = "unused top-level alias".to_string();
        let expected = BTreeSet::from([
            runtime_memory_embedding_text(&surface_texts.episode),
            runtime_memory_embedding_text(&surface_texts.observation),
            runtime_memory_embedding_text(&surface_texts.derived),
        ]);
        scenario.entities.clear();
        scenario.events = vec![remember];

        assert_eq!(scenario.runtime_embedding_inputs(), expected);
        assert!(
            !scenario
                .runtime_embedding_inputs()
                .contains("unused top-level alias")
        );
    }

    type Admission = (FixtureLocation, &'static str, FixtureAdmissionKind);

    fn admission_of(error: FixtureError) -> Admission {
        match error {
            FixtureError::Admission {
                location,
                field,
                kind,
            } => (location, field, kind),
            other => panic!("expected an admission error, got {other}"),
        }
    }

    fn admission(fixtures: &ContinuityFixtureSet) -> Admission {
        let bytes = serde_json::to_vec(fixtures).unwrap();
        admission_of(parse_fixture_bytes(&bytes).unwrap_err())
    }

    fn shape(value: &Value) -> (FixtureLocation, Option<String>) {
        match parse_fixture_bytes(&serde_json::to_vec(value).unwrap()).unwrap_err() {
            FixtureError::Shape {
                location, field, ..
            } => (location, field),
            other => panic!("expected a shape error, got {other}"),
        }
    }

    fn expected_admission(
        fixture_id: &str,
        event_id: Option<&str>,
        field: &'static str,
        kind: FixtureAdmissionKind,
    ) -> Admission {
        (
            FixtureLocation::Scenario {
                fixture_id: fixture_id.to_string(),
                event_id: event_id.map(str::to_string),
            },
            field,
            kind,
        )
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

    fn query_event_id(scenario: &ContinuityScenario) -> String {
        scenario
            .events
            .iter()
            .find(|event| matches!(event, InteractionEvent::Query { .. }))
            .unwrap()
            .event_id()
            .to_string()
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

    const CORRECTION_TARGET_KINDS: &[ContinuityObjectKind] = &[
        ContinuityObjectKind::Episode,
        ContinuityObjectKind::Observation,
        ContinuityObjectKind::DerivedMemory,
    ];
    const FORGET_TARGET_KINDS: &[ContinuityObjectKind] = &[
        ContinuityObjectKind::Episode,
        ContinuityObjectKind::Observation,
        ContinuityObjectKind::DerivedMemory,
        ContinuityObjectKind::MemoryThread,
    ];
    const LINK_ENDPOINT_KINDS: &[ContinuityObjectKind] = &[
        ContinuityObjectKind::Episode,
        ContinuityObjectKind::Observation,
        ContinuityObjectKind::Entity,
        ContinuityObjectKind::MemoryThread,
        ContinuityObjectKind::DerivedMemory,
    ];

    #[test]
    fn public_parser_keeps_the_last_repeated_object_key() {
        let canonical = include_str!("../fixtures/continuity_v3.json");
        let repeated = canonical.replacen('{', "{\"seed\":123,", 1);
        assert_eq!(
            parse_fixture_bytes(repeated.as_bytes()).unwrap(),
            parse_fixture_bytes(canonical.as_bytes()).unwrap()
        );
    }

    #[test]
    fn public_parser_attributes_root_shape_errors_before_scenario_errors() {
        let mut value: Value =
            serde_json::from_str(include_str!("../fixtures/continuity_v3.json")).unwrap();
        value["scenarios"][0]["entities"][0]["entity_type"] = Value::from("unknown-kind");
        assert_eq!(
            shape(&value),
            (FixtureLocation::scenario("long-gap-recall"), None)
        );

        for field in ["aaa_root_typo", "zzz_root_typo"] {
            value[field] = Value::Bool(true);
            assert_eq!(
                shape(&value),
                (FixtureLocation::Root, Some(field.to_string()))
            );
            value.as_object_mut().unwrap().remove(field);
        }

        value["seed"] = Value::from("wrong-type");
        assert_eq!(shape(&value), (FixtureLocation::Root, None));
    }

    #[test]
    fn public_parser_rejects_out_of_order_events() {
        let mut value: Value =
            serde_json::from_str(include_str!("../fixtures/continuity_v3.json")).unwrap();
        let scenario = &mut value["scenarios"][0];
        let fixture_id = scenario["fixture_id"].as_str().unwrap().to_string();
        let event = scenario["events"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap();
        let event_id = event["event_id"].as_str().unwrap().to_string();
        event["timestamp"] = Value::from("1900-01-01T00:00:00Z");
        assert_eq!(
            admission_of(parse_fixture_bytes(&serde_json::to_vec(&value).unwrap()).unwrap_err()),
            expected_admission(
                &fixture_id,
                Some(&event_id),
                "timestamp",
                FixtureAdmissionKind::NotChronological,
            )
        );
    }

    #[test]
    fn public_parser_rejects_undeclared_remember_entities() {
        let mut value: Value =
            serde_json::from_str(include_str!("../fixtures/continuity_v3.json")).unwrap();
        let scenario = &mut value["scenarios"][0];
        let fixture_id = scenario["fixture_id"].as_str().unwrap().to_string();
        let event = scenario["events"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|event| event["kind"] == "remember")
            .unwrap();
        let event_id = event["event_id"].as_str().unwrap().to_string();
        event["entity_external_ids"] = serde_json::json!(["undeclared-entity"]);
        assert_eq!(
            admission_of(parse_fixture_bytes(&serde_json::to_vec(&value).unwrap()).unwrap_err()),
            expected_admission(
                &fixture_id,
                Some(&event_id),
                "remember.entity_external_ids",
                FixtureAdmissionKind::Undeclared("undeclared-entity".to_string()),
            )
        );
    }

    #[test]
    fn public_parser_rejects_unassigned_query_embedding_input() {
        let mut value: Value =
            serde_json::from_str(include_str!("../fixtures/continuity_v3.json")).unwrap();
        let scenario = &mut value["scenarios"][0];
        let fixture_id = scenario["fixture_id"].as_str().unwrap().to_string();
        let event = scenario["events"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|event| event["kind"] == "query")
            .unwrap();
        let event_id = event["event_id"].as_str().unwrap().to_string();
        event["text"] = Value::from("unassigned-query");
        assert_eq!(
            admission_of(parse_fixture_bytes(&serde_json::to_vec(&value).unwrap()).unwrap_err()),
            expected_admission(
                &fixture_id,
                Some(&event_id),
                "query.text",
                FixtureAdmissionKind::UnassignedEmbeddingInput("unassigned-query".to_string()),
            )
        );
    }

    #[test]
    fn public_parser_rejects_v1_and_retired_caller_supplied_identity_fields() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut v1 = serde_json::to_value(&fixtures).unwrap();
        v1["schema_version"] = Value::from(1);
        let error = parse_fixture_bytes(&serde_json::to_vec(&v1).unwrap()).unwrap_err();
        assert_eq!(
            admission_of(error),
            (
                FixtureLocation::Root,
                "schema_version",
                FixtureAdmissionKind::UnsupportedSchemaVersion { found: 1 }
            )
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
            let fixture_id = scenario["fixture_id"].as_str().unwrap().to_string();
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
            assert_eq!(
                shape(&value),
                (
                    FixtureLocation::Scenario {
                        fixture_id,
                        event_id: None
                    },
                    Some(field.to_string())
                )
            );
        }

        let scenario_zero = FixtureLocation::Scenario {
            fixture_id: fixtures.scenarios[0].fixture_id.clone(),
            event_id: None,
        };
        let mut value = serde_json::to_value(&fixtures).unwrap();
        value["scenarios"][0]["entities"][0]
            .as_object_mut()
            .unwrap()
            .insert("memory_id".to_string(), Value::from("retired-id"));
        assert_eq!(
            shape(&value),
            (scenario_zero.clone(), Some("memory_id".to_string()))
        );

        let mut value = serde_json::to_value(&fixtures).unwrap();
        value["scenarios"][0]
            .as_object_mut()
            .unwrap()
            .insert("collection_name".to_string(), Value::from("retired-name"));
        assert_eq!(
            shape(&value),
            (scenario_zero, Some("collection_name".to_string()))
        );
    }

    #[test]
    fn public_parser_rejects_unknown_entity_kinds_before_validation() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut value = serde_json::to_value(&fixtures).unwrap();
        value["scenarios"][0]["entities"][0]["entity_type"] = Value::from("inferred-from-label");

        assert_eq!(
            shape(&value),
            (
                FixtureLocation::Scenario {
                    fixture_id: fixtures.scenarios[0].fixture_id.clone(),
                    event_id: None
                },
                None
            )
        );
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
        assert_eq!(
            shape(&value),
            (
                FixtureLocation::Scenario {
                    fixture_id: fixtures.scenarios[0].fixture_id.clone(),
                    event_id: None
                },
                Some("irrelevant_external_ids".to_string())
            )
        );

        let mut fixtures = fixtures;
        expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall))
            .irrelevant_external_ids
            .clear();
        fixtures.validate().unwrap();
    }

    #[test]
    fn public_parser_rejects_duplicate_or_overlapping_relevance_labels() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let query = query_event_id(scenario);
        let expected = expected_mut(scenario);
        let relevant = expected.relevant_external_ids[0].clone();
        let irrelevant = expected.irrelevant_external_ids[0].clone();
        expected.relevant_external_ids.push(relevant.clone());
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&query),
                "query.expected.relevant_external_ids",
                FixtureAdmissionKind::Duplicate(relevant.clone())
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall))
            .irrelevant_external_ids
            .push(irrelevant.clone());
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&query),
                "query.expected.irrelevant_external_ids",
                FixtureAdmissionKind::Duplicate(irrelevant)
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        expected_mut(scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall))
            .irrelevant_external_ids
            .push(relevant.clone());
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&query),
                "query.expected.irrelevant_external_ids",
                FixtureAdmissionKind::Overlap(relevant)
            )
        );
    }

    #[test]
    fn public_parser_rejects_relevance_labels_before_external_id_admission() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let query = query_event_id(scenario);
        scenario.events.swap(1, 2);
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&query),
                "query.expected.irrelevant_external_ids",
                FixtureAdmissionKind::NotAdmitted("memory-recent".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_dangling_correction_and_forget_targets() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let correct = scenario.events[1].event_id().to_string();
        let InteractionEvent::Correct {
            target_external_id, ..
        } = &mut scenario.events[1]
        else {
            panic!("expected correction event");
        };
        *target_external_id = "missing-correction-target".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&correct),
                "correct.target_external_id",
                FixtureAdmissionKind::NotAdmitted("missing-correction-target".to_string())
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let forget = scenario.events[3].event_id().to_string();
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "missing-forget-target".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::NotAdmitted("missing-forget-target".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_correction_targets_the_driver_cannot_correct() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let correct = scenario.events[1].event_id().to_string();
        let InteractionEvent::Correct {
            target_external_id, ..
        } = &mut scenario.events[1]
        else {
            panic!("expected correction event");
        };
        *target_external_id = "entity-person".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&correct),
                "correct.target_external_id",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "entity-person".to_string(),
                    found: ContinuityObjectKind::Entity,
                    allowed: CORRECTION_TARGET_KINDS,
                }
            )
        );

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
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&correct),
                "correct.target_external_id",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "correction-link".to_string(),
                    found: ContinuityObjectKind::MemoryLink,
                    allowed: CORRECTION_TARGET_KINDS,
                }
            )
        );
    }

    #[test]
    fn public_parser_rejects_forget_targets_the_driver_cannot_forget() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let forget = scenario.events[3].event_id().to_string();
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "entity-person".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "entity-person".to_string(),
                    found: ContinuityObjectKind::Entity,
                    allowed: FORGET_TARGET_KINDS,
                }
            )
        );

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
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "forget-link".to_string(),
                    found: ContinuityObjectKind::MemoryLink,
                    allowed: FORGET_TARGET_KINDS,
                }
            )
        );
    }

    #[test]
    fn public_parser_rejects_empty_or_duplicate_forget_targets() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let forget = scenario.events[3].event_id().to_string();
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = &mut scenario.events[3]
        else {
            panic!("expected forget event");
        };
        target_external_ids.clear();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::Empty
            )
        );

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
        let duplicate = target_external_ids[0].clone();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::Duplicate(duplicate)
            )
        );
    }

    #[test]
    fn public_parser_rejects_conflicting_default_and_episode_surface_text() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::SurfaceContribution);
        let remember = scenario.events[0].event_id().to_string();
        let InteractionEvent::Remember { text, .. } = &mut scenario.events[0] else {
            panic!("expected remember event");
        };
        *text = "Conflicting Episode text".to_string();

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "surface-contribution",
                Some(&remember),
                "remember.text",
                FixtureAdmissionKind::DiffersFrom("remember.surface_texts.episode")
            )
        );
    }

    #[test]
    fn public_parser_rejects_dangling_link_endpoints() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let link = scenario.events[1].event_id().to_string();
        let InteractionEvent::Link {
            from_external_id, ..
        } = &mut scenario.events[1]
        else {
            panic!("expected link event");
        };
        *from_external_id = "missing-link-source".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "cross-store-stress",
                Some(&link),
                "link.from_external_id",
                FixtureAdmissionKind::NotAdmitted("missing-link-source".to_string())
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let InteractionEvent::Link { to_external_id, .. } = &mut scenario.events[1] else {
            panic!("expected link event");
        };
        *to_external_id = "missing-link-target".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "cross-store-stress",
                Some(&link),
                "link.to_external_id",
                FixtureAdmissionKind::NotAdmitted("missing-link-target".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_memory_links_as_link_endpoints() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        insert_link_before(scenario, 2, "nested-link", "restart-link");

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "cross-store-stress",
                Some("event-test-link-nested-link"),
                "link.from_external_id",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "restart-link".to_string(),
                    found: ContinuityObjectKind::MemoryLink,
                    allowed: LINK_ENDPOINT_KINDS,
                }
            )
        );
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
        let remember = scenario.events[1].event_id().to_string();
        let InteractionEvent::Remember { external_id, .. } = &mut scenario.events[1] else {
            panic!("expected remember event");
        };
        *external_id = "memory-dormant:observation".to_string();

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&remember),
                "remember.external_id",
                FixtureAdmissionKind::Duplicate("memory-dormant:observation".to_string())
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::ThreadDrift);
        let remember = scenario.events[0].event_id().to_string();
        let InteractionEvent::Remember {
            thread: Some(thread),
            ..
        } = &mut scenario.events[0]
        else {
            panic!("expected threaded remember event");
        };
        thread.thread_external_id = "entity-person".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "thread-drift",
                Some(&remember),
                "remember.thread.thread_external_id",
                FixtureAdmissionKind::Collision {
                    admitted_kind: ContinuityObjectKind::Entity
                }
            )
        );
    }

    #[test]
    fn public_parser_rejects_queryless_scenarios() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        scenario
            .events
            .retain(|event| !matches!(event, InteractionEvent::Query { .. }));

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                None,
                "events",
                FixtureAdmissionKind::MissingQuery
            )
        );
    }

    #[test]
    fn public_parser_rejects_operations_after_the_final_query() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let terminal_index = scenario.events.len();
        insert_link_before(
            scenario,
            terminal_index,
            "post-query-link",
            "memory-dormant",
        );

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                None,
                "events",
                FixtureAdmissionKind::MustEndWithQuery
            )
        );
    }

    #[test]
    fn public_parser_rejects_relations_outside_the_facade_vocabulary() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let link = scenario.events[1].event_id().to_string();
        let InteractionEvent::Link { relation, .. } = &mut scenario.events[1] else {
            panic!("expected link event");
        };
        *relation = "invented_relation".to_string();

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "cross-store-stress",
                Some(&link),
                "link.relation",
                FixtureAdmissionKind::UnsupportedRelation("invented_relation".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_duplicate_created_external_ids() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let remember = scenario.events[1].event_id().to_string();
        let InteractionEvent::Remember { external_id, .. } = &mut scenario.events[1] else {
            panic!("expected remember event");
        };
        *external_id = "memory-dormant".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&remember),
                "remember.external_id",
                FixtureAdmissionKind::Duplicate("memory-dormant".to_string())
            )
        );
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
        let event_id = duplicate.event_id().to_string();
        let InteractionEvent::Query { query_id, .. } = &duplicate else {
            unreachable!()
        };
        let query_id = query_id.clone();
        scenario.events.push(duplicate.clone());

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some(&event_id),
                "event_id",
                FixtureAdmissionKind::Duplicate(event_id.clone())
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CorrectionChains);
        let mut duplicate = duplicate;
        let InteractionEvent::Query { event_id, .. } = &mut duplicate else {
            unreachable!()
        };
        *event_id = "event-duplicate-query-id".to_string();
        scenario.events.push(duplicate);
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "correction-chains",
                Some("event-duplicate-query-id"),
                "query.query_id",
                FixtureAdmissionKind::Duplicate(query_id)
            )
        );
    }

    #[test]
    fn public_parser_rejects_invalid_salience_and_thread_confidence() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let remember = scenario.events[0].event_id().to_string();
        let InteractionEvent::Remember { salience, .. } = &mut scenario.events[0] else {
            panic!("expected remember event");
        };
        *salience = 1.1;
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "long-gap-recall",
                Some(&remember),
                "remember.salience",
                FixtureAdmissionKind::OutOfUnitInterval(1.1)
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::ThreadDrift);
        let (threaded, confidence) = scenario
            .events
            .iter_mut()
            .find_map(|event| match event {
                InteractionEvent::Remember {
                    event_id,
                    thread: Some(thread),
                    ..
                } => Some((event_id.clone(), &mut thread.confidence)),
                _ => None,
            })
            .expect("thread drift fixture has thread membership");
        *confidence = -0.1;
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "thread-drift",
                Some(&threaded),
                "remember.thread.confidence",
                FixtureAdmissionKind::OutOfUnitInterval(-0.1)
            )
        );

        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::LongGapRecall);
        let InteractionEvent::Remember { salience, .. } = &mut scenario.events[0] else {
            panic!("expected remember event");
        };
        *salience = f32::NAN;
        let (location, field, kind) = admission_of(scenario.validate().unwrap_err());
        assert_eq!(
            (location, field),
            (
                FixtureLocation::Scenario {
                    fixture_id: "long-gap-recall".to_string(),
                    event_id: Some(remember)
                },
                "remember.salience"
            )
        );
        assert!(
            matches!(kind, FixtureAdmissionKind::OutOfUnitInterval(value) if value.is_nan()),
            "{kind:?}"
        );
    }

    #[test]
    fn public_parser_rejects_restart_without_a_following_query() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
        let (restart_index, restart_event_id) = scenario
            .events
            .iter()
            .enumerate()
            .find_map(|(index, event)| match event {
                InteractionEvent::Restart { event_id, .. } => Some((index, event_id.clone())),
                _ => None,
            })
            .unwrap();
        scenario.events.truncate(restart_index + 1);

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "cross-store-stress",
                Some(&restart_event_id),
                "restart",
                FixtureAdmissionKind::RestartWithoutFollowingQuery
            )
        );
    }

    #[test]
    fn public_parser_rejects_restart_flags_the_runtime_cannot_honor() {
        for (unsupported_field, field) in [
            ("reopen_graph", "restart.reopen_graph"),
            ("reopen_stats", "restart.reopen_stats"),
        ] {
            let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
            let scenario = scenario_mut(&mut fixtures, ScenarioPattern::CrossStoreStress);
            let restart = scenario
                .events
                .iter_mut()
                .find(|event| matches!(event, InteractionEvent::Restart { .. }))
                .unwrap();
            let InteractionEvent::Restart {
                event_id,
                reopen_graph,
                reopen_stats,
                ..
            } = restart
            else {
                unreachable!()
            };
            let restart_event_id = event_id.clone();
            match unsupported_field {
                "reopen_graph" => *reopen_graph = false,
                "reopen_stats" => *reopen_stats = false,
                _ => unreachable!(),
            }

            assert_eq!(
                admission(&fixtures),
                expected_admission(
                    "cross-store-stress",
                    Some(&restart_event_id),
                    field,
                    FixtureAdmissionKind::RestartMustReopen
                )
            );
        }
    }

    #[test]
    fn fixture_reader_rejects_an_incompatible_schema_version() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        fixtures.schema_version = CONTINUITY_FIXTURE_SCHEMA_VERSION + 1;
        assert_eq!(
            admission(&fixtures),
            (
                FixtureLocation::Root,
                "schema_version",
                FixtureAdmissionKind::UnsupportedSchemaVersion {
                    found: u64::from(CONTINUITY_FIXTURE_SCHEMA_VERSION + 1)
                }
            )
        );
    }

    #[test]
    fn public_parser_accepts_explicit_v3_providers_and_rejects_v2_actionably() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let bytes = canonical_fixture_bytes(&fixtures).unwrap();
        let parsed = parse_fixture_bytes(&bytes).unwrap();
        assert_eq!(parsed.schema_version, CONTINUITY_FIXTURE_SCHEMA_VERSION);
        assert!(
            parsed.scenarios.iter().any(|scenario| {
                scenario.embedding.provider_name() == "controllable_similarity"
            })
        );
        assert!(
            parsed
                .scenarios
                .iter()
                .any(|scenario| scenario.embedding.provider_name() == "frozen")
        );

        let mut v2 = serde_json::to_value(&fixtures).unwrap();
        v2["schema_version"] = Value::from(2);
        v2["scenarios"].as_array_mut().unwrap().truncate(1);
        v2["scenarios"][0]["embedding"]
            .as_object_mut()
            .unwrap()
            .remove("provider");
        let error = parse_fixture_bytes(&serde_json::to_vec(&v2).unwrap()).unwrap_err();
        assert_eq!(
            admission_of(error),
            (
                FixtureLocation::Root,
                "schema_version",
                FixtureAdmissionKind::UnsupportedSchemaVersion { found: 2 }
            )
        );
    }

    #[test]
    fn public_parser_rejects_partial_malformed_and_typoed_v3_embedding_blocks() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let base = serde_json::to_value(&fixtures).unwrap();
        let scenario_zero = FixtureLocation::Scenario {
            fixture_id: fixtures.scenarios[0].fixture_id.clone(),
            event_id: None,
        };

        let mut missing_provider = base.clone();
        missing_provider["scenarios"][0]["embedding"]
            .as_object_mut()
            .unwrap()
            .remove("provider");
        assert_eq!(
            shape(&missing_provider),
            (scenario_zero.clone(), Some("provider".to_string()))
        );

        let mut typoed = base.clone();
        typoed["scenarios"][0]["embedding"] = serde_json::json!({"provder": "frozen"});
        assert_eq!(
            shape(&typoed),
            (scenario_zero.clone(), Some("provder".to_string()))
        );

        let mut malformed_frozen = base.clone();
        malformed_frozen["scenarios"][0]["embedding"] =
            serde_json::json!({"provider": "frozen", "vector_size": 8});
        assert_eq!(
            shape(&malformed_frozen),
            (scenario_zero.clone(), Some("vector_size".to_string()))
        );

        let mut partial = base;
        partial["scenarios"][0]["embedding"] = serde_json::json!({
            "provider": "controllable_similarity",
            "seed": 1,
            "vector_size": 8
        });
        assert_eq!(
            shape(&partial),
            (scenario_zero, Some("noise_magnitude".to_string()))
        );
    }

    #[test]
    fn schema_v3_admits_downstream_patterns_and_couples_abstention_labels() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let fixture_id = fixtures.scenarios[0].fixture_id.clone();
        let query = query_event_id(&fixtures.scenarios[0]);
        fixtures.scenarios[0].pattern = ScenarioPattern::Abstention;
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                &fixture_id,
                Some(&query),
                "query.expected.relevant_external_ids",
                FixtureAdmissionKind::ForbiddenForPattern(ScenarioPattern::Abstention)
            )
        );

        expected_mut(&mut fixtures.scenarios[0])
            .relevant_external_ids
            .clear();
        parse_fixture_bytes(&serde_json::to_vec(&fixtures).unwrap()).unwrap();

        fixtures.scenarios[0].pattern = ScenarioPattern::Autobiographical;
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                &fixture_id,
                Some(&query),
                "query.expected.relevant_external_ids",
                FixtureAdmissionKind::Empty
            )
        );
    }
}
