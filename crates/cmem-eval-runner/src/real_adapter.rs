use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use character_memory::{
    CharacterMemory, ContinuitySectionLimits, DerivedMemoryDraft, DerivedType, EmbeddingProvider,
    EntityDraft, EntityType, EpisodeDraft, LifecycleFilterAction, LifecycleFilterReason, MemoryId,
    MemoryLinkDraft, MemoryObjectDraft, MemoryThreadDraft, ObjectType, ObservationDraft,
    RelationType, RememberDraft, RetentionState, RetrievalCandidateLimits, RetrievalContext,
    Settings, Stability, ThreadStatus,
};
use chrono::{DateTime, Utc};
use cmem_eval_core::{
    BenchmarkRunConfig, EpisodeInput, GraphEnrichmentInput, MemoryAdapter, MemoryEndpointInput,
    ObservationInput, RetrievalTelemetry, RetrieveInput, RetrievedContextPack, RetrievedItem,
};
use qdrant_client::Qdrant;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

const UUID_NAMESPACE: Uuid = Uuid::from_u128(0x9b6af7a4_9076_49bb_9231_84d1ed632cf1);

pub struct CharacterMemoryAdapter {
    config: BenchmarkRunConfig,
    namespaces: Arc<Mutex<HashMap<String, NamespaceState>>>,
}

struct NamespaceState {
    memory: CharacterMemory,
    collection_name: String,
    episode_ids: HashMap<String, MemoryId>,
    observation_ids: HashMap<String, MemoryId>,
    entity_ids: HashMap<String, MemoryId>,
    thread_ids: HashMap<String, MemoryId>,
    derived_memory_ids: HashMap<String, MemoryId>,
    reverse_episode_ids: HashMap<MemoryId, String>,
    reverse_observation_ids: HashMap<MemoryId, (String, String)>,
    reverse_thread_ids: HashMap<MemoryId, String>,
    reverse_derived_memory_ids: HashMap<MemoryId, String>,
}

impl CharacterMemoryAdapter {
    pub async fn new(config: &BenchmarkRunConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            namespaces: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn create_namespace_state(&self, namespace: &str) -> Result<NamespaceState> {
        let collection_name = self.collection_name(namespace);
        let settings = self.settings()?;
        let memory = if self.config.backend.embedding.provider == "deterministic" {
            let vector_size = self.config.backend.embedding.vector_size.unwrap_or(3072);
            CharacterMemory::new_with_embedding_provider(
                settings,
                collection_name.clone(),
                Box::new(DeterministicEmbeddingProvider { vector_size }),
            )
            .await?
        } else {
            CharacterMemory::new(settings, collection_name.clone()).await?
        };

        Ok(NamespaceState {
            memory,
            collection_name,
            episode_ids: HashMap::new(),
            observation_ids: HashMap::new(),
            entity_ids: HashMap::new(),
            thread_ids: HashMap::new(),
            derived_memory_ids: HashMap::new(),
            reverse_episode_ids: HashMap::new(),
            reverse_observation_ids: HashMap::new(),
            reverse_thread_ids: HashMap::new(),
            reverse_derived_memory_ids: HashMap::new(),
        })
    }

    fn settings(&self) -> Result<Settings> {
        let qdrant = self.qdrant_connection_string()?;
        let oxigraph = self
            .config
            .backend
            .oxigraph_connection_string
            .clone()
            .or_else(|| env::var("OXIGRAPH_CONNECTION_STRING").ok())
            .unwrap_or_else(|| "memory://in-memory".to_string());
        let openai_api_key = env::var(&self.config.backend.openai_api_key_env)
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .unwrap_or_else(|_| {
                if self.config.backend.embedding.provider == "deterministic" {
                    "deterministic-unused".to_string()
                } else {
                    String::new()
                }
            });
        if openai_api_key.is_empty() {
            bail!(
                "{} is required for OpenAI live embeddings",
                self.config.backend.openai_api_key_env
            );
        }

        let external_config = config::Config::builder()
            .set_override("qdrant_connection_string", qdrant)?
            .set_override("oxigraph_connection_string", oxigraph)?
            .set_override("openai_api_key", openai_api_key)?
            .set_override(
                "embedding_model",
                self.config.backend.embedding.model.clone(),
            )?
            .build()?;

        Settings::new(external_config).map_err(Into::into)
    }

    fn qdrant_connection_string(&self) -> Result<String> {
        self.config
            .backend
            .qdrant_connection_string
            .clone()
            .or_else(|| env::var("QDRANT_CONNECTION_STRING").ok())
            .context("QDRANT_CONNECTION_STRING is required for live Character Memory runs")
    }

    fn collection_name(&self, namespace: &str) -> String {
        let prefix = self
            .config
            .backend
            .namespace_prefix
            .as_deref()
            .unwrap_or("cmem_eval");
        format!(
            "{}_{}_{}_{}",
            sanitize_collection_segment(prefix),
            sanitize_collection_segment(&self.config.run_id),
            sanitize_collection_segment(namespace),
            Uuid::new_v4().simple()
        )
    }

    async fn cleanup_collection_if_enabled(&self, collection_name: &str) -> Result<()> {
        if !self.config.backend.cleanup.enabled {
            return Ok(());
        }
        validate_cleanup_target(
            collection_name,
            self.config
                .backend
                .cleanup
                .require_collection_prefix
                .as_deref(),
        )?;
        let qdrant = self.qdrant_connection_string()?;
        Qdrant::from_url(&qdrant)
            .build()?
            .delete_collection(collection_name)
            .await
            .with_context(|| format!("delete Qdrant collection {collection_name}"))?;
        Ok(())
    }
}

#[async_trait]
impl MemoryAdapter for CharacterMemoryAdapter {
    async fn reset_namespace(&self, namespace: &str) -> Result<()> {
        let collection_name = {
            let namespaces = self.namespaces.lock().await;
            namespaces
                .get(namespace)
                .map(|state| state.collection_name.clone())
        };
        if let Some(collection_name) = collection_name {
            self.cleanup_collection_if_enabled(&collection_name).await?;
            let mut namespaces = self.namespaces.lock().await;
            namespaces.remove(namespace);
        }
        Ok(())
    }

    async fn remember_episode(&self, input: EpisodeInput) -> Result<String> {
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&input.namespace) {
            let state = self.create_namespace_state(&input.namespace).await?;
            namespaces.insert(input.namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&input.namespace)
            .expect("namespace state inserted");
        let id = deterministic_id(&input.namespace, "episode", &input.external_id);
        let mut draft = EpisodeDraft::new(input.summary);
        draft.id = Some(id);
        draft.source_conversation_id = Some(input.external_id.clone());
        draft.raw_ref = Some(format!(
            "eval://{}/episode/{}",
            input.namespace, input.external_id
        ));
        draft.started_at = parse_timestamp(input.started_at.as_deref())?;
        draft.ended_at = parse_timestamp(input.ended_at.as_deref())?;

        state
            .memory
            .remember(RememberDraft::new([MemoryObjectDraft::Episode(draft)]))
            .await?;
        state.episode_ids.insert(input.external_id.clone(), id);
        state
            .reverse_episode_ids
            .insert(id, input.external_id.clone());

        Ok(id.to_string())
    }

    async fn remember_observation(&self, input: ObservationInput) -> Result<String> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered episodes: {}", input.namespace))?;
        let episode_id = *state
            .episode_ids
            .get(&input.episode_external_id)
            .ok_or_else(|| {
                anyhow!(
                    "observation {} references unknown episode external_id {}",
                    input.external_id,
                    input.episode_external_id
                )
            })?;
        let id = deterministic_id(&input.namespace, "observation", &input.external_id);
        let mut draft = ObservationDraft::new(episode_id, input.text);
        draft.id = Some(id);
        draft.raw_ref = Some(format!(
            "eval://{}/observation/{}",
            input.namespace, input.external_id
        ));
        draft.observed_at = parse_timestamp(input.observed_at.as_deref())?;

        state
            .memory
            .remember(RememberDraft::new([MemoryObjectDraft::Observation(draft)]))
            .await?;
        state.observation_ids.insert(input.external_id.clone(), id);
        state.reverse_observation_ids.insert(
            id,
            (input.external_id.clone(), input.episode_external_id.clone()),
        );

        Ok(id.to_string())
    }

    async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()> {
        let mut namespaces = self.namespaces.lock().await;
        let state = namespaces
            .get_mut(&input.namespace)
            .ok_or_else(|| anyhow!("namespace has no remembered episodes: {}", input.namespace))?;

        let mut objects = Vec::new();
        let mut links = Vec::new();
        let mut pending_entities = HashMap::new();
        let mut pending_threads = HashMap::new();
        let mut pending_derived = HashMap::new();

        for entity in &input.entities {
            pending_entities.insert(
                entity.external_id.clone(),
                deterministic_id(&input.namespace, "entity", &entity.external_id),
            );
        }
        for thread in &input.threads {
            pending_threads.insert(
                thread.external_id.clone(),
                deterministic_id(&input.namespace, "memory_thread", &thread.external_id),
            );
        }
        for memory in &input.derived_memories {
            pending_derived.insert(
                memory.external_id.clone(),
                deterministic_id(&input.namespace, "derived_memory", &memory.external_id),
            );
        }

        for entity in input.entities {
            let id = pending_entities[&entity.external_id];
            let mut draft = EntityDraft::new(parse_entity_type(&entity.entity_type)?, entity.name);
            draft.id = Some(id);
            draft.aliases = entity.aliases;
            draft.canonical_key = entity.canonical_key;
            draft.summary = entity.summary;
            objects.push(MemoryObjectDraft::Entity(draft));
        }

        for thread in input.threads {
            let id = pending_threads[&thread.external_id];
            let mut draft = MemoryThreadDraft::new(thread.title, thread.summary);
            draft.id = Some(id);
            draft.status = parse_thread_status(&thread.status)?;
            draft.last_touched_at = parse_timestamp(thread.last_touched_at.as_deref())?;
            draft.salience_score = thread.salience_score;
            draft.canonical_key = thread.canonical_key;
            objects.push(MemoryObjectDraft::MemoryThread(draft));
        }

        for memory in input.derived_memories {
            let id = pending_derived[&memory.external_id];
            let mut draft =
                DerivedMemoryDraft::new(parse_derived_type(&memory.derived_type)?, memory.text);
            draft.id = Some(id);
            draft.derived_from_episode_ids = resolve_ids(
                "episode",
                &memory.source_episode_external_ids,
                &state.episode_ids,
                &HashMap::new(),
            )?;
            draft.derived_from_observation_ids = resolve_ids(
                "observation",
                &memory.source_observation_external_ids,
                &state.observation_ids,
                &HashMap::new(),
            )?;
            draft.thread_ids = resolve_ids(
                "memory_thread",
                &memory.thread_external_ids,
                &state.thread_ids,
                &pending_threads,
            )?;
            draft.entity_ids = resolve_ids(
                "entity",
                &memory.entity_external_ids,
                &state.entity_ids,
                &pending_entities,
            )?;
            draft.confidence = memory.confidence;
            draft.salience_score = memory.salience_score;
            draft.stability = parse_stability(&memory.stability)?;
            draft.is_current = memory.is_current;
            draft.supersedes = resolve_ids(
                "derived_memory",
                &memory.supersedes_external_ids,
                &state.derived_memory_ids,
                &pending_derived,
            )?;
            objects.push(MemoryObjectDraft::DerivedMemory(draft));
        }

        for link in input.links {
            let (from_type, from_id) = resolve_endpoint(
                &link.from,
                state,
                &pending_entities,
                &pending_threads,
                &pending_derived,
            )?;
            let (to_type, to_id) = resolve_endpoint(
                &link.to,
                state,
                &pending_entities,
                &pending_threads,
                &pending_derived,
            )?;
            let mut draft = MemoryLinkDraft::new(
                from_type,
                from_id,
                parse_relation_type(&link.relation)?,
                to_type,
                to_id,
            );
            draft.id = Some(deterministic_id(
                &input.namespace,
                "memory_link",
                &link.external_id,
            ));
            draft.confidence = link.confidence;
            draft.rationale = link.rationale;
            links.push(draft);
        }

        if objects.is_empty() && links.is_empty() {
            return Ok(());
        }

        state
            .memory
            .remember(RememberDraft::new(objects).with_links(links))
            .await?;
        state.entity_ids.extend(pending_entities);
        for (external_id, id) in pending_threads {
            state.thread_ids.insert(external_id.clone(), id);
            state.reverse_thread_ids.insert(id, external_id);
        }
        for (external_id, id) in pending_derived {
            state.derived_memory_ids.insert(external_id.clone(), id);
            state.reverse_derived_memory_ids.insert(id, external_id);
        }
        Ok(())
    }

    async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
        let mut namespaces = self.namespaces.lock().await;
        if !namespaces.contains_key(&input.namespace) {
            let state = self.create_namespace_state(&input.namespace).await?;
            namespaces.insert(input.namespace.clone(), state);
        }
        let state = namespaces
            .get_mut(&input.namespace)
            .expect("namespace state inserted");

        let mut context = RetrievalContext::new(input.query);
        context.include_trace = input.include_debug_rationale;
        context.candidate_limits = RetrievalCandidateLimits {
            max_vector_candidates: input.top_k_episodes + input.top_k_observations + 16,
            max_graph_roots: (input.top_k_episodes + input.top_k_observations).max(1),
        };
        context.section_limits = ContinuitySectionLimits {
            relevant_episodes: input.top_k_episodes,
            salient_observations: input.top_k_observations,
            derived_memories: if input.include_derived_memories {
                12
            } else {
                0
            },
            preferences: if input.include_derived_memories { 8 } else { 0 },
            relationship_notes: if input.include_derived_memories { 8 } else { 0 },
            open_loops: if input.include_derived_memories { 8 } else { 0 },
            commitments: if input.include_derived_memories { 8 } else { 0 },
            character_signals: if input.include_derived_memories { 8 } else { 0 },
            ..ContinuitySectionLimits::default()
        };
        context.object_type_defaults = vec![ObjectType::Episode, ObjectType::Observation];
        if input.include_derived_memories {
            context.object_type_defaults.push(ObjectType::DerivedMemory);
        }
        if input.include_threads {
            context.object_type_defaults.push(ObjectType::MemoryThread);
        }
        if input.include_entities {
            context.object_type_defaults.push(ObjectType::Entity);
        }

        let outcome = state.memory.retrieve(context).await?;
        Ok(flatten_outcome(state, outcome))
    }
}

fn flatten_outcome(
    state: &NamespaceState,
    outcome: character_memory::RetrieveOutcome,
) -> RetrievedContextPack {
    let telemetry = telemetry_from_outcome(&outcome);
    let mut trace_by_id: HashMap<MemoryId, (Option<f64>, usize)> = HashMap::new();
    if let Some(trace) = &outcome.trace {
        for candidate in &trace.vector_candidates {
            trace_by_id.insert(
                candidate.object.id,
                (Some(candidate.score as f64), candidate.rank),
            );
        }
    }

    let mut items = Vec::new();
    for thread in outcome.pack.active_threads {
        let (score, rank) = trace_by_id
            .get(&thread.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "memory_thread".to_string(),
            internal_id: thread.id.to_string(),
            external_id: state.reverse_thread_ids.get(&thread.id).cloned(),
            episode_external_id: None,
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(thread.summary),
        });
    }

    for episode in outcome.pack.relevant_episodes {
        let external_id = state
            .reverse_episode_ids
            .get(&episode.id)
            .cloned()
            .or(episode.source_conversation_id.clone());
        let (score, rank) = trace_by_id
            .get(&episode.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "episode".to_string(),
            internal_id: episode.id.to_string(),
            external_id,
            episode_external_id: None,
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(episode.summary),
        });
    }

    for observation in outcome.pack.salient_observations {
        let mapped = state.reverse_observation_ids.get(&observation.id).cloned();
        let (external_id, episode_external_id) = mapped
            .map(|(obs_id, ep_id)| (Some(obs_id), Some(ep_id)))
            .unwrap_or_else(|| {
                (
                    observation.raw_ref.clone(),
                    state
                        .reverse_episode_ids
                        .get(&observation.episode_id)
                        .cloned(),
                )
            });
        let (score, rank) = trace_by_id
            .get(&observation.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "observation".to_string(),
            internal_id: observation.id.to_string(),
            external_id,
            episode_external_id,
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(observation.text),
        });
    }

    let mut seen_derived = HashSet::new();
    for derived in outcome
        .pack
        .derived_memories
        .into_iter()
        .chain(outcome.pack.preferences)
        .chain(outcome.pack.relationship_notes)
        .chain(outcome.pack.open_loops)
        .chain(outcome.pack.commitments)
        .chain(outcome.pack.character_signals)
    {
        if !seen_derived.insert(derived.memory.id) {
            continue;
        }
        let (score, rank) = trace_by_id
            .get(&derived.memory.id)
            .copied()
            .unwrap_or((None, items.len() + 1));
        items.push(RetrievedItem {
            kind: "derived_memory".to_string(),
            internal_id: derived.memory.id.to_string(),
            external_id: state
                .reverse_derived_memory_ids
                .get(&derived.memory.id)
                .cloned(),
            episode_external_id: derived
                .source_episode_ids
                .first()
                .and_then(|id| state.reverse_episode_ids.get(id))
                .cloned(),
            score,
            rank,
            rationale: vec![outcome.rationale.summary.clone()],
            text: Some(derived.memory.text),
        });
    }

    items.sort_by_key(|item| item.rank);
    let context_text = render_context_text(&items);
    let context_char_count = context_text.chars().count();
    let context_word_count = context_text.split_whitespace().count();

    RetrievedContextPack {
        items,
        context_text,
        context_char_count,
        context_word_count,
        telemetry,
    }
}

fn telemetry_from_outcome(outcome: &character_memory::RetrieveOutcome) -> RetrievalTelemetry {
    let trace = outcome.trace.as_ref();
    let returned_ids = returned_object_ids(outcome);
    let suppressed_or_deleted_returned_count = trace.map(|trace| {
        trace
            .lifecycle_filter_decisions
            .iter()
            .filter(|decision| {
                returned_ids.contains(&decision.object.id)
                    && decision.action == LifecycleFilterAction::Included
                    && (matches!(
                        decision.retention_state,
                        Some(RetentionState::Suppressed | RetentionState::Deleted)
                    ) || matches!(
                        decision.reason,
                        LifecycleFilterReason::SuppressedIncludedByPolicy
                            | LifecycleFilterReason::DeletedIncludedByPolicy
                    ))
            })
            .count()
    });
    let superseded_current_returned_count = trace.map(|trace| {
        trace
            .lifecycle_filter_decisions
            .iter()
            .filter(|decision| {
                returned_ids.contains(&decision.object.id)
                    && decision.action == LifecycleFilterAction::Included
                    && (decision.is_current == Some(false)
                        || !decision.superseded_by.is_empty()
                        || matches!(
                            decision.reason,
                            LifecycleFilterReason::NonCurrentIncludedByPolicy
                                | LifecycleFilterReason::SupersededIncludedByPolicy
                        ))
            })
            .count()
    });
    let graph_object_missing_omitted_count = trace.map(|trace| {
        trace
            .stale_candidate_omissions
            .iter()
            .filter(|omission| {
                matches!(
                    omission.reason,
                    character_memory::StaleCandidateReason::GraphObjectMissing
                )
            })
            .count()
            + trace
                .lifecycle_filter_decisions
                .iter()
                .filter(|decision| {
                    !returned_ids.contains(&decision.object.id)
                        && decision.action == LifecycleFilterAction::Omitted
                        && decision.reason == LifecycleFilterReason::GraphObjectMissing
                })
                .count()
    });
    let graph_object_missing_returned_count = trace.map(|trace| {
        trace
            .lifecycle_filter_decisions
            .iter()
            .filter(|decision| {
                returned_ids.contains(&decision.object.id)
                    && decision.action == LifecycleFilterAction::Included
                    && decision.reason == LifecycleFilterReason::GraphObjectMissing
            })
            .count()
    });
    RetrievalTelemetry {
        trace_available: trace.is_some(),
        vector_candidate_count: Some(outcome.rationale.vector_candidate_count),
        graph_relation_count: trace.map(|trace| trace.graph_relations.len()),
        graph_verified_count: Some(outcome.rationale.graph_verified_count),
        stale_candidate_omission_count: Some(outcome.rationale.stale_candidate_omission_count),
        lifecycle_omission_count: Some(outcome.rationale.lifecycle_omission_count),
        lifecycle_filter_decision_count: trace.map(|trace| trace.lifecycle_filter_decisions.len()),
        suppressed_or_deleted_returned_count,
        superseded_current_returned_count,
        graph_object_missing_omitted_count,
        graph_object_missing_returned_count,
        section_assignment_count: trace.map(|trace| trace.section_assignments.len()),
        section_assignment_counts: trace
            .map(|trace| {
                let mut counts = BTreeMap::new();
                for assignment in &trace.section_assignments {
                    *counts
                        .entry(format!("{:?}", assignment.section).to_ascii_snake_case())
                        .or_insert(0) += 1;
                }
                counts
            })
            .unwrap_or_default(),
        stale_candidate_omission_reasons: outcome
            .rationale
            .stale_candidate_omission_reasons
            .iter()
            .map(|summary| {
                (
                    format!("{:?}", summary.reason).to_ascii_snake_case(),
                    summary.count,
                )
            })
            .collect(),
        lifecycle_omission_reasons: outcome
            .rationale
            .lifecycle_omission_reasons
            .iter()
            .map(|summary| {
                (
                    format!("{:?}", summary.reason).to_ascii_snake_case(),
                    summary.count,
                )
            })
            .collect(),
    }
}

fn returned_object_ids(outcome: &character_memory::RetrieveOutcome) -> HashSet<MemoryId> {
    outcome
        .pack
        .active_threads
        .iter()
        .map(|thread| thread.id)
        .chain(
            outcome
                .pack
                .relevant_episodes
                .iter()
                .map(|episode| episode.id),
        )
        .chain(
            outcome
                .pack
                .salient_observations
                .iter()
                .map(|observation| observation.id),
        )
        .chain(
            outcome
                .pack
                .derived_memories
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .preferences
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .relationship_notes
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .open_loops
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .commitments
                .iter()
                .map(|derived| derived.memory.id),
        )
        .chain(
            outcome
                .pack
                .character_signals
                .iter()
                .map(|derived| derived.memory.id),
        )
        .collect()
}

trait SnakeCaseDebug {
    fn to_ascii_snake_case(&self) -> String;
}

impl SnakeCaseDebug for str {
    fn to_ascii_snake_case(&self) -> String {
        let mut out = String::new();
        for (idx, ch) in self.chars().enumerate() {
            if ch.is_ascii_uppercase() {
                if idx > 0 {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch);
            }
        }
        out
    }
}

fn render_context_text(items: &[RetrievedItem]) -> String {
    items
        .iter()
        .filter_map(|item| {
            item.text.as_ref().map(|text| {
                format!(
                    "[{}:{} rank={}] {}",
                    item.kind,
                    item.external_id.as_deref().unwrap_or("unknown"),
                    item.rank,
                    text
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn deterministic_id(namespace: &str, kind: &str, external_id: &str) -> MemoryId {
    Uuid::new_v5(
        &UUID_NAMESPACE,
        format!("{namespace}\0{kind}\0{external_id}").as_bytes(),
    )
}

fn parse_timestamp(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .with_context(|| {
                    format!(
                        "parse RFC3339 timestamp {value}; dataset loaders should normalize official benchmark timestamps before live adapter ingestion"
                    )
                })
        })
        .transpose()
}

fn resolve_ids(
    kind: &str,
    external_ids: &[String],
    persisted: &HashMap<String, MemoryId>,
    pending: &HashMap<String, MemoryId>,
) -> Result<Vec<MemoryId>> {
    external_ids
        .iter()
        .map(|external_id| {
            persisted
                .get(external_id)
                .or_else(|| pending.get(external_id))
                .copied()
                .ok_or_else(|| {
                    anyhow!("{kind} enrichment references unknown external_id {external_id}")
                })
        })
        .collect()
}

fn resolve_endpoint(
    endpoint: &MemoryEndpointInput,
    state: &NamespaceState,
    pending_entities: &HashMap<String, MemoryId>,
    pending_threads: &HashMap<String, MemoryId>,
    pending_derived: &HashMap<String, MemoryId>,
) -> Result<(ObjectType, MemoryId)> {
    let object_type = parse_object_type(&endpoint.object_type)?;
    if object_type == ObjectType::MemoryLink {
        bail!("memory links cannot be endpoints in enrichment links");
    }
    let id = match object_type {
        ObjectType::Episode => state.episode_ids.get(&endpoint.external_id).copied(),
        ObjectType::Observation => state.observation_ids.get(&endpoint.external_id).copied(),
        ObjectType::Entity => state
            .entity_ids
            .get(&endpoint.external_id)
            .or_else(|| pending_entities.get(&endpoint.external_id))
            .copied(),
        ObjectType::MemoryThread => state
            .thread_ids
            .get(&endpoint.external_id)
            .or_else(|| pending_threads.get(&endpoint.external_id))
            .copied(),
        ObjectType::DerivedMemory => state
            .derived_memory_ids
            .get(&endpoint.external_id)
            .or_else(|| pending_derived.get(&endpoint.external_id))
            .copied(),
        ObjectType::MemoryLink => None,
    }
    .ok_or_else(|| {
        anyhow!(
            "link endpoint {:?} references unknown external_id {}",
            object_type,
            endpoint.external_id
        )
    })?;
    Ok((object_type, id))
}

fn parse_entity_type(value: &str) -> Result<EntityType> {
    parse_snake_enum(value, "entity_type")
}

fn parse_derived_type(value: &str) -> Result<DerivedType> {
    parse_snake_enum(value, "derived_type")
}

fn parse_thread_status(value: &str) -> Result<ThreadStatus> {
    parse_snake_enum(value, "thread.status")
}

fn parse_stability(value: &str) -> Result<Stability> {
    parse_snake_enum(value, "derived_memory.stability")
}

fn parse_relation_type(value: &str) -> Result<RelationType> {
    parse_snake_enum(value, "link.relation")
}

fn parse_object_type(value: &str) -> Result<ObjectType> {
    parse_snake_enum(value, "endpoint.object_type")
}

fn parse_snake_enum<T>(value: &str, field: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .with_context(|| format!("parse {field} value {value:?}"))
}

fn validate_cleanup_target(collection_name: &str, required_prefix: Option<&str>) -> Result<()> {
    let Some(required_prefix) = required_prefix
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
    else {
        bail!("cleanup is enabled but no required collection prefix was configured");
    };
    let sanitized_prefix = sanitize_collection_segment(required_prefix);
    if sanitized_prefix.len() < 3 {
        bail!("cleanup required collection prefix is too broad");
    }
    if !collection_name.starts_with(&sanitized_prefix) {
        bail!(
            "refusing to cleanup collection {collection_name}; it does not start with required eval prefix {sanitized_prefix}"
        );
    }
    Ok(())
}

fn sanitize_collection_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

struct DeterministicEmbeddingProvider {
    vector_size: usize,
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddingProvider {
    fn vector_size(&self) -> usize {
        self.vector_size
    }

    async fn generate_embedding<'a>(
        &self,
        text: &'a str,
    ) -> std::result::Result<Vec<f32>, character_memory::CustomError> {
        Ok(self.vector_for_text(text))
    }

    async fn bulk_generate_embeddings<'a>(
        &self,
        texts: &'a [&'a str],
    ) -> std::result::Result<Vec<Vec<f32>>, character_memory::CustomError> {
        Ok(texts
            .iter()
            .map(|text| self.vector_for_text(text))
            .collect())
    }
}

impl DeterministicEmbeddingProvider {
    fn vector_for_text(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; self.vector_size];
        for token in text.split(|ch: char| !ch.is_alphanumeric()) {
            if token.is_empty() {
                continue;
            }
            let idx = stable_hash(token) % self.vector_size;
            embedding[idx] += 1.0;
        }
        if embedding.iter().all(|value| *value == 0.0) {
            embedding[0] = 1.0;
        }
        embedding
    }
}

fn stable_hash(text: &str) -> usize {
    text.bytes().fold(2166136261usize, |hash, byte| {
        hash.wrapping_mul(16777619) ^ usize::from(byte.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use character_memory::{
        CURRENT_SCHEMA_VERSION, ContinuityContextPack, Episode, LifecycleFilterDecision,
        MemoryObjectRef, Modality, RetrievalRationale, RetrievalTrace, RetrieveOutcome,
    };

    #[test]
    fn deterministic_ids_are_stable_and_namespaced() {
        let first = deterministic_id("n1", "episode", "s1");
        assert_eq!(first, deterministic_id("n1", "episode", "s1"));
        assert_ne!(first, deterministic_id("n2", "episode", "s1"));
        assert_ne!(first, deterministic_id("n1", "observation", "s1"));
    }

    #[test]
    fn renders_context_text_with_external_ids() {
        let text = render_context_text(&[RetrievedItem {
            kind: "observation".to_string(),
            internal_id: "i".to_string(),
            external_id: Some("s1:turn:1".to_string()),
            episode_external_id: Some("s1".to_string()),
            score: Some(0.5),
            rank: 1,
            rationale: vec![],
            text: Some("hello".to_string()),
        }]);
        assert!(text.contains("observation:s1:turn:1"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn telemetry_leakage_counts_only_final_returned_items() {
        let returned_id = deterministic_id("n", "episode", "returned");
        let omitted_id = deterministic_id("n", "episode", "omitted");
        let outcome = RetrieveOutcome {
            pack: ContinuityContextPack {
                relevant_episodes: vec![episode(returned_id)],
                ..ContinuityContextPack::empty()
            },
            rationale: RetrievalRationale::new("test"),
            trace: Some(RetrievalTrace {
                lifecycle_filter_decisions: vec![
                    LifecycleFilterDecision {
                        object: MemoryObjectRef::new(ObjectType::Episode, returned_id),
                        retention_state: Some(RetentionState::Suppressed),
                        is_current: None,
                        superseded_by: Vec::new(),
                        action: LifecycleFilterAction::Included,
                        reason: LifecycleFilterReason::SuppressedIncludedByPolicy,
                    },
                    LifecycleFilterDecision {
                        object: MemoryObjectRef::new(ObjectType::Episode, omitted_id),
                        retention_state: Some(RetentionState::Suppressed),
                        is_current: None,
                        superseded_by: Vec::new(),
                        action: LifecycleFilterAction::Included,
                        reason: LifecycleFilterReason::SuppressedIncludedByPolicy,
                    },
                ],
                ..RetrievalTrace::empty()
            }),
        };

        let telemetry = telemetry_from_outcome(&outcome);

        assert_eq!(telemetry.suppressed_or_deleted_returned_count, Some(1));
    }

    #[test]
    fn parses_rfc3339_timestamp_and_ignores_empty() {
        assert!(parse_timestamp(None).unwrap().is_none());
        assert!(parse_timestamp(Some("")).unwrap().is_none());
        assert_eq!(
            parse_timestamp(Some("2023-05-30T23:40:00Z"))
                .unwrap()
                .unwrap()
                .to_rfc3339(),
            "2023-05-30T23:40:00+00:00"
        );
    }

    #[test]
    fn rejects_unormalized_timestamp_with_actionable_context() {
        let err = parse_timestamp(Some("2023/05/30 (Tue) 23:40"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("dataset loaders should normalize"));
    }

    #[test]
    fn cleanup_target_requires_eval_prefix_match() {
        validate_cleanup_target("bench_lme_longmemeval_s_v0_1_lme_q1_abc", Some("bench:lme"))
            .unwrap();

        let err = validate_cleanup_target("prod_collection", Some("bench:lme"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("refusing to cleanup"));
    }

    #[test]
    fn cleanup_target_rejects_missing_prefix() {
        let err = validate_cleanup_target("bench_lme_q1", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no required collection prefix"));
    }

    fn episode(id: MemoryId) -> Episode {
        let now = Utc::now();
        Episode {
            id,
            object_type: ObjectType::Episode,
            modality: Modality::Chat,
            source_conversation_id: Some("external".to_string()),
            started_at: None,
            ended_at: None,
            participant_entity_ids: Vec::new(),
            summary: "summary".to_string(),
            raw_ref: Some("external".to_string()),
            salience_score: 0.5,
            retention_state: RetentionState::Active,
            created_at: now,
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        }
    }
}
