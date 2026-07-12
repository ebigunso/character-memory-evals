use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use chrono::{SecondsFormat, Utc};
use cmem_eval_core::{
    CommitWriteOptions, CorrectMemoryInput, CorrectionTargetInput, DerivedMemoryInput, EntityInput,
    ForgetMemoryInput, GraphEnrichmentInput, LinkMemoryInput, MemoryAdapter, MemoryEndpointInput,
    MemoryLinkInput, NamespaceLifecycleResult, PrepareWriteInput, ReplacementDerivedMemoryInput,
    RetrievalConfig, RetrieveInput, RetrievedContextPack, SourceProvenanceInput,
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
    pub operation_counts: BTreeMap<String, usize>,
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

    for event in &scenario.events {
        match event {
            InteractionEvent::Remember {
                event_id,
                memory_id,
                external_id,
                timestamp,
                text,
                entity_external_ids,
                ..
            } => {
                let observation_external_id = format!("{external_id}:observation");
                let mut plan = runtime
                    .adapter()
                    .prepare(PrepareWriteInput {
                        namespace: scenario.namespace.clone(),
                        content: text.clone(),
                        episode_external_id: external_id.clone(),
                        observation_external_id,
                        raw_refs: vec![format!(
                            "continuity://{}/{event_id}?at={}",
                            scenario.fixture_id,
                            timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                        )],
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
                    },
                );
                let association_links = entity_external_ids
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
                if !association_links.is_empty() {
                    let association_count = association_links.len();
                    runtime
                        .adapter()
                        .remember_enrichment(GraphEnrichmentInput {
                            namespace: scenario.namespace.clone(),
                            links: association_links,
                            ..GraphEnrichmentInput::default()
                        })
                        .await?;
                    for _ in 0..association_count {
                        increment(&mut run.operation_counts, "link");
                    }
                }
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
                let (target, supersedes_external_ids) = match target.object_type.as_str() {
                    "episode" | "observation" => (
                        CorrectionTargetInput::SourceObject {
                            object_type: target.object_type.clone(),
                            external_id: target_external_id.clone(),
                            original_raw_ref: None,
                            original_source_ref: None,
                        },
                        Vec::new(),
                    ),
                    "derived_memory" => (
                        CorrectionTargetInput::DerivedMemory {
                            external_id: target_external_id.clone(),
                        },
                        vec![target_external_id.clone()],
                    ),
                    object_type => bail!(
                        "scenario {:?} cannot correct object type {object_type:?}",
                        scenario.fixture_id
                    ),
                };
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
                    },
                );
                history.push(format!(
                    "{}|link|{from_external_id}|{relation}|{to_external_id}",
                    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                ));
            }
            InteractionEvent::Restart { timestamp, .. } => {
                runtime.restart(scenario).await?;
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
                let pack = runtime
                    .adapter()
                    .retrieve(RetrieveInput {
                        mode: retrieval.mode,
                        namespace: scenario.namespace.clone(),
                        query: text.clone(),
                        query_date: Some(timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)),
                        top_k_episodes: retrieval.top_k_episodes,
                        top_k_observations: retrieval.top_k_observations,
                        include_derived_memories: retrieval.include_derived_memories,
                        include_threads: retrieval.include_threads,
                        include_entities: retrieval.include_entities,
                        include_debug_rationale: true,
                    })
                    .await?;
                increment(&mut run.operation_counts, "retrieve");
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

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};
    use cmem_eval_core::{MockMemoryAdapter, RetrievalMode};
    use uuid::Uuid;

    #[derive(Default)]
    struct MockRuntime {
        adapter: MockMemoryAdapter,
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

    async fn run_all() -> (Vec<ContinuityQueryTrace>, BTreeMap<String, usize>) {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let mut traces = Vec::new();
        let mut operation_counts = BTreeMap::new();
        for scenario in &fixtures.scenarios {
            let mut runtime = MockRuntime::default();
            let run = run_continuity_scenario(&mut runtime, scenario, &retrieval())
                .await
                .unwrap();
            traces.extend(run.traces);
            for (operation, count) in run.operation_counts {
                *operation_counts.entry(operation).or_default() += count;
            }
        }
        (traces, operation_counts)
    }

    fn temporary_trace_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cmem-continuity-{}.jsonl", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn scenario_library_exercises_every_scripted_adapter_operation() {
        let (traces, counts) = run_all().await;
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED);
        let expected_link_count = fixtures
            .scenarios
            .iter()
            .flat_map(|scenario| &scenario.events)
            .map(|event| match event {
                InteractionEvent::Remember {
                    entity_external_ids,
                    ..
                } => 2 * entity_external_ids.len(),
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
    }

    #[tokio::test]
    async fn mock_driver_is_deterministic_for_identical_fixtures() {
        let first = run_all().await;
        let second = run_all().await;
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn trace_reader_rejects_corrupt_bytes_after_a_valid_trace() {
        let (traces, _) = run_all().await;
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
    async fn trace_reader_rejects_an_incompatible_schema_version() {
        let (mut traces, _) = run_all().await;
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
