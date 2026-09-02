use crate::config::RetrievalMode;
use crate::{
    CandidateProducerKind, CandidateValidationIssueRecord, CandidateValidationStatus,
    ContextPackSection, DerivedType, EntityType, GraphExpansionBoundedReason, GraphFailureMode,
    LifecycleFilterReason, LifecycleOperationKind, LifecycleOutcomeRecord, MemoryCandidateKind,
    ObjectRefRecord, ObjectType, RationaleOrigin, RelationType, RepairMarkerRecord, RetentionState,
    RetrievalSectionBudgets, RetrievalSurfacePolicy, SelectivityCountScope, SelectivityDecision,
    Stability, StaleCandidateReason, SupersessionRecord, ThreadStatus, WriteOperationKind,
    WriteOutcomeRecord, WriteResult, deterministic_operation_id,
};
use anyhow::{Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

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
    telemetry: RetrievalTelemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextRenderer {
    PlainText,
    WithIdentity,
}

impl RetrievedContextPack {
    pub fn from_ranked_items(
        items: Vec<RetrievedItem>,
        telemetry: RetrievalTelemetry,
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
            telemetry,
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
    pub fn telemetry(&self) -> &RetrievalTelemetry {
        &self.telemetry
    }

    pub fn into_parts(self) -> (Vec<RetrievedItem>, String, usize, usize, RetrievalTelemetry) {
        (
            self.items,
            self.context_text,
            self.context_char_count,
            self.context_word_count,
            self.telemetry,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RetrievalTelemetry {
    pub trace_available: bool,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub vector_candidate_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub configured_candidate_limits: Option<ConfiguredCandidateLimits>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub configured_graph_limits: Option<ConfiguredGraphLimits>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub configured_section_limits: Option<RetrievalSectionBudgets>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub configured_object_types: Option<Vec<ObjectType>>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub configured_lifecycle_policy: Option<ConfiguredLifecyclePolicy>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub query_embedding_dimension: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub returned_vector_candidate_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub unique_graph_root_candidate_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub selected_graph_root_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub graph_root_omission_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub graph_relation_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub graph_expansion: Option<GraphExpansionSummary>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub selectivity_summary: Option<SelectivitySummary>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub section_pressure: Option<Vec<SectionPressureSummary>>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub graph_verified_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub stale_candidate_omission_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub lifecycle_omission_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub lifecycle_filter_decision_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub suppressed_or_deleted_returned_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub superseded_current_returned_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub unsafe_lifecycle_returned_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub graph_object_missing_omitted_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub graph_object_missing_returned_count: Option<usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub section_assignment_count: Option<usize>,
    pub section_assignment_counts: BTreeMap<ContextPackSection, usize>,
    pub stale_candidate_omission_reasons: BTreeMap<StaleCandidateReason, usize>,
    pub lifecycle_omission_reasons: BTreeMap<LifecycleFilterReason, usize>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub fanout_utilization: Option<Vec<RetrievalFanoutUtilization>>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub selectivity_decisions: Option<Vec<RetrievalSelectivityDecision>>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub rationale_categories_by_internal_id:
        Option<BTreeMap<String, Vec<RetrievalRationaleCategory>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredCandidateLimits {
    pub max_vector_candidates: usize,
    pub max_graph_roots: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredGraphLimits {
    pub max_depth: u8,
    pub max_nodes: usize,
    pub max_fanout_per_node: usize,
    pub max_hub_edges: usize,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub timeout_ms: Option<u64>,
    pub failure_mode: GraphFailureMode,
    pub allowed_relation_types: Vec<RelationType>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredLifecyclePolicy {
    pub include_archived: bool,
    pub include_suppressed: bool,
    pub include_deleted: bool,
    pub include_non_current: bool,
    pub include_superseded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct GraphExpansionSummary {
    pub attempted_root_count: usize,
    pub expanded_root_count: usize,
    pub missing_root_count: usize,
    pub expanded_object_count: usize,
    pub expanded_relation_count: usize,
    pub filtered_node_count: usize,
    pub bounded_failure_count: usize,
    pub bounded_failure_reasons: BTreeMap<GraphExpansionBoundedReason, usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SelectivitySummary {
    pub decision_count: usize,
    pub high_selectivity_count: usize,
    pub low_selectivity_supported_count: usize,
    pub low_selectivity_rejected_count: usize,
    pub fallback_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SectionPressureSummary {
    pub section: ContextPackSection,
    pub limit: usize,
    pub included_count: usize,
    pub omitted_by_limit_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetrievalFanoutUtilization {
    pub root_internal_id: String,
    pub root_object_type: ObjectType,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub root_external_id: Option<String>,
    pub relation: RelationType,
    pub object_type: ObjectType,
    pub configured_cap: usize,
    pub selected_cap: usize,
    pub retained_count: usize,
    pub omitted_by_fanout_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RetrievalSelectivityDecision {
    pub root_internal_id: String,
    pub root_object_type: ObjectType,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub root_external_id: Option<String>,
    pub relation: RelationType,
    pub object_type: ObjectType,
    pub count_scope: SelectivityCountScope,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub score: Option<f64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub entity_count: Option<u64>,
    #[serde(deserialize_with = "crate::serde_contract::required_option")]
    pub global_count: Option<u64>,
    pub support_factor: f64,
    pub chosen_fanout: usize,
    pub max_fanout: usize,
    pub decision: SelectivityDecision,
    pub fallback: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalRationaleCategory {
    Semantic,
    Entity,
    Thread,
    Temporal,
    Salience,
    Scope,
    Lifecycle,
    GraphBound,
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
    pub outcome: LifecycleOutcomeRecord,
}

impl LifecycleMutationResult {
    pub fn clean(operation_id: impl Into<String>, operation: LifecycleOperationKind) -> Self {
        Self {
            mutated_object_refs: Vec::new(),
            mutated_link_external_ids: Vec::new(),
            vector_maintained_object_refs: Vec::new(),
            superseded: Vec::new(),
            outcome: LifecycleOutcomeRecord::clean(operation_id, operation),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedCandidate {
    pub kind: MemoryCandidateKind,
    pub internal_id: String,
    pub external_id: Option<String>,
    pub producer_kind: CandidateProducerKind,
    pub rationale_origin: RationaleOrigin,
    pub rationale: Option<String>,
    #[serde(default)]
    pub source: SourceProvenanceInput,
}

pub type CandidateValidationResult = crate::CandidateValidationRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreparedWritePlan {
    pub namespace: String,
    pub operation_internal_id: String,
    pub idempotency_key: String,
    pub input: PrepareWriteInput,
    pub candidates: Vec<PreparedCandidate>,
    #[serde(default)]
    pub validations: Vec<CandidateValidationResult>,
    #[serde(default)]
    pub backend_plan: serde_json::Value,
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
    pub repair_needed: Vec<RepairMarkerRecord>,
    pub outcome: WriteOutcomeRecord,
}

impl CommitWriteResult {
    pub fn clean(operation_id: impl Into<String>) -> Self {
        Self {
            persisted_object_refs: Vec::new(),
            persisted_link_external_ids: Vec::new(),
            vector_indexed_object_refs: Vec::new(),
            repair_needed: Vec::new(),
            outcome: WriteOutcomeRecord::clean(operation_id, WriteOperationKind::ExplicitCommit),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NamespaceLifecycleResult {
    pub namespace: String,
    pub restored_identity_count: usize,
}

#[async_trait]
pub trait MemoryAdapter: Send + Sync {
    /// Open a fresh namespace. Implementations must not silently attach to
    /// durable state that already exists for the same namespace identity.
    async fn open_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult>;
    /// Reconstruct a namespace against its durable stores and primary identity
    /// registry. Missing durable lifecycle state is an error.
    async fn reattach_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult>;
    /// Remove durable state before opening a fresh namespace identity. This is
    /// distinct from optional post-run cleanup policy.
    async fn reset_namespace(&self, namespace: &str) -> Result<()>;
    /// Apply optional post-run cleanup after result artifacts are durable.
    async fn cleanup_namespace(&self, namespace: &str) -> Result<()> {
        self.reset_namespace(namespace).await
    }
    async fn remember_episode(&self, input: EpisodeInput) -> Result<WriteResult<String>>;
    async fn remember_episodes(
        &self,
        inputs: Vec<EpisodeInput>,
    ) -> Result<WriteResult<Vec<String>>>;
    async fn remember_observation(&self, input: ObservationInput) -> Result<WriteResult<String>>;
    async fn remember_observations(
        &self,
        inputs: Vec<ObservationInput>,
    ) -> Result<WriteResult<Vec<String>>>;
    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<WriteOutcomeRecord>;
    async fn link(&self, input: LinkMemoryInput) -> Result<WriteResult<LinkMemoryResult>>;
    async fn correct(&self, input: CorrectMemoryInput) -> Result<LifecycleMutationResult>;
    async fn forget(&self, input: ForgetMemoryInput) -> Result<LifecycleMutationResult>;
    async fn prepare(&self, input: PrepareWriteInput) -> Result<PreparedWritePlan>;
    async fn validate_plan(
        &self,
        plan: &PreparedWritePlan,
    ) -> Result<Vec<CandidateValidationResult>>;
    async fn commit(
        &self,
        plan: PreparedWritePlan,
        options: CommitWriteOptions,
    ) -> Result<CommitWriteResult>;
    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack>;
}

#[derive(Debug, Clone, Default)]
pub struct MockMemoryAdapter {
    state: Arc<Mutex<HashMap<String, NamespaceState>>>,
}

#[derive(Debug, Clone, Default)]
struct NamespaceState {
    episodes: Vec<EpisodeInput>,
    observations: Vec<ObservationInput>,
    derived_memories: Vec<DerivedMemoryInput>,
    links: Vec<MemoryLinkInput>,
    suppressed_derived_memory_ids: HashSet<String>,
}

#[async_trait]
impl MemoryAdapter for MockMemoryAdapter {
    async fn open_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        if state.contains_key(namespace) {
            bail!("namespace is already open: {namespace}");
        }
        state.insert(namespace.to_string(), NamespaceState::default());
        Ok(NamespaceLifecycleResult {
            namespace: namespace.to_string(),
            restored_identity_count: 0,
        })
    }

    async fn reattach_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
        let state = self.state.lock().expect("mock memory mutex poisoned");
        let restored_identity_count = state
            .get(namespace)
            .map(|namespace| {
                namespace.episodes.len()
                    + namespace.observations.len()
                    + namespace.derived_memories.len()
                    + namespace.links.len()
            })
            .ok_or_else(|| anyhow::anyhow!("namespace does not exist: {namespace}"))?;
        Ok(NamespaceLifecycleResult {
            namespace: namespace.to_string(),
            restored_identity_count,
        })
    }

    async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state.remove(namespace);
        Ok(())
    }

    async fn remember_episode(&self, input: EpisodeInput) -> Result<WriteResult<String>> {
        let result = self.remember_episodes(vec![input]).await?;
        let [internal_id]: [String; 1] = result
            .value
            .try_into()
            .expect("single-item episode batch returned one id");
        Ok(WriteResult {
            value: internal_id,
            outcome: result.outcome,
        })
    }

    async fn remember_episodes(
        &self,
        inputs: Vec<EpisodeInput>,
    ) -> Result<WriteResult<Vec<String>>> {
        let namespace = inputs
            .first()
            .map(|input| input.namespace.clone())
            .ok_or_else(|| anyhow::anyhow!("episode batch must not be empty"))?;
        if inputs.iter().any(|input| input.namespace != namespace) {
            bail!("episode batch spans multiple namespaces");
        }
        let ids = inputs
            .iter()
            .map(|input| format!("mock:episode:{}", input.external_id))
            .collect::<Vec<_>>();
        let operation_id =
            deterministic_operation_id(&namespace, "remember_plan", ids.iter().map(String::as_str));
        let objects = inputs
            .iter()
            .zip(&ids)
            .map(|(input, internal_id)| ObjectRefRecord {
                object_type: ObjectType::Episode,
                internal_id: internal_id.clone(),
                external_id: Some(input.external_id.clone()),
            })
            .collect::<Vec<_>>();
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace_state = state.entry(namespace).or_default();
        namespace_state.episodes.extend(inputs);
        let mut outcome = WriteOutcomeRecord::clean(operation_id, WriteOperationKind::TypedIngest);
        outcome.persisted_objects = objects.clone();
        outcome.vector_indexed_objects = objects;
        Ok(WriteResult {
            value: ids,
            outcome,
        })
    }

    async fn remember_observation(&self, input: ObservationInput) -> Result<WriteResult<String>> {
        let result = self.remember_observations(vec![input]).await?;
        let [internal_id]: [String; 1] = result
            .value
            .try_into()
            .expect("single-item observation batch returned one id");
        Ok(WriteResult {
            value: internal_id,
            outcome: result.outcome,
        })
    }

    async fn remember_observations(
        &self,
        inputs: Vec<ObservationInput>,
    ) -> Result<WriteResult<Vec<String>>> {
        let namespace = inputs
            .first()
            .map(|input| input.namespace.clone())
            .ok_or_else(|| anyhow::anyhow!("observation batch must not be empty"))?;
        if inputs.iter().any(|input| input.namespace != namespace) {
            bail!("observation batch spans multiple namespaces");
        }
        let ids = inputs
            .iter()
            .map(|input| format!("mock:observation:{}", input.external_id))
            .collect::<Vec<_>>();
        let operation_id =
            deterministic_operation_id(&namespace, "remember_plan", ids.iter().map(String::as_str));
        let objects = inputs
            .iter()
            .zip(&ids)
            .map(|(input, internal_id)| ObjectRefRecord {
                object_type: ObjectType::Observation,
                internal_id: internal_id.clone(),
                external_id: Some(input.external_id.clone()),
            })
            .collect::<Vec<_>>();
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace_state = state.entry(namespace).or_default();
        namespace_state.observations.extend(inputs);
        let mut outcome = WriteOutcomeRecord::clean(operation_id, WriteOperationKind::TypedIngest);
        outcome.persisted_objects = objects.clone();
        outcome.vector_indexed_objects = objects;
        Ok(WriteResult {
            value: ids,
            outcome,
        })
    }

    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<WriteOutcomeRecord> {
        let identity = input
            .entities
            .iter()
            .map(|item| item.external_id.as_str())
            .chain(input.threads.iter().map(|item| item.external_id.as_str()))
            .chain(
                input
                    .derived_memories
                    .iter()
                    .map(|item| item.external_id.as_str()),
            )
            .chain(input.links.iter().map(|item| item.external_id.as_str()))
            .collect::<Vec<_>>();
        let operation_id =
            deterministic_operation_id(&input.namespace, "remember_enrichment", identity);
        let persisted_objects = input
            .entities
            .iter()
            .map(|item| mock_object_ref(ObjectType::Entity, &item.external_id))
            .chain(
                input
                    .threads
                    .iter()
                    .map(|item| mock_object_ref(ObjectType::MemoryThread, &item.external_id)),
            )
            .chain(
                input
                    .derived_memories
                    .iter()
                    .map(|item| mock_object_ref(ObjectType::DerivedMemory, &item.external_id)),
            )
            .collect::<Vec<_>>();
        let persisted_link_internal_ids = input
            .links
            .iter()
            .map(|item| format!("mock:memory_link:{}", item.external_id))
            .collect();
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace).or_default();
        namespace.derived_memories.extend(input.derived_memories);
        let mut outcome = WriteOutcomeRecord::clean(operation_id, WriteOperationKind::TypedIngest);
        outcome.vector_indexed_objects = persisted_objects.clone();
        outcome.persisted_objects = persisted_objects;
        outcome.persisted_link_internal_ids = persisted_link_internal_ids;
        Ok(outcome)
    }

    async fn link(&self, input: LinkMemoryInput) -> Result<WriteResult<LinkMemoryResult>> {
        let internal_id = format!("mock:memory_link:{}", input.link.external_id);
        let operation_id =
            deterministic_operation_id(&input.namespace, "link", [input.link.external_id.as_str()]);
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state
            .entry(input.namespace)
            .or_default()
            .links
            .push(input.link.clone());
        let value = LinkMemoryResult {
            internal_id: internal_id.clone(),
            external_id: input.link.external_id,
        };
        let mut outcome = WriteOutcomeRecord::clean(operation_id, WriteOperationKind::TypedIngest);
        outcome.persisted_link_internal_ids.push(internal_id);
        Ok(WriteResult { value, outcome })
    }

    async fn correct(&self, input: CorrectMemoryInput) -> Result<LifecycleMutationResult> {
        if input.targets.is_empty() {
            bail!("correction requires at least one target");
        }
        if input.rationale.trim().is_empty() {
            bail!("correction rationale must not be empty");
        }
        if !source_provenance_has_reference(&input.correction_origin) {
            bail!("correction origin provenance is required");
        }
        for target in &input.targets {
            if let CorrectionTargetInput::SourceObject { object_type, .. } = target
                && !matches!(object_type, ObjectType::Episode | ObjectType::Observation)
            {
                bail!("unsupported correction source object type: {object_type}");
            }
        }
        for replacement in &input.replacements {
            if replacement.memory.text.trim().is_empty() {
                bail!("replacement derived memory text must not be empty");
            }
            if replacement.memory.source_episode_external_ids.is_empty()
                && replacement
                    .memory
                    .source_observation_external_ids
                    .is_empty()
            {
                bail!("replacement derived memory requires episode or observation provenance");
            }
            if !source_provenance_has_reference(&replacement.correction_origin_provenance) {
                bail!("replacement correction origin provenance is required");
            }
        }

        let operation_identity = serde_json::to_string(&input)?;
        let operation_id =
            deterministic_operation_id(&input.namespace, "correct", [operation_identity.as_str()]);

        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace.clone()).or_default();
        let mut mutated_object_refs = Vec::new();
        let mut suppressed = HashSet::new();
        for target in &input.targets {
            match target {
                CorrectionTargetInput::DerivedMemory { external_id } => {
                    suppressed.insert(external_id.clone());
                    mutated_object_refs.push(MemoryEndpointInput {
                        object_type: ObjectType::DerivedMemory,
                        external_id: external_id.clone(),
                    });
                }
                CorrectionTargetInput::SourceObject {
                    object_type,
                    external_id,
                    ..
                } => {
                    mutated_object_refs.push(MemoryEndpointInput {
                        object_type: *object_type,
                        external_id: external_id.clone(),
                    });
                    if input.cascade_policy.apply_to_provenanced_derived_memories {
                        for memory in &namespace.derived_memories {
                            let matches_source = match object_type {
                                ObjectType::Episode => memory
                                    .source_episode_external_ids
                                    .iter()
                                    .any(|id| id == external_id),
                                ObjectType::Observation => memory
                                    .source_observation_external_ids
                                    .iter()
                                    .any(|id| id == external_id),
                                _ => false,
                            };
                            if matches_source {
                                suppressed.insert(memory.external_id.clone());
                            }
                        }
                    }
                }
            }
        }
        suppressed.extend(input.superseded_derived_memory_external_ids.iter().cloned());
        if input.lifecycle_policy.suppress_superseded_derived_memories {
            namespace
                .suppressed_derived_memory_ids
                .extend(suppressed.iter().cloned());
        }

        let mut superseded = Vec::new();
        for replacement in input.replacements {
            for superseded_external_id in &replacement.memory.supersedes_external_ids {
                superseded.push(SupersessionResult {
                    superseded_external_id: superseded_external_id.clone(),
                    superseded_by_external_id: replacement.memory.external_id.clone(),
                });
            }
            namespace
                .suppressed_derived_memory_ids
                .remove(&replacement.memory.external_id);
            mutated_object_refs.push(MemoryEndpointInput {
                object_type: ObjectType::DerivedMemory,
                external_id: replacement.memory.external_id.clone(),
            });
            namespace.derived_memories.push(replacement.memory);
        }

        let mut result =
            LifecycleMutationResult::clean(operation_id, LifecycleOperationKind::Correct);
        result.mutated_object_refs = mutated_object_refs;
        result.superseded = superseded;
        result.outcome.requested_targets = input
            .targets
            .iter()
            .map(|target| match target {
                CorrectionTargetInput::DerivedMemory { external_id } => {
                    mock_object_ref(ObjectType::DerivedMemory, external_id)
                }
                CorrectionTargetInput::SourceObject {
                    object_type,
                    external_id,
                    ..
                } => mock_object_ref(*object_type, external_id),
            })
            .collect();
        result.outcome.graph_mutated_objects = result
            .mutated_object_refs
            .iter()
            .map(|reference| mock_object_ref(reference.object_type, &reference.external_id))
            .collect();
        result.outcome.vector_maintained_objects = result.outcome.graph_mutated_objects.clone();
        result.outcome.superseded = result
            .superseded
            .iter()
            .map(|record| SupersessionRecord {
                superseded_internal_id: format!(
                    "mock:derived_memory:{}",
                    record.superseded_external_id
                ),
                superseded_by_internal_id: format!(
                    "mock:derived_memory:{}",
                    record.superseded_by_external_id
                ),
            })
            .collect();
        Ok(result)
    }

    async fn forget(&self, input: ForgetMemoryInput) -> Result<LifecycleMutationResult> {
        if input.targets.is_empty() {
            bail!("forget requires at least one target");
        }
        if input.rationale.trim().is_empty() {
            bail!("forget rationale must not be empty");
        }
        for target in &input.targets {
            if !matches!(
                target.object_type,
                ObjectType::Episode
                    | ObjectType::Observation
                    | ObjectType::DerivedMemory
                    | ObjectType::MemoryThread
            ) {
                bail!(
                    "unsupported forget target object type: {}",
                    target.object_type
                );
            }
        }

        let operation_identity = serde_json::to_string(&input)?;
        let operation_id =
            deterministic_operation_id(&input.namespace, "forget", [operation_identity.as_str()]);

        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let Some(namespace) = state.get_mut(&input.namespace) else {
            return Ok(LifecycleMutationResult::clean(
                operation_id,
                LifecycleOperationKind::Forget,
            ));
        };
        for target in &input.targets {
            match target.object_type {
                ObjectType::Episode => {
                    namespace
                        .episodes
                        .retain(|episode| episode.external_id != target.external_id);
                    if input.cascade_policy.apply_to_derived_from_target {
                        namespace.derived_memories.retain(|memory| {
                            !memory
                                .source_episode_external_ids
                                .contains(&target.external_id)
                        });
                    }
                }
                ObjectType::Observation => {
                    namespace
                        .observations
                        .retain(|observation| observation.external_id != target.external_id);
                    if input.cascade_policy.apply_to_derived_from_target {
                        namespace.derived_memories.retain(|memory| {
                            !memory
                                .source_observation_external_ids
                                .contains(&target.external_id)
                        });
                    }
                }
                ObjectType::DerivedMemory => namespace
                    .derived_memories
                    .retain(|memory| memory.external_id != target.external_id),
                ObjectType::MemoryThread => {
                    if input.cascade_policy.apply_to_thread_members {
                        namespace.derived_memories.retain(|memory| {
                            !memory.thread_external_ids.contains(&target.external_id)
                        });
                    }
                }
                _ => unreachable!("forget targets were validated before mutation"),
            }
        }
        let requested_targets = input
            .targets
            .iter()
            .map(|target| mock_object_ref(target.object_type, &target.external_id))
            .collect::<Vec<_>>();
        let mut result =
            LifecycleMutationResult::clean(operation_id, LifecycleOperationKind::Forget);
        result.mutated_object_refs = input.targets;
        result.outcome.requested_targets = requested_targets.clone();
        result.outcome.graph_mutated_objects = requested_targets.clone();
        result.outcome.vector_maintained_objects = requested_targets;
        Ok(result)
    }

    async fn prepare(&self, input: PrepareWriteInput) -> Result<PreparedWritePlan> {
        let operation_internal_id = format!(
            "mock:operation:{}:{}:{}",
            input.namespace, input.episode_external_id, input.observation_external_id
        );
        let idempotency_key = input
            .idempotency_key
            .clone()
            .unwrap_or_else(|| format!("mock:remember:{operation_internal_id}"));
        let source = SourceProvenanceInput {
            external_refs: input
                .raw_refs
                .iter()
                .cloned()
                .map(|raw_ref| ExternalSourceRefInput {
                    source_ref: None,
                    raw_ref: Some(raw_ref),
                })
                .collect(),
            ..SourceProvenanceInput::default()
        };
        let episode_id = format!("mock:episode:{}", input.episode_external_id);
        let observation_id = format!("mock:observation:{}", input.observation_external_id);
        let mut candidates = vec![
            PreparedCandidate {
                kind: MemoryCandidateKind::Episode,
                internal_id: episode_id.clone(),
                external_id: Some(input.episode_external_id.clone()),
                producer_kind: CandidateProducerKind::DeterministicHelper,
                rationale_origin: RationaleOrigin::Unavailable,
                rationale: None,
                source: source.clone(),
            },
            PreparedCandidate {
                kind: MemoryCandidateKind::Observation,
                internal_id: observation_id.clone(),
                external_id: Some(input.observation_external_id.clone()),
                producer_kind: CandidateProducerKind::DeterministicHelper,
                rationale_origin: RationaleOrigin::Unavailable,
                rationale: None,
                source: SourceProvenanceInput {
                    episode_external_ids: vec![input.episode_external_id.clone()],
                    ..source
                },
            },
        ];
        if input.include_vector_index_candidates {
            for (kind, target) in [("episode", &episode_id), ("observation", &observation_id)] {
                candidates.push(PreparedCandidate {
                    kind: MemoryCandidateKind::VectorIndex,
                    internal_id: format!("mock:vector_index:{kind}:{target}"),
                    external_id: None,
                    producer_kind: CandidateProducerKind::DeterministicHelper,
                    rationale_origin: RationaleOrigin::Unavailable,
                    rationale: None,
                    source: SourceProvenanceInput::default(),
                });
            }
        }
        if input.include_stats_update_candidates {
            for (kind, target) in [("episode", &episode_id), ("observation", &observation_id)] {
                candidates.push(PreparedCandidate {
                    kind: MemoryCandidateKind::StatsUpdate,
                    internal_id: format!("mock:stats_update:{kind}:{target}"),
                    external_id: None,
                    producer_kind: CandidateProducerKind::DeterministicHelper,
                    rationale_origin: RationaleOrigin::Unavailable,
                    rationale: None,
                    source: SourceProvenanceInput::default(),
                });
            }
        }

        Ok(PreparedWritePlan {
            namespace: input.namespace.clone(),
            operation_internal_id,
            idempotency_key,
            input,
            candidates,
            validations: Vec::new(),
            backend_plan: serde_json::Value::Null,
        })
    }

    async fn validate_plan(
        &self,
        plan: &PreparedWritePlan,
    ) -> Result<Vec<CandidateValidationResult>> {
        Ok(mock_plan_validations(plan))
    }

    async fn commit(
        &self,
        plan: PreparedWritePlan,
        options: CommitWriteOptions,
    ) -> Result<CommitWriteResult> {
        let validations = mock_plan_validations(&plan);
        if validations
            .iter()
            .any(|validation| validation.status == CandidateValidationStatus::Invalid)
        {
            bail!("cannot commit invalid prepared write plan");
        }
        let episode_external_id = plan.input.episode_external_id.clone();
        let observation_external_id = plan.input.observation_external_id.clone();
        let namespace = plan.namespace.clone();
        self.remember_episode(EpisodeInput {
            external_id: episode_external_id.clone(),
            namespace: namespace.clone(),
            summary: plan.input.content.clone(),
            started_at: plan.input.episode_started_at.clone(),
            ended_at: None,
            participants: Vec::new(),
            metadata: serde_json::Value::Null,
        })
        .await?;
        self.remember_observation(ObservationInput {
            external_id: observation_external_id.clone(),
            episode_external_id: episode_external_id.clone(),
            namespace,
            speaker: None,
            text: plan.input.content,
            observed_at: plan.input.observation_observed_at,
            metadata: serde_json::Value::Null,
        })
        .await?;
        let persisted_object_refs = vec![
            MemoryEndpointInput {
                object_type: ObjectType::Episode,
                external_id: episode_external_id,
            },
            MemoryEndpointInput {
                object_type: ObjectType::Observation,
                external_id: observation_external_id,
            },
        ];
        let mut result = CommitWriteResult::clean(plan.operation_internal_id);
        result.vector_indexed_object_refs = if options.update_vectors {
            persisted_object_refs.clone()
        } else {
            Vec::new()
        };
        result.persisted_object_refs = persisted_object_refs;
        result.outcome.validations = validations;
        result.outcome.persisted_objects = result
            .persisted_object_refs
            .iter()
            .map(|reference| mock_object_ref(reference.object_type, &reference.external_id))
            .collect();
        result.outcome.vector_indexed_objects = result
            .vector_indexed_object_refs
            .iter()
            .map(|reference| mock_object_ref(reference.object_type, &reference.external_id))
            .collect();
        Ok(result)
    }

    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        let state = self.state.lock().expect("mock memory mutex poisoned");
        let Some(ns) = state.get(&input.namespace) else {
            return Ok(RetrievedContextPack::default());
        };

        let mut items = Vec::new();
        if selects_object_type(&input, ObjectType::Episode) {
            let mut episodes = ns.episodes.clone();
            episodes.sort_by(|a, b| {
                score_text(&input.query, &b.summary)
                    .partial_cmp(&score_text(&input.query, &a.summary))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for episode in episodes
                .into_iter()
                .take(input.surface_policy.sections.relevant_episodes)
            {
                let score = score_text(&input.query, &episode.summary);
                items.push(RetrievedItem {
                    kind: ObjectType::Episode,
                    internal_id: format!("mock:episode:{}", episode.external_id),
                    external_id: Some(episode.external_id),
                    episode_external_id: None,
                    score: Some(score),
                    rank: 0,
                    rationale: vec!["mock_lexical_overlap".to_string()],
                    text: Some(episode.summary),
                });
            }
        }

        if selects_object_type(&input, ObjectType::Observation) {
            let mut observations = ns.observations.clone();
            observations.sort_by(|a, b| {
                score_text(&input.query, &b.text)
                    .partial_cmp(&score_text(&input.query, &a.text))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for observation in observations
                .into_iter()
                .take(input.surface_policy.sections.salient_observations)
            {
                let score = score_text(&input.query, &observation.text);
                items.push(RetrievedItem {
                    kind: ObjectType::Observation,
                    internal_id: format!("mock:observation:{}", observation.external_id),
                    external_id: Some(observation.external_id),
                    episode_external_id: Some(observation.episode_external_id),
                    score: Some(score),
                    rank: 0,
                    rationale: vec!["mock_lexical_overlap".to_string()],
                    text: Some(observation.text),
                });
            }
        }

        if selects_object_type(&input, ObjectType::DerivedMemory) {
            let mut derived_memories = ns
                .derived_memories
                .iter()
                .filter(|memory| {
                    !ns.suppressed_derived_memory_ids
                        .contains(&memory.external_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            derived_memories.sort_by(|a, b| {
                score_text(&input.query, &b.text)
                    .partial_cmp(&score_text(&input.query, &a.text))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for memory in derived_memories
                .into_iter()
                .take(input.surface_policy.sections.derived_memories)
            {
                let score = score_text(&input.query, &memory.text);
                items.push(RetrievedItem {
                    kind: ObjectType::DerivedMemory,
                    internal_id: format!("mock:derived_memory:{}", memory.external_id),
                    external_id: Some(memory.external_id),
                    episode_external_id: memory.source_episode_external_ids.first().cloned(),
                    score: Some(score),
                    rank: 0,
                    rationale: vec!["mock_lexical_overlap".to_string()],
                    text: Some(memory.text),
                });
            }
        }

        items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.internal_id.cmp(&b.internal_id))
        });
        for (idx, item) in items.iter_mut().enumerate() {
            item.rank = idx + 1;
        }

        Ok(RetrievedContextPack::from_ranked_items(
            items,
            RetrievalTelemetry::default(),
            ContextRenderer::PlainText,
        ))
    }
}

fn selects_object_type(input: &RetrieveInput, object_type: ObjectType) -> bool {
    input.surface_policy.object_types.contains(&object_type)
}

fn source_provenance_has_reference(provenance: &SourceProvenanceInput) -> bool {
    !provenance.episode_external_ids.is_empty()
        || !provenance.observation_external_ids.is_empty()
        || provenance.external_refs.iter().any(|reference| {
            reference
                .source_ref
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
                || reference
                    .raw_ref
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
        })
}

fn mock_plan_validations(plan: &PreparedWritePlan) -> Vec<CandidateValidationResult> {
    plan.candidates
        .iter()
        .enumerate()
        .map(|(candidate_index, candidate)| {
            let mut errors = Vec::new();
            if plan.input.content.trim().is_empty()
                && candidate.kind == MemoryCandidateKind::Episode
            {
                errors.push(CandidateValidationIssueRecord::EmptyEpisodeSummary);
            }
            if candidate
                .external_id
                .as_deref()
                .is_some_and(|external_id| external_id.trim().is_empty())
            {
                errors.push(CandidateValidationIssueRecord::MissingCandidateId);
            }
            CandidateValidationResult {
                candidate_index,
                candidate_kind: candidate.kind,
                status: if errors.is_empty() {
                    CandidateValidationStatus::Valid
                } else {
                    CandidateValidationStatus::Invalid
                },
                errors,
                warnings: Vec::new(),
            }
        })
        .collect()
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

fn mock_object_ref(object_type: ObjectType, external_id: &str) -> ObjectRefRecord {
    ObjectRefRecord {
        object_type,
        internal_id: format!("mock:{}:{external_id}", object_type.as_str()),
        external_id: Some(external_id.to_string()),
    }
}

fn score_text(query: &str, text: &str) -> f64 {
    let query_words = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<std::collections::BTreeSet<_>>();
    if query_words.is_empty() {
        return 0.0;
    }
    let text_words = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<std::collections::BTreeSet<_>>();
    let hits = query_words
        .iter()
        .filter(|word| text_words.contains(*word))
        .count();
    hits as f64 / query_words.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retrieve_input(
        mode: RetrievalMode,
        query: &str,
        episode_limit: usize,
        observation_limit: usize,
        include_derived_memories: bool,
    ) -> RetrieveInput {
        let mut surface_policy = RetrievalSurfacePolicy::default();
        surface_policy.sections.relevant_episodes = episode_limit;
        surface_policy.sections.salient_observations = observation_limit;
        if !include_derived_memories {
            surface_policy
                .object_types
                .retain(|kind| *kind != ObjectType::DerivedMemory);
        }
        RetrieveInput {
            mode,
            namespace: "n".into(),
            query: query.into(),
            query_date: None,
            surface_policy,
        }
    }

    #[test]
    fn telemetry_v2_round_trip_keeps_explicit_null_graph_root_counters() {
        let encoded = serde_json::to_value(RetrievalTelemetry::default()).unwrap();
        assert_eq!(
            encoded["unique_graph_root_candidate_count"],
            serde_json::Value::Null
        );
        assert_eq!(
            encoded["selected_graph_root_count"],
            serde_json::Value::Null
        );
        assert_eq!(
            encoded["graph_root_omission_count"],
            serde_json::Value::Null
        );
        let telemetry: RetrievalTelemetry = serde_json::from_value(encoded).unwrap();
        assert_eq!(telemetry.unique_graph_root_candidate_count, None);
        assert_eq!(telemetry.selected_graph_root_count, None);
        assert_eq!(telemetry.graph_root_omission_count, None);
    }

    #[tokio::test]
    async fn mock_namespace_lifecycle_distinguishes_open_from_reattach() {
        let adapter = MockMemoryAdapter::default();
        assert!(adapter.reattach_namespace("n").await.is_err());
        assert_eq!(
            adapter
                .open_namespace("n")
                .await
                .unwrap()
                .restored_identity_count,
            0
        );
        assert!(adapter.open_namespace("n").await.is_err());
        adapter
            .remember_episode(EpisodeInput {
                external_id: "episode".into(),
                namespace: "n".into(),
                summary: "restart lifecycle".into(),
                started_at: None,
                ended_at: None,
                participants: Vec::new(),
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(
            adapter
                .reattach_namespace("n")
                .await
                .unwrap()
                .restored_identity_count,
            1
        );
    }

    #[tokio::test]
    async fn mock_adapter_preserves_external_ids() {
        let adapter = MockMemoryAdapter::default();
        adapter.reset_namespace("n").await.unwrap();
        adapter
            .remember_episode(EpisodeInput {
                external_id: "s1".into(),
                namespace: "n".into(),
                summary: "Conversation about chat native design".into(),
                started_at: None,
                ended_at: None,
                participants: vec!["user".into()],
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();
        adapter
            .remember_observation(ObservationInput {
                external_id: "s1:turn:1".into(),
                episode_external_id: "s1".into(),
                namespace: "n".into(),
                speaker: Some("user".into()),
                text: "Keep the first version chat native".into(),
                observed_at: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();

        let pack = adapter
            .retrieve(retrieve_input(
                RetrievalMode::Hybrid,
                "chat native first version",
                5,
                5,
                false,
            ))
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == ObjectType::Observation && item.external_id.as_deref() == Some("s1:turn:1")
        }));
    }

    #[tokio::test]
    async fn mock_adapter_batch_ingest_matches_single_item_retrieval() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_episodes(vec![
                EpisodeInput {
                    external_id: "s1".into(),
                    namespace: "n".into(),
                    summary: "Conversation about chat native design".into(),
                    started_at: None,
                    ended_at: None,
                    participants: vec!["user".into()],
                    metadata: serde_json::json!({}),
                },
                EpisodeInput {
                    external_id: "s2".into(),
                    namespace: "n".into(),
                    summary: "Conversation about unrelated travel".into(),
                    started_at: None,
                    ended_at: None,
                    participants: vec!["user".into()],
                    metadata: serde_json::json!({}),
                },
            ])
            .await
            .unwrap();
        adapter
            .remember_observations(vec![
                ObservationInput {
                    external_id: "s1:turn:1".into(),
                    episode_external_id: "s1".into(),
                    namespace: "n".into(),
                    speaker: Some("user".into()),
                    text: "Keep the first version chat native".into(),
                    observed_at: None,
                    metadata: serde_json::json!({}),
                },
                ObservationInput {
                    external_id: "s2:turn:1".into(),
                    episode_external_id: "s2".into(),
                    namespace: "n".into(),
                    speaker: Some("user".into()),
                    text: "Book a train ticket".into(),
                    observed_at: None,
                    metadata: serde_json::json!({}),
                },
            ])
            .await
            .unwrap();

        let pack = adapter
            .retrieve(retrieve_input(
                RetrievalMode::Hybrid,
                "chat native first version",
                5,
                5,
                false,
            ))
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == ObjectType::Observation && item.external_id.as_deref() == Some("s1:turn:1")
        }));
        assert!(pack.items.iter().any(|item| {
            item.kind == ObjectType::Episode && item.external_id.as_deref() == Some("s1")
        }));
    }

    #[tokio::test]
    async fn mock_adapter_retrieves_derived_memories_when_enabled() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![DerivedMemoryInput {
                    external_id: "dm1".into(),
                    derived_type: DerivedType::Reflection,
                    text: "The user prefers chat native design.".into(),
                    source_episode_external_ids: vec!["s1".into()],
                    source_observation_external_ids: vec![],
                    thread_external_ids: vec![],
                    entity_external_ids: vec![],
                    confidence: 1.0,
                    salience_score: 0.5,
                    stability: Stability::Medium,
                    is_current: true,
                    supersedes_external_ids: vec![],
                    metadata: serde_json::json!({}),
                }],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();

        let pack = adapter
            .retrieve(retrieve_input(
                RetrievalMode::Hybrid,
                "chat native",
                5,
                5,
                true,
            ))
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == ObjectType::DerivedMemory && item.external_id.as_deref() == Some("dm1")
        }));
    }

    #[tokio::test]
    async fn mock_adapter_honors_object_type_selection() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_episode(EpisodeInput {
                external_id: "episode-1".into(),
                namespace: "n".into(),
                summary: "Shared selector contract".into(),
                started_at: None,
                ended_at: None,
                participants: vec!["user".into()],
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();
        adapter
            .remember_observation(ObservationInput {
                external_id: "observation-1".into(),
                episode_external_id: "episode-1".into(),
                namespace: "n".into(),
                speaker: Some("user".into()),
                text: "Shared selector contract".into(),
                observed_at: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();

        for selected in [ObjectType::Episode, ObjectType::Observation] {
            let mut input = retrieve_input(
                RetrievalMode::Hybrid,
                "shared selector contract",
                5,
                5,
                false,
            );
            input.surface_policy.object_types = vec![selected];
            let pack = adapter.retrieve(input).await.unwrap();

            assert!(!pack.items.is_empty(), "selected={selected}");
            assert!(
                pack.items.iter().all(|item| item.kind == selected),
                "selected={selected} items={:?}",
                pack.items
            );
        }
    }

    #[tokio::test]
    async fn mock_staged_write_is_deterministic_validated_and_committed() {
        let adapter = MockMemoryAdapter::default();
        let input = PrepareWriteInput {
            namespace: "n".into(),
            content: "The user prefers a chat-native first version.".into(),
            episode_external_id: "s1".into(),
            observation_external_id: "s1:turn:1".into(),
            episode_started_at: Some("2025-01-02T03:04:05Z".into()),
            observation_observed_at: Some("2025-01-02T03:04:05Z".into()),
            raw_refs: vec!["raw://conversation/s1".into()],
            idempotency_key: Some("continuity-step-1".into()),
            include_vector_index_candidates: true,
            include_stats_update_candidates: true,
        };

        let first = adapter.prepare(input.clone()).await.unwrap();
        let second = adapter.prepare(input).await.unwrap();
        assert_eq!(first.operation_internal_id, second.operation_internal_id);
        assert_eq!(first.candidates, second.candidates);
        assert_eq!(
            first.candidates[0].producer_kind,
            CandidateProducerKind::DeterministicHelper
        );
        assert_eq!(
            first.candidates[0].rationale_origin,
            RationaleOrigin::Unavailable
        );
        assert!(adapter.state.lock().unwrap().get("n").is_none());

        let validations = adapter.validate_plan(&first).await.unwrap();
        assert!(
            validations
                .iter()
                .all(|validation| validation.status == CandidateValidationStatus::Valid)
        );
        let outcome = adapter
            .commit(first, CommitWriteOptions::default())
            .await
            .unwrap();
        assert_eq!(outcome.persisted_object_refs.len(), 2);
        assert_eq!(outcome.vector_indexed_object_refs.len(), 2);
        let state = adapter.state.lock().unwrap();
        let namespace = state.get("n").unwrap();
        assert_eq!(namespace.episodes[0].external_id, "s1");
        assert_eq!(namespace.observations[0].external_id, "s1:turn:1");
        assert_eq!(
            namespace.episodes[0].started_at.as_deref(),
            Some("2025-01-02T03:04:05Z")
        );
        assert_eq!(
            namespace.observations[0].observed_at.as_deref(),
            Some("2025-01-02T03:04:05Z")
        );
    }

    #[tokio::test]
    async fn mock_link_round_trips_external_id() {
        let adapter = MockMemoryAdapter::default();
        let outcome = adapter
            .link(LinkMemoryInput {
                namespace: "n".into(),
                link: MemoryLinkInput {
                    external_id: "link-1".into(),
                    from: MemoryEndpointInput {
                        object_type: ObjectType::Episode,
                        external_id: "s1".into(),
                    },
                    relation: RelationType::DerivedFrom,
                    to: MemoryEndpointInput {
                        object_type: ObjectType::Observation,
                        external_id: "s1:turn:1".into(),
                    },
                    confidence: 1.0,
                    rationale: Some("test link".into()),
                },
            })
            .await
            .unwrap();

        assert_eq!(outcome.value.external_id, "link-1");
        assert_eq!(outcome.value.internal_id, "mock:memory_link:link-1");
        assert_eq!(
            outcome.outcome.stats_update_status,
            crate::StatsUpdateStatusRecord::default()
        );
        assert_eq!(adapter.state.lock().unwrap()["n"].links.len(), 1);
    }

    #[tokio::test]
    async fn mock_correction_appends_replacement_and_suppresses_original() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_episode(EpisodeInput {
                external_id: "s1".into(),
                namespace: "n".into(),
                summary: "Original episode".into(),
                started_at: None,
                ended_at: None,
                participants: Vec::new(),
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![derived_memory("old", "s1", "old preference", vec![])],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();

        let origin = SourceProvenanceInput {
            episode_external_ids: vec!["s1".into()],
            ..SourceProvenanceInput::default()
        };
        let outcome = adapter
            .correct(CorrectMemoryInput {
                namespace: "n".into(),
                targets: vec![CorrectionTargetInput::DerivedMemory {
                    external_id: "old".into(),
                }],
                replacements: vec![ReplacementDerivedMemoryInput {
                    memory: derived_memory("new", "s1", "new preference", vec!["old".into()]),
                    original_source_provenance: origin.clone(),
                    correction_origin_provenance: origin.clone(),
                }],
                superseded_derived_memory_external_ids: vec!["old".into()],
                correction_origin: origin,
                rationale: "The user corrected the preference.".into(),
                lifecycle_policy: CorrectionLifecyclePolicyInput::default(),
                cascade_policy: CorrectionCascadePolicyInput::default(),
                include_trace: true,
            })
            .await
            .unwrap();

        assert!(outcome.mutated_object_refs.iter().any(|reference| {
            reference.object_type == ObjectType::DerivedMemory && reference.external_id == "new"
        }));
        let state = adapter.state.lock().unwrap();
        let namespace = state.get("n").unwrap();
        assert_eq!(namespace.derived_memories.len(), 2);
        assert!(
            namespace
                .derived_memories
                .iter()
                .any(|memory| memory.external_id == "old")
        );
        assert!(namespace.suppressed_derived_memory_ids.contains("old"));
    }

    #[tokio::test]
    async fn mock_correction_validation_failure_is_atomic() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![derived_memory("old", "s1", "old preference", vec![])],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();
        adapter
            .state
            .lock()
            .unwrap()
            .get_mut("n")
            .unwrap()
            .suppressed_derived_memory_ids
            .insert("preexisting-suppression".into());
        let (before_memories, before_suppressions) = {
            let state = adapter.state.lock().unwrap();
            let namespace = &state["n"];
            (
                namespace
                    .derived_memories
                    .iter()
                    .map(|memory| (memory.external_id.clone(), memory.text.clone()))
                    .collect::<BTreeMap<_, _>>(),
                namespace.suppressed_derived_memory_ids.clone(),
            )
        };
        let origin = SourceProvenanceInput {
            episode_external_ids: vec!["s1".into()],
            ..SourceProvenanceInput::default()
        };
        let error = adapter
            .correct(CorrectMemoryInput {
                namespace: "n".into(),
                targets: vec![CorrectionTargetInput::DerivedMemory {
                    external_id: "old".into(),
                }],
                replacements: vec![
                    ReplacementDerivedMemoryInput {
                        memory: derived_memory("valid", "s1", "valid replacement", vec![]),
                        original_source_provenance: origin.clone(),
                        correction_origin_provenance: origin.clone(),
                    },
                    ReplacementDerivedMemoryInput {
                        memory: derived_memory("invalid", "s1", " ", vec![]),
                        original_source_provenance: origin.clone(),
                        correction_origin_provenance: origin.clone(),
                    },
                ],
                superseded_derived_memory_external_ids: vec!["old".into()],
                correction_origin: origin,
                rationale: "test atomic validation".into(),
                lifecycle_policy: CorrectionLifecyclePolicyInput::default(),
                cascade_policy: CorrectionCascadePolicyInput::default(),
                include_trace: false,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("text must not be empty"));
        let state = adapter.state.lock().unwrap();
        let namespace = &state["n"];
        let after_memories = namespace
            .derived_memories
            .iter()
            .map(|memory| (memory.external_id.clone(), memory.text.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(after_memories, before_memories);
        assert_eq!(after_memories["old"], "old preference");
        assert_eq!(namespace.suppressed_derived_memory_ids, before_suppressions);
    }

    #[tokio::test]
    async fn mock_forget_removes_target_with_minimal_semantics() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![derived_memory("dm1", "s1", "forget me", vec![])],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();
        let outcome = adapter
            .forget(ForgetMemoryInput {
                namespace: "n".into(),
                targets: vec![MemoryEndpointInput {
                    object_type: ObjectType::DerivedMemory,
                    external_id: "dm1".into(),
                }],
                rationale: "No longer relevant.".into(),
                suppression_policy: SuppressionPolicyInput::default(),
                archive_policy: ArchivePolicyInput::default(),
                cascade_policy: ForgetCascadePolicyInput::default(),
                target_retention_state: RetentionState::Suppressed,
                target_thread_status: None,
                include_trace: false,
            })
            .await
            .unwrap();

        assert_eq!(outcome.mutated_object_refs[0].external_id, "dm1");
        assert!(
            adapter.state.lock().unwrap()["n"]
                .derived_memories
                .is_empty()
        );
    }

    #[tokio::test]
    async fn mock_forget_validation_failure_is_atomic() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![derived_memory("dm1", "s1", "keep me", vec![])],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();
        adapter
            .state
            .lock()
            .unwrap()
            .get_mut("n")
            .unwrap()
            .suppressed_derived_memory_ids
            .insert("preexisting-suppression".into());
        let (before_memories, before_suppressions) = {
            let state = adapter.state.lock().unwrap();
            let namespace = &state["n"];
            (
                namespace
                    .derived_memories
                    .iter()
                    .map(|memory| (memory.external_id.clone(), memory.text.clone()))
                    .collect::<BTreeMap<_, _>>(),
                namespace.suppressed_derived_memory_ids.clone(),
            )
        };

        let error = adapter
            .forget(ForgetMemoryInput {
                namespace: "n".into(),
                targets: vec![
                    MemoryEndpointInput {
                        object_type: ObjectType::DerivedMemory,
                        external_id: "dm1".into(),
                    },
                    MemoryEndpointInput {
                        object_type: ObjectType::MemoryLink,
                        external_id: "later".into(),
                    },
                ],
                rationale: "test atomic validation".into(),
                suppression_policy: SuppressionPolicyInput::default(),
                archive_policy: ArchivePolicyInput::default(),
                cascade_policy: ForgetCascadePolicyInput::default(),
                target_retention_state: RetentionState::Suppressed,
                target_thread_status: None,
                include_trace: false,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("unsupported forget target"));
        let state = adapter.state.lock().unwrap();
        let namespace = &state["n"];
        let after_memories = namespace
            .derived_memories
            .iter()
            .map(|memory| (memory.external_id.clone(), memory.text.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(after_memories, before_memories);
        assert_eq!(after_memories["dm1"], "keep me");
        assert_eq!(namespace.suppressed_derived_memory_ids, before_suppressions);
    }

    fn derived_memory(
        external_id: &str,
        episode_external_id: &str,
        text: &str,
        supersedes_external_ids: Vec<String>,
    ) -> DerivedMemoryInput {
        DerivedMemoryInput {
            external_id: external_id.into(),
            derived_type: DerivedType::Reflection,
            text: text.into(),
            source_episode_external_ids: vec![episode_external_id.into()],
            source_observation_external_ids: Vec::new(),
            thread_external_ids: Vec::new(),
            entity_external_ids: Vec::new(),
            confidence: 1.0,
            salience_score: 0.5,
            stability: Stability::Medium,
            is_current: true,
            supersedes_external_ids,
            metadata: serde_json::Value::Null,
        }
    }
}
