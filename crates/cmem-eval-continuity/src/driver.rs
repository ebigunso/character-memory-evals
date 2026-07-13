use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use chrono::{SecondsFormat, Utc};
use cmem_eval_core::{
    CommitWriteOptions, CorrectMemoryInput, CorrectionTargetInput, DerivedMemoryInput, EntityInput,
    ForgetMemoryInput, GraphEnrichmentInput, LinkMemoryInput, MemoryAdapter, MemoryEndpointInput,
    MemoryLinkInput, MemoryThreadInput, NamespaceLifecycleResult, PrepareWriteInput,
    ReplacementDerivedMemoryInput, RetrievalConfig, RetrieveInput, RetrievedContextPack,
    SourceProvenanceInput,
};
use serde::{Deserialize, Serialize};

use crate::{ContinuityScenario, ExpectedRelevance, InteractionEvent, ScenarioPattern};

pub const CONTINUITY_TRACE_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
}

#[derive(Debug, Default, PartialEq)]
pub struct ContinuityScenarioRun {
    pub traces: Vec<ContinuityQueryTrace>,
    pub query_latencies_ms: BTreeMap<String, u128>,
    pub operation_counts: BTreeMap<String, usize>,
    pub restart_observations: Vec<RestartObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestartProbeSnapshot {
    pub returned_object_ids: Vec<String>,
    pub relevant_returned_count: usize,
    pub expected_relevant_count: usize,
    pub recall: Option<f64>,
    pub graph_relation_count: Option<usize>,
    pub graph_verified_count: Option<usize>,
    pub fanout_decision_count: Option<usize>,
    pub selectivity_decision_count: Option<usize>,
    pub scored_selectivity_count: Option<usize>,
    pub fallback_selectivity_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestartProbeDelta {
    pub returned_object_count: i64,
    pub relevant_returned_count: i64,
    pub recall: Option<f64>,
    pub graph_relation_count: Option<i64>,
    pub graph_verified_count: Option<i64>,
    pub fanout_decision_count: Option<i64>,
    pub selectivity_decision_count: Option<i64>,
    pub scored_selectivity_count: Option<i64>,
    pub fallback_selectivity_count: Option<i64>,
    pub stable_returned_objects: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    for trace in traces {
        serde_json::to_writer(&mut file, trace)?;
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
        let trace: ContinuityQueryTrace = serde_json::from_str(&line).with_context(|| {
            format!(
                "parse continuity trace line {line_number} from {}",
                path.display()
            )
        })?;
        if trace.schema_version != CONTINUITY_TRACE_SCHEMA_VERSION {
            bail!(
                "continuity trace line {line_number} in {} has schema_version {:?}; expected {:?}",
                path.display(),
                trace.schema_version,
                CONTINUITY_TRACE_SCHEMA_VERSION
            );
        }
        traces.push(trace);
    }
    Ok(traces)
}

#[async_trait]
pub trait ContinuityRuntime: Send {
    fn adapter(&self) -> &dyn MemoryAdapter;

    async fn restart(&mut self, scenario: &ContinuityScenario) -> Result<NamespaceLifecycleResult>;
}

#[derive(Debug, Clone)]
struct AdmittedObject {
    object_type: String,
    source_episode_external_id: Option<String>,
    original_raw_ref: Option<String>,
    original_source_ref: Option<String>,
}

pub async fn run_continuity_scenario(
    runtime: &mut dyn ContinuityRuntime,
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
    let mut admitted = scenario
        .entities
        .iter()
        .map(|entity| {
            (
                entity.external_id.clone(),
                AdmittedObject {
                    object_type: "entity".to_string(),
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
                entity_type: adapter_entity_type(&entity.entity_type)?.to_string(),
                name: entity.label.clone(),
                aliases: Vec::new(),
                canonical_key: Some(entity.external_id.clone()),
                summary: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    runtime
        .adapter()
        .remember_enrichment(GraphEnrichmentInput {
            namespace: scenario.namespace.clone(),
            entities,
            ..GraphEnrichmentInput::default()
        })
        .await?;
    increment(&mut run.operation_counts, "remember");

    for (event_index, event) in scenario.events.iter().enumerate() {
        match event {
            InteractionEvent::Remember {
                event_id,
                memory_id,
                external_id,
                timestamp,
                text,
                entity_external_ids,
                thread,
                salience,
            } => {
                let observation_external_id = format!("{external_id}:observation");
                let scripted_timestamp = timestamp.to_rfc3339_opts(SecondsFormat::Secs, true);
                let original_raw_ref = format!(
                    "continuity://{}/{event_id}?at={}",
                    scenario.fixture_id, scripted_timestamp
                );
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
                            "continuity:{}:{event_id}:{memory_id}",
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
                    .any(|validation| validation.status == "invalid")
                {
                    bail!(
                        "scenario {:?} event {event_id:?} produced an invalid write plan: {validations:?}",
                        scenario.fixture_id
                    );
                }
                plan.validations = validations;
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
                admitted.insert(
                    external_id.clone(),
                    AdmittedObject {
                        object_type: "episode".to_string(),
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: Some(original_raw_ref),
                        original_source_ref: Some(external_id.clone()),
                    },
                );
                let derived_external_id = format!("{external_id}:derived");
                let mut threads = Vec::new();
                let mut association_links = entity_external_ids
                    .iter()
                    .enumerate()
                    .flat_map(|(index, entity_external_id)| {
                        [
                            (
                                "mentions",
                                "mentions",
                                ("episode", external_id),
                                ("entity", entity_external_id),
                            ),
                            (
                                "involves",
                                "involves",
                                ("entity", entity_external_id),
                                ("episode", external_id),
                            ),
                        ]
                        .into_iter()
                        .map(move |(suffix, relation, from, to)| MemoryLinkInput {
                            external_id: format!(
                                "continuity:{}:{event_id}:entity-{suffix}-{index:04}",
                                scenario.fixture_id
                            ),
                            from: MemoryEndpointInput {
                                object_type: from.0.to_string(),
                                external_id: from.1.clone(),
                            },
                            relation: relation.to_string(),
                            to: MemoryEndpointInput {
                                object_type: to.0.to_string(),
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
                            title: text.clone(),
                            summary: String::new(),
                            status: "active".to_string(),
                            last_touched_at: Some(scripted_timestamp.clone()),
                            salience_score: *salience,
                            canonical_key: Some(thread.thread_external_id.clone()),
                        });
                        admitted.insert(
                            thread.thread_external_id.clone(),
                            AdmittedObject {
                                object_type: "memory_thread".to_string(),
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
                            object_type: "derived_memory".to_string(),
                            external_id: derived_external_id.clone(),
                        },
                        relation: "part_of_thread".to_string(),
                        to: MemoryEndpointInput {
                            object_type: "memory_thread".to_string(),
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
                runtime
                    .adapter()
                    .remember_enrichment(GraphEnrichmentInput {
                        namespace: scenario.namespace.clone(),
                        threads,
                        derived_memories: vec![DerivedMemoryInput {
                            external_id: derived_external_id.clone(),
                            derived_type: "reflection".to_string(),
                            text: text.clone(),
                            source_episode_external_ids: vec![external_id.clone()],
                            source_observation_external_ids: vec![observation_external_id.clone()],
                            thread_external_ids,
                            entity_external_ids: entity_external_ids.clone(),
                            confidence: thread.as_ref().map_or(1.0, |thread| thread.confidence),
                            salience_score: *salience,
                            stability: "medium".to_string(),
                            is_current: true,
                            supersedes_external_ids: Vec::new(),
                            metadata: serde_json::json!({
                                "continuity_event_id": event_id,
                                "fixture_memory_id": memory_id,
                                "timestamp": timestamp,
                            }),
                        }],
                        links: association_links,
                        ..GraphEnrichmentInput::default()
                    })
                    .await?;
                for _ in 0..association_count {
                    increment(&mut run.operation_counts, "link");
                }
                admitted.insert(
                    derived_external_id,
                    AdmittedObject {
                        object_type: "derived_memory".to_string(),
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: None,
                        original_source_ref: None,
                    },
                );
                history.push(format!(
                    "{}|remember|{external_id}|{text}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                ));
            }
            InteractionEvent::Correct {
                event_id,
                replacement_memory_id,
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
                runtime
                    .adapter()
                    .correct(CorrectMemoryInput {
                        namespace: scenario.namespace.clone(),
                        targets: vec![target],
                        replacements: vec![ReplacementDerivedMemoryInput {
                            memory: DerivedMemoryInput {
                                external_id: replacement_external_id.clone(),
                                derived_type: "reflection".to_string(),
                                text: replacement_text.clone(),
                                source_episode_external_ids: vec![
                                    source_episode_external_id.clone(),
                                ],
                                source_observation_external_ids: Vec::new(),
                                thread_external_ids: Vec::new(),
                                entity_external_ids: Vec::new(),
                                confidence: 1.0,
                                salience_score: 1.0,
                                stability: "medium".to_string(),
                                is_current: true,
                                supersedes_external_ids: supersedes_external_ids.clone(),
                                metadata: serde_json::json!({
                                    "continuity_event_id": event_id,
                                    "fixture_memory_id": replacement_memory_id,
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
                increment(&mut run.operation_counts, "correct");
                admitted.insert(
                    replacement_external_id.clone(),
                    AdmittedObject {
                        object_type: "derived_memory".to_string(),
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
                target_external_id,
                timestamp,
            } => {
                let target = admitted.get(target_external_id).with_context(|| {
                    format!(
                        "scenario {:?} forget target was not admitted: {target_external_id}",
                        scenario.fixture_id
                    )
                })?;
                if !matches!(
                    target.object_type.as_str(),
                    "episode" | "observation" | "derived_memory" | "memory_thread"
                ) {
                    bail!(
                        "scenario {:?} cannot forget object type {:?}",
                        scenario.fixture_id,
                        target.object_type
                    );
                }
                runtime
                    .adapter()
                    .forget(ForgetMemoryInput {
                        namespace: scenario.namespace.clone(),
                        targets: vec![MemoryEndpointInput {
                            object_type: target.object_type.clone(),
                            external_id: target_external_id.clone(),
                        }],
                        rationale: format!("fixture-scripted forget {event_id}"),
                        suppression_policy: Default::default(),
                        archive_policy: Default::default(),
                        cascade_policy: Default::default(),
                        target_retention_state: "suppressed".to_string(),
                        target_thread_status: None,
                        include_trace: true,
                    })
                    .await?;
                increment(&mut run.operation_counts, "forget");
                history.push(format!(
                    "{}|forget|{target_external_id}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
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
                runtime
                    .adapter()
                    .link(LinkMemoryInput {
                        namespace: scenario.namespace.clone(),
                        link: MemoryLinkInput {
                            external_id: external_id.clone(),
                            from,
                            relation: relation.clone(),
                            to,
                            confidence: 1.0,
                            rationale: Some(format!("fixture-scripted link {event_id}")),
                        },
                    })
                    .await?;
                increment(&mut run.operation_counts, "link");
                admitted.insert(
                    external_id.clone(),
                    AdmittedObject {
                        object_type: "memory_link".to_string(),
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
                });
            }
        }
    }

    Ok(run)
}

async fn retrieve_query(
    adapter: &dyn MemoryAdapter,
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
            top_k_episodes: retrieval.top_k_episodes,
            top_k_observations: retrieval.top_k_observations,
            include_derived_memories: retrieval.include_derived_memories,
            include_threads: retrieval.include_threads,
            include_entities: retrieval.include_entities,
            include_debug_rationale: true,
        })
        .await
}

fn restart_probe_snapshot(
    pack: &RetrievedContextPack,
    expected: &ExpectedRelevance,
) -> RestartProbeSnapshot {
    let mut returned_object_ids = pack
        .items
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
            pack.items.iter().any(|item| {
                item.external_id.as_ref() == Some(external_id)
                    || item.episode_external_id.as_ref() == Some(external_id)
            })
        })
        .count();
    let expected_relevant_count = expected.relevant_external_ids.len();
    let telemetry = &pack.telemetry;
    RestartProbeSnapshot {
        returned_object_ids,
        relevant_returned_count,
        expected_relevant_count,
        recall: (expected_relevant_count > 0)
            .then_some(relevant_returned_count as f64 / expected_relevant_count as f64),
        graph_relation_count: telemetry.graph_relation_count,
        graph_verified_count: telemetry.graph_verified_count,
        fanout_decision_count: telemetry.fanout_utilization.as_ref().map(Vec::len),
        selectivity_decision_count: telemetry.selectivity_decisions.as_ref().map(Vec::len),
        scored_selectivity_count: telemetry.selectivity_decisions.as_ref().map(|decisions| {
            decisions
                .iter()
                .filter(|decision| decision.score.is_some())
                .count()
        }),
        fallback_selectivity_count: telemetry.selectivity_decisions.as_ref().map(|decisions| {
            decisions
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
    if object.object_type == "memory_link" {
        bail!("scenario {fixture_id:?} cannot use a memory link as a link endpoint");
    }
    Ok(MemoryEndpointInput {
        object_type: object.object_type.clone(),
        external_id: external_id.to_string(),
    })
}

fn correction_target_input(
    fixture_id: &str,
    target_external_id: &str,
    target: &AdmittedObject,
) -> Result<(CorrectionTargetInput, Vec<String>)> {
    match target.object_type.as_str() {
        "episode" | "observation" => {
            if target.original_raw_ref.is_none() && target.original_source_ref.is_none() {
                bail!(
                    "scenario {fixture_id:?} source correction target {target_external_id:?} has no authoritative original reference"
                );
            }
            Ok((
                CorrectionTargetInput::SourceObject {
                    object_type: target.object_type.clone(),
                    external_id: target_external_id.to_string(),
                    original_raw_ref: target.original_raw_ref.clone(),
                    original_source_ref: target.original_source_ref.clone(),
                },
                Vec::new(),
            ))
        }
        "derived_memory" => Ok((
            CorrectionTargetInput::DerivedMemory {
                external_id: target_external_id.to_string(),
            },
            vec![target_external_id.to_string()],
        )),
        object_type => bail!("scenario {fixture_id:?} cannot correct object type {object_type:?}"),
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, operation: &str) {
    *counts.entry(operation.to_string()).or_default() += 1;
}

fn adapter_entity_type(fixture_entity_type: &str) -> Result<&'static str> {
    Ok(match fixture_entity_type {
        "location" => "place",
        "person" => "person",
        "organization" => "organization",
        unsupported => bail!(
            "unsupported continuity fixture entity_type {unsupported:?}; add an explicit facade mapping"
        ),
    })
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};
    use cmem_eval_core::{
        CandidateValidationResult, CommitWriteResult, EpisodeInput, LifecycleMutationResult,
        LinkMemoryResult, MockMemoryAdapter, ObservationInput, PreparedWritePlan, RetrievalMode,
    };
    use uuid::Uuid;

    #[derive(Default)]
    struct MockRuntime {
        adapter: MockMemoryAdapter,
    }

    #[derive(Clone, Default)]
    struct RecordingAdapter {
        inner: MockMemoryAdapter,
        prepared: Arc<Mutex<Vec<PrepareWriteInput>>>,
        enrichments: Arc<Mutex<Vec<GraphEnrichmentInput>>>,
    }

    #[async_trait]
    impl MemoryAdapter for RecordingAdapter {
        async fn open_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
            self.inner.open_namespace(namespace).await
        }

        async fn reattach_namespace(&self, namespace: &str) -> Result<NamespaceLifecycleResult> {
            self.inner.reattach_namespace(namespace).await
        }

        async fn reset_namespace(&self, namespace: &str) -> Result<()> {
            self.inner.reset_namespace(namespace).await
        }

        async fn remember_episode(&self, input: EpisodeInput) -> Result<String> {
            self.inner.remember_episode(input).await
        }

        async fn remember_observation(&self, input: ObservationInput) -> Result<String> {
            self.inner.remember_observation(input).await
        }

        async fn remember_enrichment(&self, input: GraphEnrichmentInput) -> Result<()> {
            self.enrichments.lock().unwrap().push(input.clone());
            self.inner.remember_enrichment(input).await
        }

        async fn link(&self, input: LinkMemoryInput) -> Result<LinkMemoryResult> {
            self.inner.link(input).await
        }

        async fn correct(&self, input: CorrectMemoryInput) -> Result<LifecycleMutationResult> {
            self.inner.correct(input).await
        }

        async fn forget(&self, input: ForgetMemoryInput) -> Result<LifecycleMutationResult> {
            self.inner.forget(input).await
        }

        async fn prepare(&self, input: PrepareWriteInput) -> Result<PreparedWritePlan> {
            self.prepared.lock().unwrap().push(input.clone());
            self.inner.prepare(input).await
        }

        async fn validate_plan(
            &self,
            plan: &PreparedWritePlan,
        ) -> Result<Vec<CandidateValidationResult>> {
            self.inner.validate_plan(plan).await
        }

        async fn commit(
            &self,
            plan: PreparedWritePlan,
            options: CommitWriteOptions,
        ) -> Result<CommitWriteResult> {
            self.inner.commit(plan, options).await
        }

        async fn retrieve(&self, input: RetrieveInput) -> Result<RetrievedContextPack> {
            self.inner.retrieve(input).await
        }
    }

    #[derive(Default)]
    struct RecordingRuntime {
        adapter: RecordingAdapter,
    }

    #[async_trait]
    impl ContinuityRuntime for MockRuntime {
        fn adapter(&self) -> &dyn MemoryAdapter {
            &self.adapter
        }

        async fn restart(
            &mut self,
            scenario: &ContinuityScenario,
        ) -> Result<NamespaceLifecycleResult> {
            self.adapter.reattach_namespace(&scenario.namespace).await
        }
    }

    #[async_trait]
    impl ContinuityRuntime for RecordingRuntime {
        fn adapter(&self) -> &dyn MemoryAdapter {
            &self.adapter
        }

        async fn restart(
            &mut self,
            scenario: &ContinuityScenario,
        ) -> Result<NamespaceLifecycleResult> {
            self.adapter
                .inner
                .reattach_namespace(&scenario.namespace)
                .await
        }
    }

    fn retrieval() -> RetrievalConfig {
        RetrievalConfig {
            mode: RetrievalMode::Hybrid,
            top_k_episodes: 8,
            top_k_observations: 8,
            include_derived_memories: true,
            include_threads: true,
            include_entities: true,
            include_debug_rationale: true,
            ..RetrievalConfig::default()
        }
    }

    async fn run_all() -> (
        Vec<ContinuityQueryTrace>,
        BTreeMap<String, usize>,
        Vec<RestartObservation>,
    ) {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let mut traces = Vec::new();
        let mut operation_counts = BTreeMap::new();
        let mut restart_observations = Vec::new();
        for scenario in &fixtures.scenarios {
            let mut runtime = MockRuntime::default();
            let run = run_continuity_scenario(&mut runtime, scenario, &retrieval())
                .await
                .unwrap();
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
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
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
        assert_eq!(traces.len(), 8);
        for operation in [
            "remember",
            "prepare",
            "validate_plan",
            "commit",
            "retrieve",
            "correct",
            "forget",
            "link",
            "restart",
        ] {
            assert!(counts.get(operation).is_some_and(|count| *count > 0));
        }
        assert_eq!(counts.get("link"), Some(&expected_link_count));
        assert_eq!(restart_observations.len(), 1);
        let restart = &restart_observations[0];
        assert!(restart.reopen_graph);
        assert!(restart.reopen_stats);
        assert!(restart.lifecycle.restored_identity_count > 0);
        assert!(restart.delta.stable_returned_objects);
        assert_eq!(restart.delta.returned_object_count, 0);
        assert_eq!(restart.delta.recall, Some(0.0));
    }

    #[test]
    fn restart_recall_matches_items_by_represented_episode_identity() {
        let pack = RetrievedContextPack {
            items: vec![cmem_eval_core::RetrievedItem {
                kind: "observation".to_string(),
                internal_id: "observation-internal".to_string(),
                external_id: Some("observation-external".to_string()),
                episode_external_id: Some("episode-relevant".to_string()),
                score: Some(1.0),
                rank: 1,
                rationale: Vec::new(),
                text: None,
            }],
            ..RetrievedContextPack::default()
        };
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
    async fn scripted_remember_executes_timestamps_threads_and_salience() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let thread_scenario = fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == ScenarioPattern::ThreadDrift)
            .unwrap();
        let mut thread_runtime = RecordingRuntime::default();
        run_continuity_scenario(&mut thread_runtime, thread_scenario, &retrieval())
            .await
            .unwrap();

        let remember_events = thread_scenario
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember { timestamp, .. } => Some(timestamp),
                _ => None,
            })
            .collect::<Vec<_>>();
        {
            let prepared = thread_runtime.adapter.prepared.lock().unwrap();
            assert_eq!(prepared.len(), remember_events.len());
            for (input, timestamp) in prepared.iter().zip(remember_events) {
                let expected = timestamp.to_rfc3339_opts(SecondsFormat::Secs, true);
                assert_eq!(input.episode_started_at.as_deref(), Some(expected.as_str()));
                assert_eq!(
                    input.observation_observed_at.as_deref(),
                    Some(expected.as_str())
                );
            }
        }

        {
            let enrichments = thread_runtime.adapter.enrichments.lock().unwrap();
            let scripted = enrichments
                .iter()
                .filter(|input| !input.derived_memories.is_empty())
                .collect::<Vec<_>>();
            assert_eq!(scripted.len(), 3);
            assert_eq!(
                scripted
                    .iter()
                    .map(|input| input.threads.len())
                    .sum::<usize>(),
                1
            );
            assert!(scripted.iter().all(|input| {
                input.derived_memories.len() == 1
                    && input.derived_memories[0].thread_external_ids == vec!["thread-1"]
            }));
            assert_eq!(
                scripted
                    .iter()
                    .flat_map(|input| &input.links)
                    .filter(|link| link.relation == "part_of_thread")
                    .map(|link| link.confidence)
                    .collect::<Vec<_>>(),
                vec![0.95, 0.65, 0.25]
            );
        }

        let salience_scenario = fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == ScenarioPattern::MixedSalienceAccumulation)
            .unwrap();
        let mut salience_runtime = RecordingRuntime::default();
        run_continuity_scenario(&mut salience_runtime, salience_scenario, &retrieval())
            .await
            .unwrap();
        let enrichments = salience_runtime.adapter.enrichments.lock().unwrap();
        assert_eq!(
            enrichments
                .iter()
                .flat_map(|input| &input.derived_memories)
                .map(|memory| memory.salience_score)
                .collect::<Vec<_>>(),
            vec![0.1, 0.5, 0.95]
        );
    }

    #[test]
    fn source_correction_target_preserves_authoritative_write_references() {
        let admitted = AdmittedObject {
            object_type: "episode".to_string(),
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
                object_type: "episode".to_string(),
                external_id: "delivery-v1".to_string(),
                original_raw_ref: Some(
                    "continuity://correction-chains/event-001?at=2025-01-01T08:00:00Z".to_string()
                ),
                original_source_ref: Some("delivery-v1".to_string()),
            }
        );
    }

    #[tokio::test]
    async fn mock_driver_is_deterministic_for_identical_fixtures() {
        let first = run_all().await;
        let second = run_all().await;
        assert_eq!(first, second);
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
            .write_all(b"{\"schema_version\":\"1.0.0\"")
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
        write_continuity_traces(&path, &traces[..1]).unwrap();

        let error = read_continuity_traces(&path).unwrap_err().to_string();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("9.9.9"), "{error}");
        assert!(error.contains(CONTINUITY_TRACE_SCHEMA_VERSION), "{error}");
    }

    #[test]
    fn fixture_entity_types_map_only_through_explicit_facade_vocabulary() {
        assert_eq!(adapter_entity_type("location").unwrap(), "place");
        assert_eq!(adapter_entity_type("person").unwrap(), "person");
        assert_eq!(adapter_entity_type("organization").unwrap(), "organization");
        assert!(adapter_entity_type("inferred-from-label").is_err());
    }
}
