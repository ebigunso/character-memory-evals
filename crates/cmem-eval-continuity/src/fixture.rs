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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalog_situations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_entity: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scenes: BTreeMap<String, Scene>,
    pub entities: Vec<EntityDeclaration>,
    pub embedding: ContinuityScenarioEmbedding,
    pub events: Vec<InteractionEvent>,
    #[serde(skip)]
    pub requirements: ScenarioRequirements,
}

/// Perceived input contains no author-supplied resolution or scoring labels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "by", rename_all = "snake_case", deny_unknown_fields)]
pub enum PerceivedReference {
    Key { key: String },
    Setting { key: String },
    Name { text: String },
    Description { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SceneParticipant {
    pub reference: PerceivedReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gold_entity: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    #[serde(default)]
    pub who: Vec<SceneParticipant>,
    #[serde(default, rename = "where", skip_serializing_if = "Option::is_none")]
    pub place: Option<PerceivedReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<PerceivedReference>,
    #[serde(default)]
    pub custom: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SceneSelection {
    Named { name: String },
    Inline { scene: Scene },
}

/// Only this projection is handed to the adapter; even textual references have
/// no field in which an author's intended entity could travel.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SceneInput {
    pub who: Vec<PerceivedReference>,
    pub place: Option<PerceivedReference>,
    pub what: Option<PerceivedReference>,
    pub custom: BTreeMap<String, String>,
}

impl Scene {
    pub fn input(&self) -> SceneInput {
        SceneInput {
            who: self
                .who
                .iter()
                .map(|person| person.reference.clone())
                .collect(),
            place: self.place.clone(),
            what: self.what.clone(),
            custom: self.custom.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredMemoryKind {
    Reflection,
    Preference,
    RelationshipNote,
    OpenLoop,
    Commitment,
    Intention,
    CharacterSignal,
    Thread,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentionTrigger {
    Participant { entity: String },
    Topic { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthoredMemory {
    pub subtype: AuthoredMemoryKind,
    pub text: String,
    pub experiences: Vec<String>,
    pub about: Vec<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterpart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<IntentionTrigger>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedWriteWarning {
    NearVerbatimRestatement,
    ChurningChain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "by", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenePartition {
    Participants,
    Setting,
    Custom { key: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallReason {
    Pair,
    Due,
    Date,
    Trigger,
    Activity,
    OwnDay,
    RecentAndSalient,
    Topic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Pair,
    Due,
    Date,
    Trigger,
    Activity,
    OwnDay,
    RecentAndSalient,
    Topic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySection {
    Threads,
    Episodes,
    Observations,
    DerivedMemories,
    Preferences,
    RelationshipNotes,
    OpenLoops,
    Commitments,
    CharacterSignals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CarriedAssertion {
    pub memory: String,
    pub reason: RecallReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<MemorySection>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OmissionReason {
    Partition,
    Resolution,
    Supersession,
    Suppression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OmittedAssertion {
    pub memory: String,
    pub reason: OmissionReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotCuedAssertion {
    pub memory: String,
    pub cue: CueKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpectedReferenceResolution {
    Resolved { entity: String },
    Ambiguous { candidates: Vec<String> },
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceAssertion {
    pub participant: PerceivedReference,
    pub resolution: ExpectedReferenceResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemorySceneAssertion {
    pub memory: String,
    pub scene: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ProbeAssertions {
    pub carried: Vec<CarriedAssertion>,
    pub in_order: Vec<Vec<String>>,
    pub omitted: Vec<OmittedAssertion>,
    pub not_cued: Vec<NotCuedAssertion>,
    pub references: Vec<ReferenceAssertion>,
    pub scenes: Vec<MemorySceneAssertion>,
    /// Counterpart entities. The loader computes ages, not the author.
    pub elapsed_since_met: Vec<String>,
    /// Memories whose age since their latest supporting evidence is checked.
    pub staleness: Vec<String>,
}

/// Recall by reason and context tokens are always reported for a probe.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ProbeMeasures {
    pub bystanders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssertionIdentity {
    pub event_id: String,
    #[serde(flatten)]
    pub assertion: AssertionSubject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", content = "subject", rename_all = "snake_case")]
pub enum AssertionSubject {
    Carried(String),
    InOrder(Vec<String>),
    Omitted(String),
    NotCued(String),
    References(PerceivedReference),
    Scene(String),
    ElapsedSinceMet(String),
    Staleness(String),
    Partition,
    WriteWarning(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioFeature {
    WriteScene,
    WriteSceneWhere,
    WriteSceneWhat,
    WriteSceneCustom,
    ProbeScene,
    NoTopic,
    ReferenceTime,
    ParticipantName,
    ParticipantDescription,
    Partition,
    Direction,
    DueDate,
    Trigger,
    AuthoredDerivedMemory,
    IntentionMemory,
    PreferenceMemory,
    ThreadProvenance,
    CueTrace,
    ReferenceTrace,
    PartitionTrace,
    MemorySceneTrace,
    ElapsedSinceMet,
    Staleness,
    OmissionReasons,
    WriteWarnings,
    PackSections,
    PackOrder,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComputedProbeGold {
    pub elapsed_since_met: BTreeMap<String, chrono::Duration>,
    pub staleness: BTreeMap<String, chrono::Duration>,
    pub partition_applied: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScenarioRequirements {
    pub features: BTreeSet<ScenarioFeature>,
    pub probes: BTreeMap<String, ComputedProbeGold>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SituatedInput {
    Experience {
        external_id: String,
        text: String,
        scene: SceneInput,
        speaker: Option<String>,
    },
    Derive {
        external_id: String,
        memory: AuthoredMemory,
    },
    Probe {
        query_id: String,
        scene: SceneInput,
        topic: Option<String>,
        partition: Option<ScenePartition>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContinuityScenarioEmbedding {
    ControllableSimilarity {
        fixture: ControllableSimilarityFixture,
        own_concept: bool,
    },
    Frozen,
}

impl ContinuityScenarioEmbedding {
    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::ControllableSimilarity { .. } => "controllable_similarity",
            Self::Frozen => "frozen",
        }
    }

    pub fn controllable_similarity(&self) -> Option<&ControllableSimilarityFixture> {
        match self {
            Self::ControllableSimilarity { fixture, .. } => Some(fixture),
            Self::Frozen => None,
        }
    }

    pub fn controllable_similarity_mut(&mut self) -> Option<&mut ControllableSimilarityFixture> {
        match self {
            Self::ControllableSimilarity { fixture, .. } => Some(fixture),
            Self::Frozen => None,
        }
    }

    pub fn controllable_similarity_provider(fixture: ControllableSimilarityFixture) -> Self {
        Self::ControllableSimilarity {
            fixture,
            own_concept: false,
        }
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
            Self::ControllableSimilarity {
                fixture,
                own_concept,
            } => {
                let mut value = serde_json::to_value(fixture).map_err(serde::ser::Error::custom)?;
                value
                    .as_object_mut()
                    .expect("controllable similarity fixture serializes as an object")
                    .insert(
                        "provider".to_string(),
                        Value::String("controllable_similarity".to_string()),
                    );
                if *own_concept {
                    value
                        .as_object_mut()
                        .unwrap()
                        .insert("own_concept".into(), Value::Bool(true));
                }
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
            "own_concept",
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
                let own_concept = fixture
                    .remove("own_concept")
                    .map(serde_json::from_value::<bool>)
                    .transpose()
                    .map_err(D::Error::custom)?
                    .unwrap_or(false);
                serde_json::from_value(Value::Object(fixture))
                    .map(|fixture| Self::ControllableSimilarity {
                        fixture,
                        own_concept,
                    })
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
    Situated,
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
    Experience {
        event_id: String,
        timestamp: DateTime<Utc>,
        text: String,
        scene: SceneSelection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speaker: Option<String>,
    },
    Derive {
        event_id: String,
        timestamp: DateTime<Utc>,
        memory: AuthoredMemory,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_warning: Option<ExpectedWriteWarning>,
    },
    Probe {
        event_id: String,
        query_id: String,
        timestamp: DateTime<Utc>,
        scene: SceneSelection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        partition: Option<ScenePartition>,
        #[serde(default)]
        assertions: Box<ProbeAssertions>,
        #[serde(default)]
        measures: ProbeMeasures,
    },
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
    InvalidSceneReference,
    NotAdmittedActivity(String),
    WriteSceneMustUseKeys,
    RequiredWith(&'static str),
    InvalidAssertion(&'static str),
}

#[derive(Debug)]
pub enum FixtureError {
    Io(std::io::Error),
    UnsupportedFormat(String),
    Toml(toml::de::Error),
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
            Self::InvalidSceneReference => write!(f, "invalid scene reference"),
            Self::NotAdmittedActivity(id) => write!(
                f,
                "activity key {id:?} must reference an earlier authored thread or open loop"
            ),
            Self::WriteSceneMustUseKeys => {
                write!(f, "write-side scenes require identity or setting keys")
            }
            Self::RequiredWith(field) => write!(f, "required with {field}"),
            Self::InvalidAssertion(reason) => write!(f, "invalid assertion: {reason}"),
        }
    }
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "continuity fixture I/O: {source}"),
            Self::UnsupportedFormat(extension) => write!(
                f,
                "unsupported continuity fixture extension {extension:?}; expected json or toml"
            ),
            Self::Toml(source) => write!(f, "continuity fixture TOML: {source}"),
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
            Self::Io(source) => Some(source),
            Self::Toml(source) => Some(source),
            Self::UnsupportedFormat(_) => None,
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
                }
                | InteractionEvent::Probe {
                    event_id, query_id, ..
                } = event
                    && !query_ids.insert(query_id)
                {
                    return Err(
                        FixtureLocation::event(&scenario.fixture_id, event_id).error(
                            if matches!(event, InteractionEvent::Probe { .. }) {
                                "probe.query_id"
                            } else {
                                "query.query_id"
                            },
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
    fn require_situated_header(&self, location: &FixtureLocation) -> Result<(), FixtureError> {
        if self.character_entity.is_none() {
            return Err(location.error(
                "character_entity",
                FixtureAdmissionKind::RequiredWith("situated events"),
            ));
        }
        if self.catalog_situations.is_empty() {
            return Err(location.error(
                "catalog_situations",
                FixtureAdmissionKind::RequiredWith("situated events"),
            ));
        }
        Ok(())
    }

    fn scene<'a>(
        &'a self,
        selection: &'a SceneSelection,
        location: &FixtureLocation,
    ) -> Result<&'a Scene, FixtureError> {
        match selection {
            SceneSelection::Named { name } => self.scenes.get(name).ok_or_else(|| {
                location.error("scene", FixtureAdmissionKind::NotAdmitted(name.clone()))
            }),
            SceneSelection::Inline { scene } => Ok(scene),
        }
    }

    /// Library-input projection. Expectations, bystanders, intended resolutions,
    /// and computed gold have no representation in this return type.
    pub fn situated_input(
        &self,
        event: &InteractionEvent,
    ) -> Result<Option<SituatedInput>, FixtureError> {
        let location = FixtureLocation::event(&self.fixture_id, event.event_id());
        Ok(match event {
            InteractionEvent::Experience {
                event_id: external_id,
                text,
                scene,
                speaker,
                ..
            } => Some(SituatedInput::Experience {
                external_id: external_id.clone(),
                text: text.clone(),
                scene: self.scene(scene, &location)?.input(),
                speaker: speaker.clone(),
            }),
            InteractionEvent::Derive {
                event_id: external_id,
                memory,
                ..
            } => Some(SituatedInput::Derive {
                external_id: external_id.clone(),
                memory: memory.clone(),
            }),
            InteractionEvent::Probe {
                query_id,
                scene,
                topic,
                partition,
                ..
            } => Some(SituatedInput::Probe {
                query_id: query_id.clone(),
                scene: self.scene(scene, &location)?.input(),
                topic: topic.clone(),
                partition: partition.clone(),
            }),
            _ => None,
        })
    }

    pub fn embedding_inputs(&self) -> BTreeSet<&str> {
        let mut inputs = self
            .entities
            .iter()
            .map(|entity| entity.label.as_str())
            .collect::<BTreeSet<_>>();
        for event in &self.events {
            match event {
                InteractionEvent::Experience { text, .. } => {
                    inputs.insert(text);
                }
                InteractionEvent::Derive { memory, .. } => {
                    inputs.insert(&memory.text);
                    if let Some(IntentionTrigger::Topic { text }) = &memory.trigger {
                        inputs.insert(text);
                    }
                }
                InteractionEvent::Probe { topic, .. } => {
                    if let Some(topic) = topic {
                        inputs.insert(topic);
                    }
                }
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
        inputs.extend(self.scene_texts());
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
                InteractionEvent::Experience { text, .. } => {
                    inputs.insert(runtime_memory_embedding_text(text));
                }
                InteractionEvent::Derive { memory, .. } => {
                    inputs.insert(runtime_memory_embedding_text(&memory.text));
                }
                InteractionEvent::Probe { topic, .. } => {
                    if let Some(topic) = topic {
                        inputs.insert(topic.clone());
                    }
                }
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
        inputs.extend(self.scene_texts().into_iter().map(str::to_string));
        inputs
    }

    fn scene_texts(&self) -> BTreeSet<&str> {
        let mut texts = BTreeSet::new();
        for event in &self.events {
            let selection = match event {
                InteractionEvent::Experience { scene, .. }
                | InteractionEvent::Probe { scene, .. } => scene,
                _ => continue,
            };
            let scene = match selection {
                SceneSelection::Named { name } => self.scenes.get(name),
                SceneSelection::Inline { scene } => Some(scene),
            };
            if let Some(scene) = scene {
                for reference in scene
                    .who
                    .iter()
                    .map(|person| &person.reference)
                    .chain(scene.place.iter())
                    .chain(scene.what.iter())
                {
                    if let PerceivedReference::Name { text }
                    | PerceivedReference::Description { text } = reference
                    {
                        texts.insert(text.as_str());
                    }
                }
            }
        }
        texts
    }

    pub fn validate(&self) -> Result<(), FixtureError> {
        self.analyze().map(|_| ())
    }

    pub(crate) fn analyze(&self) -> Result<ScenarioRequirements, FixtureError> {
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
        if let Some(character) = &self.character_entity {
            require_entity(&scenario, "character_entity", character, &declared_entities)?;
        }
        require_distinct(&scenario, "catalog_situations", &self.catalog_situations)?;
        for situation in &self.catalog_situations {
            require_non_empty(&scenario, "catalog_situations", situation)?;
        }
        for (name, scene) in &self.scenes {
            require_non_empty(&scenario, "scenes", name)?;
            validate_scene(scene, false, &scenario, &declared_entities, None)?;
        }
        let mut requirements = ScenarioRequirements::default();
        let mut support_times = BTreeMap::new();
        let mut authored_memories = BTreeMap::new();
        let mut activities = BTreeSet::new();
        let mut last_met = BTreeMap::new();
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
                InteractionEvent::Experience {
                    event_id: external_id,
                    text,
                    scene,
                    speaker,
                    ..
                } => {
                    self.require_situated_header(&location)?;
                    let scene = self.scene(scene, &location)?;
                    validate_scene(
                        scene,
                        true,
                        &location,
                        &declared_entities,
                        Some(&activities),
                    )?;
                    if let Some(speaker) = speaker {
                        require_entity(
                            &location,
                            "experience.speaker",
                            speaker,
                            &declared_entities,
                        )?;
                    }
                    require_non_empty(&location, "experience.text", text)?;
                    require_embedding_input(
                        &location,
                        "experience.text",
                        assigned_inputs.as_ref(),
                        text,
                    )?;
                    admit_external_id(
                        &location,
                        "experience.external_id",
                        external_id,
                        ContinuityObjectKind::Episode,
                        &mut admitted_external_ids,
                    )?;
                    admit_external_id(
                        &location,
                        "experience.observation_external_id",
                        &observation_external_id(external_id),
                        ContinuityObjectKind::Observation,
                        &mut admitted_external_ids,
                    )?;
                    support_times.insert(external_id.clone(), timestamp);
                    support_times.insert(observation_external_id(external_id), timestamp);
                    authored_memories.insert(external_id.clone(), ContinuityObjectKind::Episode);
                    let keys = scene
                        .who
                        .iter()
                        .filter_map(|participant| match &participant.reference {
                            PerceivedReference::Key { key } => Some(key.as_str()),
                            _ => None,
                        })
                        .collect::<BTreeSet<_>>();
                    let character = self.character_entity.as_deref().unwrap();
                    if keys.contains(character) {
                        for counterpart in keys.into_iter().filter(|key| *key != character) {
                            last_met.insert(counterpart.to_string(), timestamp);
                        }
                    }
                    requirements.features.insert(ScenarioFeature::WriteScene);
                    if scene.place.is_some() {
                        requirements
                            .features
                            .insert(ScenarioFeature::WriteSceneWhere);
                    }
                    if scene.what.is_some() {
                        requirements
                            .features
                            .insert(ScenarioFeature::WriteSceneWhat);
                    }
                    if !scene.custom.is_empty() {
                        requirements
                            .features
                            .insert(ScenarioFeature::WriteSceneCustom);
                    }
                }
                InteractionEvent::Derive {
                    event_id: external_id,
                    memory,
                    expected_warning,
                    ..
                } => {
                    self.require_situated_header(&location)?;
                    require_non_empty(&location, "derive.text", &memory.text)?;
                    require_embedding_input(
                        &location,
                        "derive.text",
                        assigned_inputs.as_ref(),
                        &memory.text,
                    )?;
                    if memory.experiences.is_empty() {
                        return Err(
                            location.error("derive.experiences", FixtureAdmissionKind::Empty)
                        );
                    }
                    require_distinct(&location, "derive.experiences", &memory.experiences)?;
                    for experience in &memory.experiences {
                        require_admitted_kind(
                            &location,
                            "derive.experiences",
                            experience,
                            &[ContinuityObjectKind::Episode],
                            &authored_memories,
                        )?;
                    }
                    require_distinct(&location, "derive.about", &memory.about)?;
                    for entity in &memory.about {
                        require_entity(&location, "derive.about", entity, &declared_entities)?;
                    }
                    require_distinct(&location, "derive.supersedes", &memory.supersedes)?;
                    for target in &memory.supersedes {
                        require_admitted_kind(
                            &location,
                            "derive.supersedes",
                            target,
                            &[
                                ContinuityObjectKind::DerivedMemory,
                                ContinuityObjectKind::MemoryThread,
                            ],
                            &authored_memories,
                        )?;
                    }
                    for (field, entity) in [
                        ("derive.actor", &memory.actor),
                        ("derive.counterpart", &memory.counterpart),
                    ] {
                        if let Some(entity) = entity {
                            require_entity(&location, field, entity, &declared_entities)?;
                        }
                    }
                    if memory.actor.is_some() != memory.counterpart.is_some() {
                        return Err(location.error(
                            "derive.direction",
                            FixtureAdmissionKind::RequiredWith("both actor and counterpart"),
                        ));
                    }
                    if memory.actor.is_some() {
                        requirements.features.insert(ScenarioFeature::Direction);
                    }
                    if memory.due.is_some() {
                        requirements.features.insert(ScenarioFeature::DueDate);
                    }
                    if let Some(trigger) = &memory.trigger {
                        match trigger {
                            IntentionTrigger::Participant { entity } => require_entity(
                                &location,
                                "derive.trigger",
                                entity,
                                &declared_entities,
                            )?,
                            IntentionTrigger::Topic { text } => {
                                require_non_empty(&location, "derive.trigger", text)?
                            }
                        }
                        requirements.features.insert(ScenarioFeature::Trigger);
                    }
                    if expected_warning.is_some() {
                        requirements.features.insert(ScenarioFeature::WriteWarnings);
                    }
                    let kind = if memory.subtype == AuthoredMemoryKind::Thread {
                        ContinuityObjectKind::MemoryThread
                    } else {
                        ContinuityObjectKind::DerivedMemory
                    };
                    admit_external_id(
                        &location,
                        "derive.external_id",
                        external_id,
                        kind,
                        &mut admitted_external_ids,
                    )?;
                    support_times.insert(
                        external_id.clone(),
                        memory
                            .experiences
                            .iter()
                            .map(|id| support_times[id])
                            .max()
                            .unwrap(),
                    );
                    authored_memories.insert(external_id.clone(), kind);
                    if matches!(
                        memory.subtype,
                        AuthoredMemoryKind::Thread | AuthoredMemoryKind::OpenLoop
                    ) {
                        activities.insert(external_id.as_str());
                    }
                    requirements
                        .features
                        .insert(ScenarioFeature::AuthoredDerivedMemory);
                    match memory.subtype {
                        AuthoredMemoryKind::Intention => {
                            requirements
                                .features
                                .insert(ScenarioFeature::IntentionMemory);
                        }
                        AuthoredMemoryKind::Preference => {
                            requirements
                                .features
                                .insert(ScenarioFeature::PreferenceMemory);
                        }
                        AuthoredMemoryKind::Thread
                            if !memory.experiences.is_empty()
                                || !memory.about.is_empty()
                                || !memory.supersedes.is_empty() =>
                        {
                            requirements
                                .features
                                .insert(ScenarioFeature::ThreadProvenance);
                        }
                        _ => {}
                    }
                }
                InteractionEvent::Probe {
                    query_id,
                    scene,
                    topic,
                    partition,
                    assertions,
                    measures,
                    ..
                } => {
                    self.require_situated_header(&location)?;
                    require_non_empty(&location, "probe.query_id", query_id)?;
                    if !query_ids.insert(query_id) {
                        return Err(location.error(
                            "probe.query_id",
                            FixtureAdmissionKind::Duplicate(query_id.clone()),
                        ));
                    }
                    let scene = self.scene(scene, &location)?;
                    validate_scene(
                        scene,
                        false,
                        &location,
                        &declared_entities,
                        Some(&activities),
                    )?;
                    for reference in scene
                        .who
                        .iter()
                        .map(|person| &person.reference)
                        .chain(scene.place.iter())
                        .chain(scene.what.iter())
                    {
                        match reference {
                            PerceivedReference::Name { .. } => {
                                requirements
                                    .features
                                    .insert(ScenarioFeature::ParticipantName);
                            }
                            PerceivedReference::Description { .. } => {
                                requirements
                                    .features
                                    .insert(ScenarioFeature::ParticipantDescription);
                            }
                            _ => {}
                        }
                    }
                    if let Some(topic) = topic {
                        require_non_empty(&location, "probe.topic", topic)?;
                        require_embedding_input(
                            &location,
                            "probe.topic",
                            assigned_inputs.as_ref(),
                            topic,
                        )?;
                    } else {
                        requirements.features.insert(ScenarioFeature::NoTopic);
                    }
                    if let Some(partition) = partition {
                        if let ScenePartition::Custom { key } = partition {
                            require_non_empty(&location, "probe.partition.key", key)?;
                            if !scene.custom.contains_key(key) {
                                return Err(location.error(
                                    "probe.partition.key",
                                    FixtureAdmissionKind::NotAdmitted(key.clone()),
                                ));
                            }
                        }
                        requirements
                            .features
                            .extend([ScenarioFeature::Partition, ScenarioFeature::PartitionTrace]);
                    }
                    let gold = ProbeAdmission {
                        location: &location,
                        entities: &declared_entities,
                        memories: &authored_memories,
                        support_times: &support_times,
                        last_met: &last_met,
                        scenes: &self.scenes,
                        timestamp,
                    }
                    .validate(
                        assertions,
                        measures,
                        scene,
                        partition.as_ref(),
                        &mut requirements.features,
                    )?;
                    requirements.probes.insert(query_id.clone(), gold);
                    requirements.features.extend([
                        ScenarioFeature::ProbeScene,
                        ScenarioFeature::ReferenceTime,
                        ScenarioFeature::OmissionReasons,
                    ]);
                }
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
                    for id in [
                        external_id.clone(),
                        observation_external_id(external_id),
                        derived_external_id(external_id),
                    ] {
                        support_times.insert(id, timestamp);
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
                        support_times.insert(thread.thread_external_id.clone(), timestamp);
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
                    support_times.insert(replacement_external_id.clone(), timestamp);
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
                    if !self.events[event_index + 1..].iter().any(|event| {
                        matches!(
                            event,
                            InteractionEvent::Query { .. } | InteractionEvent::Probe { .. }
                        )
                    }) {
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
        if !matches!(
            self.events.last(),
            Some(InteractionEvent::Query { .. } | InteractionEvent::Probe { .. })
        ) {
            return Err(scenario.error("events", FixtureAdmissionKind::MustEndWithQuery));
        }
        Ok(requirements)
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
    /// Identities use the authored subject, so list and participant order are immaterial.
    pub fn assertion_identities(&self) -> BTreeSet<AssertionIdentity> {
        let mut subjects = Vec::new();
        match self {
            Self::Probe {
                assertions,
                partition,
                ..
            } => {
                subjects.extend(
                    assertions
                        .carried
                        .iter()
                        .map(|a| AssertionSubject::Carried(a.memory.clone())),
                );
                subjects.extend(
                    assertions
                        .in_order
                        .iter()
                        .cloned()
                        .map(AssertionSubject::InOrder),
                );
                subjects.extend(
                    assertions
                        .omitted
                        .iter()
                        .map(|a| AssertionSubject::Omitted(a.memory.clone())),
                );
                subjects.extend(
                    assertions
                        .not_cued
                        .iter()
                        .map(|a| AssertionSubject::NotCued(a.memory.clone())),
                );
                subjects.extend(
                    assertions
                        .references
                        .iter()
                        .map(|a| AssertionSubject::References(a.participant.clone())),
                );
                subjects.extend(
                    assertions
                        .scenes
                        .iter()
                        .map(|a| AssertionSubject::Scene(a.memory.clone())),
                );
                subjects.extend(
                    assertions
                        .elapsed_since_met
                        .iter()
                        .cloned()
                        .map(AssertionSubject::ElapsedSinceMet),
                );
                subjects.extend(
                    assertions
                        .staleness
                        .iter()
                        .cloned()
                        .map(AssertionSubject::Staleness),
                );
                if partition.is_some() {
                    subjects.push(AssertionSubject::Partition);
                }
            }
            Self::Derive {
                event_id,
                expected_warning: Some(_),
                ..
            } => {
                subjects.push(AssertionSubject::WriteWarning(event_id.clone()));
            }
            _ => {}
        }
        subjects
            .into_iter()
            .map(|assertion| AssertionIdentity {
                event_id: self.event_id().to_string(),
                assertion,
            })
            .collect()
    }

    pub fn event_id(&self) -> &str {
        match self {
            Self::Experience { event_id, .. }
            | Self::Derive { event_id, .. }
            | Self::Probe { event_id, .. }
            | Self::Remember { event_id, .. }
            | Self::Correct { event_id, .. }
            | Self::Forget { event_id, .. }
            | Self::Link { event_id, .. }
            | Self::Restart { event_id, .. }
            | Self::Query { event_id, .. } => event_id,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Experience { timestamp, .. }
            | Self::Derive { timestamp, .. }
            | Self::Probe { timestamp, .. }
            | Self::Remember { timestamp, .. }
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

/// Select the authoring format by extension; both formats use the same admission.
pub fn read_fixture(path: &std::path::Path) -> Result<ContinuityFixtureSet, FixtureError> {
    let bytes = std::fs::read(path).map_err(FixtureError::Io)?;
    parse_fixture_source(path, &bytes)
}

pub fn parse_fixture_source(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<ContinuityFixtureSet, FixtureError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => parse_fixture_bytes(bytes),
        Some("toml") => {
            let text = std::str::from_utf8(bytes).map_err(|source| {
                FixtureError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, source))
            })?;
            let value: toml::Value = toml::from_str(text).map_err(FixtureError::Toml)?;
            let bytes = serde_json::to_vec(&value).map_err(FixtureError::Json)?;
            parse_fixture_bytes(&bytes)
        }
        extension => Err(FixtureError::UnsupportedFormat(
            extension.unwrap_or_default().to_string(),
        )),
    }
}

pub fn parse_fixture_bytes(bytes: &[u8]) -> Result<ContinuityFixtureSet, FixtureError> {
    let value: Value = serde_json::from_slice(bytes).map_err(FixtureError::Json)?;
    let mut fixtures: ContinuityFixtureSet =
        serde_json::from_value(value).map_err(|source| shape_error(bytes, source))?;
    if fixtures.schema_version != CONTINUITY_FIXTURE_SCHEMA_VERSION {
        fixtures.validate()?;
    }
    for scenario in &mut fixtures.scenarios {
        let inputs = scenario
            .embedding_inputs()
            .into_iter()
            .map(str::to_string)
            .chain(scenario.runtime_embedding_inputs())
            .collect::<BTreeSet<_>>();
        if let ContinuityScenarioEmbedding::ControllableSimilarity {
            fixture,
            own_concept: true,
        } = &mut scenario.embedding
        {
            fixture
                .assign_own_concepts(inputs)
                .map_err(|source| FixtureError::Embedding {
                    location: FixtureLocation::scenario(&scenario.fixture_id),
                    source,
                })?;
        }
    }
    fixtures.validate()?;
    for scenario in &mut fixtures.scenarios {
        scenario.requirements = scenario.analyze()?;
    }
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

fn require_entity(
    location: &FixtureLocation,
    field: &'static str,
    id: &str,
    entities: &BTreeSet<&str>,
) -> Result<(), FixtureError> {
    if !entities.contains(id) {
        return Err(location.error(field, FixtureAdmissionKind::Undeclared(id.to_string())));
    }
    Ok(())
}

fn validate_scene(
    scene: &Scene,
    write: bool,
    location: &FixtureLocation,
    entities: &BTreeSet<&str>,
    activities: Option<&BTreeSet<&str>>,
) -> Result<(), FixtureError> {
    let mut participants = BTreeSet::new();
    for person in &scene.who {
        if !participants.insert(&person.reference) {
            return Err(location.error(
                "scene.who",
                FixtureAdmissionKind::Duplicate(serde_json::to_string(&person.reference).unwrap()),
            ));
        }
        if matches!(person.reference, PerceivedReference::Setting { .. }) {
            return Err(location.error("scene.who", FixtureAdmissionKind::InvalidSceneReference));
        }
        if let Some(gold) = &person.gold_entity {
            require_entity(location, "scene.who.gold_entity", gold, entities)?;
            if let PerceivedReference::Key { key } = &person.reference
                && key != gold
            {
                return Err(location.error(
                    "scene.who.gold_entity",
                    FixtureAdmissionKind::DiffersFrom("scene.who.reference.key"),
                ));
            }
        }
    }
    for reference in scene
        .who
        .iter()
        .map(|person| &person.reference)
        .chain(scene.place.iter())
        .map(|reference| (reference, false))
        .chain(scene.what.iter().map(|reference| (reference, true)))
    {
        match reference {
            (PerceivedReference::Key { key }, true) => {
                require_non_empty(location, "scene.what.key", key)?;
                // Named scenes precede events; activity identity is admitted when the scene is used.
                if activities.is_some_and(|activities| !activities.contains(key.as_str())) {
                    return Err(location.error(
                        "scene.what.key",
                        FixtureAdmissionKind::NotAdmittedActivity(key.clone()),
                    ));
                }
            }
            (PerceivedReference::Key { key }, false) => {
                require_entity(location, "scene.reference.key", key, entities)?
            }
            (PerceivedReference::Setting { key }, _) => {
                require_non_empty(location, "scene.reference.key", key)?
            }
            (PerceivedReference::Name { text } | PerceivedReference::Description { text }, _) => {
                if write {
                    return Err(location.error(
                        "scene.reference",
                        FixtureAdmissionKind::WriteSceneMustUseKeys,
                    ));
                }
                require_non_empty(location, "scene.reference.text", text)?;
            }
        }
    }
    for (key, value) in &scene.custom {
        require_non_empty(location, "scene.custom.key", key)?;
        require_non_empty(location, "scene.custom.value", value)?;
    }
    Ok(())
}

struct ProbeAdmission<'a> {
    location: &'a FixtureLocation,
    entities: &'a BTreeSet<&'a str>,
    memories: &'a BTreeMap<String, ContinuityObjectKind>,
    support_times: &'a BTreeMap<String, DateTime<Utc>>,
    last_met: &'a BTreeMap<String, DateTime<Utc>>,
    scenes: &'a BTreeMap<String, Scene>,
    timestamp: DateTime<Utc>,
}

impl ProbeAdmission<'_> {
    fn validate(
        &self,
        assertions: &ProbeAssertions,
        measures: &ProbeMeasures,
        scene: &Scene,
        partition: Option<&ScenePartition>,
        features: &mut BTreeSet<ScenarioFeature>,
    ) -> Result<ComputedProbeGold, FixtureError> {
        let location = self.location;
        let memory = |field, id: &str| {
            require_admitted_kind(
                location,
                field,
                id,
                &[
                    ContinuityObjectKind::Episode,
                    ContinuityObjectKind::DerivedMemory,
                    ContinuityObjectKind::MemoryThread,
                ],
                self.memories,
            )
        };
        let mut carried = BTreeSet::new();
        for assertion in &assertions.carried {
            memory("probe.assertions.carried", &assertion.memory)?;
            if !carried.insert(assertion.memory.as_str()) {
                return Err(location.error(
                    "probe.assertions.carried",
                    FixtureAdmissionKind::Duplicate(assertion.memory.clone()),
                ));
            }
            if assertion.section.is_some() {
                features.insert(ScenarioFeature::PackSections);
            }
        }
        let mut omitted = BTreeSet::new();
        for assertion in &assertions.omitted {
            memory("probe.assertions.omitted", &assertion.memory)?;
            if !omitted.insert(&assertion.memory) {
                return Err(location.error(
                    "probe.assertions.omitted",
                    FixtureAdmissionKind::Duplicate(assertion.memory.clone()),
                ));
            }
            if carried.contains(assertion.memory.as_str()) {
                return Err(location.error(
                    "probe.assertions.omitted",
                    FixtureAdmissionKind::Overlap(assertion.memory.clone()),
                ));
            }
            if assertion.reason == OmissionReason::Partition && partition.is_none() {
                return Err(location.error(
                    "probe.assertions.omitted",
                    FixtureAdmissionKind::RequiredWith("partition omission"),
                ));
            }
            features.insert(ScenarioFeature::OmissionReasons);
        }
        require_distinct(
            location,
            "probe.assertions.in_order",
            assertions.in_order.iter().flatten(),
        )?;
        for order in &assertions.in_order {
            if order.len() < 2 {
                return Err(location.error(
                    "probe.assertions.in_order",
                    FixtureAdmissionKind::InvalidAssertion(
                        "order needs at least two carried memories",
                    ),
                ));
            }
            for id in order {
                memory("probe.assertions.in_order", id)?;
                if !carried.contains(id.as_str()) {
                    return Err(location.error(
                        "probe.assertions.in_order",
                        FixtureAdmissionKind::NotAdmitted(id.clone()),
                    ));
                }
            }
            features.insert(ScenarioFeature::PackOrder);
        }
        require_distinct(
            location,
            "probe.assertions.not_cued",
            assertions
                .not_cued
                .iter()
                .map(|assertion| &assertion.memory),
        )?;
        for assertion in &assertions.not_cued {
            memory("probe.assertions.not_cued", &assertion.memory)?;
            features.insert(ScenarioFeature::CueTrace);
        }
        let mut references = BTreeSet::new();
        for assertion in &assertions.references {
            let person = scene
                .who
                .iter()
                .find(|person| person.reference == assertion.participant)
                .ok_or_else(|| {
                    location.error(
                        "probe.assertions.references",
                        FixtureAdmissionKind::InvalidSceneReference,
                    )
                })?;
            if !references.insert(&assertion.participant) {
                return Err(location.error(
                    "probe.assertions.references",
                    FixtureAdmissionKind::Duplicate(
                        serde_json::to_string(&assertion.participant).unwrap(),
                    ),
                ));
            }
            match &assertion.resolution {
                ExpectedReferenceResolution::Resolved { entity } => {
                    require_entity(
                        location,
                        "probe.assertions.references",
                        entity,
                        self.entities,
                    )?;
                    if person
                        .gold_entity
                        .as_ref()
                        .is_some_and(|gold| gold != entity)
                    {
                        return Err(location.error(
                            "probe.assertions.references",
                            FixtureAdmissionKind::DiffersFrom("scene.who.gold_entity"),
                        ));
                    }
                }
                ExpectedReferenceResolution::Ambiguous { candidates } => {
                    if candidates.len() < 2 {
                        return Err(location.error(
                            "probe.assertions.references",
                            FixtureAdmissionKind::InvalidAssertion(
                                "ambiguous reference needs at least two candidates",
                            ),
                        ));
                    }
                    require_distinct(location, "probe.assertions.references", candidates)?;
                    for entity in candidates {
                        require_entity(
                            location,
                            "probe.assertions.references",
                            entity,
                            self.entities,
                        )?;
                    }
                    if person
                        .gold_entity
                        .as_ref()
                        .is_some_and(|gold| !candidates.contains(gold))
                    {
                        return Err(location.error(
                            "probe.assertions.references",
                            FixtureAdmissionKind::InvalidAssertion(
                                "candidates exclude the intended entity",
                            ),
                        ));
                    }
                }
                ExpectedReferenceResolution::Unknown => {}
            }
            features.insert(ScenarioFeature::ReferenceTrace);
        }
        require_distinct(
            location,
            "probe.assertions.scenes",
            assertions.scenes.iter().map(|assertion| &assertion.memory),
        )?;
        for assertion in &assertions.scenes {
            memory("probe.assertions.scenes", &assertion.memory)?;
            if !carried.contains(assertion.memory.as_str()) {
                return Err(location.error(
                    "probe.assertions.scenes",
                    FixtureAdmissionKind::NotAdmitted(assertion.memory.clone()),
                ));
            }
            if !self.scenes.contains_key(&assertion.scene) {
                return Err(location.error(
                    "probe.assertions.scenes",
                    FixtureAdmissionKind::NotAdmitted(assertion.scene.clone()),
                ));
            }
            features.insert(ScenarioFeature::MemorySceneTrace);
        }
        let mut gold = ComputedProbeGold {
            partition_applied: partition.is_some(),
            ..Default::default()
        };
        require_distinct(
            location,
            "probe.assertions.elapsed_since_met",
            &assertions.elapsed_since_met,
        )?;
        for counterpart in &assertions.elapsed_since_met {
            require_entity(
                location,
                "probe.assertions.elapsed_since_met",
                counterpart,
                self.entities,
            )?;
            let met = self.last_met.get(counterpart).ok_or_else(|| {
                location.error(
                    "probe.assertions.elapsed_since_met",
                    FixtureAdmissionKind::InvalidAssertion(
                        "no earlier authored meeting with this counterpart",
                    ),
                )
            })?;
            gold.elapsed_since_met
                .insert(counterpart.clone(), self.timestamp - *met);
            features.insert(ScenarioFeature::ElapsedSinceMet);
        }
        require_distinct(
            location,
            "probe.assertions.staleness",
            &assertions.staleness,
        )?;
        for id in &assertions.staleness {
            memory("probe.assertions.staleness", id)?;
            if !carried.contains(id.as_str()) {
                return Err(location.error(
                    "probe.assertions.staleness",
                    FixtureAdmissionKind::NotAdmitted(id.clone()),
                ));
            }
            let supported = self.support_times.get(id).ok_or_else(|| {
                location.error(
                    "probe.assertions.staleness",
                    FixtureAdmissionKind::InvalidAssertion("memory has no supporting timestamp"),
                )
            })?;
            gold.staleness
                .insert(id.clone(), self.timestamp - *supported);
            features.insert(ScenarioFeature::Staleness);
        }
        require_distinct(location, "probe.measures.bystanders", &measures.bystanders)?;
        for id in &measures.bystanders {
            memory("probe.measures.bystanders", id)?;
            if carried.contains(id.as_str()) {
                return Err(location.error(
                    "probe.measures.bystanders",
                    FixtureAdmissionKind::Overlap(id.clone()),
                ));
            }
        }
        Ok(gold)
    }
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

fn require_distinct<'a>(
    location: &FixtureLocation,
    field: &'static str,
    values: impl IntoIterator<Item = &'a String>,
) -> Result<(), FixtureError> {
    let mut seen = BTreeSet::new();
    match values
        .into_iter()
        .find(|value| !seen.insert(value.as_str()))
    {
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

    const SITUATED_CASE: &str = r#"
schema_version = 3
seed = 7
[[scenarios]]
fixture_id = "encounter"
namespace = "encounter"
pattern = "situated"
catalog_situations = ["D4", "D11"]
character_entity = "self"
entities = [
  {external_id = "self", label = "Character", entity_type = "person", is_hub = false},
  {external_id = "intended-entity", label = "Ada", entity_type = "person", is_hub = false},
  {external_id = "other", label = "Bert", entity_type = "person", is_hub = false},
]
[scenarios.embedding]
provider = "controllable_similarity"
own_concept = true
seed = 7
vector_size = 16
noise_magnitude = 0.01
clusters = {}
concepts = {}
[scenarios.scenes.pair]
who = [{reference = {by = "key", key = "self"}}, {reference = {by = "key", key = "intended-entity"}}]
where = {by = "setting", key = "workshop"}
custom = {project = "garden"}
[scenarios.scenes.other]
who = [{reference = {by = "key", key = "self"}}, {reference = {by = "key", key = "other"}}]
[scenarios.scenes.encounter]
who = [{reference = {by = "key", key = "self"}}, {reference = {by = "name", text = "Ada"}, gold_entity = "intended-entity"}]
where = {by = "setting", key = "workshop"}
[[scenarios.events]]
kind = "experience"
event_id = "early-meeting"
timestamp = "2024-01-01T09:00:00Z"
text = "We planned the garden."
scene = {kind = "named", name = "pair"}
speaker = "self"
[[scenarios.events]]
kind = "experience"
event_id = "last-meeting"
timestamp = "2024-01-03T09:00:00Z"
text = "I will bring the book."
scene = {kind = "named", name = "pair"}
[[scenarios.events]]
kind = "experience"
event_id = "distractor"
timestamp = "2024-01-04T09:00:00Z"
text = "Bert painted a fence."
scene = {kind = "named", name = "other"}
[[scenarios.events]]
kind = "derive"
event_id = "old-note"
timestamp = "2024-01-04T10:00:00Z"
memory = {subtype = "relationship_note", text = "We plan together.", experiences = ["early-meeting"], about = ["intended-entity"]}
[[scenarios.events]]
kind = "derive"
event_id = "promise"
timestamp = "2024-01-05T09:00:00Z"
expected_warning = "near_verbatim_restatement"
[scenarios.events.memory]
subtype = "commitment"
text = "I will bring the book."
experiences = ["early-meeting", "last-meeting"]
about = ["intended-entity"]
supersedes = ["old-note"]
actor = "self"
counterpart = "intended-entity"
due = "2024-01-10T09:00:00Z"
trigger = {kind = "participant", entity = "intended-entity"}
[[scenarios.events]]
kind = "probe"
event_id = "reunion"
query_id = "encounter-probe"
timestamp = "2024-01-08T09:00:00Z"
scene = {kind = "named", name = "encounter"}
partition = {by = "participants"}
[scenarios.events.assertions]
carried = [
  {memory = "last-meeting", reason = "pair", section = "episodes"},
  {memory = "promise", reason = "due", section = "commitments"},
]
in_order = [["last-meeting", "promise"]]
omitted = [{memory = "old-note", reason = "supersession"}]
not_cued = [{memory = "distractor", cue = "date"}]
references = [{participant = {by = "name", text = "Ada"}, resolution = {status = "resolved", entity = "intended-entity"}}]
scenes = [{memory = "last-meeting", scene = "pair"}]
elapsed_since_met = ["intended-entity"]
staleness = ["promise"]
[scenarios.events.measures]
bystanders = ["distractor"]
"#;

    fn situated_value() -> Value {
        toml::from_str(SITUATED_CASE).unwrap()
    }

    fn parse_as(value: &Value, extension: &str) -> Result<ContinuityFixtureSet, FixtureError> {
        let bytes = if extension == "toml" {
            toml::to_string(value).unwrap().into_bytes()
        } else {
            serde_json::to_vec(value).unwrap()
        };
        parse_fixture_source(
            std::path::Path::new(&format!("scenario.{extension}")),
            &bytes,
        )
    }

    #[test]
    fn situated_toml_and_json_share_admission_computed_gold_and_needed_features() {
        let file = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
        std::fs::write(file.path(), SITUATED_CASE).unwrap();
        let from_file = read_fixture(file.path()).unwrap();
        assert_eq!(from_file, parse_as(&situated_value(), "json").unwrap());
        let scenario = &from_file.scenarios[0];
        let gold = &scenario.requirements.probes["encounter-probe"];
        assert_eq!(
            gold.elapsed_since_met["intended-entity"],
            chrono::Duration::days(5)
        );
        // The memory was authored three days ago, but supported five days ago.
        assert_eq!(gold.staleness["promise"], chrono::Duration::days(5));
        assert!(gold.partition_applied);
        assert_eq!(
            scenario.requirements.features,
            BTreeSet::from([
                ScenarioFeature::WriteScene,
                ScenarioFeature::WriteSceneWhere,
                ScenarioFeature::WriteSceneCustom,
                ScenarioFeature::ProbeScene,
                ScenarioFeature::NoTopic,
                ScenarioFeature::ReferenceTime,
                ScenarioFeature::ParticipantName,
                ScenarioFeature::Partition,
                ScenarioFeature::Direction,
                ScenarioFeature::DueDate,
                ScenarioFeature::Trigger,
                ScenarioFeature::AuthoredDerivedMemory,
                ScenarioFeature::CueTrace,
                ScenarioFeature::ReferenceTrace,
                ScenarioFeature::PartitionTrace,
                ScenarioFeature::MemorySceneTrace,
                ScenarioFeature::ElapsedSinceMet,
                ScenarioFeature::Staleness,
                ScenarioFeature::OmissionReasons,
                ScenarioFeature::WriteWarnings,
                ScenarioFeature::PackSections,
                ScenarioFeature::PackOrder,
            ])
        );
        let provider = cmem_eval::ControllableSimilarityEmbeddingProvider::new(
            scenario
                .embedding
                .controllable_similarity()
                .unwrap()
                .clone(),
        )
        .unwrap();
        assert_ne!(
            provider.concept_for_text("Ada"),
            provider.concept_for_text("Bert")
        );
        assert!(provider.concept_for_text("Ada").is_some());
    }

    #[test]
    fn situated_activity_keys_resolve_prior_threads_and_open_loops_at_use() {
        for extension in ["json", "toml"] {
            for subtype in ["thread", "open_loop"] {
                for named in [true, false] {
                    for write in [true, false] {
                        let mut value = situated_value();
                        let scenario = &mut value["scenarios"][0];
                        scenario["events"][3]["memory"]["subtype"] = Value::from(subtype);
                        let mut scene = scenario["scenes"]["pair"].clone();
                        scene["what"] = serde_json::json!({"by": "key", "key": "old-note"});
                        let selection = if named {
                            scenario["scenes"]["activity"] = scene;
                            serde_json::json!({"kind": "named", "name": "activity"})
                        } else {
                            serde_json::json!({"kind": "inline", "scene": scene})
                        };
                        let event_id = if write {
                            "activity-experience"
                        } else {
                            "reunion"
                        };
                        if write {
                            scenario["events"].as_array_mut().unwrap().insert(5, serde_json::json!({
                                "kind": "experience", "event_id": event_id,
                                "timestamp": "2024-01-07T09:00:00Z", "text": "Working on the activity.",
                                "scene": selection
                            }));
                        } else {
                            scenario["events"][5]["scene"] = selection;
                            scenario["events"][5]["assertions"]["references"] =
                                serde_json::json!([]);
                        }
                        let loaded = parse_as(&value, extension).unwrap();
                        let scenario = &loaded.scenarios[0];
                        match scenario
                            .situated_input(&scenario.events[5])
                            .unwrap()
                            .unwrap()
                        {
                            SituatedInput::Experience { scene, .. }
                            | SituatedInput::Probe { scene, .. } => {
                                assert_eq!(
                                    scene.what,
                                    Some(PerceivedReference::Key {
                                        key: "old-note".into()
                                    })
                                );
                            }
                            _ => unreachable!(),
                        }

                        for id in [
                            "unknown",
                            "self",
                            "last-meeting",
                            "promise",
                            "future-activity",
                        ] {
                            let mut invalid = value.clone();
                            let scenario = &mut invalid["scenarios"][0];
                            if named {
                                scenario["scenes"]["activity"]["what"]["key"] = Value::from(id);
                            } else {
                                scenario["events"][5]["scene"]["scene"]["what"]["key"] =
                                    Value::from(id);
                            }
                            if id == "future-activity" {
                                let mut future = scenario["events"][3].clone();
                                future["event_id"] = Value::from(id);
                                future["timestamp"] = Value::from("2024-01-09T09:00:00Z");
                                scenario["events"].as_array_mut().unwrap().push(future);
                            }
                            assert_eq!(
                                admission_of(parse_as(&invalid, extension).unwrap_err()),
                                expected_admission(
                                    "encounter",
                                    Some(event_id),
                                    "scene.what.key",
                                    FixtureAdmissionKind::NotAdmittedActivity(id.into())
                                ),
                                "{extension}, {subtype}, named={named}, write={write}, key={id}"
                            );
                        }

                        for participant in [true, false] {
                            let mut invalid = value.clone();
                            let scenario = &mut invalid["scenarios"][0];
                            let scene = if named {
                                &mut scenario["scenes"]["activity"]
                            } else {
                                &mut scenario["events"][5]["scene"]["scene"]
                            };
                            let entity_reference = if participant {
                                &mut scene["who"][0]["reference"]
                            } else {
                                &mut scene["where"]
                            };
                            *entity_reference = serde_json::json!({"by": "key", "key": "old-note"});
                            assert_eq!(
                                admission_of(parse_as(&invalid, extension).unwrap_err()),
                                expected_admission(
                                    "encounter",
                                    if named { None } else { Some(event_id) },
                                    "scene.reference.key",
                                    FixtureAdmissionKind::Undeclared("old-note".into())
                                )
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn situated_inputs_cannot_carry_gold_resolutions_assertions_or_measures() {
        for by in ["name", "description"] {
            let mut value = situated_value();
            value["scenarios"][0]["scenes"]["encounter"]["who"][1]["reference"] =
                serde_json::json!({"by": by, "text": "person in a blue coat"});
            value["scenarios"][0]["events"][5]["assertions"]["references"][0]["participant"] =
                serde_json::json!({"by": by, "text": "person in a blue coat"});
            let fixtures = parse_as(&value, "json").unwrap();
            let scenario = &fixtures.scenarios[0];
            let input = scenario
                .situated_input(scenario.events.last().unwrap())
                .unwrap()
                .unwrap();
            let encoded = serde_json::to_value(&input).unwrap();
            assert_eq!(
                encoded["scene"]["who"][1],
                serde_json::json!({"by": by, "text": "person in a blue coat"})
            );
            let text = encoded.to_string();
            for gold in [
                "intended-entity",
                "assertions",
                "bystanders",
                "gold_entity",
                "promise",
                "staleness",
            ] {
                assert!(!text.contains(gold), "{gold} leaked: {text}");
            }
            let derive = scenario
                .situated_input(&scenario.events[4])
                .unwrap()
                .unwrap();
            assert!(
                !serde_json::to_string(&derive)
                    .unwrap()
                    .contains("expected_warning")
            );
            assert_eq!(
                scenario
                    .requirements
                    .features
                    .contains(&ScenarioFeature::ParticipantDescription),
                by == "description"
            );
        }
    }

    #[test]
    fn situated_rejections_are_typed_in_both_formats() {
        for extension in ["toml", "json"] {
            let mut value = situated_value();
            value["scenarios"][0]["events"][5]["assertions"]["carried"][0]["memory"] =
                Value::from("undeclared");
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("reunion"),
                    "probe.assertions.carried",
                    FixtureAdmissionKind::NotAdmitted("undeclared".into())
                )
            );

            let mut value = situated_value();
            value["scenarios"][0]["events"][0]["scene"] =
                serde_json::json!({"kind": "named", "name": "encounter"});
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("early-meeting"),
                    "scene.reference",
                    FixtureAdmissionKind::WriteSceneMustUseKeys
                )
            );

            for (field, change) in [
                ("reason", false),
                ("carrried", true),
                ("requirements", true),
            ] {
                let mut value = situated_value();
                if !change {
                    value["scenarios"][0]["events"][5]["assertions"]["omitted"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove(field);
                } else if field == "requirements" {
                    value["scenarios"][0][field] = serde_json::json!({});
                } else {
                    value["scenarios"][0]["events"][5]["assertions"][field] = serde_json::json!([]);
                }
                match parse_as(&value, extension).unwrap_err() {
                    FixtureError::Shape {
                        location,
                        field: actual,
                        ..
                    } => {
                        assert_eq!(location, FixtureLocation::scenario("encounter"));
                        assert_eq!(actual.as_deref(), Some(field));
                    }
                    other => panic!("expected typed shape error, got {other}"),
                }
            }
        }
    }

    #[test]
    fn situated_support_and_assertions_cannot_reference_future_or_contradictory_labels() {
        let cases = [
            (
                "future",
                "derive.experiences",
                FixtureAdmissionKind::NotAdmitted("promise".into()),
            ),
            (
                "bystander",
                "probe.measures.bystanders",
                FixtureAdmissionKind::Overlap("promise".into()),
            ),
            (
                "partition",
                "probe.assertions.omitted",
                FixtureAdmissionKind::RequiredWith("partition omission"),
            ),
        ];
        for (case, field, kind) in cases {
            let mut value = situated_value();
            let events = &mut value["scenarios"][0]["events"];
            match case {
                "future" => events[3]["memory"]["experiences"] = serde_json::json!(["promise"]),
                "bystander" => events[5]["measures"]["bystanders"] = serde_json::json!(["promise"]),
                "partition" => {
                    events[5].as_object_mut().unwrap().remove("partition");
                    events[5]["assertions"]["omitted"][0]["reason"] = Value::from("partition");
                }
                _ => unreachable!(),
            }
            let event = if case == "future" {
                "old-note"
            } else {
                "reunion"
            };
            assert_eq!(
                admission_of(parse_as(&value, "json").unwrap_err()),
                expected_admission("encounter", Some(event), field, kind)
            );
        }
    }

    #[test]
    fn situated_memory_labels_require_distinct_prior_authored_ids() {
        let labels = [
            (
                "assertions",
                "carried",
                "/0/memory",
                "probe.assertions.carried",
            ),
            (
                "assertions",
                "omitted",
                "/0/memory",
                "probe.assertions.omitted",
            ),
            (
                "assertions",
                "in_order",
                "/0/0",
                "probe.assertions.in_order",
            ),
            (
                "assertions",
                "not_cued",
                "/0/memory",
                "probe.assertions.not_cued",
            ),
            (
                "assertions",
                "scenes",
                "/0/memory",
                "probe.assertions.scenes",
            ),
            (
                "assertions",
                "staleness",
                "/0",
                "probe.assertions.staleness",
            ),
            ("measures", "bystanders", "/0", "probe.measures.bystanders"),
        ];
        for extension in ["json", "toml"] {
            for (group, list, pointer, field) in labels {
                for id in [
                    "unknown".to_string(),
                    "future".to_string(),
                    "legacy".to_string(),
                    observation_external_id("last-meeting"),
                ] {
                    let mut value = situated_value();
                    let events = value["scenarios"][0]["events"].as_array_mut().unwrap();
                    *events[5][group][list].pointer_mut(pointer).unwrap() = Value::from(id.clone());
                    let mut future = events[0].clone();
                    future["event_id"] = Value::from("future");
                    future["timestamp"] = Value::from("2024-01-09T09:00:00Z");
                    events.push(future);
                    events.insert(
                        5,
                        serde_json::json!({
                            "kind": "remember", "event_id": "legacy-event", "external_id": "legacy",
                        "timestamp": "2024-01-06T09:00:00Z", "text": "Legacy memory",
                            "entity_external_ids": [], "salience": 0.5
                        }),
                    );
                    assert_eq!(
                        admission_of(parse_as(&value, extension).unwrap_err()),
                        expected_admission(
                            "encounter",
                            Some("reunion"),
                            field,
                            FixtureAdmissionKind::NotAdmitted(id)
                        ),
                        "{extension}: {field}"
                    );
                }

                let mut value = situated_value();
                let values = &mut value["scenarios"][0]["events"][5][group][list];
                let duplicate = values
                    .pointer(pointer)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                let first = values[0].clone();
                values.as_array_mut().unwrap().push(first);
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        "encounter",
                        Some("reunion"),
                        field,
                        FixtureAdmissionKind::Duplicate(duplicate)
                    ),
                    "{extension}: {field}"
                );
            }
        }
    }

    #[test]
    fn situated_assertion_identity_uses_subjects_and_reference_values() {
        let mut value = situated_value();
        let before = parse_as(&value, "json").unwrap();
        let event = &before.scenarios[0].events[5];
        let identities = event.assertion_identities();
        assert_eq!(identities.len(), 10);
        assert!(identities.contains(&AssertionIdentity {
            event_id: "reunion".into(),
            assertion: AssertionSubject::References(PerceivedReference::Name {
                text: "Ada".into()
            }),
        }));
        value["scenarios"][0]["events"][5]["assertions"]["carried"]
            .as_array_mut()
            .unwrap()
            .reverse();
        value["scenarios"][0]["scenes"]["encounter"]["who"]
            .as_array_mut()
            .unwrap()
            .reverse();
        let after = parse_as(&value, "toml").unwrap();
        assert_eq!(
            identities,
            after.scenarios[0].events[5].assertion_identities()
        );
        let experience = before.scenarios[0]
            .situated_input(&before.scenarios[0].events[0])
            .unwrap()
            .unwrap();
        assert!(
            matches!(experience, SituatedInput::Experience { external_id, .. } if external_id == "early-meeting")
        );

        for resolution in [
            serde_json::json!({"status": "ambiguous", "candidates": ["intended-entity", "other"]}),
            serde_json::json!({"status": "unknown"}),
        ] {
            let mut value = situated_value();
            value["scenarios"][0]["events"][5]["assertions"]["references"][0]["resolution"] =
                resolution;
            let scene = value["scenarios"][0]["scenes"]["encounter"].clone();
            value["scenarios"][0]["events"][5]["scene"] =
                serde_json::json!({"kind": "inline", "scene": scene});
            parse_as(&value, "toml").unwrap();
        }
    }

    #[test]
    fn situated_other_references_and_declarations_are_admitted_by_kind() {
        let cases = [
            ("/character_entity", None, "character_entity", false),
            (
                "/scenes/pair/who/0/reference/key",
                None,
                "scene.reference.key",
                false,
            ),
            (
                "/scenes/encounter/who/1/gold_entity",
                None,
                "scene.who.gold_entity",
                false,
            ),
            (
                "/events/0/speaker",
                Some("early-meeting"),
                "experience.speaker",
                false,
            ),
            ("/events/0/scene/name", Some("early-meeting"), "scene", true),
            (
                "/events/4/memory/experiences/0",
                Some("promise"),
                "derive.experiences",
                true,
            ),
            (
                "/events/4/memory/about/0",
                Some("promise"),
                "derive.about",
                false,
            ),
            (
                "/events/4/memory/supersedes/0",
                Some("promise"),
                "derive.supersedes",
                true,
            ),
            (
                "/events/4/memory/actor",
                Some("promise"),
                "derive.actor",
                false,
            ),
            (
                "/events/4/memory/counterpart",
                Some("promise"),
                "derive.counterpart",
                false,
            ),
            (
                "/events/5/assertions/references/0/resolution/entity",
                Some("reunion"),
                "probe.assertions.references",
                false,
            ),
        ];
        for (pointer, event, field, memory) in cases {
            let mut value = situated_value();
            *value["scenarios"][0].pointer_mut(pointer).unwrap() = Value::from("unknown");
            assert_eq!(
                admission_of(parse_as(&value, "json").unwrap_err()),
                expected_admission(
                    "encounter",
                    event,
                    field,
                    if memory {
                        FixtureAdmissionKind::NotAdmitted("unknown".into())
                    } else {
                        FixtureAdmissionKind::Undeclared("unknown".into())
                    }
                )
            );
        }
        for (case, event, field, kind) in [
            (
                "duplicate_event",
                Some("early-meeting"),
                "event_id",
                FixtureAdmissionKind::Duplicate("early-meeting".into()),
            ),
            (
                "chronology",
                Some("last-meeting"),
                "timestamp",
                FixtureAdmissionKind::NotChronological,
            ),
            (
                "participant",
                None,
                "scene.who",
                FixtureAdmissionKind::Duplicate(r#"{"by":"key","key":"self"}"#.into()),
            ),
            (
                "reference",
                Some("reunion"),
                "probe.assertions.references",
                FixtureAdmissionKind::InvalidSceneReference,
            ),
            (
                "candidates",
                Some("reunion"),
                "probe.assertions.references",
                FixtureAdmissionKind::Undeclared("unknown".into()),
            ),
        ] {
            let mut value = situated_value();
            let scenario = &mut value["scenarios"][0];
            match case {
                "duplicate_event" => {
                    scenario["events"][1]["event_id"] = Value::from("early-meeting")
                }
                "chronology" => {
                    scenario["events"][1]["timestamp"] = Value::from("2023-12-01T00:00:00Z")
                }
                "participant" => {
                    let duplicate = scenario["scenes"]["pair"]["who"][0].clone();
                    scenario["scenes"]["pair"]["who"]
                        .as_array_mut()
                        .unwrap()
                        .push(duplicate);
                }
                "reference" => {
                    scenario["events"][5]["assertions"]["references"][0]["participant"]["text"] =
                        Value::from("Nobody")
                }
                "candidates" => {
                    scenario["events"][5]["assertions"]["references"][0]["resolution"] = serde_json::json!({"status": "ambiguous", "candidates": ["intended-entity", "unknown"]})
                }
                _ => unreachable!(),
            }
            assert_eq!(
                admission_of(parse_as(&value, "toml").unwrap_err()),
                expected_admission("encounter", event, field, kind)
            );
        }
    }

    #[test]
    fn own_concept_is_opt_in_and_schema_version_is_still_exact() {
        let mut value = serde_json::to_value(admission_fixture()).unwrap();
        let event = value["scenarios"][0]["events"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap();
        event["text"] = Value::from("new topic");
        assert_eq!(
            admission_of(parse_as(&value, "json").unwrap_err()),
            expected_admission(
                "admission-test",
                Some("event-query"),
                "query.text",
                FixtureAdmissionKind::UnassignedEmbeddingInput("new topic".into())
            )
        );
        value["scenarios"][0]["embedding"]["own_concept"] = Value::Bool(true);
        let loaded = parse_as(&value, "json").unwrap();
        let provider = cmem_eval::ControllableSimilarityEmbeddingProvider::new(
            loaded.scenarios[0]
                .embedding
                .controllable_similarity()
                .unwrap()
                .clone(),
        )
        .unwrap();
        assert!(provider.vector_for_text("new topic").is_ok());
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["schema_version"] = Value::from(2);
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                (
                    FixtureLocation::Root,
                    "schema_version",
                    FixtureAdmissionKind::UnsupportedSchemaVersion { found: 2 }
                )
            );
        }
    }

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
                participant_entity_external_ids: Vec::new(),
                speaker_entity_external_id: None,
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

    // One hand-authored scenario covers every event shape used by admission tests.
    // Keep its inputs explicit so the generator cannot repair a malformed test.
    fn admission_fixture() -> ContinuityFixtureSet {
        let fixtures: ContinuityFixtureSet = serde_json::from_value(serde_json::json!({
            "schema_version": CONTINUITY_FIXTURE_SCHEMA_VERSION,
            "seed": 1,
            "scenarios": [{
                "fixture_id": "admission-test",
                "namespace": "admission-test",
                "pattern": "long_gap_recall",
                "entities": [{
                    "external_id": "entity-person", "entity_type": "person",
                    "label": "Person", "is_hub": false
                }],
                "embedding": {
                    "provider": "controllable_similarity", "seed": 1,
                    "vector_size": 1, "noise_magnitude": 0.0,
                    "clusters": {"test": [1.0]},
                    "concepts": {"test": {"cluster": "test", "inputs": [
                        "Person", "Episode", "Observation", "Derived",
                        "Contrast", "Correction", "Question"
                    ]}}
                },
                "events": [
                    {
                        "kind": "remember", "event_id": "event-remember",
                        "external_id": "memory-one", "timestamp": "2024-01-01T00:00:00Z",
                        "text": "Episode", "surface_texts": {
                            "episode": "Episode", "observation": "Observation", "derived": "Derived"
                        },
                        "entity_external_ids": ["entity-person"], "salience": 0.5,
                        "thread": {"thread_external_id": "thread-one", "confidence": 0.5}
                    },
                    {
                        "kind": "remember", "event_id": "event-contrast",
                        "external_id": "memory-two", "timestamp": "2024-01-01T00:01:00Z",
                        "text": "Contrast", "entity_external_ids": [], "thread": null,
                        "salience": 0.5
                    },
                    {
                        "kind": "link", "event_id": "event-link", "external_id": "memory-link",
                        "timestamp": "2024-01-01T00:02:00Z", "from_external_id": "memory-one",
                        "relation": "mentions", "to_external_id": "entity-person"
                    },
                    {
                        "kind": "correct", "event_id": "event-correct",
                        "target_external_id": "memory-one", "replacement_external_id": "memory-corrected",
                        "timestamp": "2024-01-01T00:03:00Z", "replacement_text": "Correction"
                    },
                    {
                        "kind": "forget", "event_id": "event-forget",
                        "target_external_ids": ["memory-two", "memory-corrected"],
                        "timestamp": "2024-01-01T00:04:00Z",
                        "suppress_derived_from_target": true, "apply_to_derived_from_target": true
                    },
                    {
                        "kind": "restart", "event_id": "event-restart",
                        "timestamp": "2024-01-01T00:05:00Z", "reopen_graph": true, "reopen_stats": true
                    },
                    {
                        "kind": "query", "event_id": "event-query", "query_id": "query-test",
                        "timestamp": "2024-01-01T00:06:00Z", "text": "Question",
                        "expected": {"relevant_external_ids": ["memory-one"], "irrelevant_external_ids": ["memory-two"]}
                    }
                ]
            }]
        })).unwrap();
        fixtures.validate().unwrap();
        fixtures
    }

    fn event_mut<'a>(
        scenario: &'a mut ContinuityScenario,
        event_id: &str,
    ) -> &'a mut InteractionEvent {
        scenario
            .events
            .iter_mut()
            .find(|event| event.event_id() == event_id)
            .unwrap()
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
        let canonical = serde_json::to_string(&admission_fixture()).unwrap();
        let repeated = canonical.replacen('{', "{\"seed\":123,", 1);
        assert_eq!(
            parse_fixture_bytes(repeated.as_bytes()).unwrap(),
            parse_fixture_bytes(canonical.as_bytes()).unwrap()
        );
    }

    #[test]
    fn public_parser_attributes_root_shape_errors_before_scenario_errors() {
        let mut value: Value = serde_json::to_value(admission_fixture()).unwrap();
        value["scenarios"][0]["entities"][0]["entity_type"] = Value::from("unknown-kind");
        assert_eq!(
            shape(&value),
            (FixtureLocation::scenario("admission-test"), None)
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
        let mut value: Value = serde_json::to_value(admission_fixture()).unwrap();
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
        let mut value: Value = serde_json::to_value(admission_fixture()).unwrap();
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
        let mut value: Value = serde_json::to_value(admission_fixture()).unwrap();
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
        let fixtures = admission_fixture();
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

        for (event_kind, field) in [
            ("remember", "memory_id"),
            ("correct", "replacement_memory_id"),
            ("link", "memory_id"),
        ] {
            let mut value = serde_json::to_value(&fixtures).unwrap();
            let scenario = &mut value["scenarios"][0];
            let fixture_id = scenario["fixture_id"].as_str().unwrap().to_string();
            let event = scenario["events"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|event| event["kind"] == event_kind)
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
        let fixtures = admission_fixture();
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
        let fixtures = admission_fixture();
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
        expected_mut(&mut fixtures.scenarios[0])
            .irrelevant_external_ids
            .clear();
        fixtures.validate().unwrap();
    }

    #[test]
    fn public_parser_rejects_duplicate_or_overlapping_relevance_labels() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let query = query_event_id(scenario);
        let expected = expected_mut(scenario);
        let relevant = expected.relevant_external_ids[0].clone();
        let irrelevant = expected.irrelevant_external_ids[0].clone();
        expected.relevant_external_ids.push(relevant.clone());
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&query),
                "query.expected.relevant_external_ids",
                FixtureAdmissionKind::Duplicate(relevant.clone())
            )
        );

        let mut fixtures = admission_fixture();
        expected_mut(&mut fixtures.scenarios[0])
            .irrelevant_external_ids
            .push(irrelevant.clone());
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&query),
                "query.expected.irrelevant_external_ids",
                FixtureAdmissionKind::Duplicate(irrelevant)
            )
        );

        let mut fixtures = admission_fixture();
        expected_mut(&mut fixtures.scenarios[0])
            .irrelevant_external_ids
            .push(relevant.clone());
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&query),
                "query.expected.irrelevant_external_ids",
                FixtureAdmissionKind::Overlap(relevant)
            )
        );
    }

    #[test]
    fn public_parser_rejects_relevance_labels_before_external_id_admission() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let query = query_event_id(scenario);
        let query_index = scenario
            .events
            .iter()
            .position(|event| event.event_id() == "event-query")
            .unwrap();
        let contrast_index = scenario
            .events
            .iter()
            .position(|event| event.event_id() == "event-contrast")
            .unwrap();
        scenario.events.swap(contrast_index, query_index);
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&query),
                "query.expected.irrelevant_external_ids",
                FixtureAdmissionKind::NotAdmitted("memory-two".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_dangling_correction_and_forget_targets() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let correct = event_mut(scenario, "event-correct").event_id().to_string();
        let InteractionEvent::Correct {
            target_external_id, ..
        } = event_mut(scenario, "event-correct")
        else {
            panic!("expected correction event");
        };
        *target_external_id = "missing-correction-target".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&correct),
                "correct.target_external_id",
                FixtureAdmissionKind::NotAdmitted("missing-correction-target".to_string())
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let forget = event_mut(scenario, "event-forget").event_id().to_string();
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = event_mut(scenario, "event-forget")
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "missing-forget-target".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::NotAdmitted("missing-forget-target".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_correction_targets_the_driver_cannot_correct() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let correct = event_mut(scenario, "event-correct").event_id().to_string();
        let InteractionEvent::Correct {
            target_external_id, ..
        } = event_mut(scenario, "event-correct")
        else {
            panic!("expected correction event");
        };
        *target_external_id = "entity-person".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&correct),
                "correct.target_external_id",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "entity-person".to_string(),
                    found: ContinuityObjectKind::Entity,
                    allowed: CORRECTION_TARGET_KINDS,
                }
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let InteractionEvent::Correct {
            target_external_id, ..
        } = event_mut(scenario, "event-correct")
        else {
            panic!("expected correction event");
        };
        *target_external_id = "memory-link".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&correct),
                "correct.target_external_id",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "memory-link".to_string(),
                    found: ContinuityObjectKind::MemoryLink,
                    allowed: CORRECTION_TARGET_KINDS,
                }
            )
        );
    }

    #[test]
    fn public_parser_rejects_forget_targets_the_driver_cannot_forget() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let forget = event_mut(scenario, "event-forget").event_id().to_string();
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = event_mut(scenario, "event-forget")
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "entity-person".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "entity-person".to_string(),
                    found: ContinuityObjectKind::Entity,
                    allowed: FORGET_TARGET_KINDS,
                }
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = event_mut(scenario, "event-forget")
        else {
            panic!("expected forget event");
        };
        target_external_ids[0] = "memory-link".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "memory-link".to_string(),
                    found: ContinuityObjectKind::MemoryLink,
                    allowed: FORGET_TARGET_KINDS,
                }
            )
        );
    }

    #[test]
    fn public_parser_rejects_empty_or_duplicate_forget_targets() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let forget = event_mut(scenario, "event-forget").event_id().to_string();
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = event_mut(scenario, "event-forget")
        else {
            panic!("expected forget event");
        };
        target_external_ids.clear();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::Empty
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let InteractionEvent::Forget {
            target_external_ids,
            ..
        } = event_mut(scenario, "event-forget")
        else {
            panic!("expected forget event");
        };
        target_external_ids[1] = target_external_ids[0].clone();
        let duplicate = target_external_ids[0].clone();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&forget),
                "forget.target_external_ids",
                FixtureAdmissionKind::Duplicate(duplicate)
            )
        );
    }

    #[test]
    fn public_parser_rejects_conflicting_default_and_episode_surface_text() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let remember = event_mut(scenario, "event-remember").event_id().to_string();
        let InteractionEvent::Remember { text, .. } = event_mut(scenario, "event-remember") else {
            panic!("expected remember event");
        };
        *text = "Conflicting Episode text".to_string();

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&remember),
                "remember.text",
                FixtureAdmissionKind::DiffersFrom("remember.surface_texts.episode")
            )
        );
    }

    #[test]
    fn public_parser_rejects_dangling_link_endpoints() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let link = event_mut(scenario, "event-link").event_id().to_string();
        let InteractionEvent::Link {
            from_external_id, ..
        } = event_mut(scenario, "event-link")
        else {
            panic!("expected link event");
        };
        *from_external_id = "missing-link-source".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&link),
                "link.from_external_id",
                FixtureAdmissionKind::NotAdmitted("missing-link-source".to_string())
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let InteractionEvent::Link { to_external_id, .. } = event_mut(scenario, "event-link")
        else {
            panic!("expected link event");
        };
        *to_external_id = "missing-link-target".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&link),
                "link.to_external_id",
                FixtureAdmissionKind::NotAdmitted("missing-link-target".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_memory_links_as_link_endpoints() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let query_index = scenario
            .events
            .iter()
            .position(|event| event.event_id() == "event-query")
            .unwrap();
        insert_link_before(scenario, query_index, "nested-link", "memory-link");

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some("event-test-link-nested-link"),
                "link.from_external_id",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "memory-link".to_string(),
                    found: ContinuityObjectKind::MemoryLink,
                    allowed: LINK_ENDPOINT_KINDS,
                }
            )
        );
    }

    #[test]
    fn public_parser_admits_persisted_observation_derived_and_thread_ids() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
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
            "memory-one:observation",
        );
        insert_link_before(
            scenario,
            query_index + 1,
            "derived-link",
            "memory-one:derived",
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
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let remember = event_mut(scenario, "event-contrast").event_id().to_string();
        let InteractionEvent::Remember { external_id, .. } = event_mut(scenario, "event-contrast")
        else {
            panic!("expected remember event");
        };
        *external_id = "memory-one:observation".to_string();

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&remember),
                "remember.external_id",
                FixtureAdmissionKind::Duplicate("memory-one:observation".to_string())
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let remember = event_mut(scenario, "event-remember").event_id().to_string();
        let InteractionEvent::Remember {
            thread: Some(thread),
            ..
        } = event_mut(scenario, "event-remember")
        else {
            panic!("expected threaded remember event");
        };
        thread.thread_external_id = "entity-person".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
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
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        scenario
            .events
            // Remove restart too: its missing-follow-up error has its own case below.
            .retain(|event| {
                !matches!(
                    event,
                    InteractionEvent::Query { .. } | InteractionEvent::Restart { .. }
                )
            });

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                None,
                "events",
                FixtureAdmissionKind::MissingQuery
            )
        );
    }

    #[test]
    fn public_parser_rejects_operations_after_the_final_query() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let terminal_index = scenario.events.len();
        insert_link_before(scenario, terminal_index, "post-query-link", "memory-one");

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                None,
                "events",
                FixtureAdmissionKind::MustEndWithQuery
            )
        );
    }

    #[test]
    fn public_parser_rejects_relations_outside_the_facade_vocabulary() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let link = event_mut(scenario, "event-link").event_id().to_string();
        let InteractionEvent::Link { relation, .. } = event_mut(scenario, "event-link") else {
            panic!("expected link event");
        };
        *relation = "invented_relation".to_string();

        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&link),
                "link.relation",
                FixtureAdmissionKind::UnsupportedRelation("invented_relation".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_duplicate_created_external_ids() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let remember = event_mut(scenario, "event-contrast").event_id().to_string();
        let InteractionEvent::Remember { external_id, .. } = event_mut(scenario, "event-contrast")
        else {
            panic!("expected remember event");
        };
        *external_id = "memory-one".to_string();
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&remember),
                "remember.external_id",
                FixtureAdmissionKind::Duplicate("memory-one".to_string())
            )
        );
    }

    #[test]
    fn public_parser_rejects_duplicate_query_ids() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
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
                "admission-test",
                Some(&event_id),
                "event_id",
                FixtureAdmissionKind::Duplicate(event_id.clone())
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let mut duplicate = duplicate;
        let InteractionEvent::Query { event_id, .. } = &mut duplicate else {
            unreachable!()
        };
        *event_id = "event-duplicate-query-id".to_string();
        scenario.events.push(duplicate);
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some("event-duplicate-query-id"),
                "query.query_id",
                FixtureAdmissionKind::Duplicate(query_id)
            )
        );
    }

    #[test]
    fn public_parser_rejects_invalid_salience_and_thread_confidence() {
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let remember = event_mut(scenario, "event-remember").event_id().to_string();
        let InteractionEvent::Remember { salience, .. } = event_mut(scenario, "event-remember")
        else {
            panic!("expected remember event");
        };
        *salience = 1.1;
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&remember),
                "remember.salience",
                FixtureAdmissionKind::OutOfUnitInterval(1.1)
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
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
            .expect("admission fixture has thread membership");
        *confidence = -0.1;
        assert_eq!(
            admission(&fixtures),
            expected_admission(
                "admission-test",
                Some(&threaded),
                "remember.thread.confidence",
                FixtureAdmissionKind::OutOfUnitInterval(-0.1)
            )
        );

        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
        let InteractionEvent::Remember { salience, .. } = event_mut(scenario, "event-remember")
        else {
            panic!("expected remember event");
        };
        *salience = f32::NAN;
        let (location, field, kind) = admission_of(scenario.validate().unwrap_err());
        assert_eq!(
            (location, field),
            (
                FixtureLocation::Scenario {
                    fixture_id: "admission-test".to_string(),
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
        let mut fixtures = admission_fixture();
        let scenario = &mut fixtures.scenarios[0];
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
                "admission-test",
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
            let mut fixtures = admission_fixture();
            let scenario = &mut fixtures.scenarios[0];
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
                    "admission-test",
                    Some(&restart_event_id),
                    field,
                    FixtureAdmissionKind::RestartMustReopen
                )
            );
        }
    }

    #[test]
    fn fixture_reader_rejects_an_incompatible_schema_version() {
        let mut fixtures = admission_fixture();
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
        let mut fixtures = admission_fixture();
        let bytes = canonical_fixture_bytes(&fixtures).unwrap();
        let parsed = parse_fixture_bytes(&bytes).unwrap();
        assert_eq!(parsed.schema_version, CONTINUITY_FIXTURE_SCHEMA_VERSION);
        assert!(
            parsed.scenarios.iter().any(|scenario| {
                scenario.embedding.provider_name() == "controllable_similarity"
            })
        );
        fixtures.scenarios[0].embedding = ContinuityScenarioEmbedding::Frozen;
        let frozen = parse_fixture_bytes(&canonical_fixture_bytes(&fixtures).unwrap()).unwrap();
        assert_eq!(frozen.scenarios[0].embedding.provider_name(), "frozen");

        let mut v2 = serde_json::to_value(admission_fixture()).unwrap();
        v2["schema_version"] = Value::from(2);
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
        let fixtures = admission_fixture();
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
        let mut fixtures = admission_fixture();
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
