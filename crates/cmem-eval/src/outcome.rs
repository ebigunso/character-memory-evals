use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use character_memory::{
    CandidateValidation, CandidateValidationStatus, DerivedType, EntityType,
    FanoutUtilizationTrace, LifecycleMutationOutcome, LinkOutcome, MemoryCandidateKind, ObjectType,
    RationaleCategory, RelationType, RememberOutcome, RepairMarker, RetentionState, RetrievalTrace,
    RetrieveOutcome, SelectivityTrace, Stability, ThreadStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordedOutcome<T> {
    pub operation_id: String,
    pub outcome: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WriteResult<T, O = RememberOutcome> {
    pub value: T,
    pub outcome: RecordedOutcome<O>,
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
