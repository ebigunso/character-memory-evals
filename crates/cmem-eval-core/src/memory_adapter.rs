use crate::bm25::{Bm25Document, Bm25Index, Bm25Score};
use crate::config::RetrievalMode;
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
    pub entity_type: String,
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
    pub status: String,
    pub last_touched_at: Option<String>,
    #[serde(default = "default_salience_score")]
    pub salience_score: f32,
    pub canonical_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DerivedMemoryInput {
    pub external_id: String,
    pub derived_type: String,
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
    pub stability: String,
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
    pub relation: String,
    pub to: MemoryEndpointInput,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEndpointInput {
    pub object_type: String,
    pub external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveInput {
    pub mode: RetrievalMode,
    pub namespace: String,
    pub query: String,
    pub query_date: Option<String>,
    pub top_k_episodes: usize,
    pub top_k_observations: usize,
    pub include_derived_memories: bool,
    pub include_threads: bool,
    pub include_entities: bool,
    pub include_debug_rationale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievedItem {
    pub kind: String,
    pub internal_id: String,
    pub external_id: Option<String>,
    pub episode_external_id: Option<String>,
    pub score: Option<f64>,
    pub rank: usize,
    pub rationale: Vec<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RetrievedContextPack {
    pub items: Vec<RetrievedItem>,
    pub context_text: String,
    pub context_char_count: usize,
    pub context_word_count: usize,
    #[serde(default)]
    pub telemetry: RetrievalTelemetry,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RetrievalTelemetry {
    #[serde(default)]
    pub trace_available: bool,
    #[serde(default)]
    pub vector_candidate_count: Option<usize>,
    #[serde(default)]
    pub graph_relation_count: Option<usize>,
    #[serde(default)]
    pub graph_verified_count: Option<usize>,
    #[serde(default)]
    pub stale_candidate_omission_count: Option<usize>,
    #[serde(default)]
    pub lifecycle_omission_count: Option<usize>,
    #[serde(default)]
    pub lifecycle_filter_decision_count: Option<usize>,
    #[serde(default)]
    pub suppressed_or_deleted_returned_count: Option<usize>,
    #[serde(default)]
    pub superseded_current_returned_count: Option<usize>,
    #[serde(default)]
    pub graph_object_missing_omitted_count: Option<usize>,
    #[serde(default)]
    pub graph_object_missing_returned_count: Option<usize>,
    #[serde(default)]
    pub section_assignment_count: Option<usize>,
    #[serde(default)]
    pub section_assignment_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub stale_candidate_omission_reasons: BTreeMap<String, usize>,
    #[serde(default)]
    pub lifecycle_omission_reasons: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IngestedObjectRefs {
    pub episode_internal_ids: Vec<String>,
    pub observation_internal_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievedExternalRef {
    pub kind: String,
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
        object_type: String,
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
    pub target_retention_state: String,
    pub target_thread_status: Option<String>,
    #[serde(default)]
    pub include_trace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LifecycleMutationResult {
    pub mutated_object_refs: Vec<MemoryEndpointInput>,
    pub mutated_link_external_ids: Vec<String>,
    pub vector_maintained_object_refs: Vec<MemoryEndpointInput>,
    pub superseded: Vec<SupersessionResult>,
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
    pub raw_refs: Vec<String>,
    pub idempotency_key: Option<String>,
    #[serde(default = "default_true")]
    pub include_vector_index_candidates: bool,
    #[serde(default = "default_true")]
    pub include_stats_update_candidates: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedCandidate {
    pub kind: String,
    pub internal_id: String,
    pub external_id: Option<String>,
    pub producer_kind: String,
    pub rationale_origin: String,
    pub rationale: Option<String>,
    #[serde(default)]
    pub source: SourceProvenanceInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateValidationResult {
    pub candidate_index: usize,
    pub candidate_kind: String,
    pub status: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CommitWriteResult {
    pub persisted_object_refs: Vec<MemoryEndpointInput>,
    pub persisted_link_external_ids: Vec<String>,
    pub vector_indexed_object_refs: Vec<MemoryEndpointInput>,
    pub repair_needed: Vec<String>,
}

#[async_trait]
pub trait MemoryAdapter: Send + Sync {
    async fn reset_namespace(&self, namespace: &str) -> Result<()>;
    async fn remember_episode(&self, input: EpisodeInput) -> Result<String>;
    async fn remember_episodes(&self, inputs: Vec<EpisodeInput>) -> Result<Vec<String>> {
        let mut ids = Vec::with_capacity(inputs.len());
        for input in inputs {
            ids.push(self.remember_episode(input).await?);
        }
        Ok(ids)
    }
    async fn remember_observation(&self, input: ObservationInput) -> Result<String>;
    async fn remember_observations(&self, inputs: Vec<ObservationInput>) -> Result<Vec<String>> {
        let mut ids = Vec::with_capacity(inputs.len());
        for input in inputs {
            ids.push(self.remember_observation(input).await?);
        }
        Ok(ids)
    }
    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()>;
    async fn link(&self, input: LinkMemoryInput) -> Result<LinkMemoryResult>;
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
    bm25_index: Option<Arc<Bm25NamespaceIndex>>,
}

#[derive(Debug, Clone, PartialEq)]
struct Bm25NamespaceIndex {
    search: Bm25Index,
    documents: BTreeMap<String, Bm25AdapterDocument>,
}

#[derive(Debug, Clone, PartialEq)]
struct Bm25AdapterDocument {
    kind: &'static str,
    internal_id: String,
    external_id: String,
    episode_external_id: Option<String>,
    text: String,
}

#[async_trait]
impl MemoryAdapter for MockMemoryAdapter {
    async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state.remove(namespace);
        Ok(())
    }

    async fn remember_episode(&self, input: EpisodeInput) -> Result<String> {
        let internal_id = format!("mock:episode:{}", input.external_id);
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace.clone()).or_default();
        namespace.bm25_index = None;
        namespace.episodes.push(input);
        Ok(internal_id)
    }

    async fn remember_observation(&self, input: ObservationInput) -> Result<String> {
        let internal_id = format!("mock:observation:{}", input.external_id);
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace.clone()).or_default();
        namespace.bm25_index = None;
        namespace.observations.push(input);
        Ok(internal_id)
    }

    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()> {
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace).or_default();
        namespace.derived_memories.extend(input.derived_memories);
        Ok(())
    }

    async fn link(&self, input: LinkMemoryInput) -> Result<LinkMemoryResult> {
        let internal_id = format!("mock:memory_link:{}", input.link.external_id);
        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        state
            .entry(input.namespace)
            .or_default()
            .links
            .push(input.link.clone());
        Ok(LinkMemoryResult {
            internal_id,
            external_id: input.link.external_id,
        })
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

        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let namespace = state.entry(input.namespace.clone()).or_default();
        let mut mutated_object_refs = Vec::new();
        let mut suppressed = HashSet::new();
        for target in &input.targets {
            match target {
                CorrectionTargetInput::DerivedMemory { external_id } => {
                    suppressed.insert(external_id.clone());
                    mutated_object_refs.push(MemoryEndpointInput {
                        object_type: "derived_memory".to_string(),
                        external_id: external_id.clone(),
                    });
                }
                CorrectionTargetInput::SourceObject {
                    object_type,
                    external_id,
                    ..
                } => {
                    if !matches!(object_type.as_str(), "episode" | "observation") {
                        bail!("unsupported correction source object type: {object_type}");
                    }
                    mutated_object_refs.push(MemoryEndpointInput {
                        object_type: object_type.clone(),
                        external_id: external_id.clone(),
                    });
                    if input.cascade_policy.apply_to_provenanced_derived_memories {
                        for memory in &namespace.derived_memories {
                            let matches_source = match object_type.as_str() {
                                "episode" => memory
                                    .source_episode_external_ids
                                    .iter()
                                    .any(|id| id == external_id),
                                "observation" => memory
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
                object_type: "derived_memory".to_string(),
                external_id: replacement.memory.external_id.clone(),
            });
            namespace.derived_memories.push(replacement.memory);
        }

        Ok(LifecycleMutationResult {
            mutated_object_refs,
            superseded,
            ..LifecycleMutationResult::default()
        })
    }

    async fn forget(&self, input: ForgetMemoryInput) -> Result<LifecycleMutationResult> {
        if input.targets.is_empty() {
            bail!("forget requires at least one target");
        }
        if input.rationale.trim().is_empty() {
            bail!("forget rationale must not be empty");
        }

        let mut state = self.state.lock().expect("mock memory mutex poisoned");
        let Some(namespace) = state.get_mut(&input.namespace) else {
            return Ok(LifecycleMutationResult::default());
        };
        for target in &input.targets {
            match target.object_type.as_str() {
                "episode" => {
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
                "observation" => {
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
                "derived_memory" => namespace
                    .derived_memories
                    .retain(|memory| memory.external_id != target.external_id),
                "memory_thread" => {
                    if input.cascade_policy.apply_to_thread_members {
                        namespace.derived_memories.retain(|memory| {
                            !memory.thread_external_ids.contains(&target.external_id)
                        });
                    }
                }
                unsupported => bail!("unsupported forget target object type: {unsupported}"),
            }
        }
        namespace.bm25_index = None;

        Ok(LifecycleMutationResult {
            mutated_object_refs: input.targets,
            ..LifecycleMutationResult::default()
        })
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
                kind: "episode".to_string(),
                internal_id: episode_id.clone(),
                external_id: Some(input.episode_external_id.clone()),
                producer_kind: "deterministic_helper".to_string(),
                rationale_origin: "unavailable".to_string(),
                rationale: None,
                source: source.clone(),
            },
            PreparedCandidate {
                kind: "observation".to_string(),
                internal_id: observation_id.clone(),
                external_id: Some(input.observation_external_id.clone()),
                producer_kind: "deterministic_helper".to_string(),
                rationale_origin: "unavailable".to_string(),
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
                    kind: "vector_index".to_string(),
                    internal_id: format!("mock:vector_index:{kind}:{target}"),
                    external_id: None,
                    producer_kind: "deterministic_helper".to_string(),
                    rationale_origin: "unavailable".to_string(),
                    rationale: None,
                    source: SourceProvenanceInput::default(),
                });
            }
        }
        if input.include_stats_update_candidates {
            for (kind, target) in [("episode", &episode_id), ("observation", &observation_id)] {
                candidates.push(PreparedCandidate {
                    kind: "stats_update".to_string(),
                    internal_id: format!("mock:stats_update:{kind}:{target}"),
                    external_id: None,
                    producer_kind: "deterministic_helper".to_string(),
                    rationale_origin: "unavailable".to_string(),
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
            .any(|validation| validation.status == "invalid")
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
            started_at: None,
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
            observed_at: None,
            metadata: serde_json::Value::Null,
        })
        .await?;
        let persisted_object_refs = vec![
            MemoryEndpointInput {
                object_type: "episode".to_string(),
                external_id: episode_external_id,
            },
            MemoryEndpointInput {
                object_type: "observation".to_string(),
                external_id: observation_external_id,
            },
        ];
        Ok(CommitWriteResult {
            vector_indexed_object_refs: if options.update_vectors {
                persisted_object_refs.clone()
            } else {
                Vec::new()
            },
            persisted_object_refs,
            ..CommitWriteResult::default()
        })
    }

    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        if input.mode == RetrievalMode::Bm25Only {
            let index = {
                let mut state = self.state.lock().expect("mock memory mutex poisoned");
                let Some(ns) = state.get_mut(&input.namespace) else {
                    return Ok(RetrievedContextPack::default());
                };
                if ns.bm25_index.is_none() {
                    ns.bm25_index = Some(Arc::new(build_bm25_index(ns)));
                }
                ns.bm25_index
                    .as_ref()
                    .expect("BM25 index was just initialized")
                    .clone()
            };
            return Ok(retrieve_bm25(&index, &input));
        }

        let state = self.state.lock().expect("mock memory mutex poisoned");
        let Some(ns) = state.get(&input.namespace) else {
            return Ok(RetrievedContextPack::default());
        };

        let mut items = Vec::new();
        let mut episodes = ns.episodes.clone();
        episodes.sort_by(|a, b| {
            score_text(&input.query, &b.summary)
                .partial_cmp(&score_text(&input.query, &a.summary))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for episode in episodes.into_iter().take(input.top_k_episodes) {
            let score = score_text(&input.query, &episode.summary);
            items.push(RetrievedItem {
                kind: "episode".to_string(),
                internal_id: format!("mock:episode:{}", episode.external_id),
                external_id: Some(episode.external_id),
                episode_external_id: None,
                score: Some(score),
                rank: 0,
                rationale: vec!["mock_lexical_overlap".to_string()],
                text: Some(episode.summary),
            });
        }

        let mut observations = ns.observations.clone();
        observations.sort_by(|a, b| {
            score_text(&input.query, &b.text)
                .partial_cmp(&score_text(&input.query, &a.text))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for observation in observations.into_iter().take(input.top_k_observations) {
            let score = score_text(&input.query, &observation.text);
            items.push(RetrievedItem {
                kind: "observation".to_string(),
                internal_id: format!("mock:observation:{}", observation.external_id),
                external_id: Some(observation.external_id),
                episode_external_id: Some(observation.episode_external_id),
                score: Some(score),
                rank: 0,
                rationale: vec!["mock_lexical_overlap".to_string()],
                text: Some(observation.text),
            });
        }

        if input.include_derived_memories {
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
            for memory in derived_memories.into_iter().take(input.top_k_observations) {
                let score = score_text(&input.query, &memory.text);
                items.push(RetrievedItem {
                    kind: "derived_memory".to_string(),
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

        let context_text = items
            .iter()
            .filter_map(|item| item.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");
        let context_char_count = context_text.chars().count();
        let context_word_count = context_text.split_whitespace().count();

        Ok(RetrievedContextPack {
            items,
            context_text,
            context_char_count,
            context_word_count,
            telemetry: RetrievalTelemetry::default(),
        })
    }
}

fn build_bm25_index(ns: &NamespaceState) -> Bm25NamespaceIndex {
    let mut adapter_documents = BTreeMap::new();
    for episode in &ns.episodes {
        let id = format!("episode:{}", episode.external_id);
        adapter_documents.insert(
            id,
            Bm25AdapterDocument {
                kind: "episode",
                internal_id: format!("mock:episode:{}", episode.external_id),
                external_id: episode.external_id.clone(),
                episode_external_id: None,
                text: episode.summary.clone(),
            },
        );
    }
    for observation in &ns.observations {
        let id = format!("observation:{}", observation.external_id);
        adapter_documents.insert(
            id,
            Bm25AdapterDocument {
                kind: "observation",
                internal_id: format!("mock:observation:{}", observation.external_id),
                external_id: observation.external_id.clone(),
                episode_external_id: Some(observation.episode_external_id.clone()),
                text: observation.text.clone(),
            },
        );
    }

    let documents = adapter_documents
        .iter()
        .map(|(id, document)| Bm25Document {
            id: id.clone(),
            text: document.text.clone(),
        })
        .collect::<Vec<_>>();

    Bm25NamespaceIndex {
        search: Bm25Index::new(&documents),
        documents: adapter_documents,
    }
}

fn retrieve_bm25(index: &Bm25NamespaceIndex, input: &RetrieveInput) -> RetrievedContextPack {
    let mut top_episodes = Vec::new();
    let mut top_observations = Vec::new();
    for score in index.search.scores(&input.query) {
        let Some(document) = index.documents.get(&score.id) else {
            continue;
        };
        match document.kind {
            "episode" => insert_top_bm25(&mut top_episodes, score, input.top_k_episodes),
            "observation" => {
                insert_top_bm25(&mut top_observations, score, input.top_k_observations);
            }
            _ => {}
        }
    }

    let mut items = Vec::new();
    for score in top_episodes.into_iter().chain(top_observations) {
        let Some(document) = index.documents.get(&score.id) else {
            continue;
        };
        items.push(RetrievedItem {
            kind: document.kind.to_string(),
            internal_id: document.internal_id.clone(),
            external_id: Some(document.external_id.clone()),
            episode_external_id: document.episode_external_id.clone(),
            score: Some(score.score),
            rank: 0,
            rationale: vec!["bm25_only".to_string()],
            text: Some(document.text.clone()),
        });
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

    let context_text = items
        .iter()
        .filter_map(|item| item.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    let context_char_count = context_text.chars().count();
    let context_word_count = context_text.split_whitespace().count();

    RetrievedContextPack {
        items,
        context_text,
        context_char_count,
        context_word_count,
        telemetry: RetrievalTelemetry::default(),
    }
}

fn insert_top_bm25(top: &mut Vec<Bm25Score>, score: Bm25Score, limit: usize) {
    if limit == 0 {
        return;
    }
    top.push(score);
    top.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    top.truncate(limit);
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
                && matches!(candidate.kind.as_str(), "episode" | "observation")
            {
                errors.push("content must not be empty".to_string());
            }
            if candidate
                .external_id
                .as_deref()
                .is_some_and(|external_id| external_id.trim().is_empty())
            {
                errors.push("external_id must not be empty".to_string());
            }
            CandidateValidationResult {
                candidate_index,
                candidate_kind: candidate.kind.clone(),
                status: if errors.is_empty() {
                    "valid".to_string()
                } else {
                    "invalid".to_string()
                },
                errors,
                warnings: Vec::new(),
            }
        })
        .collect()
}

fn default_thread_status() -> String {
    "active".to_string()
}

fn default_stability() -> String {
    "medium".to_string()
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

fn default_suppressed_retention_state() -> String {
    "suppressed".to_string()
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
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: "n".into(),
                query: "chat native first version".into(),
                query_date: None,
                top_k_episodes: 5,
                top_k_observations: 5,
                include_derived_memories: false,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == "observation" && item.external_id.as_deref() == Some("s1:turn:1")
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
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: "n".into(),
                query: "chat native first version".into(),
                query_date: None,
                top_k_episodes: 5,
                top_k_observations: 5,
                include_derived_memories: false,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == "observation" && item.external_id.as_deref() == Some("s1:turn:1")
        }));
        assert!(
            pack.items
                .iter()
                .any(|item| item.kind == "episode" && item.external_id.as_deref() == Some("s1"))
        );
    }

    #[tokio::test]
    async fn mock_adapter_retrieves_derived_memories_when_enabled() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![DerivedMemoryInput {
                    external_id: "dm1".into(),
                    derived_type: "reflection".into(),
                    text: "The user prefers chat native design.".into(),
                    source_episode_external_ids: vec!["s1".into()],
                    source_observation_external_ids: vec![],
                    thread_external_ids: vec![],
                    entity_external_ids: vec![],
                    confidence: 1.0,
                    salience_score: 0.5,
                    stability: "medium".into(),
                    is_current: true,
                    supersedes_external_ids: vec![],
                    metadata: serde_json::json!({}),
                }],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();

        let pack = adapter
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Hybrid,
                namespace: "n".into(),
                query: "chat native".into(),
                query_date: None,
                top_k_episodes: 5,
                top_k_observations: 5,
                include_derived_memories: true,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert!(pack.items.iter().any(|item| {
            item.kind == "derived_memory" && item.external_id.as_deref() == Some("dm1")
        }));
    }

    #[tokio::test]
    async fn mock_adapter_bm25_ranks_episode_and_observation_matches() {
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
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Bm25Only,
                namespace: "n".into(),
                query: "chat native first version".into(),
                query_date: None,
                top_k_episodes: 1,
                top_k_observations: 1,
                include_derived_memories: false,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert_eq!(pack.items.len(), 2);
        assert!(pack.items.iter().any(|item| {
            item.kind == "episode"
                && item.external_id.as_deref() == Some("s1")
                && item.rationale == vec!["bm25_only"]
        }));
        assert!(pack.items.iter().any(|item| {
            item.kind == "observation" && item.external_id.as_deref() == Some("s1:turn:1")
        }));
    }

    #[tokio::test]
    async fn mock_adapter_bm25_excludes_derived_memories() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_enrichment(GraphEnrichmentInput {
                namespace: "n".into(),
                derived_memories: vec![DerivedMemoryInput {
                    external_id: "dm1".into(),
                    derived_type: "reflection".into(),
                    text: "The user prefers chat native design.".into(),
                    source_episode_external_ids: vec!["s1".into()],
                    source_observation_external_ids: vec![],
                    thread_external_ids: vec![],
                    entity_external_ids: vec![],
                    confidence: 1.0,
                    salience_score: 0.5,
                    stability: "medium".into(),
                    is_current: true,
                    supersedes_external_ids: vec![],
                    metadata: serde_json::json!({}),
                }],
                ..GraphEnrichmentInput::default()
            })
            .await
            .unwrap();

        let pack = adapter
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Bm25Only,
                namespace: "n".into(),
                query: "chat native".into(),
                query_date: None,
                top_k_episodes: 5,
                top_k_observations: 5,
                include_derived_memories: true,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert!(pack.items.is_empty());
    }

    #[tokio::test]
    async fn mock_adapter_bm25_cache_invalidates_after_observation_ingest() {
        let adapter = MockMemoryAdapter::default();
        adapter
            .remember_observation(ObservationInput {
                external_id: "s1:turn:1".into(),
                episode_external_id: "s1".into(),
                namespace: "n".into(),
                speaker: Some("user".into()),
                text: "Book a train ticket".into(),
                observed_at: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();

        let first = adapter
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Bm25Only,
                namespace: "n".into(),
                query: "chat native".into(),
                query_date: None,
                top_k_episodes: 1,
                top_k_observations: 1,
                include_derived_memories: false,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();
        assert_eq!(first.items[0].external_id.as_deref(), Some("s1:turn:1"));

        adapter
            .remember_observation(ObservationInput {
                external_id: "s1:turn:2".into(),
                episode_external_id: "s1".into(),
                namespace: "n".into(),
                speaker: Some("user".into()),
                text: "Keep the interface chat native".into(),
                observed_at: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();

        let second = adapter
            .retrieve(RetrieveInput {
                mode: RetrievalMode::Bm25Only,
                namespace: "n".into(),
                query: "chat native".into(),
                query_date: None,
                top_k_episodes: 1,
                top_k_observations: 1,
                include_derived_memories: false,
                include_threads: false,
                include_entities: false,
                include_debug_rationale: false,
            })
            .await
            .unwrap();

        assert_eq!(second.items[0].external_id.as_deref(), Some("s1:turn:2"));
    }

    #[tokio::test]
    async fn mock_staged_write_is_deterministic_validated_and_committed() {
        let adapter = MockMemoryAdapter::default();
        let input = PrepareWriteInput {
            namespace: "n".into(),
            content: "The user prefers a chat-native first version.".into(),
            episode_external_id: "s1".into(),
            observation_external_id: "s1:turn:1".into(),
            raw_refs: vec!["raw://conversation/s1".into()],
            idempotency_key: Some("continuity-step-1".into()),
            include_vector_index_candidates: true,
            include_stats_update_candidates: true,
        };

        let first = adapter.prepare(input.clone()).await.unwrap();
        let second = adapter.prepare(input).await.unwrap();
        assert_eq!(first.operation_internal_id, second.operation_internal_id);
        assert_eq!(first.candidates, second.candidates);
        assert_eq!(first.candidates[0].producer_kind, "deterministic_helper");
        assert_eq!(first.candidates[0].rationale_origin, "unavailable");
        assert!(adapter.state.lock().unwrap().get("n").is_none());

        let validations = adapter.validate_plan(&first).await.unwrap();
        assert!(
            validations
                .iter()
                .all(|validation| validation.status == "valid")
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
                        object_type: "episode".into(),
                        external_id: "s1".into(),
                    },
                    relation: "derived_from".into(),
                    to: MemoryEndpointInput {
                        object_type: "observation".into(),
                        external_id: "s1:turn:1".into(),
                    },
                    confidence: 1.0,
                    rationale: Some("test link".into()),
                },
            })
            .await
            .unwrap();

        assert_eq!(outcome.external_id, "link-1");
        assert_eq!(outcome.internal_id, "mock:memory_link:link-1");
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
            reference.object_type == "derived_memory" && reference.external_id == "new"
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
                    object_type: "derived_memory".into(),
                    external_id: "dm1".into(),
                }],
                rationale: "No longer relevant.".into(),
                suppression_policy: SuppressionPolicyInput::default(),
                archive_policy: ArchivePolicyInput::default(),
                cascade_policy: ForgetCascadePolicyInput::default(),
                target_retention_state: "suppressed".into(),
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

    fn derived_memory(
        external_id: &str,
        episode_external_id: &str,
        text: &str,
        supersedes_external_ids: Vec<String>,
    ) -> DerivedMemoryInput {
        DerivedMemoryInput {
            external_id: external_id.into(),
            derived_type: "reflection".into(),
            text: text.into(),
            source_episode_external_ids: vec![episode_external_id.into()],
            source_observation_external_ids: Vec::new(),
            thread_external_ids: Vec::new(),
            entity_external_ids: Vec::new(),
            confidence: 1.0,
            salience_score: 0.5,
            stability: "medium".into(),
            is_current: true,
            supersedes_external_ids,
            metadata: serde_json::Value::Null,
        }
    }
}
