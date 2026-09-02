use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::fmt;

macro_rules! snake_case_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

snake_case_enum!(ObjectType {
    Episode,
    Observation,
    Entity,
    MemoryThread,
    DerivedMemory,
    MemoryLink,
});
snake_case_enum!(EntityType {
    Person,
    User,
    Assistant,
    Project,
    Concept,
    Tool,
    Document,
    Place,
    Organization,
    Other,
});
snake_case_enum!(DerivedType {
    Reflection,
    UserPreference,
    AssistantPreference,
    Commitment,
    OpenLoop,
    CharacterSignal,
    RelationshipNote,
    ProjectNote,
    Claim,
    Correction,
});
snake_case_enum!(RelationType {
    HasObservation,
    ObservedIn,
    Mentions,
    Involves,
    About,
    DerivedFrom,
    PartOfThread,
    Supports,
    Contradicts,
    Supersedes,
    Resolves,
    CreatesOpenLoop,
    FulfillsCommitment,
    AssociatedWith,
});
snake_case_enum!(RetentionState {
    Active,
    Suppressed,
    Archived,
    Deleted
});
snake_case_enum!(Stability { Low, Medium, High });
snake_case_enum!(ThreadStatus {
    Active,
    Dormant,
    Resolved,
    Archived
});

macro_rules! string_contract {
    ($name:ident { $($variant:ident => $token:literal),+ $(,)? }) => {
        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $token),+ }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl TryFrom<&str> for $name {
            type Error = anyhow::Error;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value {
                    $($token => Ok(Self::$variant),)+
                    _ => anyhow::bail!("unsupported {} token {value:?}", stringify!($name)),
                }
            }
        }
    };
}

string_contract!(ObjectType {
    Episode => "episode",
    Observation => "observation",
    Entity => "entity",
    MemoryThread => "memory_thread",
    DerivedMemory => "derived_memory",
    MemoryLink => "memory_link",
});
string_contract!(EntityType {
    Person => "person", User => "user", Assistant => "assistant", Project => "project",
    Concept => "concept", Tool => "tool", Document => "document", Place => "place",
    Organization => "organization", Other => "other",
});
string_contract!(DerivedType {
    Reflection => "reflection", UserPreference => "user_preference",
    AssistantPreference => "assistant_preference", Commitment => "commitment",
    OpenLoop => "open_loop", CharacterSignal => "character_signal",
    RelationshipNote => "relationship_note", ProjectNote => "project_note",
    Claim => "claim", Correction => "correction",
});
string_contract!(RelationType {
    HasObservation => "has_observation", ObservedIn => "observed_in", Mentions => "mentions",
    Involves => "involves", About => "about", DerivedFrom => "derived_from",
    PartOfThread => "part_of_thread", Supports => "supports", Contradicts => "contradicts",
    Supersedes => "supersedes", Resolves => "resolves", CreatesOpenLoop => "creates_open_loop",
    FulfillsCommitment => "fulfills_commitment", AssociatedWith => "associated_with",
});
string_contract!(RetentionState {
    Active => "active", Suppressed => "suppressed", Archived => "archived", Deleted => "deleted",
});
string_contract!(Stability { Low => "low", Medium => "medium", High => "high" });
string_contract!(ThreadStatus {
    Active => "active", Dormant => "dormant", Resolved => "resolved", Archived => "archived",
});
snake_case_enum!(MemoryCandidateKind {
    Episode,
    Observation,
    Entity,
    MemoryThread,
    DerivedMemory,
    MemoryLink,
    VectorIndex,
    StatsUpdate,
});
snake_case_enum!(CandidateProducerKind {
    Caller,
    DeterministicHelper,
    RuleProcessor,
    ModelProcessor,
    ImportTool,
    System,
    Unknown,
});
snake_case_enum!(RationaleOrigin {
    ProvidedByCaller,
    ProvidedByProcessor,
    InferredByProcessor,
    Unavailable,
});
snake_case_enum!(CandidateValidationStatus { Valid, Invalid });
snake_case_enum!(SelectivityCountScope {
    Current,
    Active,
    Total
});
snake_case_enum!(SelectivityDecision {
    HighSelectivity,
    LowSelectivitySupported,
    LowSelectivityRejected,
    ConservativeFallback,
});
snake_case_enum!(GraphExpansionBoundedReason {
    NodeLimit,
    Timeout,
    HubLimit
});
snake_case_enum!(GraphFailureMode {
    AllowPartialResults,
    FailClosed
});
snake_case_enum!(ContextPackSection {
    ActiveThreads,
    RelevantEpisodes,
    SalientObservations,
    DerivedMemories,
    Preferences,
    RelationshipNotes,
    OpenLoops,
    Commitments,
    CharacterSignals,
    Omitted,
});
string_contract!(ContextPackSection {
    ActiveThreads => "active_threads",
    RelevantEpisodes => "relevant_episodes",
    SalientObservations => "salient_observations",
    DerivedMemories => "derived_memories",
    Preferences => "preferences",
    RelationshipNotes => "relationship_notes",
    OpenLoops => "open_loops",
    Commitments => "commitments",
    CharacterSignals => "character_signals",
    Omitted => "omitted",
});
snake_case_enum!(StaleCandidateReason {
    GraphObjectMissing,
    LifecycleMismatch,
    CurrentnessMismatch,
    Superseded,
    SectionLimit,
    GraphExpansionBounded,
});
snake_case_enum!(LifecycleFilterReason {
    Active,
    ArchivedIncludedByPolicy,
    SuppressedIncludedByPolicy,
    DeletedIncludedByPolicy,
    NonCurrentIncludedByPolicy,
    SupersededIncludedByPolicy,
    ArchivedOmitted,
    SuppressedOmitted,
    DeletedOmitted,
    NonCurrentOmitted,
    SupersededOmitted,
    GraphObjectMissing,
    GraphExpansionBounded,
});
snake_case_enum!(PlanIdentityField {
    OperationId,
    IdempotencyKey
});
snake_case_enum!(CandidateTimestampField {
    CreatedAt,
    UpdatedAt,
    LastTouchedAt
});
snake_case_enum!(CandidateScoreField {
    EpisodeSalience,
    ObservationSalience,
    MemoryThreadSalience,
    DerivedMemoryConfidence,
    DerivedMemorySalience,
    MemoryLinkConfidence,
});
snake_case_enum!(MemoryLinkEndpoint { From, To });
snake_case_enum!(CandidateProvenanceIssue {
    NonCallerClaimedCallerRationale,
    EmptyRationaleText,
    EmptyExternalReference,
});
snake_case_enum!(CandidateSourceSpanIssue {
    EmptySourceRef,
    EmptyRawRef,
    EmptyMessageId,
    EmptyTranscriptSegmentId,
    InvalidTurnRange,
    InvalidCharRange,
    InvalidByteRange,
    InvalidTimestampRange,
});
snake_case_enum!(CandidateReferenceRole {
    DerivedSourceEpisode,
    DerivedSourceObservation,
    MemoryLinkFrom,
    MemoryLinkTo,
    VectorIndexTarget,
    StatsUpdateSubject,
    StatsUpdateObject,
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct ObjectRefRecord {
    pub object_type: ObjectType,
    pub internal_id: String,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateValidationIssueRecord {
    MissingPlanIdentity {
        field: PlanIdentityField,
    },
    MissingCandidateId,
    MissingCandidateSchemaVersion,
    MissingTimestamp {
        field: CandidateTimestampField,
    },
    ObjectTypeMismatch {
        expected: ObjectType,
        actual: ObjectType,
    },
    EmptyEpisodeSummary,
    MissingEpisodeReference,
    MissingDerivedSource,
    InvalidScore {
        field: CandidateScoreField,
        actual: String,
    },
    UnsupportedMemoryLinkEndpoint {
        endpoint: MemoryLinkEndpoint,
    },
    SelfLink {
        referenced: ObjectRefRecord,
    },
    MissingObjectSchemaVersion,
    MemoryLinkRejectedByAdmissionPolicy,
    SuppressedMemoryMarkedCurrent,
    SupersedingMemoryMarkedCurrent,
    InvalidProvenance {
        reason: CandidateProvenanceIssue,
    },
    InvalidSourceSpan {
        reason: CandidateSourceSpanIssue,
    },
    EmptyVectorEmbeddingText,
    IncompleteStatsRelationObjectPair,
    UnknownObjectRef {
        role: CandidateReferenceRole,
        referenced: ObjectRefRecord,
    },
    ReferenceNotInPlan {
        role: CandidateReferenceRole,
        referenced: ObjectRefRecord,
    },
    DuplicateObservationEcho {
        echo_surface: String,
        matching_episode_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidateValidationRecord {
    pub candidate_index: usize,
    pub candidate_kind: MemoryCandidateKind,
    pub status: CandidateValidationStatus,
    pub errors: Vec<CandidateValidationIssueRecord>,
    pub warnings: Vec<CandidateValidationIssueRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidateCountRecord {
    pub candidate_kind: MemoryCandidateKind,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VectorDatabaseErrorKind {
    Response,
    ResourceExhausted,
    Conversion,
    InvalidUri,
    NoSnapshotFound,
    Io { io_kind: IoErrorKindRecord },
    HttpTimeout,
    HttpConnect,
    HttpStatus,
    Http,
    JsonToPayload,
    PayloadDeserialization,
}

const VECTOR_DATABASE_ERROR_KIND_VARIANTS: &[&str] = &[
    "response",
    "resource_exhausted",
    "conversion",
    "invalid_uri",
    "no_snapshot_found",
    "io",
    "http_timeout",
    "http_connect",
    "http_status",
    "http",
    "json_to_payload",
    "payload_deserialization",
];

struct VectorDatabaseErrorKindVisitor;

impl<'de> Visitor<'de> for VectorDatabaseErrorKindVisitor {
    type Value = VectorDatabaseErrorKind;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a strictly shaped vector database error kind")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut kind = None;
        let mut io_kind = None;

        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "kind" => {
                    if kind.is_some() {
                        return Err(A::Error::duplicate_field("kind"));
                    }
                    kind = Some(map.next_value::<String>()?);
                }
                "io_kind" => {
                    if io_kind.is_some() {
                        return Err(A::Error::duplicate_field("io_kind"));
                    }
                    io_kind = Some(map.next_value::<IoErrorKindRecord>()?);
                }
                field => {
                    return Err(A::Error::unknown_field(field, &["kind", "io_kind"]));
                }
            }
        }

        let kind = kind.ok_or_else(|| A::Error::missing_field("kind"))?;
        let unit_kind = match kind.as_str() {
            "response" => VectorDatabaseErrorKind::Response,
            "resource_exhausted" => VectorDatabaseErrorKind::ResourceExhausted,
            "conversion" => VectorDatabaseErrorKind::Conversion,
            "invalid_uri" => VectorDatabaseErrorKind::InvalidUri,
            "no_snapshot_found" => VectorDatabaseErrorKind::NoSnapshotFound,
            "io" => {
                return Ok(VectorDatabaseErrorKind::Io {
                    io_kind: io_kind.ok_or_else(|| A::Error::missing_field("io_kind"))?,
                });
            }
            "http_timeout" => VectorDatabaseErrorKind::HttpTimeout,
            "http_connect" => VectorDatabaseErrorKind::HttpConnect,
            "http_status" => VectorDatabaseErrorKind::HttpStatus,
            "http" => VectorDatabaseErrorKind::Http,
            "json_to_payload" => VectorDatabaseErrorKind::JsonToPayload,
            "payload_deserialization" => VectorDatabaseErrorKind::PayloadDeserialization,
            variant => {
                return Err(A::Error::unknown_variant(
                    variant,
                    VECTOR_DATABASE_ERROR_KIND_VARIANTS,
                ));
            }
        };

        if io_kind.is_some() {
            return Err(A::Error::unknown_field("io_kind", &["kind"]));
        }
        Ok(unit_kind)
    }
}

impl<'de> Deserialize<'de> for VectorDatabaseErrorKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(VectorDatabaseErrorKindVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum IoErrorKindRecord {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    HostUnreachable,
    NetworkUnreachable,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    NetworkDown,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    ReadOnlyFilesystem,
    StaleNetworkFileHandle,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    StorageFull,
    NotSeekable,
    QuotaExceeded,
    FileTooLarge,
    ResourceBusy,
    ExecutableFileBusy,
    Deadlock,
    CrossesDevices,
    TooManyLinks,
    InvalidFilename,
    ArgumentListTooLong,
    Interrupted,
    Unsupported,
    UnexpectedEof,
    OutOfMemory,
    Other,
    Unrecognized,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TransportStatus {
    Ok,
    Cancelled,
    Unknown,
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    DataLoss,
    Unauthenticated,
    Unrecognized(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VectorDatabaseErrorRecord {
    pub backend: String,
    pub kind: VectorDatabaseErrorKind,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub status: Option<TransportStatus>,
    pub message: String,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub retry_after_seconds: Option<u64>,
}

snake_case_enum!(EmbeddingTransportErrorKind {
    Timeout,
    Connect,
    Request,
    Body,
    Other
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EmbeddingErrorRecord {
    MissingApiKey,
    ProviderVectorSizeMismatch {
        expected: usize,
        actual: usize,
    },
    BlankInput {
        #[serde(deserialize_with = "crate::serde_contract::required_option")]
        index: Option<usize>,
    },
    Transport {
        transport_kind: EmbeddingTransportErrorKind,
        detail: String,
    },
    HttpStatus {
        status: u16,
        body: String,
    },
    InvalidJson {
        detail: String,
    },
    MissingData,
    CountMismatch {
        expected: usize,
        actual: usize,
    },
    MissingIndex {
        item: usize,
    },
    IndexOutOfRange {
        index: usize,
        expected_count: usize,
    },
    DuplicateIndex {
        index: usize,
    },
    MissingEmbedding {
        item: usize,
    },
    DimensionMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
    NonNumericValue {
        index: usize,
        component: usize,
    },
    MissingResponseIndex {
        index: usize,
    },
    Unrecognized {
        detail: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "cause",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum VectorIndexingCauseRecord {
    Embedding(EmbeddingErrorRecord),
    CardinalityMismatch { expected: usize, actual: usize },
    VectorDatabase(VectorDatabaseErrorRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "cause", rename_all = "snake_case", deny_unknown_fields)]
pub enum StatsUpdateCauseRecord {
    EndpointHydration {
        error: GraphQueryErrorRecord,
    },
    EdgeWrite {
        error: RetrievalStatsStoreErrorRecord,
    },
    ObjectStateWrite {
        error: RetrievalStatsStoreErrorRecord,
    },
    HealthCheck {
        error: RetrievalStatsStoreErrorRecord,
    },
    HealthMark {
        error: RetrievalStatsStoreErrorRecord,
    },
    StoreUnhealthy {
        #[serde(deserialize_with = "crate::serde_contract::required_option")]
        health_cause: Option<RetrievalStatsHealthCauseRecord>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphQueryErrorRecord {
    Selection { detail: String },
    Hydration { detail: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RetrievalStatsStoreErrorRecord {
    Sqlite {
        detail: String,
    },
    Filesystem {
        io_kind: IoErrorKindRecord,
        detail: String,
    },
    LockPoisoned,
    HealthSerialization {
        detail: String,
    },
    HealthDeserialization {
        detail: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RetrievalStatsHealthCauseRecord {
    StoreInitialization {
        error: RetrievalStatsStoreErrorRecord,
    },
    EndpointHydration {
        error: GraphQueryErrorRecord,
    },
    EdgeWrite {
        error: RetrievalStatsStoreErrorRecord,
    },
    ObjectStateWrite {
        error: RetrievalStatsStoreErrorRecord,
    },
    HealthCheck {
        error: RetrievalStatsStoreErrorRecord,
    },
    CounterRead {
        error: RetrievalStatsStoreErrorRecord,
    },
    GlobalCounterRead {
        error: RetrievalStatsStoreErrorRecord,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepairMarkerRecord {
    VectorIndex {
        unindexed_objects: Vec<ObjectRefRecord>,
        cause: VectorIndexingCauseRecord,
    },
    StatsUpdate {
        object_internal_ids: Vec<String>,
        causes: Vec<StatsUpdateCauseRecord>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VectorIndexingFailureRecord {
    pub unindexed_objects: Vec<ObjectRefRecord>,
    pub cause: VectorIndexingCauseRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatsUpdateFailureRecord {
    pub failed_object_internal_ids: Vec<String>,
    pub causes: Vec<StatsUpdateCauseRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct StatsUpdateStatusRecord {
    pub updated_object_internal_ids: Vec<String>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub failure: Option<StatsUpdateFailureRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriteOperationKind {
    TypedIngest,
    ExplicitCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriteOutcomeRecord {
    pub operation_id: String,
    pub operation: WriteOperationKind,
    pub persisted_objects: Vec<ObjectRefRecord>,
    pub persisted_link_internal_ids: Vec<String>,
    pub vector_indexed_objects: Vec<ObjectRefRecord>,
    pub validations: Vec<CandidateValidationRecord>,
    pub candidate_counts: Vec<CandidateCountRecord>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub vector_indexing_failure: Option<VectorIndexingFailureRecord>,
    pub stats_update_status: StatsUpdateStatusRecord,
    pub repair_needed: Vec<RepairMarkerRecord>,
}

impl WriteOutcomeRecord {
    pub fn clean(operation_id: impl Into<String>, operation: WriteOperationKind) -> Self {
        Self {
            operation_id: operation_id.into(),
            operation,
            persisted_objects: Vec::new(),
            persisted_link_internal_ids: Vec::new(),
            vector_indexed_objects: Vec::new(),
            validations: Vec::new(),
            candidate_counts: Vec::new(),
            vector_indexing_failure: None,
            stats_update_status: StatsUpdateStatusRecord::default(),
            repair_needed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriteResult<T> {
    pub value: T,
    pub outcome: WriteOutcomeRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleOperationKind {
    Correct,
    Forget,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VectorMaintenanceOperation {
    Delete,
    Upsert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VectorMaintenanceFailureItemRecord {
    pub operation: VectorMaintenanceOperation,
    pub objects: Vec<ObjectRefRecord>,
    pub cause: VectorIndexingCauseRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupersessionRecord {
    pub superseded_internal_id: String,
    pub superseded_by_internal_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleWarningReason {
    CascadeSuppressesCurrentReplacement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LifecycleWarningRecord {
    pub reason: LifecycleWarningReason,
    pub affected_internal_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LifecycleOutcomeRecord {
    pub operation_id: String,
    pub operation: LifecycleOperationKind,
    pub requested_targets: Vec<ObjectRefRecord>,
    pub graph_mutated_objects: Vec<ObjectRefRecord>,
    pub graph_mutated_link_internal_ids: Vec<String>,
    pub vector_maintained_objects: Vec<ObjectRefRecord>,
    pub vector_maintenance_failures: Vec<VectorMaintenanceFailureItemRecord>,
    pub stats_update_status: StatsUpdateStatusRecord,
    pub superseded: Vec<SupersessionRecord>,
    pub warnings: Vec<LifecycleWarningRecord>,
}

impl LifecycleOutcomeRecord {
    pub fn clean(operation_id: impl Into<String>, operation: LifecycleOperationKind) -> Self {
        Self {
            operation_id: operation_id.into(),
            operation,
            requested_targets: Vec::new(),
            graph_mutated_objects: Vec::new(),
            graph_mutated_link_internal_ids: Vec::new(),
            vector_maintained_objects: Vec::new(),
            vector_maintenance_failures: Vec::new(),
            stats_update_status: StatsUpdateStatusRecord::default(),
            superseded: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DegradationSummary {
    pub any_degradation: bool,
}

pub fn deterministic_operation_id<'a>(
    namespace: &str,
    operation: &str,
    identity_parts: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut digest = Sha256::new();
    for part in [namespace, operation] {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    for part in identity_parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    let encoded = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{operation}:{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_warning_reason_matches_exhaustive_wire_tokens() {
        let reason = LifecycleWarningReason::CascadeSuppressesCurrentReplacement;
        let expected = match reason {
            LifecycleWarningReason::CascadeSuppressesCurrentReplacement => {
                serde_json::json!("cascade_suppresses_current_replacement")
            }
        };
        assert_eq!(serde_json::to_value(&reason).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<LifecycleWarningReason>(expected).unwrap(),
            reason
        );
    }

    #[test]
    fn vector_database_error_kind_matches_the_character_memory_wire_shape() {
        let cases = vec![
            (
                VectorDatabaseErrorKind::Response,
                serde_json::json!({ "kind": "response" }),
            ),
            (
                VectorDatabaseErrorKind::ResourceExhausted,
                serde_json::json!({ "kind": "resource_exhausted" }),
            ),
            (
                VectorDatabaseErrorKind::Conversion,
                serde_json::json!({ "kind": "conversion" }),
            ),
            (
                VectorDatabaseErrorKind::InvalidUri,
                serde_json::json!({ "kind": "invalid_uri" }),
            ),
            (
                VectorDatabaseErrorKind::NoSnapshotFound,
                serde_json::json!({ "kind": "no_snapshot_found" }),
            ),
            (
                VectorDatabaseErrorKind::Io {
                    io_kind: IoErrorKindRecord::PermissionDenied,
                },
                serde_json::json!({
                    "kind": "io",
                    "io_kind": { "kind": "permission_denied" }
                }),
            ),
            (
                VectorDatabaseErrorKind::HttpTimeout,
                serde_json::json!({ "kind": "http_timeout" }),
            ),
            (
                VectorDatabaseErrorKind::HttpConnect,
                serde_json::json!({ "kind": "http_connect" }),
            ),
            (
                VectorDatabaseErrorKind::HttpStatus,
                serde_json::json!({ "kind": "http_status" }),
            ),
            (
                VectorDatabaseErrorKind::Http,
                serde_json::json!({ "kind": "http" }),
            ),
            (
                VectorDatabaseErrorKind::JsonToPayload,
                serde_json::json!({ "kind": "json_to_payload" }),
            ),
            (
                VectorDatabaseErrorKind::PayloadDeserialization,
                serde_json::json!({ "kind": "payload_deserialization" }),
            ),
        ];

        for (kind, expected) in cases {
            assert_eq!(serde_json::to_value(&kind).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<VectorDatabaseErrorKind>(expected).unwrap(),
                kind
            );
        }
    }

    #[test]
    fn vector_database_error_kind_rejects_unknown_outer_fields() {
        for value in [
            serde_json::json!({
                "kind": "response",
                "unexpected_unit_field": true
            }),
            serde_json::json!({
                "kind": "io",
                "io_kind": { "kind": "permission_denied" },
                "unexpected_io_field": true
            }),
        ] {
            let error = serde_json::from_value::<VectorDatabaseErrorKind>(value).unwrap_err();
            assert!(
                error.to_string().contains("unexpected_"),
                "unknown outer field should be named in the error: {error}"
            );
        }
    }

    #[test]
    fn vector_database_error_kind_rejects_duplicate_fields_before_overwrite() {
        for (json, field) in [
            (r#"{"kind":"response","kind":"response"}"#, "kind"),
            (r#"{"kind":"response","kind":"http"}"#, "kind"),
            (
                r#"{"kind":"io","io_kind":{"kind":"permission_denied"},"io_kind":{"kind":"not_found"}}"#,
                "io_kind",
            ),
            (
                r#"{"kind":"io","io_kind":{"kind":"permission_denied","kind":"not_found"}}"#,
                "kind",
            ),
        ] {
            let error = serde_json::from_str::<VectorDatabaseErrorKind>(json).unwrap_err();
            let error = error.to_string();
            assert!(
                error.contains("duplicate field") && error.contains(field),
                "duplicate {field} should be rejected and named: {error}"
            );
        }
    }

    #[test]
    fn every_io_error_kind_record_round_trips_through_serde() {
        let kinds = vec![
            IoErrorKindRecord::NotFound,
            IoErrorKindRecord::PermissionDenied,
            IoErrorKindRecord::ConnectionRefused,
            IoErrorKindRecord::ConnectionReset,
            IoErrorKindRecord::HostUnreachable,
            IoErrorKindRecord::NetworkUnreachable,
            IoErrorKindRecord::ConnectionAborted,
            IoErrorKindRecord::NotConnected,
            IoErrorKindRecord::AddrInUse,
            IoErrorKindRecord::AddrNotAvailable,
            IoErrorKindRecord::NetworkDown,
            IoErrorKindRecord::BrokenPipe,
            IoErrorKindRecord::AlreadyExists,
            IoErrorKindRecord::WouldBlock,
            IoErrorKindRecord::NotADirectory,
            IoErrorKindRecord::IsADirectory,
            IoErrorKindRecord::DirectoryNotEmpty,
            IoErrorKindRecord::ReadOnlyFilesystem,
            IoErrorKindRecord::StaleNetworkFileHandle,
            IoErrorKindRecord::InvalidInput,
            IoErrorKindRecord::InvalidData,
            IoErrorKindRecord::TimedOut,
            IoErrorKindRecord::WriteZero,
            IoErrorKindRecord::StorageFull,
            IoErrorKindRecord::NotSeekable,
            IoErrorKindRecord::QuotaExceeded,
            IoErrorKindRecord::FileTooLarge,
            IoErrorKindRecord::ResourceBusy,
            IoErrorKindRecord::ExecutableFileBusy,
            IoErrorKindRecord::Deadlock,
            IoErrorKindRecord::CrossesDevices,
            IoErrorKindRecord::TooManyLinks,
            IoErrorKindRecord::InvalidFilename,
            IoErrorKindRecord::ArgumentListTooLong,
            IoErrorKindRecord::Interrupted,
            IoErrorKindRecord::Unsupported,
            IoErrorKindRecord::UnexpectedEof,
            IoErrorKindRecord::OutOfMemory,
            IoErrorKindRecord::Other,
            IoErrorKindRecord::Unrecognized,
        ];

        for kind in kinds {
            let value = serde_json::to_value(&kind).unwrap();
            assert_eq!(
                serde_json::from_value::<IoErrorKindRecord>(value).unwrap(),
                kind
            );
        }

        assert_eq!(
            serde_json::to_value(IoErrorKindRecord::Unrecognized).unwrap(),
            serde_json::json!({ "kind": "unrecognized" })
        );
    }

    #[test]
    fn typed_stats_cause_records_round_trip_without_flattening() {
        let graph_errors = vec![
            GraphQueryErrorRecord::Selection {
                detail: "selection".into(),
            },
            GraphQueryErrorRecord::Hydration {
                detail: "hydration".into(),
            },
        ];
        for error in graph_errors {
            let value = serde_json::to_value(&error).unwrap();
            assert_eq!(
                serde_json::from_value::<GraphQueryErrorRecord>(value).unwrap(),
                error
            );
        }

        let store_errors = vec![
            RetrievalStatsStoreErrorRecord::Sqlite {
                detail: "sqlite".into(),
            },
            RetrievalStatsStoreErrorRecord::Filesystem {
                io_kind: IoErrorKindRecord::PermissionDenied,
                detail: "filesystem".into(),
            },
            RetrievalStatsStoreErrorRecord::LockPoisoned,
            RetrievalStatsStoreErrorRecord::HealthSerialization {
                detail: "serialize".into(),
            },
            RetrievalStatsStoreErrorRecord::HealthDeserialization {
                detail: "deserialize".into(),
            },
        ];
        for error in store_errors {
            let value = serde_json::to_value(&error).unwrap();
            assert_eq!(
                serde_json::from_value::<RetrievalStatsStoreErrorRecord>(value).unwrap(),
                error
            );
        }

        let health_causes = vec![
            RetrievalStatsHealthCauseRecord::StoreInitialization {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
            RetrievalStatsHealthCauseRecord::EndpointHydration {
                error: GraphQueryErrorRecord::Hydration {
                    detail: "hydrate".into(),
                },
            },
            RetrievalStatsHealthCauseRecord::EdgeWrite {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
            RetrievalStatsHealthCauseRecord::ObjectStateWrite {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
            RetrievalStatsHealthCauseRecord::HealthCheck {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
            RetrievalStatsHealthCauseRecord::CounterRead {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
            RetrievalStatsHealthCauseRecord::GlobalCounterRead {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
        ];
        for cause in health_causes {
            let value = serde_json::to_value(&cause).unwrap();
            assert_eq!(
                serde_json::from_value::<RetrievalStatsHealthCauseRecord>(value).unwrap(),
                cause
            );
        }

        let causes = vec![
            StatsUpdateCauseRecord::EndpointHydration {
                error: GraphQueryErrorRecord::Selection {
                    detail: "select".into(),
                },
            },
            StatsUpdateCauseRecord::EdgeWrite {
                error: RetrievalStatsStoreErrorRecord::Sqlite {
                    detail: "edge".into(),
                },
            },
            StatsUpdateCauseRecord::ObjectStateWrite {
                error: RetrievalStatsStoreErrorRecord::Filesystem {
                    io_kind: IoErrorKindRecord::TimedOut,
                    detail: "state".into(),
                },
            },
            StatsUpdateCauseRecord::HealthCheck {
                error: RetrievalStatsStoreErrorRecord::LockPoisoned,
            },
            StatsUpdateCauseRecord::HealthMark {
                error: RetrievalStatsStoreErrorRecord::HealthSerialization {
                    detail: "health".into(),
                },
            },
            StatsUpdateCauseRecord::StoreUnhealthy {
                health_cause: Some(RetrievalStatsHealthCauseRecord::GlobalCounterRead {
                    error: RetrievalStatsStoreErrorRecord::HealthDeserialization {
                        detail: "counter".into(),
                    },
                }),
            },
        ];
        let value = serde_json::to_value(&causes).unwrap();
        assert_eq!(value[0]["cause"], "endpoint_hydration");
        assert_eq!(value[0]["error"]["kind"], "selection");
        assert_eq!(value[5]["health_cause"]["operation"], "global_counter_read");
        assert_eq!(
            serde_json::from_value::<Vec<StatsUpdateCauseRecord>>(value).unwrap(),
            causes
        );
        assert!(
            serde_json::from_value::<StatsUpdateCauseRecord>(serde_json::json!({
                "cause": "health_check",
                "error": { "kind": "lock_poisoned" },
                "unexpected_v2_field": true
            }))
            .unwrap_err()
            .to_string()
            .contains("unknown field")
        );
    }
}
