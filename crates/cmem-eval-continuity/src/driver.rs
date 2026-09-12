use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use cmem_eval::{
    BenchmarkRunConfig, CandidateValidationStatus, CharacterMemoryAdapter, CommitWriteOptions,
    CorrectMemoryInput, CorrectionTargetInput, DerivedMemoryInput, DerivedType,
    EmbeddingRuntimeBinding, EntityInput, EntityType, EpisodeInput, ForgetCascadePolicyInput,
    ForgetMemoryInput, GraphEnrichmentInput, LinkMemoryInput, MemoryEndpointInput, MemoryLinkInput,
    MemoryThreadInput, NamespaceLifecycleResult, ObjectType, ObservationInput, PrepareWriteInput,
    RelationType, ReplacementDerivedMemoryInput, RetentionState, RetrievalConfig, RetrieveInput,
    RetrievedContextPack, SourceProvenanceInput, Stability, SuppressionPolicyInput, ThreadStatus,
};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;

use crate::{
    ContinuityEntityKind, ContinuityScenario, ExpectedRelevance, InteractionEvent, ScenarioPattern,
    derived_external_id, observation_external_id,
};

pub const CONTINUITY_TRACE_SCHEMA_VERSION: &str = "3.0.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContinuityQueryTrace {
    pub schema_version: String,
    pub fixture_id: String,
    pub namespace: String,
    pub pattern: ScenarioPattern,
    pub event_id: String,
    pub query_id: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub query: String,
    pub expected: ExpectedRelevance,
    pub history_text: String,
    pub retrieval: RetrievedContextPack,
    pub write_outcomes: Vec<cmem_eval::RecordedOutcome<cmem_eval::RememberOutcome>>,
    pub link_outcomes: Vec<cmem_eval::RecordedOutcome<cmem_eval::LinkOutcome>>,
    pub lifecycle_outcomes: Vec<cmem_eval::RecordedOutcome<cmem_eval::LifecycleMutationOutcome>>,
}

#[derive(Debug, Default, PartialEq)]
pub struct ContinuityScenarioRun {
    pub traces: Vec<ContinuityQueryTrace>,
    pub query_latencies_ms: BTreeMap<String, u128>,
    pub operation_counts: BTreeMap<String, usize>,
    pub restart_observations: Vec<RestartObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestartProbeSnapshot {
    pub returned_object_ids: Vec<String>,
    pub relevant_returned_count: usize,
    pub expected_relevant_count: usize,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub recall: Option<f64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub graph_relation_count: Option<usize>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub graph_verified_count: Option<usize>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub fanout_decision_count: Option<usize>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub selectivity_decision_count: Option<usize>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub scored_selectivity_count: Option<usize>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub fallback_selectivity_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestartProbeDelta {
    pub returned_object_count: i64,
    pub relevant_returned_count: i64,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub recall: Option<f64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub graph_relation_count: Option<i64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub graph_verified_count: Option<i64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub fanout_decision_count: Option<i64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub selectivity_decision_count: Option<i64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub scored_selectivity_count: Option<i64>,
    #[serde(deserialize_with = "cmem_eval::serde_contract::required_option")]
    pub fallback_selectivity_count: Option<i64>,
    pub stable_returned_objects: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestartObservation {
    pub event_id: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub reopen_graph: bool,
    pub reopen_stats: bool,
    pub lifecycle: NamespaceLifecycleResult,
    pub probe_query_id: String,
    pub before_restart: RestartProbeSnapshot,
    pub after_restart: RestartProbeSnapshot,
    pub delta: RestartProbeDelta,
}

pub fn write_continuity_traces(path: &Path, traces: &[ContinuityQueryTrace]) -> Result<()> {
    for (index, trace) in traces.iter().enumerate() {
        if trace.schema_version != CONTINUITY_TRACE_SCHEMA_VERSION {
            bail!(
                "continuity trace at index {index} has schema_version {:?}; expected {:?}",
                trace.schema_version,
                CONTINUITY_TRACE_SCHEMA_VERSION
            );
        }
    }
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    for trace in traces {
        let mut canonical = trace.clone();
        canonical
            .write_outcomes
            .sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        canonical
            .link_outcomes
            .sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
        canonical
            .lifecycle_outcomes
            .sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        serde_json::to_writer(&mut file, &canonical)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

pub fn read_continuity_traces(path: &Path) -> Result<Vec<ContinuityQueryTrace>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut traces = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let line = line.with_context(|| {
            format!(
                "read continuity trace line {line_number} from {}",
                path.display()
            )
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let schema_version = cmem_eval::serde_contract::schema_version_from_str(&line)
            .with_context(|| {
                format!(
                    "parse continuity trace line {line_number} from {}",
                    path.display()
                )
            })?;
        let trace = match schema_version.as_deref() {
            Some(CONTINUITY_TRACE_SCHEMA_VERSION) => {
                cmem_eval::serde_contract::reject_duplicate_json_keys(&line)?;
                serde_json::from_str(&line)?
            }
            Some(version) => bail!(
                "continuity trace line {line_number} in {} has schema_version {version:?}; expected {:?}",
                path.display(),
                CONTINUITY_TRACE_SCHEMA_VERSION
            ),
            None => bail!(
                "continuity trace line {line_number} in {} is missing schema_version",
                path.display()
            ),
        };
        traces.push(trace);
    }
    Ok(traces)
}

pub struct ContinuityRuntime {
    active: Option<Box<CharacterMemoryAdapter>>,
    config: Box<BenchmarkRunConfig>,
    embedding_binding: EmbeddingRuntimeBinding,
}

impl ContinuityRuntime {
    pub async fn new(
        config: &BenchmarkRunConfig,
        embedding_binding: EmbeddingRuntimeBinding,
    ) -> Result<Self> {
        let adapter =
            CharacterMemoryAdapter::new_with_binding(config, embedding_binding.clone()).await?;
        Ok(Self {
            active: Some(Box::new(adapter)),
            config: Box::new(config.clone()),
            embedding_binding,
        })
    }

    pub fn adapter(&self) -> &CharacterMemoryAdapter {
        self.active
            .as_ref()
            .expect("continuity runtime always holds an active adapter")
            .as_ref()
    }

    pub async fn restart(
        &mut self,
        scenario: &ContinuityScenario,
    ) -> Result<NamespaceLifecycleResult> {
        let previous = self
            .active
            .take()
            .context("continuity runtime lost its active adapter")?;
        previous.close().await?;
        let (replacement, lifecycle) = CharacterMemoryAdapter::reconstruct_with_binding(
            self.config.as_ref(),
            &scenario.namespace,
            self.embedding_binding.clone(),
        )
        .await?;
        self.active = Some(Box::new(replacement));
        Ok(lifecycle)
    }
}

#[derive(Debug, Clone)]
struct AdmittedObject {
    object_type: ObjectType,
    source_episode_external_id: Option<String>,
    original_raw_ref: Option<String>,
    original_source_ref: Option<String>,
}

pub async fn run_continuity_scenario(
    runtime: &mut ContinuityRuntime,
    scenario: &ContinuityScenario,
    retrieval: &RetrievalConfig,
) -> Result<ContinuityScenarioRun> {
    scenario.validate()?;
    runtime
        .adapter()
        .reset_namespace(&scenario.namespace)
        .await?;
    runtime
        .adapter()
        .open_namespace(&scenario.namespace)
        .await?;

    let mut run = ContinuityScenarioRun::default();
    let mut write_outcomes = Vec::new();
    let mut link_outcomes = Vec::new();
    let mut lifecycle_outcomes = Vec::new();
    let mut admitted = scenario
        .entities
        .iter()
        .map(|entity| {
            (
                entity.external_id.clone(),
                AdmittedObject {
                    object_type: ObjectType::Entity,
                    source_episode_external_id: None,
                    original_raw_ref: None,
                    original_source_ref: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut history = Vec::new();
    let entities = scenario
        .entities
        .iter()
        .map(|entity| {
            Ok(EntityInput {
                external_id: entity.external_id.clone(),
                entity_type: adapter_entity_type(entity.entity_type),
                name: entity.label.clone(),
                aliases: Vec::new(),
                canonical_key: Some(entity.external_id.clone()),
                summary: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    write_outcomes.extend(
        runtime
            .adapter()
            .remember_enrichment(GraphEnrichmentInput {
                namespace: scenario.namespace.clone(),
                entities,
                ..GraphEnrichmentInput::default()
            })
            .await?,
    );
    increment(&mut run.operation_counts, "remember");

    for (event_index, event) in scenario.events.iter().enumerate() {
        match event {
            InteractionEvent::Remember {
                event_id,
                external_id,
                timestamp,
                text,
                surface_texts,
                entity_external_ids,
                thread,
                salience,
            } => {
                let observation_external_id = observation_external_id(external_id);
                let scripted_timestamp = timestamp.to_rfc3339_opts(SecondsFormat::Secs, true);
                let original_raw_ref = format!(
                    "continuity://{}/{event_id}?at={}",
                    scenario.fixture_id, scripted_timestamp
                );
                let (episode_text, observation_text, derived_text) = surface_texts
                    .as_ref()
                    .map(|surface_texts| {
                        (
                            surface_texts.episode.as_str(),
                            surface_texts.observation.as_str(),
                            surface_texts.derived.as_str(),
                        )
                    })
                    .unwrap_or((text.as_str(), text.as_str(), text.as_str()));
                if surface_texts.is_some() {
                    let result = runtime
                        .adapter()
                        .remember_episode(EpisodeInput {
                            external_id: external_id.clone(),
                            namespace: scenario.namespace.clone(),
                            summary: episode_text.to_string(),
                            started_at: Some(scripted_timestamp.clone()),
                            ended_at: None,
                            participants: Vec::new(),
                            metadata: serde_json::json!({
                                "continuity_event_id": event_id,
                                "timestamp": timestamp,
                            }),
                        })
                        .await?;
                    write_outcomes.push(result.outcome);
                    increment(&mut run.operation_counts, "remember_episode");
                    let result = runtime
                        .adapter()
                        .remember_observation(ObservationInput {
                            external_id: observation_external_id.clone(),
                            episode_external_id: external_id.clone(),
                            namespace: scenario.namespace.clone(),
                            speaker: None,
                            text: observation_text.to_string(),
                            observed_at: Some(scripted_timestamp.clone()),
                            metadata: serde_json::json!({
                                "continuity_event_id": event_id,
                                "timestamp": timestamp,
                            }),
                        })
                        .await?;
                    write_outcomes.push(result.outcome);
                    increment(&mut run.operation_counts, "remember_observation");
                } else {
                    let mut plan = runtime
                        .adapter()
                        .prepare(PrepareWriteInput {
                            namespace: scenario.namespace.clone(),
                            content: text.clone(),
                            episode_external_id: external_id.clone(),
                            observation_external_id: observation_external_id.clone(),
                            episode_started_at: Some(scripted_timestamp.clone()),
                            observation_observed_at: Some(scripted_timestamp.clone()),
                            raw_refs: vec![original_raw_ref.clone()],
                            idempotency_key: Some(format!(
                                "continuity:{}:{event_id}:{external_id}",
                                scenario.fixture_id
                            )),
                            include_vector_index_candidates: true,
                            include_stats_update_candidates: true,
                        })
                        .await?;
                    increment(&mut run.operation_counts, "prepare");
                    let validations = runtime.adapter().validate_plan(&plan).await?;
                    increment(&mut run.operation_counts, "validate_plan");
                    if validations
                        .iter()
                        .any(|validation| validation.status == CandidateValidationStatus::Invalid)
                    {
                        bail!(
                            "scenario {:?} event {event_id:?} produced an invalid write plan: {validations:?}",
                            scenario.fixture_id
                        );
                    }
                    plan.plan.validations = validations;
                    let commit = runtime
                        .adapter()
                        .commit(plan, CommitWriteOptions::default())
                        .await?;
                    increment(&mut run.operation_counts, "commit");
                    if !commit.repair_needed.is_empty() {
                        bail!(
                            "scenario {:?} event {event_id:?} committed with repair-needed markers: {:?}",
                            scenario.fixture_id,
                            commit.repair_needed
                        );
                    }
                    if commit.vector_indexed_object_refs.is_empty() {
                        bail!(
                            "scenario {:?} event {event_id:?} committed without vector-indexed objects",
                            scenario.fixture_id
                        );
                    }
                    write_outcomes.push(commit.outcome);
                }
                admitted.insert(
                    external_id.clone(),
                    AdmittedObject {
                        object_type: ObjectType::Episode,
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: surface_texts.is_none().then(|| original_raw_ref.clone()),
                        original_source_ref: Some(external_id.clone()),
                    },
                );
                admitted.insert(
                    observation_external_id.clone(),
                    AdmittedObject {
                        object_type: ObjectType::Observation,
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: surface_texts.is_none().then_some(original_raw_ref),
                        original_source_ref: Some(external_id.clone()),
                    },
                );
                let derived_external_id = derived_external_id(external_id);
                let mut threads = Vec::new();
                let mut association_links = entity_external_ids
                    .iter()
                    .enumerate()
                    .flat_map(|(index, entity_external_id)| {
                        [
                            (
                                "mentions",
                                RelationType::Mentions,
                                (ObjectType::Episode, external_id),
                                (ObjectType::Entity, entity_external_id),
                            ),
                            (
                                "involves",
                                RelationType::Involves,
                                (ObjectType::Entity, entity_external_id),
                                (ObjectType::Episode, external_id),
                            ),
                        ]
                        .into_iter()
                        .map(move |(suffix, relation, from, to)| MemoryLinkInput {
                            external_id: format!(
                                "continuity:{}:{event_id}:entity-{suffix}-{index:04}",
                                scenario.fixture_id
                            ),
                            from: MemoryEndpointInput {
                                object_type: from.0,
                                external_id: from.1.clone(),
                            },
                            relation,
                            to: MemoryEndpointInput {
                                object_type: to.0,
                                external_id: to.1.clone(),
                            },
                            confidence: 1.0,
                            rationale: Some(format!(
                                "fixture-scripted entity association {event_id}"
                            )),
                        })
                    })
                    .collect::<Vec<_>>();
                let thread_external_ids = if let Some(thread) = thread {
                    if !admitted.contains_key(&thread.thread_external_id) {
                        threads.push(MemoryThreadInput {
                            external_id: thread.thread_external_id.clone(),
                            title: episode_text.to_string(),
                            summary: String::new(),
                            status: ThreadStatus::Active,
                            last_touched_at: Some(scripted_timestamp.clone()),
                            salience_score: *salience,
                            canonical_key: Some(thread.thread_external_id.clone()),
                        });
                        admitted.insert(
                            thread.thread_external_id.clone(),
                            AdmittedObject {
                                object_type: ObjectType::MemoryThread,
                                source_episode_external_id: None,
                                original_raw_ref: None,
                                original_source_ref: None,
                            },
                        );
                    }
                    association_links.push(MemoryLinkInput {
                        external_id: format!(
                            "continuity:{}:{event_id}:derived-thread",
                            scenario.fixture_id
                        ),
                        from: MemoryEndpointInput {
                            object_type: ObjectType::DerivedMemory,
                            external_id: derived_external_id.clone(),
                        },
                        relation: RelationType::PartOfThread,
                        to: MemoryEndpointInput {
                            object_type: ObjectType::MemoryThread,
                            external_id: thread.thread_external_id.clone(),
                        },
                        confidence: thread.confidence,
                        rationale: Some(format!("fixture-scripted thread membership {event_id}")),
                    });
                    vec![thread.thread_external_id.clone()]
                } else {
                    Vec::new()
                };
                let association_count = association_links.len();
                write_outcomes.extend(
                    runtime
                        .adapter()
                        .remember_enrichment(GraphEnrichmentInput {
                            namespace: scenario.namespace.clone(),
                            threads,
                            derived_memories: vec![DerivedMemoryInput {
                                external_id: derived_external_id.clone(),
                                derived_type: DerivedType::Reflection,
                                text: derived_text.to_string(),
                                source_episode_external_ids: vec![external_id.clone()],
                                source_observation_external_ids: vec![
                                    observation_external_id.clone(),
                                ],
                                thread_external_ids,
                                entity_external_ids: entity_external_ids.clone(),
                                confidence: thread.as_ref().map_or(1.0, |thread| thread.confidence),
                                salience_score: *salience,
                                stability: Stability::Medium,
                                is_current: true,
                                supersedes_external_ids: Vec::new(),
                                metadata: serde_json::json!({
                                    "continuity_event_id": event_id,
                                    "timestamp": timestamp,
                                }),
                            }],
                            links: association_links,
                            ..GraphEnrichmentInput::default()
                        })
                        .await?,
                );
                for _ in 0..association_count {
                    increment(&mut run.operation_counts, "link");
                }
                admitted.insert(
                    derived_external_id,
                    AdmittedObject {
                        object_type: ObjectType::DerivedMemory,
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: None,
                        original_source_ref: None,
                    },
                );
                let history_text = if surface_texts.is_some() {
                    format!(
                        "episode={episode_text}|observation={observation_text}|derived={derived_text}"
                    )
                } else {
                    text.clone()
                };
                history.push(format!(
                    "{}|remember|{external_id}|{history_text}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                ));
            }
            InteractionEvent::Correct {
                event_id,
                target_external_id,
                replacement_external_id,
                timestamp,
                replacement_text,
            } => {
                let target = admitted.get(target_external_id).with_context(|| {
                    format!(
                        "scenario {:?} correction target was not admitted: {target_external_id}",
                        scenario.fixture_id
                    )
                })?;
                let source_episode_external_id = target
                    .source_episode_external_id
                    .clone()
                    .context("correction target has no source episode")?;
                let provenance = SourceProvenanceInput {
                    episode_external_ids: vec![source_episode_external_id.clone()],
                    ..SourceProvenanceInput::default()
                };
                let (target, supersedes_external_ids) =
                    correction_target_input(&scenario.fixture_id, target_external_id, target)?;
                let result = runtime
                    .adapter()
                    .correct(CorrectMemoryInput {
                        namespace: scenario.namespace.clone(),
                        targets: vec![target],
                        replacements: vec![ReplacementDerivedMemoryInput {
                            memory: DerivedMemoryInput {
                                external_id: replacement_external_id.clone(),
                                derived_type: DerivedType::Reflection,
                                text: replacement_text.clone(),
                                source_episode_external_ids: vec![
                                    source_episode_external_id.clone(),
                                ],
                                source_observation_external_ids: Vec::new(),
                                thread_external_ids: Vec::new(),
                                entity_external_ids: Vec::new(),
                                confidence: 1.0,
                                salience_score: 1.0,
                                stability: Stability::Medium,
                                is_current: true,
                                supersedes_external_ids: supersedes_external_ids.clone(),
                                metadata: serde_json::json!({
                                    "continuity_event_id": event_id,
                                    "timestamp": timestamp,
                                }),
                            },
                            original_source_provenance: provenance.clone(),
                            correction_origin_provenance: provenance.clone(),
                        }],
                        superseded_derived_memory_external_ids: supersedes_external_ids,
                        correction_origin: provenance,
                        rationale: format!("fixture-scripted correction {event_id}"),
                        lifecycle_policy: Default::default(),
                        cascade_policy: Default::default(),
                        include_trace: true,
                    })
                    .await?;
                lifecycle_outcomes.push(result.outcome);
                increment(&mut run.operation_counts, "correct");
                admitted.insert(
                    replacement_external_id.clone(),
                    AdmittedObject {
                        object_type: ObjectType::DerivedMemory,
                        source_episode_external_id: Some(source_episode_external_id),
                        original_raw_ref: None,
                        original_source_ref: None,
                    },
                );
                history.push(format!(
                    "{}|correct|{target_external_id}|{replacement_external_id}|{replacement_text}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                ));
            }
            InteractionEvent::Forget {
                event_id,
                target_external_ids,
                timestamp,
                suppress_derived_from_target,
                apply_to_derived_from_target,
            } => {
                let targets = target_external_ids
                    .iter()
                    .map(|target_external_id| {
                        let target = admitted.get(target_external_id).with_context(|| {
                            format!(
                                "scenario {:?} forget target was not admitted: {target_external_id}",
                                scenario.fixture_id
                            )
                        })?;
                        if !matches!(
                            target.object_type,
                            ObjectType::Episode
                                | ObjectType::Observation
                                | ObjectType::DerivedMemory
                                | ObjectType::MemoryThread
                        ) {
                            bail!(
                                "scenario {:?} cannot forget object type {:?}",
                                scenario.fixture_id,
                                target.object_type
                            );
                        }
                        Ok(MemoryEndpointInput {
                            object_type: target.object_type,
                            external_id: target_external_id.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let result = runtime
                    .adapter()
                    .forget(ForgetMemoryInput {
                        namespace: scenario.namespace.clone(),
                        targets,
                        rationale: format!("fixture-scripted forget {event_id}"),
                        suppression_policy: SuppressionPolicyInput {
                            suppress_derived_from_target: *suppress_derived_from_target,
                            ..SuppressionPolicyInput::default()
                        },
                        archive_policy: Default::default(),
                        cascade_policy: ForgetCascadePolicyInput {
                            apply_to_derived_from_target: *apply_to_derived_from_target,
                            ..ForgetCascadePolicyInput::default()
                        },
                        target_retention_state: RetentionState::Suppressed,
                        target_thread_status: None,
                        include_trace: true,
                    })
                    .await?;
                lifecycle_outcomes.push(result.outcome);
                increment(&mut run.operation_counts, "forget");
                history.push(format!(
                    "{}|forget|{}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true),
                    target_external_ids.join(",")
                ));
            }
            InteractionEvent::Link {
                event_id,
                external_id,
                timestamp,
                from_external_id,
                relation,
                to_external_id,
                ..
            } => {
                let from = endpoint(&scenario.fixture_id, &admitted, from_external_id)?;
                let to = endpoint(&scenario.fixture_id, &admitted, to_external_id)?;
                let result = runtime
                    .adapter()
                    .link(LinkMemoryInput {
                        namespace: scenario.namespace.clone(),
                        link: MemoryLinkInput {
                            external_id: external_id.clone(),
                            from,
                            relation: serde_json::from_value(serde_json::Value::String(
                                relation.clone(),
                            ))?,
                            to,
                            confidence: 1.0,
                            rationale: Some(format!("fixture-scripted link {event_id}")),
                        },
                    })
                    .await?;
                link_outcomes.push(result.outcome);
                increment(&mut run.operation_counts, "link");
                admitted.insert(
                    external_id.clone(),
                    AdmittedObject {
                        object_type: ObjectType::MemoryLink,
                        source_episode_external_id: None,
                        original_raw_ref: None,
                        original_source_ref: None,
                    },
                );
                history.push(format!(
                    "{}|link|{from_external_id}|{relation}|{to_external_id}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                ));
            }
            InteractionEvent::Restart {
                event_id,
                timestamp,
                reopen_graph,
                reopen_stats,
            } => {
                let (probe_query_id, probe_timestamp, probe_text, probe_expected) = scenario.events
                    [event_index + 1..]
                    .iter()
                    .find_map(|event| match event {
                        InteractionEvent::Query {
                            query_id,
                            timestamp,
                            text,
                            expected,
                            ..
                        } => Some((query_id, timestamp, text, expected)),
                        _ => None,
                    })
                    .with_context(|| {
                        format!(
                            "scenario {:?} restart event {:?} has no following scripted query for re-measurement",
                            scenario.fixture_id, event_id
                        )
                    })?;
                let before_pack = retrieve_query(
                    runtime.adapter(),
                    scenario,
                    retrieval,
                    probe_timestamp,
                    probe_text,
                )
                .await?;
                let before_restart = restart_probe_snapshot(&before_pack, probe_expected);
                let lifecycle = runtime.restart(scenario).await?;
                let after_pack = retrieve_query(
                    runtime.adapter(),
                    scenario,
                    retrieval,
                    probe_timestamp,
                    probe_text,
                )
                .await?;
                let after_restart = restart_probe_snapshot(&after_pack, probe_expected);
                let delta = restart_probe_delta(&before_restart, &after_restart);
                run.restart_observations.push(RestartObservation {
                    event_id: event_id.clone(),
                    timestamp: *timestamp,
                    reopen_graph: *reopen_graph,
                    reopen_stats: *reopen_stats,
                    lifecycle,
                    probe_query_id: probe_query_id.clone(),
                    before_restart,
                    after_restart,
                    delta,
                });
                increment(&mut run.operation_counts, "restart");
                history.push(format!(
                    "{}|restart",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                ));
            }
            InteractionEvent::Query {
                event_id,
                query_id,
                timestamp,
                text,
                expected,
            } => {
                let query_started_at = Instant::now();
                let pack =
                    retrieve_query(runtime.adapter(), scenario, retrieval, timestamp, text).await?;
                let latency_ms = query_started_at.elapsed().as_millis();
                increment(&mut run.operation_counts, "retrieve");
                run.query_latencies_ms.insert(query_id.clone(), latency_ms);
                run.traces.push(ContinuityQueryTrace {
                    schema_version: CONTINUITY_TRACE_SCHEMA_VERSION.to_string(),
                    fixture_id: scenario.fixture_id.clone(),
                    namespace: scenario.namespace.clone(),
                    pattern: scenario.pattern,
                    event_id: event_id.clone(),
                    query_id: query_id.clone(),
                    timestamp: *timestamp,
                    query: text.clone(),
                    expected: expected.clone(),
                    history_text: history.join("\n"),
                    retrieval: pack,
                    write_outcomes: write_outcomes.clone(),
                    link_outcomes: link_outcomes.clone(),
                    lifecycle_outcomes: lifecycle_outcomes.clone(),
                });
            }
        }
    }

    Ok(run)
}

async fn retrieve_query(
    adapter: &CharacterMemoryAdapter,
    scenario: &ContinuityScenario,
    retrieval: &RetrievalConfig,
    timestamp: &chrono::DateTime<Utc>,
    text: &str,
) -> Result<RetrievedContextPack> {
    adapter
        .retrieve(RetrieveInput {
            mode: retrieval.mode,
            namespace: scenario.namespace.clone(),
            query: text.to_string(),
            query_date: Some(timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)),
            surface_policy: {
                let mut policy = retrieval.surface_policy.clone();
                policy.include_debug_rationale = true;
                policy
            },
        })
        .await
}

fn restart_probe_snapshot(
    pack: &RetrievedContextPack,
    expected: &ExpectedRelevance,
) -> RestartProbeSnapshot {
    let mut returned_object_ids = pack
        .items()
        .iter()
        .map(|item| {
            item.external_id
                .clone()
                .unwrap_or_else(|| format!("{}:{}", item.kind, item.internal_id))
        })
        .collect::<Vec<_>>();
    returned_object_ids.sort();
    returned_object_ids.dedup();
    let relevant_returned_count = expected
        .relevant_external_ids
        .iter()
        .filter(|external_id| {
            pack.items().iter().any(|item| {
                item.external_id.as_ref() == Some(external_id)
                    || item.episode_external_id.as_ref() == Some(external_id)
            })
        })
        .count();
    let expected_relevant_count = expected.relevant_external_ids.len();
    let outcome = pack.outcomes().first();
    let native_trace = outcome.and_then(|outcome| outcome.trace.as_ref());
    RestartProbeSnapshot {
        returned_object_ids,
        relevant_returned_count,
        expected_relevant_count,
        recall: (expected_relevant_count > 0)
            .then_some(relevant_returned_count as f64 / expected_relevant_count as f64),
        graph_relation_count: native_trace.map(|trace| trace.graph_relations.len()),
        graph_verified_count: outcome.map(|outcome| outcome.rationale.graph_verified_count),
        fanout_decision_count: native_trace.map(|trace| trace.fanout_utilization.len()),
        selectivity_decision_count: native_trace.map(|trace| trace.selectivity_decisions.len()),
        scored_selectivity_count: native_trace.map(|trace| {
            trace
                .selectivity_decisions
                .iter()
                .filter(|decision| decision.score.is_some())
                .count()
        }),
        fallback_selectivity_count: native_trace.map(|trace| {
            trace
                .selectivity_decisions
                .iter()
                .filter(|decision| decision.fallback)
                .count()
        }),
    }
}

fn restart_probe_delta(
    before: &RestartProbeSnapshot,
    after: &RestartProbeSnapshot,
) -> RestartProbeDelta {
    RestartProbeDelta {
        returned_object_count: signed_delta(
            before.returned_object_ids.len(),
            after.returned_object_ids.len(),
        ),
        relevant_returned_count: signed_delta(
            before.relevant_returned_count,
            after.relevant_returned_count,
        ),
        recall: option_f64_delta(before.recall, after.recall),
        graph_relation_count: option_usize_delta(
            before.graph_relation_count,
            after.graph_relation_count,
        ),
        graph_verified_count: option_usize_delta(
            before.graph_verified_count,
            after.graph_verified_count,
        ),
        fanout_decision_count: option_usize_delta(
            before.fanout_decision_count,
            after.fanout_decision_count,
        ),
        selectivity_decision_count: option_usize_delta(
            before.selectivity_decision_count,
            after.selectivity_decision_count,
        ),
        scored_selectivity_count: option_usize_delta(
            before.scored_selectivity_count,
            after.scored_selectivity_count,
        ),
        fallback_selectivity_count: option_usize_delta(
            before.fallback_selectivity_count,
            after.fallback_selectivity_count,
        ),
        stable_returned_objects: before.returned_object_ids == after.returned_object_ids,
    }
}

fn signed_delta(before: usize, after: usize) -> i64 {
    after as i64 - before as i64
}

fn option_usize_delta(before: Option<usize>, after: Option<usize>) -> Option<i64> {
    Some(signed_delta(before?, after?))
}

fn option_f64_delta(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    Some(after? - before?)
}

fn endpoint(
    fixture_id: &str,
    admitted: &BTreeMap<String, AdmittedObject>,
    external_id: &str,
) -> Result<MemoryEndpointInput> {
    let object = admitted.get(external_id).with_context(|| {
        format!("scenario {fixture_id:?} link endpoint was not admitted: {external_id}")
    })?;
    if object.object_type == ObjectType::MemoryLink {
        bail!("scenario {fixture_id:?} cannot use a memory link as a link endpoint");
    }
    Ok(MemoryEndpointInput {
        object_type: object.object_type,
        external_id: external_id.to_string(),
    })
}

fn correction_target_input(
    fixture_id: &str,
    target_external_id: &str,
    target: &AdmittedObject,
) -> Result<(CorrectionTargetInput, Vec<String>)> {
    match target.object_type {
        ObjectType::Episode | ObjectType::Observation => {
            if target.original_raw_ref.is_none() && target.original_source_ref.is_none() {
                bail!(
                    "scenario {fixture_id:?} source correction target {target_external_id:?} has no authoritative original reference"
                );
            }
            Ok((
                CorrectionTargetInput::SourceObject {
                    object_type: target.object_type,
                    external_id: target_external_id.to_string(),
                    original_raw_ref: target.original_raw_ref.clone(),
                    original_source_ref: target.original_source_ref.clone(),
                },
                Vec::new(),
            ))
        }
        ObjectType::DerivedMemory => Ok((
            CorrectionTargetInput::DerivedMemory {
                external_id: target_external_id.to_string(),
            },
            vec![target_external_id.to_string()],
        )),
        ObjectType::Entity | ObjectType::MemoryThread | ObjectType::MemoryLink => bail!(
            "scenario {fixture_id:?} cannot correct object type {:?}",
            target.object_type
        ),
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, operation: &str) {
    *counts.entry(operation.to_string()).or_default() += 1;
}

fn adapter_entity_type(fixture_entity_type: ContinuityEntityKind) -> EntityType {
    match fixture_entity_type {
        ContinuityEntityKind::Location => EntityType::Place,
        ContinuityEntityKind::Person => EntityType::Person,
        ContinuityEntityKind::Organization => EntityType::Organization,
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};
    use cmem_eval::{RetrievalMode, RetrievalSectionBudgets, RetrievalSurfacePolicy};
    use uuid::Uuid;

    async fn run_embedded(scenario: &ContinuityScenario) -> ContinuityScenarioRun {
        let directory = tempfile::tempdir().unwrap();
        let mut config = BenchmarkRunConfig {
            run_id: "driver-test".into(),
            dataset: cmem_eval::DatasetId::new("continuity").unwrap(),
            backend: Default::default(),
            retrieval: retrieval(),
            ingest: Default::default(),
            metrics: Default::default(),
        };
        config.backend.namespace_prefix = Some("cmem_eval_driver_test".into());
        config.backend.cleanup.require_collection_prefix = Some("cmem_eval_driver_test".into());
        config.backend.identity_registry_dir =
            Some(directory.path().join("identities").display().to_string());
        config.backend.oxigraph_persistence_path =
            Some(directory.path().join("graph").display().to_string());
        config.backend.retrieval_stats_path =
            Some(directory.path().join("stats.sqlite").display().to_string());
        let binding = if let Some(fixture) = scenario.embedding.controllable_similarity() {
            config.backend.embedding.provider =
                cmem_eval::EmbeddingProviderConfig::ControllableSimilarity;
            config.backend.embedding.vector_size = Some(fixture.vector_size);
            EmbeddingRuntimeBinding::Controllable {
                dimension_policy: cmem_eval::ControllableDimensionPolicy::Exact {
                    vector_size: fixture.vector_size,
                },
                fixture: fixture.clone(),
            }
        } else {
            let store_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/embeddings/task22_real_store.json");
            let store = cmem_eval::FrozenEmbeddingProvider::load(
                &store_path,
                "text-embedding-3-large",
                3072,
            )
            .unwrap();
            config.backend.embedding.provider = cmem_eval::EmbeddingProviderConfig::Frozen;
            config.backend.embedding.vector_size = Some(store.vector_size());
            config.backend.embedding.store_path = Some(store_path.display().to_string());
            EmbeddingRuntimeBinding::Frozen {
                dimension_policy: store.dimension_policy(),
                store,
            }
        };
        let mut runtime = ContinuityRuntime::new(&config, binding).await.unwrap();
        let run = run_continuity_scenario(&mut runtime, scenario, &config.retrieval)
            .await
            .unwrap();
        runtime
            .adapter()
            .reset_namespace(&scenario.namespace)
            .await
            .unwrap();
        runtime.active.take().unwrap().close().await.unwrap();
        run
    }

    fn retrieval() -> RetrievalConfig {
        RetrievalConfig {
            mode: RetrievalMode::Hybrid,
            surface_policy: RetrievalSurfacePolicy {
                sections: RetrievalSectionBudgets {
                    salient_observations: 8,
                    ..RetrievalSectionBudgets::default()
                },
                include_debug_rationale: true,
                ..RetrievalSurfacePolicy::default()
            },
        }
    }

    async fn run_all() -> (
        Vec<ContinuityQueryTrace>,
        BTreeMap<String, usize>,
        Vec<RestartObservation>,
    ) {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let mut traces = Vec::new();
        let mut operation_counts = BTreeMap::new();
        let mut restart_observations = Vec::new();
        for scenario in &fixtures.scenarios {
            let run = run_embedded(scenario).await;
            traces.extend(run.traces);
            restart_observations.extend(run.restart_observations);
            for (operation, count) in run.operation_counts {
                *operation_counts.entry(operation).or_default() += count;
            }
        }
        (traces, operation_counts, restart_observations)
    }

    fn temporary_trace_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cmem-continuity-{}.jsonl", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn scenario_library_exercises_every_scripted_adapter_operation() {
        let (traces, counts, restart_observations) = run_all().await;
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let expected_link_count = fixtures
            .scenarios
            .iter()
            .flat_map(|scenario| &scenario.events)
            .map(|event| match event {
                InteractionEvent::Remember {
                    entity_external_ids,
                    thread,
                    ..
                } => 2 * entity_external_ids.len() + usize::from(thread.is_some()),
                InteractionEvent::Link { .. } => 1,
                _ => 0,
            })
            .sum::<usize>();
        assert_eq!(traces.len(), 23);
        for operation in [
            "remember",
            "prepare",
            "validate_plan",
            "commit",
            "remember_episode",
            "remember_observation",
            "retrieve",
            "correct",
            "forget",
            "link",
            "restart",
        ] {
            assert!(counts.get(operation).is_some_and(|count| *count > 0));
        }
        assert_eq!(counts.get("link"), Some(&expected_link_count));
        assert!(traces.iter().any(|trace| {
            trace.write_outcomes.iter().any(|outcome| {
                !outcome.outcome.persisted_link_ids.is_empty()
                    && outcome.outcome.stats_update_status.failure.is_none()
            })
        }));
        assert_eq!(restart_observations.len(), 1);
        let restart = &restart_observations[0];
        assert!(restart.reopen_graph);
        assert!(restart.reopen_stats);
        assert!(restart.lifecycle.restored_identity_count > 0);
        assert!(restart.delta.stable_returned_objects);
        assert_eq!(restart.delta.returned_object_count, 0);
        assert_eq!(restart.delta.recall, Some(0.0));
    }

    #[tokio::test]
    async fn parser_admitted_implicit_objects_are_available_to_driver_operations() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();

        let mut correction = fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == ScenarioPattern::CorrectionChains)
            .unwrap()
            .clone();
        let InteractionEvent::Correct {
            target_external_id, ..
        } = &mut correction.events[1]
        else {
            panic!("expected correction event");
        };
        *target_external_id = "delivery-v1:observation".to_string();
        correction.validate().unwrap();
        run_embedded(&correction).await;

        let mut thread = fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == ScenarioPattern::ThreadDrift)
            .unwrap()
            .clone();
        let query_index = thread
            .events
            .iter()
            .position(|event| matches!(event, InteractionEvent::Query { .. }))
            .unwrap();
        let timestamp = thread.events[query_index - 1].timestamp() + chrono::Duration::seconds(1);
        let thread_external_id = thread
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
        thread.events.insert(
            query_index,
            InteractionEvent::Link {
                event_id: "event-test-observation-link".to_string(),
                external_id: "test-observation-link".to_string(),
                timestamp,
                from_external_id: "thread-focus:observation".to_string(),
                relation: "mentions".to_string(),
                to_external_id: "thread-focus:derived".to_string(),
            },
        );
        thread.events.insert(
            query_index + 1,
            InteractionEvent::Forget {
                event_id: "event-test-thread-forget".to_string(),
                target_external_ids: vec![thread_external_id],
                timestamp: timestamp + chrono::Duration::seconds(1),
                suppress_derived_from_target: false,
                apply_to_derived_from_target: false,
            },
        );
        thread.validate().unwrap();
        run_embedded(&thread).await;
    }

    #[test]
    fn restart_recall_matches_items_by_represented_episode_identity() {
        let pack = RetrievedContextPack::from_ranked_items(
            vec![cmem_eval::RetrievedItem {
                kind: ObjectType::Observation,
                internal_id: "observation-internal".to_string(),
                external_id: Some("observation-external".to_string()),
                episode_external_id: Some("episode-relevant".to_string()),
                score: Some(1.0),
                rank: 1,
                rationale: Vec::new(),
                text: None,
            }],
            Default::default(),
            cmem_eval::ContextRenderer::PlainText,
        );
        let expected = ExpectedRelevance {
            relevant_external_ids: vec!["episode-relevant".to_string()],
            irrelevant_external_ids: vec!["episode-negative".to_string()],
        };

        let snapshot = restart_probe_snapshot(&pack, &expected);

        assert_eq!(snapshot.relevant_returned_count, 1);
        assert_eq!(snapshot.recall, Some(1.0));
        assert_eq!(
            snapshot.returned_object_ids,
            vec!["observation-external".to_string()]
        );
    }

    #[tokio::test]
    async fn scripted_remember_persists_timestamps_threads_and_salience() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        for pattern in [
            ScenarioPattern::ThreadDrift,
            ScenarioPattern::MixedSalienceAccumulation,
            ScenarioPattern::SurfaceContribution,
        ] {
            let scenario = fixtures
                .scenarios
                .iter()
                .find(|scenario| scenario.pattern == pattern)
                .unwrap();
            let run = run_embedded(scenario).await;
            let packs = run
                .traces
                .iter()
                .flat_map(|trace| trace.retrieval.outcomes())
                .map(|outcome| &outcome.pack)
                .collect::<Vec<_>>();
            assert!(packs.iter().any(|pack| !pack.relevant_episodes.is_empty()));
            for pack in packs {
                for episode in &pack.relevant_episodes {
                    let external = episode.source_conversation_id.as_deref().unwrap();
                    let (timestamp, text) = scenario
                        .events
                        .iter()
                        .find_map(|event| match event {
                            InteractionEvent::Remember {
                                external_id,
                                timestamp,
                                text,
                                surface_texts,
                                ..
                            } if external_id == external => Some((
                                timestamp,
                                surface_texts
                                    .as_ref()
                                    .map_or(text, |surface| &surface.episode),
                            )),
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(episode.started_at.as_ref(), Some(timestamp));
                    assert_eq!(&episode.summary, text);
                }
                for derived in &pack.derived_memories {
                    let expected = scenario
                        .events
                        .iter()
                        .find_map(|event| match event {
                            InteractionEvent::Remember {
                                text,
                                surface_texts,
                                salience,
                                thread,
                                ..
                            } if surface_texts
                                .as_ref()
                                .map_or(text, |surface| &surface.derived)
                                == &derived.memory.text =>
                            {
                                Some((salience, thread))
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(derived.memory.salience_score, *expected.0);
                    if expected.1.is_some() {
                        assert_eq!(derived.memory.thread_ids.len(), 1);
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn correction_forget_records_explicit_native_targets() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == ScenarioPattern::CorrectionChains)
            .unwrap();
        let run = run_embedded(scenario).await;
        assert!(
            run.traces
                .iter()
                .flat_map(|trace| &trace.lifecycle_outcomes)
                .any(|record| record
                    .outcome
                    .trace
                    .as_ref()
                    .is_some_and(|trace| trace.requested_targets.len() == 2))
        );
    }

    #[test]
    fn source_correction_target_preserves_authoritative_write_references() {
        let admitted = AdmittedObject {
            object_type: ObjectType::Episode,
            source_episode_external_id: Some("delivery-v1".to_string()),
            original_raw_ref: Some(
                "continuity://correction-chains/event-001?at=2025-01-01T08:00:00Z".to_string(),
            ),
            original_source_ref: Some("delivery-v1".to_string()),
        };

        let (target, supersedes) =
            correction_target_input("correction-chains", "delivery-v1", &admitted).unwrap();

        assert_eq!(supersedes, Vec::<String>::new());
        assert_eq!(
            target,
            CorrectionTargetInput::SourceObject {
                object_type: ObjectType::Episode,
                external_id: "delivery-v1".to_string(),
                original_raw_ref: Some(
                    "continuity://correction-chains/event-001?at=2025-01-01T08:00:00Z".to_string()
                ),
                original_source_ref: Some("delivery-v1".to_string()),
            }
        );
    }

    #[tokio::test]
    async fn trace_writer_canonicalizes_outcomes_by_operation() {
        let (mut traces, _, _) = run_all().await;
        let mut trace = traces.remove(0);
        let template = trace.write_outcomes.first().unwrap().clone();
        trace.write_outcomes = ["b", "c", "a"]
            .into_iter()
            .map(|id| {
                let mut record = template.clone();
                record.operation_id = id.into();
                record
            })
            .collect();
        let lifecycle_template = traces
            .iter()
            .flat_map(|trace| &trace.lifecycle_outcomes)
            .next()
            .unwrap()
            .clone();
        trace.lifecycle_outcomes = ["b", "c", "a"]
            .into_iter()
            .map(|id| {
                let mut record = lifecycle_template.clone();
                record.operation_id = id.into();
                record
            })
            .collect();
        let first_path = temporary_trace_path();
        let second_path = temporary_trace_path();
        write_continuity_traces(&first_path, std::slice::from_ref(&trace)).unwrap();
        trace.write_outcomes.reverse();
        trace.lifecycle_outcomes.reverse();
        write_continuity_traces(&second_path, &[trace]).unwrap();

        let first = std::fs::read_to_string(&first_path).unwrap();
        assert_eq!(first, std::fs::read_to_string(&second_path).unwrap());
        let value: Value = serde_json::from_str(first.trim()).unwrap();
        for family in ["write_outcomes", "lifecycle_outcomes"] {
            let operation_ids = value[family]
                .as_array()
                .unwrap()
                .iter()
                .map(|outcome| outcome["operation_id"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(operation_ids, vec!["a", "b", "c"]);
        }

        std::fs::remove_file(first_path).unwrap();
        std::fs::remove_file(second_path).unwrap();
    }

    #[tokio::test]
    async fn trace_reader_rejects_corrupt_bytes_after_a_valid_trace() {
        let (traces, _, _) = run_all().await;
        let path = temporary_trace_path();
        write_continuity_traces(&path, &traces[..1]).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&[0xff, b'\n'])
            .unwrap();

        let error = read_continuity_traces(&path).unwrap_err().to_string();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("read continuity trace line 2"), "{error}");
    }

    #[tokio::test]
    async fn trace_reader_rejects_truncated_json_after_a_valid_trace() {
        let (traces, _, _) = run_all().await;
        let path = temporary_trace_path();
        write_continuity_traces(&path, &traces[..1]).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"schema_version\":\"2.2.0\"")
            .unwrap();

        let error = read_continuity_traces(&path).unwrap_err().to_string();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("parse continuity trace line 2"), "{error}");
    }

    #[tokio::test]
    async fn trace_reader_rejects_an_incompatible_schema_version() {
        let (mut traces, _, _) = run_all().await;
        traces[0].schema_version = "9.9.9".to_string();
        let path = temporary_trace_path();
        std::fs::write(&path, "preserved\n").unwrap();
        let error = write_continuity_traces(&path, &[traces[1].clone(), traces[0].clone()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("index 1"), "{error}");
        assert!(error.contains("9.9.9"), "{error}");
        assert!(error.contains(CONTINUITY_TRACE_SCHEMA_VERSION), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserved\n");
        std::fs::remove_file(&path).unwrap();
    }

    #[tokio::test]
    async fn trace_reader_round_trips_current_and_rejects_superseded_schemas() {
        let (traces, _, _) = run_all().await;
        let path = temporary_trace_path();
        write_continuity_traces(&path, &traces[..1]).unwrap();
        let decoded = read_continuity_traces(&path).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].schema_version, CONTINUITY_TRACE_SCHEMA_VERSION);
        assert_eq!(decoded[0].fixture_id, traces[0].fixture_id);
        assert_eq!(decoded[0].query_id, traces[0].query_id);

        let mut legacy = serde_json::to_value(&traces[0]).unwrap();
        let object = legacy.as_object_mut().unwrap();
        object.remove("write_outcomes");
        object.remove("lifecycle_outcomes");

        for version in ["1.0.0", "2.0.0"] {
            legacy["schema_version"] = Value::String(version.to_string());
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string(&legacy).unwrap()),
            )
            .unwrap();
            let error = read_continuity_traces(&path).unwrap_err().to_string();
            assert!(error.contains("schema_version"), "{error}");
            assert!(error.contains(version), "{error}");
            assert!(error.contains(CONTINUITY_TRACE_SCHEMA_VERSION), "{error}");
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[tokio::test]
    async fn trace_reader_rejects_owned_shape_drift() {
        let (traces, _, _) = run_all().await;
        let path = temporary_trace_path();
        let value = serde_json::to_value(&traces[0]).unwrap();
        for field in [
            "write_outcomes",
            "link_outcomes",
            "lifecycle_outcomes",
            "retrieval",
        ] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            std::fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
            assert!(read_continuity_traces(&path).is_err());
        }
        let mut unknown = value;
        unknown["retrieval"]["unexpected_field"] = Value::Bool(true);
        std::fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert!(read_continuity_traces(&path).is_err());
        std::fs::write(
            &path,
            r#"{"schema_version":"3.0.0","schema_version":"3.0.0"}"#,
        )
        .unwrap();
        assert!(format!("{:#}", read_continuity_traces(&path).unwrap_err()).contains("duplicate"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn trace_reader_rejects_missing_schema_version() {
        let path = temporary_trace_path();
        std::fs::write(&path, "{}\n").unwrap();
        let error = read_continuity_traces(&path).unwrap_err().to_string();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("missing schema_version"), "{error}");
    }

    #[test]
    fn fixture_entity_types_map_only_through_explicit_facade_vocabulary() {
        assert_eq!(
            adapter_entity_type(ContinuityEntityKind::Location),
            EntityType::Place
        );
        assert_eq!(
            adapter_entity_type(ContinuityEntityKind::Person),
            EntityType::Person
        );
        assert_eq!(
            adapter_entity_type(ContinuityEntityKind::Organization),
            EntityType::Organization
        );
    }
}
