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
    fn same_context(&self, other: &Self) -> bool {
        self.place == other.place
            && self.what == other.what
            && self.custom == other.custom
            && self
                .who
                .iter()
                .map(|p| &p.reference)
                .collect::<BTreeSet<_>>()
                == other
                    .who
                    .iter()
                    .map(|p| &p.reference)
                    .collect::<BTreeSet<_>>()
    }

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedWriteWarning {
    NearVerbatimRestatement,
    ChurningChain,
}

pub type RecallReason = CueKind;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Pair,
    Place,
    Due,
    Date,
    Trigger,
    Activity,
    OwnDay,
    RecentAndSalient,
    Topic,
}

impl CueKind {
    pub const ALL: [Self; 9] = [
        Self::Pair,
        Self::Place,
        Self::Due,
        Self::Date,
        Self::Trigger,
        Self::Activity,
        Self::OwnDay,
        Self::RecentAndSalient,
        Self::Topic,
    ];

    pub(crate) fn native(self) -> Option<cmem_eval::character_memory::CueKind> {
        use cmem_eval::character_memory::CueKind as Native;
        match self {
            Self::Topic => Some(Native::Topic),
            Self::Pair => Some(Native::Participant),
            Self::Place => Some(Native::Place),
            Self::Activity => Some(Native::Activity),
            Self::Due | Self::Date | Self::Trigger | Self::OwnDay | Self::RecentAndSalient => None,
        }
    }
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
pub struct CueAssertion {
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
    pub cued: Vec<CueAssertion>,
    pub not_cued: Vec<CueAssertion>,
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
    Cued { memory: String, cue: CueKind },
    NotCued { memory: String, cue: CueKind },
    References(PerceivedReference),
    Scene { memory: String, scene: String },
    ElapsedSinceMet(String),
    Staleness(String),
    WriteWarning(ExpectedWriteWarning),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioFeature {
    WriteScene,
    WriteSceneWhere,
    WriteSceneWhat,
    WriteSceneCustom,
    ProbeScene,
    ProbeActivity,
    NoTopic,
    ReferenceTime,
    ParticipantName,
    ParticipantDescription,
    PlaceName,
    PlaceDescription,
    ActivityName,
    ActivityDescription,
    Direction,
    DueDate,
    Trigger,
    AuthoredDerivedMemory,
    IntentionMemory,
    PreferenceMemory,
    ThreadProvenance,
    CueTrace,
    PairCounterpartCue,
    DueCue,
    DateCue,
    TriggerCue,
    OwnDayCue,
    RecentAndSalientCue,
    ReferenceTrace,
    DescriptionReferenceResolution,
    MemorySceneTrace,
    ElapsedSinceMet,
    Staleness,
    OmissionReasons,
    ResolutionOmission,
    WriteWarnings,
    PackSections,
    PackOrder,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComputedProbeGold {
    pub elapsed_since_met: BTreeMap<String, chrono::Duration>,
    pub staleness: BTreeMap<String, chrono::Duration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScenarioRequirements {
    pub features: BTreeSet<ScenarioFeature>,
    pub probes: BTreeMap<String, ComputedProbeGold>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SituatedInput {
    Experience {
        external_id: String,
        text: String,
        scene: SceneInput,
        speaker: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        salience: Option<f32>,
    },
    Derive {
        external_id: String,
        memory: AuthoredMemory,
    },
    Probe {
        query_id: String,
        scene: SceneInput,
        topic: Option<String>,
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
    /// Embedding input as well as display text. Scenario authors must assign it
    /// exactly once in `embedding.concepts`; the generator uses the concept of
    /// the first `Remember` that explicitly references this entity, or the
    /// deterministic `entity_background` concept when no event references it.
    pub label: String,
    pub is_hub: bool,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        salience: Option<f32>,
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
    MissingSceneCharacter,
    NotAdmittedActivity(String),
    RequiredWith(&'static str),
    InvalidAssertion(&'static str),
}

#[derive(Debug)]
pub enum FixtureError {
    Io(std::io::Error),
    UnsupportedFormat(String),
    Toml(toml::de::Error),
    NonFiniteTomlFloat {
        path: String,
    },
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
            Self::MissingSceneCharacter => {
                write!(
                    f,
                    "scene must include the scenario character as a participant"
                )
            }
            Self::NotAdmittedActivity(id) => write!(
                f,
                "activity key {id:?} must reference an earlier authored thread or open loop"
            ),
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
            Self::NonFiniteTomlFloat { path } => {
                write!(f, "continuity fixture TOML {path}: float must be finite")
            }
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
            Self::UnsupportedFormat(_) | Self::NonFiniteTomlFloat { .. } => None,
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
                salience,
                ..
            } => Some(SituatedInput::Experience {
                external_id: external_id.clone(),
                text: text.clone(),
                scene: self.scene(scene, &location)?.input(),
                speaker: speaker.clone(),
                salience: *salience,
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
                ..
            } => Some(SituatedInput::Probe {
                query_id: query_id.clone(),
                scene: self.scene(scene, &location)?.input(),
                topic: topic.clone(),
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

    /// Embedding lookup inventory: write surfaces collapse whitespace, while
    /// queries trim only outer whitespace, preserving internal whitespace.
    /// Unsupported scene/trigger fields retain authored text until their native
    /// consumers define a lookup contract.
    pub fn runtime_embedding_inputs(&self) -> BTreeSet<String> {
        let mut inputs = self
            .entities
            .iter()
            .map(|entity| runtime_memory_embedding_text(&entity.label))
            .collect::<BTreeSet<_>>();
        for event in &self.events {
            match event {
                InteractionEvent::Experience { text, scene, .. } => {
                    inputs.insert(runtime_memory_embedding_text(text));
                    let scene = match scene {
                        SceneSelection::Named { name } => self.scenes.get(name),
                        SceneSelection::Inline { scene } => Some(scene),
                    };
                    if let Some(scene) = scene {
                        inputs.insert(runtime_episode_embedding_text(text, scene));
                    }
                }
                InteractionEvent::Derive { memory, .. } => {
                    inputs.insert(runtime_memory_embedding_text(&memory.text));
                    if let Some(IntentionTrigger::Topic { text }) = &memory.trigger {
                        inputs.insert(text.clone());
                    }
                }
                InteractionEvent::Probe { topic, .. } => {
                    if let Some(topic) = topic {
                        inputs.insert(topic.trim().to_string());
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
                    inputs.insert(text.trim().to_string());
                }
                InteractionEvent::Forget { .. }
                | InteractionEvent::Link { .. }
                | InteractionEvent::Restart { .. } => {}
            }
        }
        inputs.extend(
            self.scene_texts()
                .into_iter()
                .map(|text| text.trim().to_string()),
        );
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
        let has_situated_events = self.events.iter().any(|event| {
            matches!(
                event,
                InteractionEvent::Experience { .. }
                    | InteractionEvent::Derive { .. }
                    | InteractionEvent::Probe { .. }
            )
        });
        if (self.pattern == ScenarioPattern::Situated) != has_situated_events {
            return Err(
                scenario.error("pattern", FixtureAdmissionKind::DiffersFrom("event family"))
            );
        }
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
                let label = runtime_memory_embedding_text(&entity.label);
                let found = embedding
                    .concepts
                    .values()
                    .flat_map(|concept| &concept.inputs)
                    .filter(|input| *input == &label)
                    .count();
                if found != 1 {
                    return Err(scenario.error(
                        "entity.label",
                        FixtureAdmissionKind::ConceptAssignments { label, found },
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
            validate_scene(
                scene,
                &scenario,
                &declared_entities,
                None,
                self.character_entity.as_deref(),
            )?;
        }
        let mut requirements = ScenarioRequirements::default();
        let mut support_times = BTreeMap::new();
        let mut authored_memories = BTreeMap::new();
        let mut memory_scenes: BTreeMap<&str, &Scene> = BTreeMap::new();
        let mut activities = BTreeSet::new();
        let mut last_met = BTreeMap::new();
        let mut query_ids = BTreeSet::new();
        let mut admitted_external_ids = self
            .entities
            .iter()
            .map(|entity| (entity.external_id.clone(), ContinuityObjectKind::Entity))
            .collect::<BTreeMap<_, _>>();
        for entity in &self.entities {
            admit_external_id(
                &scenario,
                "entity.naming_belief_external_id",
                &naming_belief_external_id(&entity.external_id),
                ContinuityObjectKind::DerivedMemory,
                &mut admitted_external_ids,
            )?;
        }
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
                    salience,
                    ..
                } => {
                    self.require_situated_header(&location)?;
                    if let Some(salience) = salience {
                        require_unit_interval(&location, "experience.salience", *salience)?;
                    }
                    let scene = self.scene(scene, &location)?;
                    validate_scene(
                        scene,
                        &location,
                        &declared_entities,
                        Some(&activities),
                        self.character_entity.as_deref(),
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
                        &runtime_memory_embedding_text(text),
                    )?;
                    require_embedding_input(
                        &location,
                        "experience.scene",
                        assigned_inputs.as_ref(),
                        &runtime_episode_embedding_text(text, scene),
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
                    memory_scenes.insert(external_id, scene);
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
                        &runtime_memory_embedding_text(&memory.text),
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
                    let source_scene = memory_scenes[memory.experiences[0].as_str()];
                    if memory
                        .experiences
                        .iter()
                        .all(|id| source_scene.same_context(memory_scenes[id.as_str()]))
                    {
                        memory_scenes.insert(external_id, source_scene);
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
                            &[ContinuityObjectKind::DerivedMemory],
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
                                require_non_empty(&location, "derive.trigger", text)?;
                                require_embedding_input(
                                    &location,
                                    "derive.trigger",
                                    assigned_inputs.as_ref(),
                                    text,
                                )?;
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
                        &location,
                        &declared_entities,
                        Some(&activities),
                        self.character_entity.as_deref(),
                    )?;
                    for (reference, name_feature, description_feature) in scene
                        .who
                        .iter()
                        .map(|person| {
                            (
                                &person.reference,
                                ScenarioFeature::ParticipantName,
                                ScenarioFeature::ParticipantDescription,
                            )
                        })
                        .chain(scene.place.iter().map(|reference| {
                            (
                                reference,
                                ScenarioFeature::PlaceName,
                                ScenarioFeature::PlaceDescription,
                            )
                        }))
                        .chain(scene.what.iter().map(|reference| {
                            (
                                reference,
                                ScenarioFeature::ActivityName,
                                ScenarioFeature::ActivityDescription,
                            )
                        }))
                    {
                        let (text, feature) = match reference {
                            PerceivedReference::Name { text } => (text, name_feature),
                            PerceivedReference::Description { text } => (text, description_feature),
                            _ => continue,
                        };
                        require_embedding_input(
                            &location,
                            "scene.reference.text",
                            assigned_inputs.as_ref(),
                            text.trim(),
                        )?;
                        requirements.features.insert(feature);
                    }
                    if matches!(scene.what, Some(PerceivedReference::Key { .. })) {
                        requirements.features.insert(ScenarioFeature::ProbeActivity);
                    }
                    if let Some(topic) = topic {
                        require_non_empty(&location, "probe.topic", topic)?;
                        require_embedding_input(
                            &location,
                            "probe.topic",
                            assigned_inputs.as_ref(),
                            topic.trim(),
                        )?;
                    } else {
                        requirements.features.insert(ScenarioFeature::NoTopic);
                    }
                    let gold = ProbeAdmission {
                        location: &location,
                        character: self.character_entity.as_deref(),
                        entities: &declared_entities,
                        memories: &authored_memories,
                        support_times: &support_times,
                        last_met: &last_met,
                        scenes: &self.scenes,
                        memory_scenes: &memory_scenes,
                        activities: &activities,
                        timestamp,
                    }
                    .validate(
                        assertions,
                        measures,
                        scene,
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
                                &runtime_memory_embedding_text(surface_text),
                            )?;
                        }
                    }
                    require_embedding_input(
                        &location,
                        "remember.text",
                        assigned_inputs.as_ref(),
                        &runtime_memory_embedding_text(text),
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
                        &runtime_memory_embedding_text(replacement_text),
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
                        text.trim(),
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

// Follows CharacterMemory src/policy/embedding_surface.rs; the strict provider
// lookup fails if the native embedding input diverges from this inventory.
fn runtime_episode_embedding_text(text: &str, scene: &Scene) -> String {
    let mut text = runtime_memory_embedding_text(text);
    let words = |reference: &PerceivedReference| match reference {
        PerceivedReference::Name { text } | PerceivedReference::Description { text } => {
            Some(runtime_memory_embedding_text(text))
        }
        _ => None,
    };
    let setting = scene
        .place
        .as_ref()
        .and_then(words)
        .map(|words| ("Setting", words));
    let participants = scene
        .who
        .iter()
        .filter_map(|person| words(&person.reference))
        .map(|words| ("With", words));
    for (label, words) in setting.into_iter().chain(participants) {
        if !words.is_empty() {
            text.push_str(&format!("\n{label}: {words}"));
        }
    }
    text
}

impl InteractionEvent {
    /// Identities use the authored subject, so list and participant order are immaterial.
    pub fn assertion_identities(&self) -> BTreeSet<AssertionIdentity> {
        let mut subjects = Vec::new();
        match self {
            Self::Probe { assertions, .. } => {
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
                subjects.extend(assertions.cued.iter().map(|a| AssertionSubject::Cued {
                    memory: a.memory.clone(),
                    cue: a.cue,
                }));
                subjects.extend(
                    assertions
                        .not_cued
                        .iter()
                        .map(|a| AssertionSubject::NotCued {
                            memory: a.memory.clone(),
                            cue: a.cue,
                        }),
                );
                subjects.extend(
                    assertions
                        .references
                        .iter()
                        .map(|a| AssertionSubject::References(a.participant.clone())),
                );
                subjects.extend(assertions.scenes.iter().map(|a| AssertionSubject::Scene {
                    memory: a.memory.clone(),
                    scene: a.scene.clone(),
                }));
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
            }
            Self::Derive {
                expected_warning: Some(warning),
                ..
            } => {
                subjects.push(AssertionSubject::WriteWarning(*warning));
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

pub(crate) fn naming_belief_external_id(entity_external_id: &str) -> String {
    format!("continuity:entity-name:{entity_external_id}")
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
            require_finite_toml(&value, "$".into())?;
            let bytes = serde_json::to_vec(&value).map_err(FixtureError::Json)?;
            parse_fixture_bytes(&bytes)
        }
        extension => Err(FixtureError::UnsupportedFormat(
            extension.unwrap_or_default().to_string(),
        )),
    }
}

fn require_finite_toml(value: &toml::Value, path: String) -> Result<(), FixtureError> {
    match value {
        toml::Value::Float(number) if !number.is_finite() => {
            return Err(FixtureError::NonFiniteTomlFloat { path });
        }
        toml::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                require_finite_toml(value, format!("{path}[{index}]"))?;
            }
        }
        toml::Value::Table(values) => {
            for (key, value) in values {
                require_finite_toml(value, format!("{path}[{}]", serde_json::json!(key)))?;
            }
        }
        _ => {}
    }
    Ok(())
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
    location: &FixtureLocation,
    entities: &BTreeSet<&str>,
    activities: Option<&BTreeSet<&str>>,
    character: Option<&str>,
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
            (PerceivedReference::Setting { .. }, true) => {
                return Err(
                    location.error("scene.what", FixtureAdmissionKind::InvalidSceneReference)
                );
            }
            (PerceivedReference::Setting { key }, false) => {
                require_non_empty(location, "scene.reference.key", key)?
            }
            (PerceivedReference::Name { text } | PerceivedReference::Description { text }, _) => {
                require_non_empty(location, "scene.reference.text", text)?;
            }
        }
    }
    for (key, value) in &scene.custom {
        require_non_empty(location, "scene.custom.key", key)?;
        require_non_empty(location, "scene.custom.value", value)?;
    }
    if !character.is_some_and(|character| {
        scene.who.iter().any(|person| {
            person.gold_entity.as_deref() == Some(character)
                || matches!(&person.reference, PerceivedReference::Key { key } if key == character)
        })
    }) {
        return Err(location.error("scene.who", FixtureAdmissionKind::MissingSceneCharacter));
    }
    Ok(())
}

struct ProbeAdmission<'a> {
    location: &'a FixtureLocation,
    character: Option<&'a str>,
    entities: &'a BTreeSet<&'a str>,
    memories: &'a BTreeMap<String, ContinuityObjectKind>,
    support_times: &'a BTreeMap<String, DateTime<Utc>>,
    last_met: &'a BTreeMap<String, DateTime<Utc>>,
    scenes: &'a BTreeMap<String, Scene>,
    memory_scenes: &'a BTreeMap<&'a str, &'a Scene>,
    activities: &'a BTreeSet<&'a str>,
    timestamp: DateTime<Utc>,
}

impl ProbeAdmission<'_> {
    fn validate(
        &self,
        assertions: &ProbeAssertions,
        measures: &ProbeMeasures,
        scene: &Scene,
        features: &mut BTreeSet<ScenarioFeature>,
    ) -> Result<ComputedProbeGold, FixtureError> {
        let location = self.location;
        if *assertions == ProbeAssertions::default() && measures.bystanders.is_empty() {
            return Err(location.error("probe.assertions", FixtureAdmissionKind::Empty));
        }
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
            features.insert(ScenarioFeature::OmissionReasons);
            if assertion.reason == OmissionReason::Resolution {
                features.insert(ScenarioFeature::ResolutionOmission);
            }
        }
        let mut orders = BTreeSet::new();
        for order in &assertions.in_order {
            require_distinct(location, "probe.assertions.in_order", order)?;
            if !orders.insert(order) {
                return Err(location.error(
                    "probe.assertions.in_order",
                    FixtureAdmissionKind::Duplicate(serde_json::to_string(order).unwrap()),
                ));
            }
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
        let mut cue_subjects = BTreeMap::new();
        for (field, expected, assertions) in [
            ("probe.assertions.cued", true, &assertions.cued),
            ("probe.assertions.not_cued", false, &assertions.not_cued),
        ] {
            for assertion in assertions {
                memory(field, &assertion.memory)?;
                let subject = (&assertion.memory, assertion.cue);
                if let Some(previous) = cue_subjects.insert(subject, expected) {
                    let subject = serde_json::to_string(&subject).unwrap();
                    return Err(location.error(
                        field,
                        if previous == expected {
                            FixtureAdmissionKind::Duplicate(subject)
                        } else {
                            FixtureAdmissionKind::Overlap(subject)
                        },
                    ));
                }
                features.insert(ScenarioFeature::CueTrace);
                let missing = match assertion.cue {
                    CueKind::Pair if !expected => Some(ScenarioFeature::PairCounterpartCue),
                    CueKind::Due => Some(ScenarioFeature::DueCue),
                    CueKind::Date => Some(ScenarioFeature::DateCue),
                    CueKind::Trigger => Some(ScenarioFeature::TriggerCue),
                    CueKind::OwnDay => Some(ScenarioFeature::OwnDayCue),
                    CueKind::RecentAndSalient => Some(ScenarioFeature::RecentAndSalientCue),
                    _ => None,
                };
                features.extend(missing);
            }
        }
        let mut references = BTreeSet::new();
        for assertion in &assertions.references {
            if matches!(
                assertion.participant,
                PerceivedReference::Description { .. }
            ) {
                features.insert(ScenarioFeature::DescriptionReferenceResolution);
            }
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
            if let PerceivedReference::Key { key } = &person.reference
                && !matches!(&assertion.resolution, ExpectedReferenceResolution::Resolved { entity } if entity == key)
            {
                return Err(location.error(
                    "probe.assertions.references",
                    FixtureAdmissionKind::DiffersFrom("scene.who.reference.key"),
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
            let asserted_scene = self.scenes.get(&assertion.scene).ok_or_else(|| {
                location.error(
                    "probe.assertions.scenes",
                    FixtureAdmissionKind::NotAdmitted(assertion.scene.clone()),
                )
            })?;
            validate_scene(
                asserted_scene,
                location,
                self.entities,
                Some(self.activities),
                self.character,
            )?;
            let source_scene = self
                .memory_scenes
                .get(assertion.memory.as_str())
                .ok_or_else(|| {
                    location.error(
                        "probe.assertions.scenes",
                        FixtureAdmissionKind::InvalidAssertion("memory has no single source scene"),
                    )
                })?;
            if !source_scene.same_context(asserted_scene) {
                return Err(location.error(
                    "probe.assertions.scenes",
                    FixtureAdmissionKind::DiffersFrom("memory.scene"),
                ));
            }
            features.insert(ScenarioFeature::MemorySceneTrace);
        }
        let mut gold = ComputedProbeGold::default();
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
            require_admitted_kind(
                location,
                "probe.measures.bystanders",
                id,
                &[
                    ContinuityObjectKind::Episode,
                    ContinuityObjectKind::DerivedMemory,
                ],
                self.memories,
            )?;
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
  {external_id = "self", label = "Character", is_hub = false},
  {external_id = "intended-entity", label = "Ada", is_hub = false},
  {external_id = "other", label = "Bert", is_hub = false},
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

    fn strict_situated_value() -> Value {
        let mut value = serde_json::to_value(parse_as(&situated_value(), "json").unwrap()).unwrap();
        value["scenarios"][0]["embedding"]["own_concept"] = Value::Bool(false);
        value
    }

    #[test]
    fn situated_salience_is_optional_and_finite_in_both_formats() {
        for extension in ["json", "toml"] {
            let original = parse_as(&situated_value(), extension).unwrap();
            let scenario = &original.scenarios[0];
            assert!(
                serde_json::to_value(&scenario.events[0])
                    .unwrap()
                    .get("salience")
                    .is_none()
            );
            assert!(
                serde_json::to_value(scenario.situated_input(&scenario.events[0]).unwrap())
                    .unwrap()
                    .get("salience")
                    .is_none()
            );
            for salience in [0.0, 0.73, 1.0] {
                let mut value = situated_value();
                value["scenarios"][0]["events"][0]["salience"] = serde_json::json!(salience);
                let admitted = parse_as(&value, extension).unwrap();
                let changed = &admitted.scenarios[0];
                assert_eq!(
                    changed.requirements.features,
                    scenario.requirements.features
                );
                assert!(
                    matches!(changed.situated_input(&changed.events[0]).unwrap(),
                    Some(SituatedInput::Experience { salience: Some(actual), .. }) if actual == salience as f32)
                );
            }
            for salience in [-0.1, 1.1, 1e99] {
                let mut value = situated_value();
                value["scenarios"][0]["events"][0]["salience"] = serde_json::json!(salience);
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        "encounter",
                        Some("early-meeting"),
                        "experience.salience",
                        FixtureAdmissionKind::OutOfUnitInterval(salience as f32),
                    )
                );
            }
        }
    }

    #[test]
    fn toml_nonfinite_values_reject_before_json_conversion_with_their_path() {
        for (key, number) in [
            ("salience", "nan"),
            ("salience", "inf"),
            ("salience", "-inf"),
        ] {
            let source = SITUATED_CASE.replace(
                "event_id = \"early-meeting\"",
                &format!("event_id = \"early-meeting\"\n{key} = {number}"),
            );
            let error =
                parse_fixture_source(std::path::Path::new("scenario.toml"), source.as_bytes())
                    .unwrap_err();
            assert!(matches!(error, FixtureError::NonFiniteTomlFloat { path }
                if path == format!("$[\"scenarios\"][0][\"events\"][0][\"{key}\"]")));
        }
        let source = SITUATED_CASE.replace("noise_magnitude = 0.01", "noise_magnitude = inf");
        assert!(matches!(
            parse_fixture_source(std::path::Path::new("scenario.toml"), source.as_bytes()).unwrap_err(),
            FixtureError::NonFiniteTomlFloat { path } if path == "$[\"scenarios\"][0][\"embedding\"][\"noise_magnitude\"]"
        ));
    }

    #[test]
    fn situated_cued_identity_and_conflicts_are_typed_in_both_formats() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            let controls = serde_json::json!([
                {"memory":"distractor", "cue":"topic"},
                {"memory":"promise", "cue":"due"}
            ]);
            value["scenarios"][0]["events"][5]["assertions"]["cued"] = controls.clone();
            let admitted = parse_as(&value, extension).unwrap();
            assert!(
                admitted.scenarios[0]
                    .requirements
                    .features
                    .contains(&ScenarioFeature::CueTrace)
            );
            let identities = admitted.scenarios[0].events[5].assertion_identities();
            assert!(identities.contains(&AssertionIdentity {
                event_id: "reunion".into(),
                assertion: AssertionSubject::Cued {
                    memory: "promise".into(),
                    cue: CueKind::Due
                }
            }));
            value["scenarios"][0]["events"][5]["assertions"]["cued"]
                .as_array_mut()
                .unwrap()
                .reverse();
            assert_eq!(
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities(),
                identities
            );
            value["scenarios"][0]["events"][5]["assertions"]["cued"] =
                serde_json::json!([controls[0].clone(), controls[0].clone()]);
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("reunion"),
                    "probe.assertions.cued",
                    FixtureAdmissionKind::Duplicate("[\"distractor\",\"topic\"]".into()),
                )
            );
            value["scenarios"][0]["events"][5]["assertions"]["cued"] =
                serde_json::json!([{"memory":"distractor","cue":"date"}]);
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("reunion"),
                    "probe.assertions.not_cued",
                    FixtureAdmissionKind::Overlap("[\"distractor\",\"date\"]".into()),
                )
            );
            value["scenarios"][0]["events"][5]["assertions"]["cued"][0]["cue"] =
                Value::from("current_state");
            assert!(matches!(
                parse_as(&value, extension),
                Err(FixtureError::Shape { .. })
            ));
        }
    }

    fn check_scene_character_admission(extension: &str) {
        for (event_index, scene_name, event_id) in
            [(0, "pair", "early-meeting"), (5, "encounter", "reunion")]
        {
            for inline in [false, true] {
                let mut value = situated_value();
                if inline {
                    let scene = value["scenarios"][0]["scenes"][scene_name].clone();
                    value["scenarios"][0]["events"][event_index]["scene"] =
                        serde_json::json!({"kind": "inline", "scene": scene});
                }
                assert!(parse_as(&value, extension).is_ok());
                let scene = if inline {
                    &mut value["scenarios"][0]["events"][event_index]["scene"]["scene"]
                } else {
                    &mut value["scenarios"][0]["scenes"][scene_name]
                };
                scene["who"].as_array_mut().unwrap().remove(0);
                // A self key outside who cannot make the character a participant.
                scene["where"] = serde_json::json!({"by": "key", "key": "self"});
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        "encounter",
                        inline.then_some(event_id),
                        "scene.who",
                        FixtureAdmissionKind::MissingSceneCharacter
                    )
                );
            }
        }
        for by in ["name", "description"] {
            for inline in [false, true] {
                let mut value = situated_value();
                let mut scene = value["scenarios"][0]["scenes"]["encounter"].clone();
                scene["who"][0] = serde_json::json!({
                    "reference": {"by": by, "text": "Character"}, "gold_entity": "self"
                });
                if inline {
                    value["scenarios"][0]["events"][5]["scene"] =
                        serde_json::json!({"kind": "inline", "scene": scene});
                } else {
                    value["scenarios"][0]["scenes"]["encounter"] = scene;
                }
                let admitted = parse_as(&value, extension).unwrap();
                assert_eq!(
                    admitted.scenarios[0].requirements.probes["encounter-probe"].elapsed_since_met
                        ["intended-entity"],
                    chrono::Duration::days(5)
                );
                let scene = if inline {
                    &mut value["scenarios"][0]["events"][5]["scene"]["scene"]
                } else {
                    &mut value["scenarios"][0]["scenes"]["encounter"]
                };
                scene["who"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("gold_entity");
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        "encounter",
                        inline.then_some("reunion"),
                        "scene.who",
                        FixtureAdmissionKind::MissingSceneCharacter
                    )
                );
            }
        }
    }

    #[test]
    fn situated_scene_character_json() {
        check_scene_character_admission("json");
    }

    #[test]
    fn situated_scene_character_toml() {
        check_scene_character_admission("toml");
    }

    #[test]
    fn situated_supersedes_rejects_thread_targets() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["scenarios"][0]["events"][3]["memory"]["subtype"] = Value::from("thread");
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("promise"),
                    "derive.supersedes",
                    FixtureAdmissionKind::UnsupportedKind {
                        external_id: "old-note".into(),
                        found: ContinuityObjectKind::MemoryThread,
                        allowed: &[ContinuityObjectKind::DerivedMemory],
                    }
                )
            );
        }
    }

    fn check_bystander_memory_kinds(extension: &str) {
        let mut value = situated_value();
        value["scenarios"][0]["events"][5]["measures"]["bystanders"] =
            serde_json::json!(["distractor", "old-note"]);
        assert!(parse_as(&value, extension).is_ok());
        value["scenarios"][0]["events"][3]["memory"]["subtype"] = Value::from("thread");
        value["scenarios"][0]["events"][4]["memory"]["supersedes"] = serde_json::json!([]);
        assert_eq!(
            admission_of(parse_as(&value, extension).unwrap_err()),
            expected_admission(
                "encounter",
                Some("reunion"),
                "probe.measures.bystanders",
                FixtureAdmissionKind::UnsupportedKind {
                    external_id: "old-note".into(),
                    found: ContinuityObjectKind::MemoryThread,
                    allowed: &[
                        ContinuityObjectKind::Episode,
                        ContinuityObjectKind::DerivedMemory
                    ],
                }
            )
        );
        // Threads remain valid subjects for the existing memory assertions.
        value["scenarios"][0]["events"][5]["measures"]["bystanders"] =
            serde_json::json!(["distractor"]);
        assert!(parse_as(&value, extension).is_ok());
    }

    #[test]
    fn situated_bystander_kinds_json() {
        check_bystander_memory_kinds("json");
    }

    #[test]
    fn situated_bystander_kinds_toml() {
        check_bystander_memory_kinds("toml");
    }

    #[test]
    fn situated_not_cued_identity_and_admission_use_memory_and_cue() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            let before =
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities();
            value["scenarios"][0]["events"][5]["assertions"]["not_cued"][0]["cue"] =
                Value::from("topic");
            let after =
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities();
            assert_eq!(before.symmetric_difference(&after).count(), 2);
            assert_eq!(
                serde_json::to_value(after.difference(&before).next().unwrap()).unwrap()["subject"],
                serde_json::json!({"memory": "distractor", "cue": "topic"})
            );
            value["scenarios"][0]["events"][5]["assertions"]["not_cued"] = serde_json::json!([
                {"memory": "distractor", "cue": "date"},
                {"memory": "distractor", "cue": "topic"}
            ]);
            let identities =
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities();
            assert_eq!(identities.len(), 10);
            value["scenarios"][0]["events"][5]["assertions"]["not_cued"]
                .as_array_mut()
                .unwrap()
                .reverse();
            assert_eq!(
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities(),
                identities
            );
            let controls = value["scenarios"][0]["events"][5]["assertions"]["not_cued"]
                .as_array_mut()
                .unwrap();
            controls.push(controls[0].clone());
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("reunion"),
                    "probe.assertions.not_cued",
                    FixtureAdmissionKind::Duplicate("[\"distractor\",\"topic\"]".into())
                )
            );
        }
    }

    #[test]
    fn situated_write_warning_identity_includes_expected_kind() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            let before =
                parse_as(&value, extension).unwrap().scenarios[0].events[4].assertion_identities();
            value["scenarios"][0]["events"][4]["expected_warning"] = Value::from("churning_chain");
            let after =
                parse_as(&value, extension).unwrap().scenarios[0].events[4].assertion_identities();
            assert_eq!(before.symmetric_difference(&after).count(), 2);
            assert_eq!(
                serde_json::to_value(after.first().unwrap()).unwrap(),
                serde_json::json!({"event_id": "promise", "kind": "write_warning", "subject": "churning_chain"})
            );
        }
    }

    #[test]
    fn situated_strict_embedding_admission_checks_normalized_write_inputs() {
        for extension in ["json", "toml"] {
            for (pointer, field, event_id) in [
                ("/entities/0/label", "entity.label", None),
                ("/events/0/text", "experience.text", Some("early-meeting")),
                ("/events/4/memory/text", "derive.text", Some("promise")),
                ("/events/5/text", "remember.text", Some("legacy")),
                (
                    "/events/5/surface_texts/episode",
                    "remember.surface_texts.episode",
                    Some("legacy"),
                ),
                (
                    "/events/5/surface_texts/observation",
                    "remember.surface_texts.observation",
                    Some("legacy"),
                ),
                (
                    "/events/5/surface_texts/derived",
                    "remember.surface_texts.derived",
                    Some("legacy"),
                ),
                (
                    "/events/6/replacement_text",
                    "correct.replacement_text",
                    Some("correction"),
                ),
            ] {
                let mut value = situated_value();
                let scenario = &mut value["scenarios"][0];
                let events = scenario["events"].as_array_mut().unwrap();
                events.insert(5, serde_json::json!({
                    "kind": "remember", "event_id": "legacy", "external_id": "legacy",
                    "timestamp": "2024-01-06T09:00:00Z", "text": "Legacy episode",
                    "entity_external_ids": [], "salience": 0.5,
                    "surface_texts": {"episode": "Legacy episode", "observation": "Legacy observation", "derived": "Legacy derived"}
                }));
                events.insert(6, serde_json::json!({
                    "kind": "correct", "event_id": "correction", "target_external_id": "legacy",
                    "replacement_external_id": "replacement", "replacement_text": "Corrected memory",
                    "timestamp": "2024-01-07T09:00:00Z"
                }));
                let normalized = "Normalized write input".to_string();
                let padded = format!("  {}\n\t", normalized.replace(' ', "  "));
                *scenario.pointer_mut(pointer).unwrap() = Value::from(padded.clone());
                if field == "remember.text" {
                    scenario["events"][5]
                        .as_object_mut()
                        .unwrap()
                        .remove("surface_texts");
                } else if field == "remember.surface_texts.episode" {
                    scenario["events"][5]["text"] = Value::from(padded.clone());
                }
                let mut strict =
                    serde_json::to_value(parse_as(&value, extension).unwrap()).unwrap();
                strict["scenarios"][0]["embedding"]["own_concept"] = Value::from(false);
                strict["scenarios"][0]["events"][5]
                    .as_object_mut()
                    .unwrap()
                    .retain(|_, value| !value.is_null());
                let mut invalid = strict.clone();
                for concept in invalid["scenarios"][0]["embedding"]["concepts"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                {
                    concept["inputs"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|input| input != &normalized);
                }
                invalid["scenarios"][0]["embedding"]["concepts"]
                    .as_object_mut()
                    .unwrap()
                    .retain(|_, concept| !concept["inputs"].as_array().unwrap().is_empty());
                let error = admission_of(parse_as(&invalid, extension).unwrap_err());
                let kind = if field == "entity.label" {
                    FixtureAdmissionKind::ConceptAssignments {
                        label: normalized.clone(),
                        found: 0,
                    }
                } else {
                    FixtureAdmissionKind::UnassignedEmbeddingInput(normalized.clone())
                };
                assert_eq!(
                    error,
                    expected_admission("encounter", event_id, field, kind),
                    "{extension}, {pointer}"
                );
                // Only the normalized write text needs an assignment; fixture text stays untouched.
                for concept in strict["scenarios"][0]["embedding"]["concepts"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                {
                    concept["inputs"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|input| input != &padded);
                }
                strict["scenarios"][0]["embedding"]["concepts"]
                    .as_object_mut()
                    .unwrap()
                    .retain(|_, concept| !concept["inputs"].as_array().unwrap().is_empty());
                assert!(
                    parse_as(&strict, extension).is_ok(),
                    "{extension}, {pointer}"
                );
            }
        }
    }

    #[test]
    fn situated_runtime_embedding_inventory_includes_topic_triggers() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["scenarios"][0]["events"][4]["memory"]["trigger"] =
                serde_json::json!({"kind": "topic", "text": "  unique trigger\ntext  "});
            let loaded = parse_as(&value, extension).unwrap();
            assert!(
                loaded.scenarios[0]
                    .runtime_embedding_inputs()
                    .contains("  unique trigger\ntext  ")
            );
        }
    }

    #[test]
    fn situated_scene_assertions_match_memory_provenance() {
        for extension in ["json", "toml"] {
            for memory in ["last-meeting", "promise"] {
                for (slot, content) in [
                    (
                        "who",
                        serde_json::json!([
                            {"reference": {"by": "key", "key": "self"}},
                            {"reference": {"by": "key", "key": "other"}}
                        ]),
                    ),
                    (
                        "where",
                        serde_json::json!({"by": "setting", "key": "elsewhere"}),
                    ),
                    ("what", serde_json::json!({"by": "key", "key": "old-note"})),
                    ("custom", serde_json::json!({"project": "another project"})),
                ] {
                    let mut value = situated_value();
                    let scenario = &mut value["scenarios"][0];
                    scenario["events"][3]["memory"]["subtype"] = Value::from("thread");
                    scenario["events"][4]["memory"]["supersedes"] = serde_json::json!([]);
                    scenario["scenes"]["mismatch"] = scenario["scenes"]["pair"].clone();
                    scenario["scenes"]["mismatch"][slot] = content;
                    scenario["events"][5]["assertions"]["scenes"] =
                        serde_json::json!([{"memory": memory, "scene": "mismatch"}]);
                    assert_eq!(
                        admission_of(parse_as(&value, extension).unwrap_err()),
                        expected_admission(
                            "encounter",
                            Some("reunion"),
                            "probe.assertions.scenes",
                            FixtureAdmissionKind::DiffersFrom("memory.scene")
                        ),
                        "{extension}, {memory}, {slot}"
                    );
                }
            }
            let mut value = situated_value();
            let scenario = &mut value["scenarios"][0];
            let mut equivalent = scenario["scenes"]["pair"].clone();
            equivalent["who"].as_array_mut().unwrap().reverse();
            equivalent["who"][0]["gold_entity"] = Value::from("intended-entity");
            scenario["scenes"]["alias"] = equivalent.clone();
            scenario["events"][1]["scene"] =
                serde_json::json!({"kind": "inline", "scene": equivalent});
            scenario["events"][5]["assertions"]["scenes"] = serde_json::json!([
                {"memory": "last-meeting", "scene": "pair"},
                {"memory": "promise", "scene": "alias"}
            ]);
            assert!(parse_as(&value, extension).is_ok());
            value["scenarios"][0]["events"][4]["memory"]["experiences"] =
                serde_json::json!(["early-meeting", "distractor"]);
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("reunion"),
                    "probe.assertions.scenes",
                    FixtureAdmissionKind::InvalidAssertion("memory has no single source scene")
                )
            );
            value["scenarios"][0]["events"][5]["assertions"]["scenes"]
                .as_array_mut()
                .unwrap()
                .pop();
            assert!(
                parse_as(&value, extension).is_ok(),
                "mixed-source derivation without a scene assertion is valid"
            );
        }
    }

    #[test]
    fn situated_scene_reference_texts_require_embedding_assignments() {
        for extension in ["json", "toml"] {
            for slot in ["who", "where", "what"] {
                for by in ["name", "description"] {
                    for named in [true, false] {
                        let mut value = strict_situated_value();
                        let scenario = &mut value["scenarios"][0];
                        let mut scene = scenario["scenes"]["encounter"].clone();
                        let reference =
                            serde_json::json!({"by": by, "text": "Unassigned scene reference"});
                        if slot == "who" {
                            scene["who"][1]["reference"] = reference;
                            scenario["events"][5]["assertions"]["references"] =
                                serde_json::json!([]);
                        } else {
                            scene[slot] = reference;
                        }
                        if named {
                            scenario["scenes"]["encounter"] = scene;
                        } else {
                            scenario["events"][5]["scene"] =
                                serde_json::json!({"kind": "inline", "scene": scene});
                        }
                        assert_eq!(
                            admission_of(parse_as(&value, extension).unwrap_err()),
                            expected_admission(
                                "encounter",
                                Some("reunion"),
                                "scene.reference.text",
                                FixtureAdmissionKind::UnassignedEmbeddingInput(
                                    "Unassigned scene reference".into()
                                )
                            ),
                            "{extension}, {slot}, {by}, named={named}"
                        );
                        value["scenarios"][0]["embedding"]["own_concept"] = Value::Bool(true);
                        assert!(parse_as(&value, extension).is_ok());
                    }
                }
            }
        }
    }

    #[test]
    fn situated_topic_triggers_require_embedding_assignments() {
        for extension in ["json", "toml"] {
            let mut value = strict_situated_value();
            value["scenarios"][0]["events"][4]["memory"]["trigger"] =
                serde_json::json!({"kind": "topic", "text": "Unassigned topic trigger"});
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some("promise"),
                    "derive.trigger",
                    FixtureAdmissionKind::UnassignedEmbeddingInput(
                        "Unassigned topic trigger".into()
                    )
                )
            );
            value["scenarios"][0]["events"][4]["memory"]["trigger"]["text"] =
                Value::from("I will bring the book.");
            assert!(parse_as(&value, extension).is_ok());
        }
    }

    #[test]
    fn situated_reference_features_follow_scene_roles() {
        let role_features = [
            "participant_name",
            "participant_description",
            "place_name",
            "place_description",
            "activity_name",
            "activity_description",
        ];
        for extension in ["json", "toml"] {
            for (slot, by, expected) in [
                ("who", "name", "participant_name"),
                ("who", "description", "participant_description"),
                ("where", "name", "place_name"),
                ("where", "description", "place_description"),
                ("what", "name", "activity_name"),
                ("what", "description", "activity_description"),
            ] {
                let mut value = situated_value();
                let mut scene =
                    serde_json::json!({"who": [{"reference": {"by": "key", "key": "self"}}]});
                let reference = serde_json::json!({"by": by, "text": "Perceived reference"});
                if slot == "who" {
                    scene["who"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!({"reference": reference}));
                } else {
                    scene[slot] = reference;
                }
                value["scenarios"][0]["events"][5]["scene"] =
                    serde_json::json!({"kind": "inline", "scene": scene});
                value["scenarios"][0]["events"][5]["assertions"]["references"] =
                    serde_json::json!([]);
                let loaded = parse_as(&value, extension).unwrap();
                let features: Vec<String> = serde_json::from_value(
                    serde_json::to_value(&loaded.scenarios[0].requirements.features).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    features
                        .iter()
                        .filter(|feature| role_features.contains(&feature.as_str()))
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    [expected],
                    "{extension}, {slot}, {by}"
                );
            }
        }
    }

    #[test]
    fn situated_order_assertions_allow_shared_members_but_have_unique_subjects() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            let assertions = &mut value["scenarios"][0]["events"][5]["assertions"];
            assertions["carried"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"memory": "early-meeting", "reason": "pair"}));
            assertions["in_order"] = serde_json::json!([
                ["last-meeting", "promise"],
                ["last-meeting", "early-meeting"]
            ]);
            let loaded = parse_as(&value, extension).unwrap();
            let identities = loaded.scenarios[0].events[5].assertion_identities();
            assert_eq!(identities.len(), 11);
            value["scenarios"][0]["events"][5]["assertions"]["in_order"]
                .as_array_mut()
                .unwrap()
                .reverse();
            assert_eq!(
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities(),
                identities
            );
            for (orders, duplicate) in [
                (
                    serde_json::json!([["last-meeting", "last-meeting"]]),
                    "last-meeting".to_string(),
                ),
                (
                    serde_json::json!([["last-meeting", "promise"], ["last-meeting", "promise"]]),
                    "[\"last-meeting\",\"promise\"]".to_string(),
                ),
            ] {
                value["scenarios"][0]["events"][5]["assertions"]["in_order"] = orders;
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        "encounter",
                        Some("reunion"),
                        "probe.assertions.in_order",
                        FixtureAdmissionKind::Duplicate(duplicate)
                    )
                );
            }
        }
    }

    #[test]
    fn situated_key_reference_assertions_resolve_to_that_key() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["scenarios"][0]["scenes"]["encounter"]["who"][1] =
                serde_json::json!({"reference": {"by": "key", "key": "intended-entity"}});
            value["scenarios"][0]["events"][5]["assertions"]["references"][0]["participant"] =
                serde_json::json!({"by": "key", "key": "intended-entity"});
            assert!(parse_as(&value, extension).is_ok());
            for resolution in [
                serde_json::json!({"status": "unknown"}),
                serde_json::json!({"status": "ambiguous", "candidates": ["intended-entity", "other"]}),
                serde_json::json!({"status": "resolved", "entity": "other"}),
            ] {
                value["scenarios"][0]["events"][5]["assertions"]["references"][0]["resolution"] =
                    resolution;
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        "encounter",
                        Some("reunion"),
                        "probe.assertions.references",
                        FixtureAdmissionKind::DiffersFrom("scene.who.reference.key")
                    )
                );
            }
        }
    }

    #[test]
    fn situated_asserted_scenes_require_prior_activities() {
        for extension in ["json", "toml"] {
            for subtype in ["thread", "open_loop"] {
                let mut value = situated_value();
                let scenario = &mut value["scenarios"][0];
                scenario["events"][3]["memory"]["subtype"] = Value::from(subtype);
                scenario["events"][4]["memory"]["supersedes"] = serde_json::json!([]);
                let mut scene = scenario["scenes"]["pair"].clone();
                scene["what"] = serde_json::json!({"by": "key", "key": "old-note"});
                scenario["scenes"]["asserted-activity"] = scene;
                let mut experience = scenario["events"][1].clone();
                experience["event_id"] = Value::from("activity-experience");
                experience["timestamp"] = Value::from("2024-01-07T09:00:00Z");
                experience["scene"] =
                    serde_json::json!({"kind": "named", "name": "asserted-activity"});
                scenario["events"][5]["assertions"]["carried"]
                    .as_array_mut()
                    .unwrap()
                    .push(
                        serde_json::json!({"memory": "activity-experience", "reason": "activity"}),
                    );
                scenario["events"][5]["assertions"]["scenes"][0]["memory"] =
                    Value::from("activity-experience");
                scenario["events"][5]["assertions"]["scenes"][0]["scene"] =
                    Value::from("asserted-activity");
                scenario["events"]
                    .as_array_mut()
                    .unwrap()
                    .insert(5, experience);
                assert!(parse_as(&value, extension).is_ok());
                for id in ["unknown", "future-activity", "promise"] {
                    let mut invalid = value.clone();
                    let mut invalid_scene =
                        invalid["scenarios"][0]["scenes"]["asserted-activity"].clone();
                    invalid_scene["what"]["key"] = Value::from(id);
                    invalid["scenarios"][0]["scenes"]["invalid-activity"] = invalid_scene;
                    invalid["scenarios"][0]["events"][6]["assertions"]["scenes"][0]["scene"] =
                        Value::from("invalid-activity");
                    if id == "future-activity" {
                        let mut future = invalid["scenarios"][0]["events"][3].clone();
                        future["event_id"] = Value::from(id);
                        future["timestamp"] = Value::from("2024-01-09T09:00:00Z");
                        invalid["scenarios"][0]["events"]
                            .as_array_mut()
                            .unwrap()
                            .push(future);
                    }
                    assert_eq!(
                        admission_of(parse_as(&invalid, extension).unwrap_err()),
                        expected_admission(
                            "encounter",
                            Some("reunion"),
                            "scene.what.key",
                            FixtureAdmissionKind::NotAdmittedActivity(id.into())
                        )
                    );
                }
            }
        }
    }

    #[test]
    fn situated_scene_assertion_identity_includes_expected_scene() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["scenarios"][0]["scenes"]["alias"] =
                value["scenarios"][0]["scenes"]["pair"].clone();
            value["scenarios"][0]["events"][5]["assertions"]["scenes"] = serde_json::json!([
                {"memory": "last-meeting", "scene": "pair"},
                {"memory": "promise", "scene": "pair"}
            ]);
            let before =
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities();
            value["scenarios"][0]["events"][5]["assertions"]["scenes"]
                .as_array_mut()
                .unwrap()
                .reverse();
            assert_eq!(
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities(),
                before
            );
            value["scenarios"][0]["events"][5]["assertions"]["scenes"][1]["scene"] =
                Value::from("alias");
            let after =
                parse_as(&value, extension).unwrap().scenarios[0].events[5].assertion_identities();
            assert_eq!(before.symmetric_difference(&after).count(), 2);
            let added = serde_json::to_value(after.difference(&before).next().unwrap()).unwrap();
            assert_eq!(
                added["subject"],
                serde_json::json!({"memory": "last-meeting", "scene": "alias"})
            );
        }
    }

    #[test]
    fn situated_setting_references_are_only_places() {
        for extension in ["json", "toml"] {
            for named in [true, false] {
                let mut value = situated_value();
                let scene = value["scenarios"][0]["scenes"]["encounter"].clone();
                if !named {
                    value["scenarios"][0]["events"][5]["scene"] =
                        serde_json::json!({"kind": "inline", "scene": scene});
                }
                assert!(parse_as(&value, extension).is_ok());
                for slot in ["what", "who"] {
                    let mut invalid = value.clone();
                    let scene = if named {
                        &mut invalid["scenarios"][0]["scenes"]["encounter"]
                    } else {
                        &mut invalid["scenarios"][0]["events"][5]["scene"]["scene"]
                    };
                    let reference = serde_json::json!({"by": "setting", "key": "opaque-setting"});
                    if slot == "who" {
                        scene["who"][0]["reference"] = reference;
                    } else {
                        scene[slot] = reference;
                    }
                    assert_eq!(
                        admission_of(parse_as(&invalid, extension).unwrap_err()),
                        expected_admission(
                            "encounter",
                            if named { None } else { Some("reunion") },
                            if slot == "who" {
                                "scene.who"
                            } else {
                                "scene.what"
                            },
                            FixtureAdmissionKind::InvalidSceneReference
                        )
                    );
                }
            }
        }
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
                ScenarioFeature::Direction,
                ScenarioFeature::DueDate,
                ScenarioFeature::Trigger,
                ScenarioFeature::AuthoredDerivedMemory,
                ScenarioFeature::CueTrace,
                ScenarioFeature::DateCue,
                ScenarioFeature::ReferenceTrace,
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
                        scenario["events"][4]["memory"]["supersedes"] = serde_json::json!([]);
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
    fn pattern_matches_event_family_in_both_formats() {
        let mut legacy = situated_value();
        legacy["scenarios"][0]["pattern"] = Value::from("long_gap_recall");
        legacy["scenarios"][0]["events"] = serde_json::json!([
            {"kind":"remember", "event_id":"visit", "external_id":"visit", "timestamp":"2024-01-01T09:00:00Z", "text":"Garden", "entity_external_ids":[], "salience":0.5},
            {"kind":"query", "event_id":"ask", "query_id":"ask", "timestamp":"2024-01-02T09:00:00Z", "text":"Garden", "expected":{"relevant_external_ids":["visit"], "irrelevant_external_ids":[]}}
        ]);
        for extension in ["json", "toml"] {
            for (mut value, wrong_pattern, fixture_id) in [
                (situated_value(), "long_gap_recall", "encounter"),
                (legacy.clone(), "situated", "encounter"),
            ] {
                assert!(parse_as(&value, extension).is_ok());
                value["scenarios"][0]["pattern"] = Value::from(wrong_pattern);
                assert_eq!(
                    admission_of(parse_as(&value, extension).unwrap_err()),
                    expected_admission(
                        fixture_id,
                        None,
                        "pattern",
                        FixtureAdmissionKind::DiffersFrom("event family")
                    )
                );
            }
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
    fn unknown_probe_keys_and_omission_reasons_are_rejected_in_both_formats() {
        for extension in ["json", "toml"] {
            for omission in [false, true] {
                let mut value = situated_value();
                let probe = &mut value["scenarios"][0]["events"][5];
                if omission {
                    probe["assertions"]["omitted"][0]["reason"] = Value::from("withheld");
                } else {
                    probe["withhold"] = serde_json::json!({"by": "participants"});
                }
                match parse_as(&value, extension).unwrap_err() {
                    FixtureError::Shape {
                        location, field, ..
                    } => {
                        assert_eq!(location, FixtureLocation::scenario("encounter"));
                        assert_eq!(field.as_deref(), (!omission).then_some("withhold"));
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
        ];
        for (case, field, kind) in cases {
            let mut value = situated_value();
            let events = &mut value["scenarios"][0]["events"];
            match case {
                "future" => events[3]["memory"]["experiences"] = serde_json::json!(["promise"]),
                "bystander" => events[5]["measures"]["bystanders"] = serde_json::json!(["promise"]),
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
                let first = values[0].clone();
                let duplicate = if list == "in_order" {
                    serde_json::to_string(&first).unwrap()
                } else if list == "not_cued" {
                    serde_json::to_string(&(&first["memory"], &first["cue"])).unwrap()
                } else {
                    values
                        .pointer(pointer)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string()
                };
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
        assert_eq!(identities.len(), 9);
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

                speaker_entity_external_id: None,
                salience: None,
                scene: cmem_eval::MemorySceneInput {
                    time: None,
                    ..Default::default()
                },
                observation_observed_at: None,
                raw_refs: Vec::new(),
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
    fn runtime_embedding_inputs_follow_write_and_query_normalization() {
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
        *text = "  target  query\n\t".to_string();

        assert_eq!(
            scenario.runtime_embedding_inputs(),
            BTreeSet::from(["target  query".to_string(), "remembered target".to_string(),])
        );
    }

    #[tokio::test]
    async fn query_embedding_admission_matches_native_trim_only_lookup() {
        const RAW: &str = "  unique  query\n\t";
        const TRIMMED: &str = "unique  query";
        for extension in ["json", "toml"] {
            for probe in [false, true] {
                let mut value = strict_situated_value();
                let scenario = &mut value["scenarios"][0];
                if probe {
                    scenario["events"][5]["topic"] = Value::from(RAW);
                } else {
                    scenario["pattern"] = Value::from("long_gap_recall");
                    scenario["events"] = serde_json::json!([
                        {"kind": "remember", "event_id": "visit", "external_id": "visit",
                         "timestamp": "2024-01-01T09:00:00Z", "text": "We planned the garden.",
                         "entity_external_ids": [], "salience": 0.5},
                        {"kind": "query", "event_id": "ask", "query_id": "ask",
                         "timestamp": "2024-01-02T09:00:00Z", "text": RAW,
                         "expected": {"relevant_external_ids": ["visit"], "irrelevant_external_ids": []}}
                    ]);
                }
                scenario["embedding"]["clusters"]["query"] = serde_json::json!(vec![1.0; 16]);
                scenario["embedding"]["concepts"]["query"] =
                    serde_json::json!({"cluster": "query", "inputs": [RAW]});
                for assigned in [RAW, "unique query"] {
                    value["scenarios"][0]["embedding"]["concepts"]["query"]["inputs"] =
                        serde_json::json!([assigned]);
                    assert_eq!(
                        admission_of(parse_as(&value, extension).unwrap_err()),
                        expected_admission(
                            "encounter",
                            Some(if probe { "reunion" } else { "ask" }),
                            if probe { "probe.topic" } else { "query.text" },
                            FixtureAdmissionKind::UnassignedEmbeddingInput(TRIMMED.into())
                        ),
                        "{extension}, probe={probe}, assignment={assigned:?}"
                    );
                }
                value["scenarios"][0]["embedding"]["concepts"]["query"]["inputs"] =
                    serde_json::json!([TRIMMED]);
                let loaded = parse_as(&value, extension).unwrap();
                let scenario = &loaded.scenarios[0];
                let inputs = scenario.runtime_embedding_inputs();
                assert!(inputs.contains(TRIMMED));
                assert!(!inputs.contains(RAW));
                assert!(!inputs.contains("unique query"));
                if probe {
                    assert!(
                        matches!(scenario.situated_input(scenario.events.last().unwrap()).unwrap(),
                        Some(SituatedInput::Probe { topic: Some(topic), .. }) if topic == RAW)
                    );
                    continue; // The driver drift test covers the complete probe scene mapping.
                }

                // Exercise the real library lookup: only the trim-only key is assigned.
                let embeddings = scenario
                    .embedding
                    .controllable_similarity()
                    .unwrap()
                    .clone();
                let mut config = BenchmarkRunConfig {
                    run_id: "query-trim-drift".into(),
                    dataset: DatasetId::new("continuity").unwrap(),
                    backend: Default::default(),
                    retrieval: Default::default(),
                    ingest: Default::default(),
                    metrics: Default::default(),
                };
                config.backend.embedding.provider = EmbeddingProviderConfig::ControllableSimilarity;
                config.backend.embedding.vector_size = Some(embeddings.vector_size);
                let binding = cmem_eval::EmbeddingRuntimeBinding::Controllable {
                    dimension_policy: cmem_eval::ControllableDimensionPolicy::Exact {
                        vector_size: embeddings.vector_size,
                    },
                    fixture: embeddings,
                };
                let directory = tempfile::tempdir().unwrap();
                let mut runtime = crate::ContinuityRuntime::new(directory.path(), &config, binding)
                    .await
                    .unwrap();
                let result =
                    crate::run_continuity_scenario(&mut runtime, scenario, &config.retrieval).await;
                runtime.cleanup(&scenario.namespace).await.unwrap();
                drop(runtime);
                directory.close().unwrap();
                let run = result.expect("native retrieval must use the admitted trim-only key");
                assert_eq!(run.traces.len(), 1);
                assert_eq!(run.traces[0].query, RAW);
            }
        }
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
                    "external_id": "entity-person",
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
                        "thread": {"thread_external_id": "thread-one"}
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
        value["scenarios"][0]["entities"][0]["label"] = Value::Bool(false);
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
    fn public_parser_rejects_naming_belief_id_collisions_in_both_formats() {
        let naming_id = "continuity:entity-name:intended-entity";
        let value: Value = toml::from_str(&SITUATED_CASE.replace("old-note", naming_id)).unwrap();
        for extension in ["json", "toml"] {
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()),
                expected_admission(
                    "encounter",
                    Some(naming_id),
                    "derive.external_id",
                    FixtureAdmissionKind::Duplicate(naming_id.into()),
                )
            );
        }
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
    fn public_parser_rejects_invalid_salience() {
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
    fn situated_probe_requires_an_assertion_or_an_authored_measure() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["scenarios"][0]["events"][5]["assertions"] = serde_json::json!({});
            // A bystander-only probe still has a measurement purpose.
            parse_as(&value, extension).unwrap();
            value["scenarios"][0]["events"][5]["measures"] = serde_json::json!({});
            assert_eq!(
                admission_of(parse_as(&value, extension).unwrap_err()).2,
                FixtureAdmissionKind::Empty
            );
            value["scenarios"][0]["events"][5]["assertions"] = serde_json::json!({
                "carried": [{"memory": "promise", "reason": "due"}]
            });
            parse_as(&value, extension).unwrap();
        }
    }

    #[test]
    fn situated_restart_requires_a_following_query_not_only_a_probe() {
        for extension in ["json", "toml"] {
            let mut value = situated_value();
            value["scenarios"][0]["events"].as_array_mut().unwrap().insert(5, serde_json::json!({
                "kind": "restart", "event_id": "restart", "timestamp": "2024-01-07T09:00:00Z",
                "reopen_graph": true, "reopen_stats": true
            }));
            let error = parse_as(&value, extension).unwrap_err();
            assert_eq!(
                admission_of(error).2,
                FixtureAdmissionKind::RestartWithoutFollowingQuery
            );
        }
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
