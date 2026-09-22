use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use cmem_eval::{
    BeliefAssertionInput, BeliefPredicate, BenchmarkRunConfig, CandidateValidationStatus,
    CharacterMemoryAdapter, CommitWriteOptions, CorrectMemoryInput, CorrectionTargetInput,
    DerivedMemoryInput, DerivedType, EmbeddingRuntimeBinding, EntityInput, EpisodeInput,
    ForgetCascadePolicyInput, ForgetMemoryInput, GraphEnrichmentInput, LinkMemoryInput,
    MemoryEndpointInput, MemoryLinkInput, MemoryThreadInput, NamespaceLifecycleResult, ObjectType,
    ObservationInput, PrepareWriteInput, PreparedWritePlan, RelationType,
    ReplacementDerivedMemoryInput, RetrievalConfig, RetrieveInput, RetrievedContextPack,
    SourceProvenanceInput, SuppressionPolicyInput, ThreadStatus,
};
use serde::{Deserialize, Serialize};

use crate::{
    AuthoredMemoryKind, ContinuityScenario, ExpectedRelevance, InteractionEvent,
    PerceivedReference, ScenarioFeature, ScenarioOutcome, ScenarioPattern, SituatedInput,
    derived_external_id, naming_belief_external_id, observation_external_id,
};

// Keep this declaration and the one scenario-to-contract mapping below together.
// Only whole features forwarded by the pinned library belong here.
pub const SUPPORTED_SCENARIO_FEATURES: &[ScenarioFeature] = &[
    ScenarioFeature::WriteScene,
    ScenarioFeature::WriteSceneWhere,
    ScenarioFeature::WriteSceneCustom,
    ScenarioFeature::ProbeScene,
    ScenarioFeature::ProbeActivity,
    ScenarioFeature::NoTopic,
    ScenarioFeature::ReferenceTime,
    ScenarioFeature::ParticipantName,
    ScenarioFeature::ParticipantDescription,
    ScenarioFeature::PlaceName,
    ScenarioFeature::PlaceDescription,
    ScenarioFeature::ReferenceTrace,
    ScenarioFeature::MemorySceneTrace,
    ScenarioFeature::CueTrace,
    ScenarioFeature::OmissionReasons,
    ScenarioFeature::AuthoredDerivedMemory,
    ScenarioFeature::PackSections,
    ScenarioFeature::PackOrder,
];

pub fn scenario_missing_features(scenario: &ContinuityScenario) -> Result<Vec<ScenarioFeature>> {
    Ok(scenario
        .analyze()?
        .features
        .into_iter()
        .filter(|feature| !SUPPORTED_SCENARIO_FEATURES.contains(feature))
        .collect())
}

enum MappedSituatedInput {
    Experience(PrepareWriteInput),
    Derive(GraphEnrichmentInput),
    Probe(RetrieveInput),
}

fn map_scene(timestamp: &str, scene: crate::SceneInput) -> Result<cmem_eval::MemorySceneInput> {
    anyhow::ensure!(
        scene.what.is_none(),
        "unsupported activity passed the feature gate"
    );
    let participants = scene
        .who
        .into_iter()
        .map(|reference| {
            let mut participant = cmem_eval::SceneParticipantInput::default();
            match reference {
                PerceivedReference::Key { key } => participant.key = Some(key),
                PerceivedReference::Name { text } => participant.name = Some(text),
                PerceivedReference::Description { text } => participant.description = Some(text),
                PerceivedReference::Setting { .. } => bail!("setting cannot be a participant"),
            }
            Ok(participant)
        })
        .collect::<Result<_>>()?;
    let mut setting = cmem_eval::character_memory::SceneSetting::default();
    match scene.place {
        Some(PerceivedReference::Key { key } | PerceivedReference::Setting { key }) => {
            setting.key = Some(key)
        }
        Some(PerceivedReference::Name { text } | PerceivedReference::Description { text }) => {
            setting.words = Some(text)
        }
        None => {}
    }
    Ok(cmem_eval::MemorySceneInput {
        time: Some(timestamp.into()),
        participants,
        setting,
        custom_values: scene.custom,
    })
}

fn map_situated_input(
    scenario: &ContinuityScenario,
    timestamp: chrono::DateTime<Utc>,
    input: SituatedInput,
) -> Result<MappedSituatedInput> {
    let namespace = scenario.namespace.as_str();
    let timestamp = timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, true);
    Ok(match input {
        SituatedInput::Experience {
            external_id,
            text,
            scene,
            speaker,
            salience,
        } => MappedSituatedInput::Experience(PrepareWriteInput {
            namespace: namespace.into(),
            content: text,
            observation_external_id: observation_external_id(&external_id),
            episode_external_id: external_id,
            scene: map_scene(&timestamp, scene)?,
            speaker_entity_external_id: speaker,
            salience,
            observation_observed_at: Some(timestamp),
            raw_refs: Vec::new(),
            include_vector_index_candidates: true,
            include_stats_update_candidates: true,
        }),
        SituatedInput::Derive {
            external_id,
            memory,
        } => {
            anyhow::ensure!(
                memory.actor.is_none()
                    && memory.counterpart.is_none()
                    && memory.due.is_none()
                    && memory.trigger.is_none(),
                "unsupported derived fields passed the feature gate"
            );
            let mut input = GraphEnrichmentInput {
                namespace: namespace.into(),
                ..Default::default()
            };
            let derived_type = match memory.subtype {
                AuthoredMemoryKind::Reflection => DerivedType::Reflection,
                AuthoredMemoryKind::RelationshipNote => DerivedType::RelationshipNote,
                AuthoredMemoryKind::OpenLoop => DerivedType::OpenLoop,
                AuthoredMemoryKind::Commitment => DerivedType::Commitment,
                AuthoredMemoryKind::CharacterSignal => DerivedType::CharacterSignal,
                _ => bail!("unsupported derived subtype passed the feature gate"),
            };
            input.derived_memories.push(DerivedMemoryInput {
                external_id,
                created_at: Some(timestamp),
                derived_type,
                text: memory.text,
                source_episode_external_ids: memory.experiences,
                source_observation_external_ids: Vec::new(),
                thread_external_ids: Vec::new(),
                entity_external_ids: memory.about,
                salience_score: 0.5,
                assertions: Vec::new(),
                given_by_application: false,
                supersedes_external_ids: memory.supersedes,
                metadata: serde_json::Value::Null,
            });
            MappedSituatedInput::Derive(input)
        }
        SituatedInput::Probe {
            mut scene, topic, ..
        } => {
            let activity = match scene.what.take() {
                None => None,
                Some(PerceivedReference::Key { key }) => Some(
                    scenario
                        .events
                        .iter()
                        .find_map(|event| match event {
                            InteractionEvent::Derive {
                                event_id, memory, ..
                            } if event_id == &key => match memory.subtype {
                                AuthoredMemoryKind::Thread => {
                                    Some(cmem_eval::ActivityInput::Thread(key.clone()))
                                }
                                AuthoredMemoryKind::OpenLoop => {
                                    Some(cmem_eval::ActivityInput::OpenLoop(key.clone()))
                                }
                                _ => None,
                            },
                            _ => None,
                        })
                        .context("activity must name an admitted thread or open loop")?,
                ),
                Some(_) => bail!("unsupported textual activity passed the feature gate"),
            };
            MappedSituatedInput::Probe(RetrieveInput {
                activity,
                cue_floors: None,
                time_range: None,
                mode: cmem_eval::RetrievalMode::Hybrid,
                namespace: namespace.into(),
                topic,
                scene: map_scene(&timestamp, scene)?,
                surface_policy: Default::default(),
            })
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityQueryTrace {
    #[serde(flatten)]
    pub result: cmem_eval::PerQuestionResult,
    pub fixture_id: String,
    pub namespace: String,
    pub event_id: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub expected: ExpectedRelevanceRecord,
    pub history_text: String,
    pub restart_observations: Vec<RestartObservation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContinuityQueryObservation {
    pub fixture_id: String,
    pub namespace: String,
    pub pattern: ScenarioPattern,
    pub event_id: String,
    pub query_id: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub query: String,
    pub expected: ExpectedRelevanceRecord,
    pub history_text: String,
    pub retrieval: RetrievedContextPack,
    pub write_outcomes: Vec<cmem_eval::RememberOutcome>,
    pub link_outcomes: Vec<cmem_eval::LinkOutcome>,
    pub lifecycle_outcomes: Vec<cmem_eval::LifecycleMutationOutcome>,
}

/// Expected labels recorded in an artifact; fixture admission remains separate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpectedRelevanceRecord {
    pub relevant_external_ids: Vec<String>,
    pub irrelevant_external_ids: Vec<String>,
}

impl From<&ExpectedRelevance> for ExpectedRelevanceRecord {
    fn from(expected: &ExpectedRelevance) -> Self {
        Self {
            relevant_external_ids: expected.relevant_external_ids.clone(),
            irrelevant_external_ids: expected.irrelevant_external_ids.clone(),
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct ContinuityScenarioRun {
    pub outcome: ScenarioOutcome,
    pub traces: Vec<ContinuityQueryObservation>,
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
    let mut file = File::create_new(path).with_context(|| format!("create {}", path.display()))?;
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
        let trace = serde_json::from_str(&line).with_context(|| {
            format!(
                "parse continuity trace line {line_number} from {}",
                path.display()
            )
        })?;
        traces.push(trace);
    }
    Ok(traces)
}

pub struct ContinuityRuntime {
    active: Option<Box<CharacterMemoryAdapter>>,
    config: Box<BenchmarkRunConfig>,
    embedding_binding: EmbeddingRuntimeBinding,
    run_root: std::path::PathBuf,
}

impl ContinuityRuntime {
    pub async fn new(
        run_root: &Path,
        config: &BenchmarkRunConfig,
        embedding_binding: EmbeddingRuntimeBinding,
    ) -> Result<Self> {
        let adapter =
            CharacterMemoryAdapter::new_with_binding(run_root, config, embedding_binding.clone())
                .await?;
        Ok(Self {
            active: Some(Box::new(adapter)),
            config: Box::new(config.clone()),
            embedding_binding,
            run_root: run_root.to_path_buf(),
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
            &self.run_root,
            self.config.as_ref(),
            &scenario.namespace,
            self.embedding_binding.clone(),
        )
        .await?;
        self.active = Some(Box::new(replacement));
        Ok(lifecycle)
    }

    pub async fn cleanup(&self, namespace: &str) -> Result<()> {
        if let Some(adapter) = &self.active {
            adapter.cleanup_namespace(namespace).await
        } else {
            // A failed restart has already closed the old handles; reconstruct only the
            // adapter configuration so its owned partial stores can still be removed.
            CharacterMemoryAdapter::new_with_binding(
                &self.run_root,
                &self.config,
                self.embedding_binding.clone(),
            )
            .await?
            .cleanup_namespace(namespace)
            .await
        }
    }
}

#[derive(Debug, Clone)]
struct AdmittedObject {
    object_type: ObjectType,
    source_episode_external_id: Option<String>,
    original_raw_ref: Option<String>,
    original_setting_key: Option<String>,
}

pub async fn run_continuity_scenario(
    runtime: &mut ContinuityRuntime,
    scenario: &ContinuityScenario,
    retrieval: &RetrievalConfig,
) -> Result<ContinuityScenarioRun> {
    let missing = scenario_missing_features(scenario)?;
    if !missing.is_empty() {
        return Ok(ContinuityScenarioRun {
            outcome: ScenarioOutcome::not_run(scenario, missing),
            ..Default::default()
        });
    }
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
                    original_setting_key: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut history = Vec::new();
    let entities = scenario
        .entities
        .iter()
        .map(|entity| EntityInput {
            external_id: entity.external_id.clone(),
        })
        .collect();
    let naming_beliefs = scenario
        .entities
        .iter()
        .map(|entity| DerivedMemoryInput {
            external_id: naming_belief_external_id(&entity.external_id),
            created_at: None,
            derived_type: DerivedType::Claim,
            text: entity.label.clone(),
            source_episode_external_ids: Vec::new(),
            source_observation_external_ids: Vec::new(),
            thread_external_ids: Vec::new(),
            entity_external_ids: vec![entity.external_id.clone()],
            assertions: vec![BeliefAssertionInput {
                subject_external_id: entity.external_id.clone(),
                predicate: BeliefPredicate::KnownAs {
                    name: entity.label.clone(),
                },
            }],
            given_by_application: true,
            salience_score: 0.5,
            supersedes_external_ids: Vec::new(),
            metadata: serde_json::Value::Null,
        })
        .collect();

    write_outcomes.extend(
        runtime
            .adapter()
            .remember_enrichment(GraphEnrichmentInput {
                namespace: scenario.namespace.clone(),
                entities,
                derived_memories: naming_beliefs,
                ..GraphEnrichmentInput::default()
            })
            .await?
            .map(|outcome| checked_write_outcome(scenario, "entities", outcome))
            .transpose()?,
    );
    increment(&mut run.operation_counts, "remember");

    for (event_index, event) in scenario.events.iter().enumerate() {
        match event {
            InteractionEvent::Experience { .. }
            | InteractionEvent::Derive { .. }
            | InteractionEvent::Probe { .. } => {
                let input = scenario.situated_input(event)?.expect("situated event");
                match map_situated_input(scenario, event.timestamp(), input)? {
                    MappedSituatedInput::Experience(input) => {
                        let external_id = input.episode_external_id.clone();
                        let observation_id = input.observation_external_id.clone();
                        let original_setting_key = input.scene.setting.key.clone();
                        history.push(format!(
                            "{}|experience|{}|{}",
                            event.timestamp(),
                            external_id,
                            input.content
                        ));
                        let plan = runtime.adapter().prepare(input).await?;
                        increment(&mut run.operation_counts, "prepare");
                        write_outcomes.push(
                            commit_validated_plan(
                                runtime.adapter(),
                                scenario,
                                event.event_id(),
                                plan,
                                &mut run.operation_counts,
                            )
                            .await?,
                        );
                        increment(&mut run.operation_counts, "experience");
                        for (id, object_type) in [
                            (external_id.clone(), ObjectType::Episode),
                            (observation_id, ObjectType::Observation),
                        ] {
                            let raw_ref =
                                format!("eval://{}/{}/{}", scenario.namespace, object_type, id);
                            admitted.insert(
                                id,
                                AdmittedObject {
                                    object_type,
                                    source_episode_external_id: Some(external_id.clone()),
                                    original_raw_ref: Some(raw_ref),
                                    original_setting_key: original_setting_key.clone(),
                                },
                            );
                        }
                    }
                    MappedSituatedInput::Derive(input) => {
                        for memory in &input.derived_memories {
                            admitted.insert(
                                memory.external_id.clone(),
                                AdmittedObject {
                                    object_type: ObjectType::DerivedMemory,
                                    source_episode_external_id: memory
                                        .source_episode_external_ids
                                        .first()
                                        .cloned(),
                                    original_raw_ref: None,
                                    original_setting_key: None,
                                },
                            );
                            history.push(format!(
                                "{}|derive|{}|{}",
                                event.timestamp(),
                                memory.external_id,
                                memory.text
                            ));
                        }
                        write_outcomes.extend(
                            runtime
                                .adapter()
                                .remember_enrichment(input)
                                .await?
                                .map(|outcome| {
                                    checked_write_outcome(scenario, event.event_id(), outcome)
                                })
                                .transpose()?,
                        );
                        increment(&mut run.operation_counts, "derive");
                    }
                    MappedSituatedInput::Probe(mut input) => {
                        let InteractionEvent::Probe {
                            query_id,
                            topic,
                            assertions,
                            measures,
                            ..
                        } = event
                        else {
                            unreachable!("mapped probe event")
                        };
                        input.mode = retrieval.mode;
                        input.surface_policy = retrieval.surface_policy.clone();
                        input.surface_policy.include_debug_rationale = true;
                        let started = Instant::now();
                        let pack = runtime.adapter().retrieve(input).await?;
                        run.query_latencies_ms
                            .insert(query_id.clone(), started.elapsed().as_millis());
                        increment(&mut run.operation_counts, "retrieve");
                        run.outcome.record_probe(scenario, event, &pack);
                        run.traces.push(ContinuityQueryObservation {
                            fixture_id: scenario.fixture_id.clone(),
                            namespace: scenario.namespace.clone(),
                            pattern: scenario.pattern,
                            event_id: event.event_id().into(),
                            query_id: query_id.clone(),
                            timestamp: event.timestamp(),
                            query: topic.clone().unwrap_or_default(),
                            expected: ExpectedRelevanceRecord {
                                relevant_external_ids: assertions
                                    .carried
                                    .iter()
                                    .map(|assertion| assertion.memory.clone())
                                    .collect(),
                                irrelevant_external_ids: measures.bystanders.clone(),
                            },
                            history_text: history.join("\n"),
                            retrieval: pack,
                            write_outcomes: write_outcomes.clone(),
                            link_outcomes: link_outcomes.clone(),
                            lifecycle_outcomes: lifecycle_outcomes.clone(),
                        });
                    }
                }
            }
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
                            scene: cmem_eval::MemorySceneInput {
                                time: Some(scripted_timestamp.clone()),
                                setting: cmem_eval::character_memory::SceneSetting {
                                    key: Some(external_id.clone()),
                                    words: None,
                                },
                                ..Default::default()
                            },
                            ended_at: None,
                            metadata: serde_json::Value::Null,
                        })
                        .await?;
                    write_outcomes.push(checked_write_outcome(scenario, event_id, result.outcome)?);
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
                            metadata: serde_json::Value::Null,
                        })
                        .await?;
                    write_outcomes.push(checked_write_outcome(scenario, event_id, result.outcome)?);
                    increment(&mut run.operation_counts, "remember_observation");
                } else {
                    let plan = runtime
                        .adapter()
                        .prepare(PrepareWriteInput {
                            namespace: scenario.namespace.clone(),
                            content: text.clone(),
                            episode_external_id: external_id.clone(),
                            observation_external_id: observation_external_id.clone(),

                            speaker_entity_external_id: None,
                            salience: None,
                            scene: cmem_eval::MemorySceneInput {
                                time: Some(scripted_timestamp.clone()),
                                setting: cmem_eval::character_memory::SceneSetting {
                                    key: Some(external_id.clone()),
                                    words: None,
                                },
                                ..Default::default()
                            },
                            observation_observed_at: Some(scripted_timestamp.clone()),
                            raw_refs: vec![original_raw_ref.clone()],
                            include_vector_index_candidates: true,
                            include_stats_update_candidates: true,
                        })
                        .await?;
                    increment(&mut run.operation_counts, "prepare");
                    write_outcomes.push(
                        commit_validated_plan(
                            runtime.adapter(),
                            scenario,
                            event_id,
                            plan,
                            &mut run.operation_counts,
                        )
                        .await?,
                    );
                }
                admitted.insert(
                    external_id.clone(),
                    AdmittedObject {
                        object_type: ObjectType::Episode,
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: surface_texts.is_none().then(|| original_raw_ref.clone()),
                        original_setting_key: Some(external_id.clone()),
                    },
                );
                admitted.insert(
                    observation_external_id.clone(),
                    AdmittedObject {
                        object_type: ObjectType::Observation,
                        source_episode_external_id: Some(external_id.clone()),
                        original_raw_ref: surface_texts.is_none().then_some(original_raw_ref),
                        original_setting_key: Some(external_id.clone()),
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
                                original_setting_key: None,
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
                                created_at: None,
                                external_id: derived_external_id.clone(),
                                derived_type: DerivedType::Reflection,
                                text: derived_text.to_string(),
                                source_episode_external_ids: vec![external_id.clone()],
                                source_observation_external_ids: vec![
                                    observation_external_id.clone(),
                                ],
                                thread_external_ids,
                                entity_external_ids: entity_external_ids.clone(),
                                salience_score: *salience,
                                assertions: Vec::new(),
                                given_by_application: false,
                                supersedes_external_ids: Vec::new(),
                                metadata: serde_json::Value::Null,
                            }],
                            links: association_links,
                            ..GraphEnrichmentInput::default()
                        })
                        .await?
                        .map(|outcome| checked_write_outcome(scenario, event_id, outcome))
                        .transpose()?,
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
                        original_setting_key: None,
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
                                created_at: None,
                                external_id: replacement_external_id.clone(),
                                derived_type: DerivedType::Reflection,
                                text: replacement_text.clone(),
                                source_episode_external_ids: vec![
                                    source_episode_external_id.clone(),
                                ],
                                source_observation_external_ids: Vec::new(),
                                thread_external_ids: Vec::new(),
                                entity_external_ids: Vec::new(),
                                salience_score: 1.0,
                                assertions: Vec::new(),
                                given_by_application: false,
                                supersedes_external_ids: supersedes_external_ids.clone(),
                                metadata: serde_json::Value::Null,
                            },
                            original_source_provenance: provenance.clone(),
                            correction_origin_provenance: provenance.clone(),
                        }],
                        superseded_derived_memory_external_ids: supersedes_external_ids,
                        correction_origin: provenance,
                        rationale: format!("fixture-scripted correction {event_id}"),
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
                        original_setting_key: None,
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
                        cascade_policy: ForgetCascadePolicyInput {
                            apply_to_derived_from_target: *apply_to_derived_from_target,
                            ..ForgetCascadePolicyInput::default()
                        },
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
                        original_setting_key: None,
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
                run.outcome.record_retrieval(&before_pack);
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
                run.outcome.record_retrieval(&after_pack);
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
                run.outcome.record_retrieval(&pack);
                increment(&mut run.operation_counts, "retrieve");
                run.query_latencies_ms.insert(query_id.clone(), latency_ms);
                run.traces.push(ContinuityQueryObservation {
                    fixture_id: scenario.fixture_id.clone(),
                    namespace: scenario.namespace.clone(),
                    pattern: scenario.pattern,
                    event_id: event_id.clone(),
                    query_id: query_id.clone(),
                    timestamp: *timestamp,
                    query: text.clone(),
                    expected: expected.into(),
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

async fn commit_validated_plan(
    adapter: &CharacterMemoryAdapter,
    scenario: &ContinuityScenario,
    event_id: &str,
    mut plan: PreparedWritePlan,
    operation_counts: &mut BTreeMap<String, usize>,
) -> Result<cmem_eval::RememberOutcome> {
    let validations = adapter.validate_plan(&plan).await?;
    increment(operation_counts, "validate_plan");
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
    let commit = adapter.commit(plan, CommitWriteOptions::default()).await?;
    increment(operation_counts, "commit");
    checked_write_outcome(scenario, event_id, commit.outcome)
}

fn checked_write_outcome(
    scenario: &ContinuityScenario,
    event_id: &str,
    recorded: cmem_eval::RememberOutcome,
) -> Result<cmem_eval::RememberOutcome> {
    let outcome = &recorded;
    if let Some(failure) = &outcome.vector_indexing_failure {
        bail!(
            "scenario {:?} event {event_id:?} committed with vector-indexing failure: {failure:?}",
            scenario.fixture_id
        );
    }
    if !outcome.repair_needed.is_empty() {
        bail!(
            "scenario {:?} event {event_id:?} committed with repair-needed markers: {:?}",
            scenario.fixture_id,
            outcome.repair_needed
        );
    }
    if let Some(failure) = &outcome.stats_update_status.failure {
        bail!(
            "scenario {:?} event {event_id:?} committed with stats-update failure: {failure:?}",
            scenario.fixture_id
        );
    }
    if outcome.vector_indexed_object_ids.is_empty() {
        bail!(
            "scenario {:?} event {event_id:?} committed without vector-indexed objects",
            scenario.fixture_id
        );
    }
    Ok(recorded)
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
            activity: None,
            cue_floors: None,
            time_range: None,
            mode: retrieval.mode,
            namespace: scenario.namespace.clone(),
            topic: Some(text.to_string()),
            scene: cmem_eval::MemorySceneInput {
                time: Some(timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)),
                ..Default::default()
            },
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
    let native_traces = pack
        .outcomes()
        .iter()
        .filter_map(|outcome| outcome.trace.as_ref())
        .collect::<Vec<_>>();
    let has_trace = !native_traces.is_empty();
    RestartProbeSnapshot {
        returned_object_ids,
        relevant_returned_count,
        expected_relevant_count,
        recall: (expected_relevant_count > 0)
            .then_some(relevant_returned_count as f64 / expected_relevant_count as f64),
        graph_relation_count: has_trace.then(|| {
            native_traces
                .iter()
                .map(|trace| trace.graph_relations.len())
                .sum()
        }),
        graph_verified_count: has_trace.then(|| {
            pack.outcomes()
                .iter()
                .map(|outcome| outcome.rationale.graph_verified_count)
                .sum()
        }),
        fanout_decision_count: has_trace.then(|| {
            native_traces
                .iter()
                .map(|trace| trace.fanout_utilization.len())
                .sum()
        }),
        selectivity_decision_count: has_trace.then(|| {
            native_traces
                .iter()
                .map(|trace| trace.selectivity_decisions.len())
                .sum()
        }),
        scored_selectivity_count: has_trace.then(|| {
            native_traces
                .iter()
                .flat_map(|trace| &trace.selectivity_decisions)
                .filter(|decision| decision.score.is_some())
                .count()
        }),
        fallback_selectivity_count: has_trace.then(|| {
            native_traces
                .iter()
                .flat_map(|trace| &trace.selectivity_decisions)
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
            if target.original_raw_ref.is_none() && target.original_setting_key.is_none() {
                bail!(
                    "scenario {fixture_id:?} source correction target {target_external_id:?} has no authoritative original reference"
                );
            }
            Ok((
                CorrectionTargetInput::SourceObject {
                    object_type: target.object_type,
                    external_id: target_external_id.to_string(),
                    original_raw_ref: target.original_raw_ref.clone(),
                    original_setting_key: target.original_setting_key.clone(),
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

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;

    use super::*;
    use crate::{CHECKED_FIXTURE_SEED, generate_fixture_set};
    use cmem_eval::{RetrievalMode, RetrievalSectionBudgets, RetrievalSurfacePolicy};

    fn persisted_trace() -> ContinuityQueryTrace {
        ContinuityQueryTrace {
            result: cmem_eval::PerQuestionResult {
                run_id: "trace-test".into(),
                question_id: "query".into(),
                question_type: None,
                question: "query?".into(),
                gold_episode_ids: vec!["episode".into()],
                gold_observation_ids: Vec::new(),
                retrieved: Vec::new(),
                context_text: String::new(),
                write_outcomes: [1, 2]
                    .into_iter()
                    .map(|id| cmem_eval::RememberOutcome {
                        persisted_object_ids: vec![
                            cmem_eval::character_memory::MemoryId::from_u128(id),
                        ],
                        persisted_link_ids: Vec::new(),
                        vector_indexed_object_ids: Vec::new(),
                        vector_indexing_failure: None,
                        stats_update_status: Default::default(),
                        repair_needed: Vec::new(),
                        diagnostics: Default::default(),
                    })
                    .collect(),
                link_outcomes: Vec::new(),
                lifecycle_outcomes: Vec::new(),
                metrics: Default::default(),
                latency_ms: 0,
                context_char_count: 0,
                context_word_count: 0,
                context: Default::default(),
                retrieval_outcomes: vec![cmem_eval::RetrieveOutcome {
                    time_range: None,
                    activity: None,
                    scene: cmem_eval::character_memory::Scene::at(
                        chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
                    ),
                    scene_references: Vec::new(),
                    memory_scenes: Vec::new(),
                    pack: cmem_eval::character_memory::ContinuityContextPack::empty(),
                    rationale: cmem_eval::character_memory::RetrievalRationale::new("trace-test"),
                    trace: Some(cmem_eval::RetrievalTrace::empty()),
                }],
                composition: Default::default(),
                integrity: Default::default(),
            },
            fixture_id: "fixture".into(),
            namespace: "namespace".into(),
            event_id: "query".into(),
            timestamp: chrono::DateTime::UNIX_EPOCH,
            expected: ExpectedRelevanceRecord {
                relevant_external_ids: vec!["episode".into()],
                irrelevant_external_ids: Vec::new(),
            },
            history_text: String::new(),
            restart_observations: Vec::new(),
        }
    }

    #[test]
    fn trace_serialization_flattens_results_and_keeps_native_trace() {
        let probe = persisted_trace();
        let encoded = serde_json::to_value(probe).unwrap();
        assert!(encoded.get("result").is_none());
        assert!(encoded.get("retrieval").is_none());
        assert!(
            encoded
                .pointer("/retrieval_outcomes/0/trace/selectivity_decisions")
                .is_some()
        );
    }

    #[test]
    fn trace_files_preserve_canonical_records_without_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let traces = [persisted_trace()];
        let read_traces = |path: &Path| read_continuity_traces(path).unwrap();

        // Persisted merged records keep the native event order.
        let mut reordered = traces[0].clone();
        reordered.result.write_outcomes.reverse();
        let round_trip = directory.path().join("round-trip.jsonl");
        write_continuity_traces(&round_trip, std::slice::from_ref(&reordered)).unwrap();
        let existing = fs::read(&round_trip).unwrap();
        assert!(write_continuity_traces(&round_trip, &[]).is_err());
        assert_eq!(fs::read(&round_trip).unwrap(), existing);
        let decoded = read_traces(&round_trip);
        assert_eq!(decoded, vec![reordered]);

        let mut additive = serde_json::to_value(&traces[0]).unwrap();
        additive["expected"]["future_annotation"] = serde_json::json!(true);
        assert!(
            serde_json::from_value::<crate::ExpectedRelevance>(additive["expected"].clone())
                .is_err()
        );
        fs::write(&round_trip, serde_json::to_vec(&additive).unwrap()).unwrap();
        assert_eq!(read_traces(&round_trip), traces[..1]);
    }

    pub(crate) fn situated_scenario() -> ContinuityScenario {
        crate::parse_fixture_bytes(&serde_json::to_vec(&serde_json::json!({
            "schema_version": 3, "seed": 7,
            "scenarios": [{
                "fixture_id": "situated-control", "namespace": "situated-control", "pattern": "situated",
                "catalog_situations": ["D4"], "character_entity": "self",
                "entities": [
                    {"external_id":"self", "label":"Character", "is_hub":false},
                    {"external_id":"ada", "label":"Ada", "is_hub":false}
                ],
                "scenes": {"pair":{"who":[{"reference":{"by":"key","key":"self"}},{"reference":{"by":"key","key":"ada"}}]}},
                "embedding": {"provider":"controllable_similarity", "own_concept":true, "seed":7, "vector_size":16, "noise_magnitude":0.01, "clusters":{}, "concepts":{}},
                "events": [
                    {"kind":"experience", "event_id":"visit", "timestamp":"2024-01-01T09:00:00.123456789Z", "text":"Garden", "scene":{"kind":"named","name":"pair"}, "speaker":"ada"},
                    {"kind":"experience", "event_id":"noise", "timestamp":"2024-01-02T09:00:00Z", "text":"Weather", "scene":{"kind":"named","name":"pair"}},
                    {"kind":"derive", "event_id":"promise", "timestamp":"2024-01-03T09:00:00.987654321Z", "memory":{"subtype":"commitment", "text":"Garden commitment", "experiences":["visit"], "about":["ada"]}},
                    {"kind":"query", "event_id":"ask", "query_id":"ask", "timestamp":"2024-01-04T09:00:00Z", "text":"Garden", "expected":{"relevant_external_ids":["visit","promise"], "irrelevant_external_ids":[]}}
                ]
            }]
        })).unwrap()).unwrap().scenarios.remove(0)
    }

    #[test]
    fn situated_gold_sentinels_never_reach_mapped_writes_or_probe_projection() {
        let mut value = serde_json::to_value(situated_scenario()).unwrap();
        value["entities"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "external_id":"gold-only-person-6e924", "label":"Visitor", "is_hub":false
            }));
        value["events"][2]["expected_warning"] = "churning_chain".into();
        value["events"][3] = serde_json::json!({
            "kind":"probe", "event_id":"probe", "query_id":"probe", "timestamp":"2024-01-04T09:00:00Z",
            "scene":{"kind":"inline", "scene":{"who":[
                {"reference":{"by":"key","key":"self"}},
                {"reference":{"by":"description","text":"the visitor in a red coat"}, "gold_entity":"gold-only-person-6e924"}
            ]}},
            "assertions":{
                "carried":[{"memory":"promise", "reason":"recent_and_salient", "section":"character_signals"}],
                "references":[{"participant":{"by":"description","text":"the visitor in a red coat"}, "resolution":{"status":"resolved","entity":"gold-only-person-6e924"}}]
            }
        });
        let fixture = crate::parse_fixture_bytes(
            &serde_json::to_vec(&serde_json::json!({
                "schema_version":3, "seed":7, "scenarios":[value]
            }))
            .unwrap(),
        )
        .unwrap();
        let scenario = &fixture.scenarios[0];
        let sentinels = [
            "recent_and_salient",
            "gold-only-person-6e924",
            "churning_chain",
            "character_signals",
        ];
        let authored = serde_json::to_string(scenario).unwrap();
        for sentinel in sentinels {
            assert!(authored.contains(sentinel));
        }
        let assert_no_gold = |encoded: String| {
            for sentinel in sentinels {
                let needle = if sentinel == "character_signals" {
                    format!(":\"{sentinel}\"")
                } else {
                    sentinel.into()
                };
                assert!(!encoded.contains(&needle), "gold leaked: {sentinel}");
            }
        };
        for event in &scenario.events {
            let input = scenario.situated_input(event).unwrap().unwrap();
            assert_no_gold(serde_json::to_string(&input).unwrap());
            let mapped = map_situated_input(scenario, event.timestamp(), input);
            match mapped.unwrap() {
                MappedSituatedInput::Experience(input) => {
                    assert_no_gold(serde_json::to_string(&input).unwrap())
                }
                MappedSituatedInput::Derive(input) => {
                    assert_no_gold(serde_json::to_string(&input).unwrap())
                }
                MappedSituatedInput::Probe(input) => {
                    assert_no_gold(serde_json::to_string(&input).unwrap())
                }
            }
        }
    }

    #[test]
    fn situated_write_rejects_failed_stats_even_after_vector_success() {
        let scenario = situated_scenario();
        let mut outcome = cmem_eval::RememberOutcome {
            persisted_object_ids: vec![uuid::Uuid::nil()],
            persisted_link_ids: Vec::new(),
            vector_indexed_object_ids: vec![uuid::Uuid::nil()],
            vector_indexing_failure: None,
            stats_update_status: Default::default(),
            repair_needed: Vec::new(),
            diagnostics: Default::default(),
        };
        assert!(checked_write_outcome(&scenario, "visit", outcome.clone()).is_ok());
        outcome.stats_update_status = cmem_eval::character_memory::StatsUpdateStatus::failed(
            [],
            [uuid::Uuid::nil()],
            vec![
                cmem_eval::character_memory::StatsUpdateCause::StoreUnhealthy {
                    health_cause: None,
                },
            ],
        );
        let error = checked_write_outcome(&scenario, "visit", outcome)
            .unwrap_err()
            .to_string();
        assert!(error.contains("stats-update failure"));
        assert!(error.contains("situated-control") && error.contains("visit"));
    }

    #[tokio::test]
    async fn situated_writes_reject_degraded_native_outcomes() {
        for missing_input in [
            "Garden",
            "Garden commitment",
            "Garden\nSetting: Glass room\nWith: Guest",
        ] {
            let mut scenario = situated_scenario();
            if missing_input.contains("Setting:") {
                let mut value = serde_json::to_value(&scenario).unwrap();
                value["scenes"]["pair"]["where"] =
                    serde_json::json!({"by":"description", "text":"Glass room"});
                value["scenes"]["pair"]["who"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"reference":{"by":"name", "text":"Guest"}}));
                scenario = crate::parse_fixture_bytes(
                    &serde_json::to_vec(
                        &serde_json::json!({"schema_version":3, "seed":7, "scenarios":[value]}),
                    )
                    .unwrap(),
                )
                .unwrap()
                .scenarios
                .remove(0);
            }
            let mut query = scenario.events.pop().unwrap();
            let InteractionEvent::Query { text, expected, .. } = &mut query else {
                unreachable!()
            };
            *text = "Weather".into();
            expected.relevant_external_ids = vec!["visit".into()];
            scenario.events.push(query);
            scenario.analyze().unwrap();
            let mut fixture = scenario
                .embedding
                .controllable_similarity()
                .unwrap()
                .clone();
            // A valid provider that cannot embed this write produces a native
            // repair-needed outcome after persisting the graph.
            fixture
                .concepts
                .retain(|_, concept| !concept.inputs.iter().any(|input| input == missing_input));
            let mut config = BenchmarkRunConfig {
                run_id: "degraded-write".into(),
                dataset: cmem_eval::DatasetId::new("continuity").unwrap(),
                backend: Default::default(),
                retrieval: retrieval(),
                ingest: Default::default(),
                metrics: Default::default(),
            };
            config.backend.embedding.vector_size = Some(fixture.vector_size);
            let directory = tempfile::tempdir().unwrap();
            let mut runtime = ContinuityRuntime::new(
                directory.path(),
                &config,
                EmbeddingRuntimeBinding::Controllable {
                    dimension_policy: cmem_eval::ControllableDimensionPolicy::FixtureDeclared,
                    fixture,
                },
            )
            .await
            .unwrap();
            let result = run_continuity_scenario(&mut runtime, &scenario, &config.retrieval).await;
            runtime.cleanup(&scenario.namespace).await.unwrap();
            let error = result.unwrap_err().to_string();
            assert!(error.contains("vector-indexing failure"), "{error}");
            assert!(
                error.contains(missing_input.lines().last().unwrap()),
                "unexpected input must be reported: {error}"
            );
        }
    }

    #[tokio::test]
    async fn situated_control_forwards_authored_fields_into_native_memories() {
        let mut scenario = situated_scenario();
        let InteractionEvent::Experience { salience, .. } = &mut scenario.events[0] else {
            unreachable!()
        };
        *salience = Some(0.83);
        assert!(scenario_missing_features(&scenario).unwrap().is_empty());
        let run = run_embedded(&scenario).await;
        assert_eq!(run.outcome.status, crate::ScenarioStatus::Passed);
        assert_eq!(run.operation_counts.get("validate_plan"), Some(&2));
        let pack = &run.traces[0].retrieval;
        let native = &pack.outcomes()[0].pack;
        let external = |id: cmem_eval::character_memory::MemoryId| {
            pack.object_refs()[&id.to_string()].external_id.clone()
        };
        let visit = native
            .relevant_episodes
            .iter()
            .find(|episode| external(episode.id) == "visit")
            .unwrap();
        assert_eq!(
            visit
                .scene
                .participants
                .iter()
                .map(|participant| external(participant.key.unwrap()))
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["self".to_string(), "ada".to_string()])
        );
        assert_eq!(visit.scene.time, scenario.events[0].timestamp());
        assert_eq!(visit.created_at, scenario.events[0].timestamp());
        assert_eq!(visit.salience_score, 0.83);
        let observation = native
            .salient_observations
            .iter()
            .find(|observation| observation.episode_id == visit.id)
            .unwrap();
        assert_eq!(external(observation.speaker_entity_id.unwrap()), "ada");
        assert_eq!(observation.salience_score, 0.83);
        assert_eq!(
            observation.observed_at,
            Some(scenario.events[0].timestamp())
        );
        let promise = native
            .commitments
            .iter()
            .find(|memory| external(memory.memory.id) == "promise")
            .unwrap();
        assert_eq!(promise.memory.derived_type, DerivedType::Commitment);
        assert_eq!(promise.memory.derived_from_episode_ids, [visit.id]);
        assert_eq!(
            promise
                .memory
                .entity_ids
                .iter()
                .map(|id| external(*id))
                .collect::<Vec<_>>(),
            ["ada"]
        );
        assert_eq!(promise.memory.created_at, scenario.events[2].timestamp());
        assert_eq!(run.operation_counts["experience"], 2);
        assert_eq!(run.operation_counts["derive"], 1);
        for (subtype, expected) in [
            (AuthoredMemoryKind::Reflection, DerivedType::Reflection),
            (
                AuthoredMemoryKind::RelationshipNote,
                DerivedType::RelationshipNote,
            ),
            (AuthoredMemoryKind::OpenLoop, DerivedType::OpenLoop),
            (
                AuthoredMemoryKind::CharacterSignal,
                DerivedType::CharacterSignal,
            ),
        ] {
            let Some(SituatedInput::Derive {
                external_id,
                mut memory,
            }) = scenario.situated_input(&scenario.events[2]).unwrap()
            else {
                panic!("derive");
            };
            memory.subtype = subtype;
            memory.supersedes = vec!["prior-state".into()];
            let MappedSituatedInput::Derive(mapped) = map_situated_input(
                &scenario,
                scenario.events[2].timestamp(),
                SituatedInput::Derive {
                    external_id,
                    memory,
                },
            )
            .unwrap() else {
                panic!("derive mapping");
            };
            assert_eq!(mapped.derived_memories[0].derived_type, expected);
            assert_eq!(
                mapped.derived_memories[0].supersedes_external_ids,
                ["prior-state"]
            );
        }
    }

    #[tokio::test]
    async fn unsupported_scenario_does_not_even_access_an_adapter() {
        for (subtype, feature) in [
            (
                AuthoredMemoryKind::Intention,
                ScenarioFeature::IntentionMemory,
            ),
            (
                AuthoredMemoryKind::Thread,
                ScenarioFeature::ThreadProvenance,
            ),
        ] {
            let mut scenario = situated_scenario();
            let InteractionEvent::Derive { memory, .. } = &mut scenario.events[2] else {
                panic!("derive");
            };
            memory.subtype = subtype;
            if subtype == AuthoredMemoryKind::Thread {
                let mut replacement = scenario.events[2].clone();
                let InteractionEvent::Derive {
                    event_id,
                    timestamp,
                    memory,
                    ..
                } = &mut replacement
                else {
                    unreachable!()
                };
                *event_id = "replacement".into();
                *timestamp += chrono::Duration::minutes(1);
                memory.subtype = AuthoredMemoryKind::Commitment;
                scenario.events.insert(3, replacement);
            }
            let directory = tempfile::tempdir().unwrap();
            let mut runtime = ContinuityRuntime {
                active: None,
                config: Box::new(BenchmarkRunConfig {
                    run_id: "not-run".into(),
                    dataset: cmem_eval::DatasetId::new("continuity").unwrap(),
                    backend: Default::default(),
                    retrieval: retrieval(),
                    ingest: Default::default(),
                    metrics: Default::default(),
                }),
                embedding_binding: EmbeddingRuntimeBinding::Controllable {
                    fixture: scenario
                        .embedding
                        .controllable_similarity()
                        .unwrap()
                        .clone(),
                    dimension_policy: cmem_eval::ControllableDimensionPolicy::FixtureDeclared,
                },
                run_root: directory.path().to_path_buf(),
            };
            let run = run_continuity_scenario(&mut runtime, &scenario, &retrieval())
                .await
                .unwrap();
            assert_eq!(run.outcome.status, crate::ScenarioStatus::NotRun);
            assert_eq!(run.outcome.missing_features, [feature]);
            assert!(run.operation_counts.is_empty());
            assert!(run.traces.is_empty());
            assert!(runtime.active.is_none());
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn unsupported_write_details_are_distinct_static_requirements() {
        for (subtype, feature) in [
            (
                AuthoredMemoryKind::Intention,
                ScenarioFeature::IntentionMemory,
            ),
            (
                AuthoredMemoryKind::Preference,
                ScenarioFeature::PreferenceMemory,
            ),
            (
                AuthoredMemoryKind::Thread,
                ScenarioFeature::ThreadProvenance,
            ),
        ] {
            let mut scenario = situated_scenario();
            let InteractionEvent::Derive { memory, .. } = &mut scenario.events[2] else {
                unreachable!()
            };
            memory.subtype = subtype;
            assert_eq!(scenario_missing_features(&scenario).unwrap(), [feature]);
        }
        for feature in [
            ScenarioFeature::WriteSceneWhere,
            ScenarioFeature::WriteSceneWhat,
            ScenarioFeature::WriteSceneCustom,
        ] {
            let mut scenario = situated_scenario();
            let scene = scenario.scenes.get_mut("pair").unwrap();
            match feature {
                ScenarioFeature::WriteSceneWhere => {
                    scene.place = Some(PerceivedReference::Setting {
                        key: "workshop".into(),
                    })
                }
                ScenarioFeature::WriteSceneWhat => {
                    let mut activity_scene = scene.clone();
                    activity_scene.what = Some(PerceivedReference::Key {
                        key: "gardening".into(),
                    });
                    let InteractionEvent::Experience { scene, .. } = &mut scenario.events[1] else {
                        unreachable!()
                    };
                    *scene = crate::SceneSelection::Inline {
                        scene: activity_scene,
                    };
                    let mut activity = scenario.events[2].clone();
                    let InteractionEvent::Derive {
                        event_id,
                        timestamp,
                        memory,
                        ..
                    } = &mut activity
                    else {
                        unreachable!()
                    };
                    *event_id = "gardening".into();
                    *timestamp = scenario.events[0].timestamp() + chrono::Duration::minutes(1);
                    memory.subtype = AuthoredMemoryKind::OpenLoop;
                    scenario.events.insert(1, activity);
                }
                ScenarioFeature::WriteSceneCustom => {
                    scene.custom.insert("project".into(), "garden".into());
                }
                _ => unreachable!(),
            }
            let missing = scenario_missing_features(&scenario).unwrap();
            assert_eq!(
                missing.contains(&feature),
                feature == ScenarioFeature::WriteSceneWhat
            );
        }
    }

    #[tokio::test]
    async fn activity_key_reaches_the_native_result() {
        use cmem_eval::character_memory::{ActivityRef, ActivityResolution};
        let mut value = serde_json::to_value(situated_scenario()).unwrap();
        value["events"][2]["memory"]["subtype"] = "open_loop".into();
        value["events"][3] = serde_json::json!({
            "kind": "probe", "event_id": "resume", "query_id": "resume",
            "timestamp": "2024-01-04T09:00:00Z",
            "scene": {"kind": "inline", "scene": {"who": [{"reference": {"by": "key", "key": "self"}}], "what": {"by": "key", "key": "promise"}}},
            "assertions": {"cued": [{"memory": "promise", "cue": "activity"}]}
        });
        let scenario: ContinuityScenario = serde_json::from_value(value).unwrap();
        assert!(scenario_missing_features(&scenario).unwrap().is_empty());
        let run = run_embedded(&scenario).await;
        assert_eq!(
            run.outcome.status,
            crate::ScenarioStatus::Passed,
            "{:?}",
            run.outcome
        );
        let pack = &run.traces[0].retrieval;
        let activity = pack.outcomes()[0].activity.as_ref().unwrap();
        assert_eq!(activity.resolution, ActivityResolution::Found);
        let ActivityRef::OpenLoop(id) = activity.activity else {
            panic!("activity kind changed")
        };
        assert_eq!(pack.object_refs()[&id.to_string()].external_id, "promise");
    }

    #[test]
    fn description_resolution_expectations_are_gated_without_gating_description_input() {
        let mut value = serde_json::to_value(situated_scenario()).unwrap();
        value["events"][3] = serde_json::json!({
            "kind": "probe", "event_id": "probe", "query_id": "probe", "timestamp": "2024-01-04T09:00:00Z",
            "scene": {"kind": "inline", "scene": {"who": [
                {"reference": {"by": "key", "key": "self"}},
                {"reference": {"by": "description", "text": "Garden"}}
            ]}},
            "assertions": {"carried": [{"memory": "visit", "reason": "pair"}]}
        });
        let scenario: ContinuityScenario = serde_json::from_value(value.clone()).unwrap();
        assert!(scenario_missing_features(&scenario).unwrap().is_empty());
        for resolution in [
            serde_json::json!({"status": "unknown"}),
            serde_json::json!({"status": "resolved", "entity": "ada"}),
            serde_json::json!({"status": "ambiguous", "candidates": ["ada", "self"]}),
        ] {
            value["events"][3]["assertions"]["references"] = serde_json::json!([
                {"participant": {"by": "description", "text": "Garden"}, "resolution": resolution}
            ]);
            let scenario: ContinuityScenario = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                scenario_missing_features(&scenario).unwrap(),
                [ScenarioFeature::DescriptionReferenceResolution]
            );
        }
    }

    #[test]
    fn admitted_supported_scenarios_map_before_any_adapter_call() {
        let mut scenarios = Vec::new();
        for filename in ["situated_v1.toml", "situated_loud_topic_v1.json"] {
            scenarios.extend(
                crate::read_fixture(
                    &Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("fixtures")
                        .join(filename),
                )
                .unwrap()
                .scenarios,
            );
        }
        for subtype in ["open_loop", "thread"] {
            let mut value = serde_json::to_value(situated_scenario()).unwrap();
            value["events"][2]["memory"]["subtype"] = subtype.into();
            value["events"][3] = serde_json::json!({
                "kind": "probe", "event_id": "resume", "query_id": "resume",
                "timestamp": "2024-01-04T09:00:00Z",
                "scene": {"kind": "inline", "scene": {"who": [{"reference": {"by": "key", "key": "self"}}], "what": {"by": "key", "key": "promise"}}},
                "assertions": {"cued": [{"memory": "promise", "cue": "activity"}]}
            });
            scenarios.push(serde_json::from_value(value).unwrap());
        }
        for scenario in scenarios {
            if !scenario_missing_features(&scenario).unwrap().is_empty() {
                continue;
            }
            for event in &scenario.events {
                if let Some(input) = scenario.situated_input(event).unwrap() {
                    map_situated_input(&scenario, event.timestamp(), input).unwrap();
                }
            }
        }
    }

    #[test]
    fn cue_support_is_derived_from_assertions_not_carried_labels() {
        for (cue, feature) in [
            ("topic", None),
            ("pair", None),
            ("place", None),
            ("activity", None),
            ("due", Some(ScenarioFeature::DueCue)),
            ("date", Some(ScenarioFeature::DateCue)),
            ("trigger", Some(ScenarioFeature::TriggerCue)),
            ("own_day", Some(ScenarioFeature::OwnDayCue)),
            (
                "recent_and_salient",
                Some(ScenarioFeature::RecentAndSalientCue),
            ),
        ] {
            for polarity in ["carried", "cued", "not_cued"] {
                let mut value = serde_json::to_value(situated_scenario()).unwrap();
                let assertion = if polarity == "carried" {
                    serde_json::json!({"memory": "visit", "reason": cue})
                } else {
                    serde_json::json!({"memory": "visit", "cue": cue})
                };
                value["events"][3] = serde_json::json!({
                    "kind": "probe", "event_id": "probe", "query_id": "probe",
                    "timestamp": "2024-01-04T09:00:00Z", "scene": {"kind": "named", "name": "pair"},
                    "assertions": {polarity: [assertion]}
                });
                let scenario: ContinuityScenario = serde_json::from_value(value).unwrap();
                let expected = match (polarity, cue) {
                    ("carried", _) => None,
                    ("not_cued", "pair") => Some(ScenarioFeature::PairCounterpartCue),
                    _ => feature,
                };
                assert_eq!(
                    scenario_missing_features(&scenario).unwrap(),
                    expected.into_iter().collect::<Vec<_>>(),
                    "{polarity} {cue}"
                );
            }
        }
    }

    #[tokio::test]
    async fn scene_slice_preserves_authored_input_and_checks_native_results() {
        use cmem_eval::character_memory::{SceneReference, SceneReferenceResolution, SourceScene};
        for place_by in ["name", "description", "setting", "key"] {
            let mut value = serde_json::to_value(situated_scenario()).unwrap();
            value["entities"].as_array_mut().unwrap().extend([
                serde_json::json!({"external_id":"jo-a", "label":"Jo", "is_hub":false}),
                serde_json::json!({"external_id":"jo-b", "label":"Jo", "is_hub":false}),
            ]);
            let place = match place_by {
                "setting" => serde_json::json!({"by":"setting", "key":"room:17"}),
                "key" => serde_json::json!({"by":"key", "key":"ada"}),
                _ => serde_json::json!({"by":place_by, "text":"  Quiet\n observatory  "}),
            };
            value["scenes"]["pair"]["where"] = place;
            value["scenes"]["pair"]["custom"] = serde_json::json!({"weather":"blue hour"});
            value["events"][2]["memory"]["experiences"] = serde_json::json!(["visit", "noise"]);
            value["scenes"]["pair"]["who"].as_array_mut().unwrap().extend([
                serde_json::json!({"reference":{"by":"name", "text":"  Jo  "}, "gold_entity":"jo-a"}),
                serde_json::json!({"reference":{"by":"description", "text":"  visitor\n in violet  "}, "gold_entity":"jo-b"}),
            ]);
            let mut probe_scene = value["scenes"]["pair"].clone();
            probe_scene["custom"] = serde_json::json!({"weather":"silver dusk"});
            probe_scene["who"].as_array_mut().unwrap().extend([
                serde_json::json!({"reference":{"by":"name", "text":"Ada"}}),
                serde_json::json!({"reference":{"by":"name", "text":"Unknown visitor"}}),
            ]);
            value["events"][3] = serde_json::json!({
                "kind":"probe", "event_id":"probe", "query_id":"probe",
                "timestamp":"2024-01-04T09:00:00.456789123Z",
                "scene":{"kind":"inline", "scene":probe_scene},
                "assertions":{
                    "carried":[{"memory":"visit", "reason":"pair"}, {"memory":"promise", "reason":"pair"}],
                    "scenes":[{"memory":"visit", "scene":"pair"}, {"memory":"promise", "scene":"pair"}],
                    "references":[
                        {"participant":{"by":"key", "key":"self"}, "resolution":{"status":"resolved", "entity":"self"}},
                        {"participant":{"by":"name", "text":"  Jo  "}, "resolution":{"status":"ambiguous", "candidates":["jo-a", "jo-b"]}},
                        {"participant":{"by":"name", "text":"Ada"}, "resolution":{"status":"resolved", "entity":"ada"}},
                        {"participant":{"by":"name", "text":"Unknown visitor"}, "resolution":{"status":"unknown"}}
                    ]
                }
            });
            if place_by == "key" {
                value["events"][3]["topic"] = serde_json::json!("  a topic  with spacing\n  ");
            }
            for extension in ["toml", "json"] {
                let root = serde_json::json!({"schema_version":3, "seed":7, "scenarios":[value]});
                let bytes = if extension == "toml" {
                    toml::to_string(&root).unwrap().into_bytes()
                } else {
                    serde_json::to_vec(&root).unwrap()
                };
                let fixture =
                    crate::parse_fixture_source(Path::new(&format!("scene.{extension}")), &bytes)
                        .unwrap();
                let scenario = &fixture.scenarios[0];
                assert!(scenario_missing_features(scenario).unwrap().is_empty());
                let run = run_embedded(scenario).await;
                assert_eq!(
                    run.outcome.status,
                    crate::ScenarioStatus::Passed,
                    "{:?}",
                    run.outcome.assertions
                );
                let pack = &run.traces[0].retrieval;
                let native = &pack.outcomes()[0];
                assert_eq!(native.scene.time, scenario.events[3].timestamp());
                assert_eq!(native.scene.custom_values["weather"], "silver dusk");
                assert_eq!(native.scene.participants[2].name.as_deref(), Some("  Jo  "));
                assert_eq!(
                    native.scene.participants[3].description.as_deref(),
                    Some("  visitor\n in violet  ")
                );
                assert_eq!(
                    native.scene.setting.words.as_deref(),
                    matches!(place_by, "name" | "description").then_some("  Quiet\n observatory  ")
                );
                assert_eq!(
                    native.scene.setting.key.as_deref(),
                    match place_by {
                        "setting" => Some("room:17"),
                        "key" => Some("ada"),
                        _ => None,
                    }
                );
                assert!(native.scene_references.iter().any(|fact| fact.reference
                    == SceneReference::ParticipantDescription { index: 3 }
                    && fact.resolution == SceneReferenceResolution::ContentCue));
                let (visit_id, visit_scene) = native
                    .memory_scenes
                    .iter()
                    .flat_map(|fact| &fact.sources)
                    .find_map(|source| match source {
                        SourceScene::Recorded { episode_id, scene }
                            if pack.object_refs()[&episode_id.to_string()].external_id
                                == "visit" =>
                        {
                            Some((*episode_id, scene))
                        }
                        _ => None,
                    })
                    .expect(
                        "recorded experience scene, whether admitted as episode or observation",
                    );
                assert_eq!(visit_scene.time, scenario.events[0].timestamp());
                assert_eq!(visit_scene.custom_values["weather"], "blue hour");
                assert_eq!(visit_scene.participants, native.scene.participants[..4]);
                assert_eq!(visit_scene.setting, native.scene.setting);
                assert_eq!(
                    pack.object_refs()[&visit_id.to_string()].external_id,
                    "visit"
                );
                let mut wrong_time = native.clone();
                for source in wrong_time
                    .memory_scenes
                    .iter_mut()
                    .flat_map(|fact| &mut fact.sources)
                {
                    if let SourceScene::Recorded { scene, .. } = source {
                        scene.time += chrono::Duration::nanoseconds(1);
                    }
                }
                let wrong_time = cmem_eval::RetrievedContextPack::from_ranked_items(
                    pack.items().to_vec(),
                    vec![wrong_time],
                    cmem_eval::ContextRenderer::PlainText,
                )
                .with_object_refs(pack.object_refs().clone());
                let checks =
                    crate::check_probe_assertions(scenario, &scenario.events[3], &wrong_time);
                let scene_statuses = checks
                    .iter()
                    .filter(|check| {
                        matches!(
                            check.identity.assertion,
                            crate::AssertionSubject::Scene { .. }
                        )
                    })
                    .map(|check| check.check.status)
                    .collect::<Vec<_>>();
                assert_eq!(scene_statuses, vec![crate::ScenarioStatus::Failed; 2]);
                // Missing native facts must fail, even though the authored gold is complete.
                let mut absent = native.clone();
                absent.scene_references.clear();
                absent.memory_scenes.clear();
                let absent = cmem_eval::RetrievedContextPack::from_ranked_items(
                    pack.items().to_vec(),
                    vec![absent],
                    cmem_eval::ContextRenderer::PlainText,
                )
                .with_object_refs(pack.object_refs().clone());
                let checks = crate::check_probe_assertions(scenario, &scenario.events[3], &absent);
                assert!(
                    checks
                        .iter()
                        .filter(|check| matches!(
                            check.identity.assertion,
                            crate::AssertionSubject::References(_)
                                | crate::AssertionSubject::Scene { .. }
                        ))
                        .all(|check| check.check.status == crate::ScenarioStatus::Failed)
                );
            }
        }
    }

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
            EmbeddingRuntimeBinding::Frozen { store }
        };
        let mut runtime = ContinuityRuntime::new(directory.path(), &config, binding)
            .await
            .unwrap();
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
        Vec<ContinuityQueryObservation>,
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
        let expected_query_count = fixtures
            .scenarios
            .iter()
            .flat_map(|scenario| &scenario.events)
            .filter(|event| matches!(event, InteractionEvent::Query { .. }))
            .count();
        assert_eq!(traces.len(), expected_query_count);
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
                !outcome.persisted_link_ids.is_empty()
                    && outcome.stats_update_status.failure.is_none()
            })
        }));
        let expected_restart_count = fixtures
            .scenarios
            .iter()
            .flat_map(|scenario| &scenario.events)
            .filter(|event| matches!(event, InteractionEvent::Restart { .. }))
            .count();
        assert_eq!(restart_observations.len(), expected_restart_count);
        for restart in &restart_observations {
            assert!(restart.reopen_graph);
            assert!(restart.reopen_stats);
            assert!(restart.lifecycle.restored_identity_count > 0);
            assert!(restart.delta.stable_returned_objects);
            assert_eq!(restart.delta.returned_object_count, 0);
            assert_eq!(restart.delta.recall, Some(0.0));
        }
    }

    #[tokio::test]
    async fn parser_admitted_implicit_objects_are_available_to_driver_operations() {
        for target in ["visit", "visit:observation"] {
            let mut scenario = situated_scenario();
            scenario.scenes.get_mut("pair").unwrap().place = Some(PerceivedReference::Setting {
                key: "garden-room-27".into(),
            });
            scenario.events.insert(
                3,
                InteractionEvent::Correct {
                    event_id: "correct-visit".into(),
                    target_external_id: target.into(),
                    replacement_external_id: "corrected-visit".into(),
                    timestamp: scenario.events[2].timestamp() + chrono::Duration::minutes(1),
                    replacement_text: "Garden commitment".into(),
                },
            );
            scenario.validate().unwrap();
            let run = run_embedded(&scenario).await;
            assert_eq!(run.operation_counts["correct"], 1);
            assert!(!run.traces[0].lifecycle_outcomes.is_empty());
        }
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
        assert_eq!(snapshot.fanout_decision_count, None);
        for native_trace in [None, Some(cmem_eval::RetrievalTrace::empty())] {
            let outcome = cmem_eval::RetrieveOutcome {
                time_range: None,
                activity: None,
                scene: cmem_eval::character_memory::Scene::at(
                    chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
                ),
                scene_references: Vec::new(),
                memory_scenes: Vec::new(),
                pack: cmem_eval::character_memory::ContinuityContextPack::empty(),
                rationale: cmem_eval::character_memory::RetrievalRationale::new("test"),
                trace: None,
            };
            let mut second = outcome.clone();
            second.rationale.graph_verified_count = 2;
            second.trace = native_trace;
            let has_trace = second.trace.is_some();
            let pack = RetrievedContextPack::from_ranked_items(
                Vec::new(),
                vec![outcome, second],
                cmem_eval::ContextRenderer::PlainText,
            );
            let snapshot = restart_probe_snapshot(&pack, &expected);
            assert_eq!(snapshot.graph_verified_count, has_trace.then_some(2));
            assert_eq!(snapshot.graph_relation_count, has_trace.then_some(0));
            assert_eq!(snapshot.fanout_decision_count, has_trace.then_some(0));
            assert_eq!(snapshot.selectivity_decision_count, has_trace.then_some(0));
            assert_eq!(snapshot.scored_selectivity_count, has_trace.then_some(0));
            assert_eq!(snapshot.fallback_selectivity_count, has_trace.then_some(0));
        }
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
                    let external = episode.scene.setting.key.as_deref().unwrap();
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
                    assert_eq!(Some(&episode.scene.time), Some(timestamp));
                    assert_eq!(&episode.summary, text);
                }
                for derived in pack
                    .derived_memories
                    .iter()
                    .filter(|derived| !derived.memory.given_by_application)
                {
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
            original_setting_key: Some("delivery-v1".to_string()),
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
                original_setting_key: Some("delivery-v1".to_string()),
            }
        );
    }
}
