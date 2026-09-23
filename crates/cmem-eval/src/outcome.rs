use serde::{Deserialize, Serialize};

pub use character_memory::{
    BeliefPredicate, CandidateValidation, CandidateValidationStatus, DerivedType,
    FanoutUtilizationTrace, LifecycleMutationOutcome, LinkOutcome, MemoryCandidateKind, ObjectType,
    RelationType, RememberOutcome, RepairMarker, RetentionState, RetrievalTrace, RetrieveOutcome,
    SelectivityTrace, ThreadStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WriteResult<T, O = RememberOutcome> {
    pub value: T,
    pub outcome: O,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DegradationSummary {
    pub any_degradation: bool,
}
