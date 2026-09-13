use crate::config::RetrievalMode;
use crate::{
    DerivedType, EntityType, ObjectType, RelationType, RepairMarker, RetentionState,
    RetrievalSurfacePolicy, Stability, ThreadStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInput {
    pub external_id: String,
    pub namespace: String,
    pub summary: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub participants: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationInput {
    pub external_id: String,
    pub episode_external_id: String,
    pub namespace: String,
    pub speaker: Option<String>,
    pub text: String,
    pub observed_at: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphEnrichmentInput {
    pub namespace: String,
    #[serde(default)]
    pub entities: Vec<EntityInput>,
    #[serde(default)]
    pub threads: Vec<MemoryThreadInput>,
    #[serde(default)]
    pub derived_memories: Vec<DerivedMemoryInput>,
    #[serde(default)]
    pub links: Vec<MemoryLinkInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshotInput {
    pub snapshot_id: String,
    pub namespace: String,
    pub dataset_item_id: String,
    pub cutoff: SnapshotCutoff,
    pub graph: GraphEnrichmentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCutoff {
    #[serde(rename = "type")]
    pub cutoff_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInput {
    pub external_id: String,
    pub entity_type: EntityType,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub canonical_key: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryThreadInput {
    pub external_id: String,
    pub title: String,
    pub summary: String,
    #[serde(default = "default_thread_status")]
    pub status: ThreadStatus,
    pub last_touched_at: Option<String>,
    #[serde(default = "default_salience_score")]
    pub salience_score: f32,
    pub canonical_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DerivedMemoryInput {
    pub external_id: String,
    pub derived_type: DerivedType,
    pub text: String,
    #[serde(default)]
    pub source_episode_external_ids: Vec<String>,
    #[serde(default)]
    pub source_observation_external_ids: Vec<String>,
    #[serde(default)]
    pub thread_external_ids: Vec<String>,
    #[serde(default)]
    pub entity_external_ids: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default = "default_salience_score")]
    pub salience_score: f32,
    #[serde(default = "default_stability")]
    pub stability: Stability,
    #[serde(default = "default_true")]
    pub is_current: bool,
    #[serde(default)]
    pub supersedes_external_ids: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryLinkInput {
    pub external_id: String,
    pub from: MemoryEndpointInput,
    pub relation: RelationType,
    pub to: MemoryEndpointInput,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEndpointInput {
    pub object_type: ObjectType,
    pub external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveInput {
    pub mode: RetrievalMode,
    pub namespace: String,
    pub query: String,
    pub query_date: Option<String>,
    pub surface_policy: RetrievalSurfacePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RetrievedItem {
    pub kind: ObjectType,
    pub internal_id: String,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub external_id: Option<String>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub episode_external_id: Option<String>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub score: Option<f64>,
    pub rank: usize,
    pub rationale: Vec<String>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RetrievedContextPack {
    items: Vec<RetrievedItem>,
    context_text: String,
    context_char_count: usize,
    context_word_count: usize,
    outcomes: Vec<crate::RetrieveOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextRenderer {
    PlainText,
    WithIdentity,
}

impl RetrievedContextPack {
    pub fn from_ranked_items(
        items: Vec<RetrievedItem>,
        outcomes: Vec<crate::RetrieveOutcome>,
        renderer: ContextRenderer,
    ) -> Self {
        let context_text = items
            .iter()
            .filter_map(|item| {
                item.text.as_ref().map(|text| match renderer {
                    ContextRenderer::PlainText => text.clone(),
                    ContextRenderer::WithIdentity => format!(
                        "[{}:{} rank={}] {}",
                        item.kind,
                        item.external_id.as_deref().unwrap_or("unknown"),
                        item.rank,
                        text
                    ),
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            context_char_count: context_text.chars().count(),
            context_word_count: context_text.split_whitespace().count(),
            items,
            context_text,
            outcomes,
        }
    }

    pub fn items(&self) -> &[RetrievedItem] {
        &self.items
    }
    pub fn context_text(&self) -> &str {
        &self.context_text
    }
    pub fn context_char_count(&self) -> usize {
        self.context_char_count
    }
    pub fn context_word_count(&self) -> usize {
        self.context_word_count
    }
    pub fn outcomes(&self) -> &[crate::RetrieveOutcome] {
        &self.outcomes
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<RetrievedItem>,
        String,
        usize,
        usize,
        Vec<crate::RetrieveOutcome>,
    ) {
        (
            self.items,
            self.context_text,
            self.context_char_count,
            self.context_word_count,
            self.outcomes,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IngestedObjectRefs {
    pub episode_internal_ids: Vec<String>,
    pub observation_internal_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievedExternalRef {
    pub kind: ObjectType,
    pub external_id: Option<String>,
    pub episode_external_id: Option<String>,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinkMemoryInput {
    pub namespace: String,
    pub link: MemoryLinkInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkMemoryResult {
    pub internal_id: String,
    pub external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SourceProvenanceInput {
    #[serde(default)]
    pub episode_external_ids: Vec<String>,
    #[serde(default)]
    pub observation_external_ids: Vec<String>,
    #[serde(default)]
    pub external_refs: Vec<ExternalSourceRefInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalSourceRefInput {
    pub source_ref: Option<String>,
    pub raw_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CorrectionTargetInput {
    DerivedMemory {
        external_id: String,
    },
    SourceObject {
        object_type: ObjectType,
        external_id: String,
        original_raw_ref: Option<String>,
        original_source_ref: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplacementDerivedMemoryInput {
    pub memory: DerivedMemoryInput,
    pub original_source_provenance: SourceProvenanceInput,
    pub correction_origin_provenance: SourceProvenanceInput,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrectionLifecyclePolicyInput {
    pub supersede_replaced_derived_memories: bool,
    pub suppress_superseded_derived_memories: bool,
    pub retain_original_source_objects: bool,
}

impl Default for CorrectionLifecyclePolicyInput {
    fn default() -> Self {
        Self {
            supersede_replaced_derived_memories: true,
            suppress_superseded_derived_memories: true,
            retain_original_source_objects: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrectionCascadePolicyInput {
    pub apply_to_provenanced_derived_memories: bool,
    pub require_original_source_match: bool,
    pub cascade_to_threads: bool,
}

impl Default for CorrectionCascadePolicyInput {
    fn default() -> Self {
        Self {
            apply_to_provenanced_derived_memories: true,
            require_original_source_match: true,
            cascade_to_threads: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrectMemoryInput {
    pub namespace: String,
    pub targets: Vec<CorrectionTargetInput>,
    #[serde(default)]
    pub replacements: Vec<ReplacementDerivedMemoryInput>,
    #[serde(default)]
    pub superseded_derived_memory_external_ids: Vec<String>,
    pub correction_origin: SourceProvenanceInput,
    pub rationale: String,
    #[serde(default)]
    pub lifecycle_policy: CorrectionLifecyclePolicyInput,
    #[serde(default)]
    pub cascade_policy: CorrectionCascadePolicyInput,
    #[serde(default)]
    pub include_trace: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuppressionPolicyInput {
    pub suppress_target: bool,
    pub suppress_derived_from_target: bool,
    pub preserve_original_raw_refs: bool,
}

impl Default for SuppressionPolicyInput {
    fn default() -> Self {
        Self {
            suppress_target: true,
            suppress_derived_from_target: true,
            preserve_original_raw_refs: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchivePolicyInput {
    pub archive_thread: bool,
    pub archive_thread_derived_memories: bool,
    pub preserve_original_raw_refs: bool,
}

impl Default for ArchivePolicyInput {
    fn default() -> Self {
        Self {
            archive_thread: true,
            archive_thread_derived_memories: false,
            preserve_original_raw_refs: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgetCascadePolicyInput {
    pub apply_to_derived_from_target: bool,
    pub apply_to_thread_members: bool,
}

impl Default for ForgetCascadePolicyInput {
    fn default() -> Self {
        Self {
            apply_to_derived_from_target: true,
            apply_to_thread_members: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgetMemoryInput {
    pub namespace: String,
    pub targets: Vec<MemoryEndpointInput>,
    pub rationale: String,
    #[serde(default)]
    pub suppression_policy: SuppressionPolicyInput,
    #[serde(default)]
    pub archive_policy: ArchivePolicyInput,
    #[serde(default)]
    pub cascade_policy: ForgetCascadePolicyInput,
    #[serde(default = "default_suppressed_retention_state")]
    pub target_retention_state: RetentionState,
    pub target_thread_status: Option<ThreadStatus>,
    #[serde(default)]
    pub include_trace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LifecycleMutationResult {
    pub mutated_object_refs: Vec<MemoryEndpointInput>,
    pub mutated_link_external_ids: Vec<String>,
    pub vector_maintained_object_refs: Vec<MemoryEndpointInput>,
    pub superseded: Vec<SupersessionResult>,
    pub outcome: crate::RecordedOutcome<crate::LifecycleMutationOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupersessionResult {
    pub superseded_external_id: String,
    pub superseded_by_external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrepareWriteInput {
    pub namespace: String,
    pub content: String,
    pub episode_external_id: String,
    pub observation_external_id: String,
    #[serde(default)]
    pub episode_started_at: Option<String>,
    #[serde(default)]
    pub observation_observed_at: Option<String>,
    #[serde(default)]
    pub raw_refs: Vec<String>,
    pub idempotency_key: Option<String>,
    #[serde(default = "default_true")]
    pub include_vector_index_candidates: bool,
    #[serde(default = "default_true")]
    pub include_stats_update_candidates: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreparedWritePlan {
    pub namespace: String,
    pub input: PrepareWriteInput,
    pub plan: character_memory::RememberWritePlan,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitWriteOptions {
    pub update_vectors: bool,
    pub update_stats: bool,
}

impl Default for CommitWriteOptions {
    fn default() -> Self {
        Self {
            update_vectors: true,
            update_stats: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitWriteResult {
    pub persisted_object_refs: Vec<MemoryEndpointInput>,
    pub persisted_link_external_ids: Vec<String>,
    pub vector_indexed_object_refs: Vec<MemoryEndpointInput>,
    pub repair_needed: Vec<RepairMarker>,
    pub outcome: crate::RecordedOutcome<crate::RememberOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NamespaceLifecycleResult {
    pub namespace: String,
    pub restored_identity_count: usize,
}

fn default_thread_status() -> ThreadStatus {
    ThreadStatus::Active
}

fn default_stability() -> Stability {
    Stability::Medium
}

fn default_confidence() -> f32 {
    1.0
}

fn default_salience_score() -> f32 {
    0.5
}

fn default_true() -> bool {
    true
}

fn default_suppressed_retention_state() -> RetentionState {
    RetentionState::Suppressed
}
