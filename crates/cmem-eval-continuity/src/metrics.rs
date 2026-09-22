use std::collections::{BTreeMap, BTreeSet};

use cmem_eval::{
    MetricFamily, MetricsConfig, RetrievalMode, RetrieveOutcome, RetrievedItem, retrieval_metrics,
};
use serde_json::{Map, Value};

use crate::{ContinuityQueryObservation, ContinuityScenario, InteractionEvent, ScenarioPattern};

const GAP_BUCKETS: [&str; 3] = ["short", "medium", "long"];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CarriedRecall {
    pub expected: usize,
    pub admitted: Option<usize>,
    pub recall: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SituatedProbeMeasures {
    pub carried_recall_by_reason: BTreeMap<String, CarriedRecall>,
    pub bystander_context_share: Option<f64>,
    pub context_tokens: Option<usize>,
}

fn empty_carried_recalls(ran: bool) -> BTreeMap<String, CarriedRecall> {
    crate::RecallReason::ALL
        .into_iter()
        .map(|reason| {
            let key = serde_json::to_value(reason)
                .expect("reason enum")
                .as_str()
                .unwrap()
                .to_string();
            (
                key,
                CarriedRecall {
                    expected: 0,
                    admitted: ran.then_some(0),
                    recall: None,
                },
            )
        })
        .collect()
}

pub(crate) fn pool_carried_recall<'a>(
    probes: impl Iterator<Item = &'a SituatedProbeMeasures>,
) -> BTreeMap<String, CarriedRecall> {
    let mut pooled = empty_carried_recalls(false);
    for probe in probes {
        for (reason, count) in &probe.carried_recall_by_reason {
            if let Some(admitted) = count.admitted {
                let total = pooled.entry(reason.clone()).or_insert(CarriedRecall {
                    expected: 0,
                    admitted: None,
                    recall: None,
                });
                total.expected += count.expected;
                *total.admitted.get_or_insert(0) += admitted;
                total.recall = (total.expected != 0)
                    .then(|| total.admitted.unwrap() as f64 / total.expected as f64);
            }
        }
    }
    pooled
}

/// Section membership and ordering come from the native pack. The registry only
/// joins its object ids to authored ids; flattened retrieval items are not used.
pub fn native_admitted_memories(
    pack: &cmem_eval::RetrievedContextPack,
) -> Vec<(String, crate::MemorySection)> {
    use crate::MemorySection;
    let mut admitted = Vec::new();
    let external = |id: cmem_eval::character_memory::MemoryId| {
        pack.object_refs()
            .get(&id.to_string())
            .map(|object| object.external_id.clone())
    };
    for outcome in pack.outcomes() {
        let native = &outcome.pack;
        let mut push = |id, section| {
            if let Some(id) = external(id) {
                admitted.push((id, section));
            }
        };
        for thread in &native.active_threads {
            push(thread.id, MemorySection::Threads);
        }
        for episode in &native.relevant_episodes {
            push(episode.id, MemorySection::Episodes);
        }
        for observation in &native.salient_observations {
            push(observation.episode_id, MemorySection::Observations);
        }
        for (section, memories) in [
            (MemorySection::DerivedMemories, &native.derived_memories),
            (MemorySection::Preferences, &native.preferences),
            (MemorySection::RelationshipNotes, &native.relationship_notes),
            (MemorySection::OpenLoops, &native.open_loops),
            (MemorySection::Commitments, &native.commitments),
            (MemorySection::CharacterSignals, &native.character_signals),
        ] {
            for memory in memories {
                push(memory.memory.id, section);
            }
        }
    }
    admitted
}

pub fn situated_probe_measures(
    scenario: &ContinuityScenario,
    assertions: &crate::ProbeAssertions,
    measures: &crate::ProbeMeasures,
    pack: Option<&cmem_eval::RetrievedContextPack>,
) -> SituatedProbeMeasures {
    let admitted = pack.map(native_admitted_memories).unwrap_or_default();
    let authored = scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Experience { event_id, .. } => Some(event_id.as_str()),
            InteractionEvent::Derive {
                event_id, memory, ..
            } if memory.subtype != crate::AuthoredMemoryKind::Thread => Some(event_id.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let counted = admitted
        .iter()
        .filter(|(id, section)| {
            *section != crate::MemorySection::Threads && authored.contains(id.as_str())
        })
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    let mut recalls = empty_carried_recalls(pack.is_some());
    for expected in &assertions.carried {
        let reason = serde_json::to_value(expected.reason)
            .expect("reason is an enum")
            .as_str()
            .expect("reason is a string")
            .to_string();
        let recall = recalls
            .get_mut(&reason)
            .expect("closed recall reason vocabulary");
        recall.expected += 1;
        if let Some(count) = &mut recall.admitted {
            *count += usize::from(admitted.iter().any(|(id, _)| id == &expected.memory));
            recall.recall = Some(*count as f64 / recall.expected as f64);
        }
    }
    SituatedProbeMeasures {
        carried_recall_by_reason: recalls,
        bystander_context_share: (!counted.is_empty()).then(|| {
            measures
                .bystanders
                .iter()
                .filter(|id| counted.contains(id.as_str()))
                .count() as f64
                / counted.len() as f64
        }),
        context_tokens: pack.map(|pack| cmem_eval::count_tokens(pack.context_text())),
    }
}

/// Native typed summaries account for each lifecycle/currency omission, even
/// when there are no candidates or no omissions in an executed retrieval.
pub fn omissions_have_reasons(outcomes: &[RetrieveOutcome]) -> bool {
    !outcomes.is_empty()
        && outcomes.iter().all(|outcome| {
            let rationale = &outcome.rationale;
            rationale.lifecycle_omission_count
                == rationale
                    .lifecycle_omission_reasons
                    .iter()
                    .map(|reason| reason.count)
                    .sum::<usize>()
                && rationale.stale_candidate_omission_count
                    == rationale
                        .stale_candidate_omission_reasons
                        .iter()
                        .map(|reason| reason.count)
                        .sum::<usize>()
        })
}

pub fn check_probe_assertions(
    scenario: &ContinuityScenario,
    event: &InteractionEvent,
    pack: &cmem_eval::RetrievedContextPack,
) -> Vec<crate::AssertionResult> {
    use crate::{AssertionSubject, CheckResult, OmissionReason};
    use cmem_eval::character_memory::{
        LifecycleFilterAction, LifecycleFilterReason, StaleCandidateReason,
    };
    let InteractionEvent::Probe {
        assertions,
        query_id,
        ..
    } = event
    else {
        return Vec::new();
    };
    let admitted = native_admitted_memories(pack);
    event
        .assertion_identities()
        .into_iter()
        .map(|identity| {
            let check = match &identity.assertion {
                AssertionSubject::Carried(id) => {
                    let expected = assertions
                        .carried
                        .iter()
                        .find(|assertion| &assertion.memory == id)
                        .expect("authored identity");
                    let passed = admitted.iter().any(|(actual, section)| {
                        actual == id && expected.section.is_none_or(|expected| expected == *section)
                    });
                    CheckResult::checked(
                        passed,
                        if passed {
                            "memory admitted in the requested native section"
                        } else {
                            "memory is absent from the requested native section"
                        },
                    )
                }
                AssertionSubject::InOrder(ids) => {
                    let positions = ids
                        .iter()
                        .map(|id| admitted.iter().position(|(actual, _)| actual == id))
                        .collect::<Option<Vec<_>>>();
                    let passed = positions.is_some_and(|positions| {
                        positions.windows(2).all(|pair| pair[0] < pair[1])
                    });
                    CheckResult::checked(
                        passed,
                        if passed {
                            "relative native pack order"
                        } else {
                            "a memory is missing or the native pack order differs"
                        },
                    )
                }
                AssertionSubject::Omitted(id) => {
                    let expected = assertions
                        .omitted
                        .iter()
                        .find(|assertion| &assertion.memory == id)
                        .expect("authored identity");
                    let ids = pack
                        .object_refs()
                        .iter()
                        .filter(|(_, object)| {
                            &object.external_id == id
                                || (object.object_type == cmem_eval::ObjectType::Observation
                                    && object.external_id == crate::observation_external_id(id))
                        })
                        .map(|(id, _)| id.as_str())
                        .collect::<BTreeSet<_>>();
                    let explained = pack
                        .outcomes()
                        .iter()
                        .filter_map(|outcome| outcome.trace.as_ref())
                        .any(|trace| {
                            trace.lifecycle_filter_decisions.iter().any(|decision| {
                                ids.contains(decision.object.id.to_string().as_str())
                                    && decision.action == LifecycleFilterAction::Omitted
                                    && matches!(
                                        (expected.reason, decision.reason),
                                        (
                                            OmissionReason::Suppression,
                                            LifecycleFilterReason::SuppressedOmitted
                                        ) | (
                                            OmissionReason::Supersession,
                                            LifecycleFilterReason::SupersededOmitted
                                        ) | (
                                            OmissionReason::Resolution,
                                            LifecycleFilterReason::ResolvedOmitted
                                        )
                                    )
                            }) || (expected.reason == OmissionReason::Supersession
                                && trace.stale_candidate_omissions.iter().any(|omission| {
                                    ids.contains(omission.candidate.id.to_string().as_str())
                                        && omission.reason == StaleCandidateReason::Superseded
                                }))
                        });
                    let passed = !admitted.iter().any(|(actual, _)| actual == id) && explained;
                    CheckResult::checked(
                        passed,
                        if passed {
                            "absence with the requested native omission reason"
                        } else {
                            "memory is present or the requested native omission reason is absent"
                        },
                    )
                }
                AssertionSubject::References(reference) => {
                    let expected = &assertions
                        .references
                        .iter()
                        .find(|assertion| &assertion.participant == reference)
                        .expect("authored identity")
                        .resolution;
                    let passed = pack.outcomes().iter().any(|outcome| {
                        outcome.scene_references.iter().any(|fact| {
                            reference_matches(reference, fact, &outcome.scene, pack)
                                && resolution_matches(expected, &fact.resolution, pack)
                        })
                    });
                    CheckResult::checked(
                        passed,
                        if passed {
                            "native reference outcome matches the authored expectation"
                        } else {
                            "native reference outcome is missing or differs"
                        },
                    )
                }
                AssertionSubject::Scene { memory, scene } => {
                    let observed = pack
                        .outcomes()
                        .iter()
                        .flat_map(|outcome| &outcome.memory_scenes)
                        .filter(|fact| {
                            pack.object_refs()
                                .get(&fact.memory.id.to_string())
                                .is_some_and(|object| {
                                    object.external_id == *memory
                                        || (object.object_type
                                            == cmem_eval::ObjectType::Observation
                                            && object.external_id
                                                == crate::observation_external_id(memory))
                                })
                        })
                        .collect::<Vec<_>>();
                    let passed = !observed.is_empty()
                        && observed.iter().all(|fact| {
                            !fact.sources.is_empty()
                                && fact.sources.iter().all(|source| match source {
                                    cmem_eval::character_memory::SourceScene::Recorded {
                                        episode_id,
                                        scene: actual,
                                    } => scene_matches(
                                        scenario,
                                        *episode_id,
                                        &scenario.scenes[scene],
                                        actual,
                                        pack,
                                    ),
                                    cmem_eval::character_memory::SourceScene::Unavailable {
                                        ..
                                    } => false,
                                })
                        });
                    CheckResult::checked(
                        passed,
                        if passed {
                            "recorded native source scenes match the authored scene"
                        } else {
                            "recorded native source scene is missing, unavailable or differs"
                        },
                    )
                }
                AssertionSubject::Cued { memory, cue }
                | AssertionSubject::NotCued { memory, cue } => {
                    let observed = native_cues(pack, memory);
                    CheckResult::checked(
                        check_cue(
                            *cue,
                            matches!(identity.assertion, AssertionSubject::Cued { .. }),
                            observed.as_ref(),
                        ),
                        "selected native cue facts must match the requested cue",
                    )
                }
                AssertionSubject::ElapsedSinceMet(entity) => {
                    let expected = scenario.analyze().expect("admitted scenario").probes[query_id]
                        .elapsed_since_met[entity];
                    let facts =
                        pack.outcomes()
                            .iter()
                            .flat_map(|outcome| {
                                outcome.scene_references.iter().flat_map(move |reference| {
                                    reference.last_interactions.iter().filter_map(
                                        move |(id, fact)| {
                                            (entity_external_id(pack, *id) == Some(entity.as_str()))
                                                .then_some((outcome, fact))
                                        },
                                    )
                                })
                            })
                            .collect::<Vec<_>>();
                    let passed = !facts.is_empty()
                        && facts.iter().all(|(outcome, fact)| {
                            fact.as_ref().is_some_and(|fact| {
                                fact.seconds_since == expected.num_seconds()
                                    && outcome.scene.time - fact.scene_time == expected
                            })
                        });
                    CheckResult::checked(
                        passed,
                        "native last interaction must match the authored elapsed time",
                    )
                }
                // These facts do not exist in the pinned library. Static support
                // gating keeps their scenarios not-run; an executed missing fact
                // must fail rather than be reconstructed from the author's gold.
                _ => CheckResult::checked(false, "required fact is absent from the native outcome"),
            };
            crate::AssertionResult { identity, check }
        })
        .collect()
}

fn entity_external_id(
    pack: &cmem_eval::RetrievedContextPack,
    id: cmem_eval::character_memory::MemoryId,
) -> Option<&str> {
    pack.object_refs()
        .get(&id.to_string())
        .filter(|object| object.object_type == cmem_eval::ObjectType::Entity)
        .map(|object| object.external_id.as_str())
}

fn reference_matches(
    expected: &crate::PerceivedReference,
    fact: &cmem_eval::character_memory::SceneReferenceResult,
    scene: &cmem_eval::character_memory::Scene,
    pack: &cmem_eval::RetrievedContextPack,
) -> bool {
    use crate::PerceivedReference;
    use cmem_eval::character_memory::SceneReference;
    match (&fact.reference, expected) {
        (SceneReference::ParticipantKey { index }, PerceivedReference::Key { key }) => {
            scene
                .participants
                .get(*index)
                .and_then(|person| person.key)
                .and_then(|id| entity_external_id(pack, id))
                == Some(key.as_str())
        }
        (SceneReference::ParticipantName { index }, PerceivedReference::Name { text }) => {
            scene
                .participants
                .get(*index)
                .and_then(|person| person.name.as_deref())
                == Some(text.as_str())
        }
        (
            SceneReference::ParticipantDescription { index },
            PerceivedReference::Description { text },
        ) => {
            scene
                .participants
                .get(*index)
                .and_then(|person| person.description.as_deref())
                == Some(text.as_str())
        }
        _ => false,
    }
}

fn resolution_matches(
    expected: &crate::ExpectedReferenceResolution,
    actual: &cmem_eval::character_memory::SceneReferenceResolution,
    pack: &cmem_eval::RetrievedContextPack,
) -> bool {
    use crate::ExpectedReferenceResolution as Expected;
    use cmem_eval::character_memory::SceneReferenceResolution as Actual;
    match (expected, actual) {
        (Expected::Resolved { entity }, Actual::Resolved { notion_id }) => {
            entity_external_id(pack, *notion_id) == Some(entity.as_str())
        }
        (Expected::Ambiguous { candidates }, Actual::Ambiguous { notion_ids }) => {
            let actual = notion_ids
                .iter()
                .map(|id| entity_external_id(pack, *id))
                .collect::<Option<BTreeSet<_>>>();
            actual.as_ref() == Some(&candidates.iter().map(String::as_str).collect())
        }
        (Expected::Unknown, Actual::Unknown) => true,
        _ => false,
    }
}

fn scene_matches(
    scenario: &ContinuityScenario,
    episode_id: cmem_eval::character_memory::MemoryId,
    expected: &crate::Scene,
    actual: &cmem_eval::character_memory::Scene,
    pack: &cmem_eval::RetrievedContextPack,
) -> bool {
    use crate::PerceivedReference;
    let time_matches = pack
        .object_refs()
        .get(&episode_id.to_string())
        .filter(|object| object.object_type == cmem_eval::ObjectType::Episode)
        .is_some_and(|object| {
            scenario.events.iter().any(|event| {
                matches!(
                    event,
                    InteractionEvent::Experience { event_id, timestamp, .. }
                        if event_id == &object.external_id && *timestamp == actual.time
                )
            })
        });
    let expected_participants = expected
        .who
        .iter()
        .map(|person| match &person.reference {
            PerceivedReference::Key { key } => (Some(key.as_str()), None, None),
            PerceivedReference::Name { text } => (None, Some(text.as_str()), None),
            PerceivedReference::Description { text } => (None, None, Some(text.as_str())),
            PerceivedReference::Setting { .. } => unreachable!("fixture admission"),
        })
        .collect::<BTreeSet<_>>();
    let actual_participants = actual
        .participants
        .iter()
        .map(|person| {
            let key = match person.key {
                Some(id) => Some(entity_external_id(pack, id)?),
                None => None,
            };
            Some((key, person.name.as_deref(), person.description.as_deref()))
        })
        .collect::<Option<BTreeSet<_>>>();
    let (key, words) = match &expected.place {
        Some(PerceivedReference::Key { key } | PerceivedReference::Setting { key }) => {
            (Some(key.as_str()), None)
        }
        Some(PerceivedReference::Name { text } | PerceivedReference::Description { text }) => {
            (None, Some(text.as_str()))
        }
        None => (None, None),
    };
    time_matches
        && expected.what.is_none()
        && actual_participants.as_ref() == Some(&expected_participants)
        && actual.setting.key.as_deref() == key
        && actual.setting.words.as_deref() == words
        && actual.custom_values == expected.custom
}

fn check_cue(
    cue: crate::CueKind,
    expected: bool,
    observed: Option<&BTreeSet<cmem_eval::character_memory::CueKind>>,
) -> bool {
    cue.native()
        .is_some_and(|cue| observed.is_some_and(|observed| observed.contains(&cue) == expected))
}

fn native_cues(
    pack: &cmem_eval::RetrievedContextPack,
    memory: &str,
) -> Option<BTreeSet<cmem_eval::character_memory::CueKind>> {
    let mut observed = None::<BTreeSet<_>>;
    for assignment in pack
        .outcomes()
        .iter()
        .filter_map(|outcome| outcome.trace.as_ref())
        .flat_map(|trace| &trace.section_assignments)
        .filter(|assignment| {
            matches!(
                assignment.reason,
                cmem_eval::character_memory::SectionAssignmentReason::Selected { .. }
            )
        })
    {
        if pack
            .object_refs()
            .get(&assignment.object.id.to_string())
            .is_some_and(|object| {
                object.external_id == memory
                    || (object.object_type == cmem_eval::ObjectType::Observation
                        && object.external_id == crate::observation_external_id(memory))
            })
        {
            observed
                .get_or_insert_with(BTreeSet::new)
                .extend(&assignment.cue_kinds);
        }
    }
    observed
}
const CONTINUITY_METRICS: [&str; 6] = [
    "continuity_gap_days",
    "hub_context_share",
    "correction_lifecycle_safe_admission_rate",
    "supersession_replacement_recall",
    "sampled_context_pollution_rate",
    "sampled_event_pollution_rate",
];

pub fn continuity_metric_family(
    config: &MetricsConfig,
    _scenarios: &[ContinuityScenario],
) -> MetricFamily {
    let mut required_metrics = CONTINUITY_METRICS
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    for k in &config.ks_session {
        for bucket in GAP_BUCKETS {
            required_metrics.insert(format!("continuity_recall_fraction_gap_{bucket}@{k}"));
        }
        required_metrics.insert(format!("temporal_recall_fraction@{k}"));
    }
    MetricFamily::new("continuity", required_metrics)
}

pub fn insert_continuity_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryObservation,
    config: &MetricsConfig,
    mode: RetrievalMode,
) {
    // Raw baselines return candidate items, not the native graph-validated context pack.
    let graph_outcomes = if mode == RetrievalMode::Hybrid {
        trace.retrieval.outcomes()
    } else {
        &[]
    };
    if trace.pattern == ScenarioPattern::Abstention {
        insert_pollution_metrics(out, trace);
        return;
    }

    let retrieved_ids = ranked_ids_for_gold(
        trace.retrieval.items(),
        &trace.expected.relevant_external_ids,
    );
    if let Some(gap_days) = gap_days(scenario, trace) {
        out.insert("continuity_gap_days".to_string(), Value::from(gap_days));
        let bucket = gap_bucket(gap_days);
        for k in &config.ks_session {
            if let Some(summary) =
                retrieval_metrics(&retrieved_ids, &trace.expected.relevant_external_ids, *k)
            {
                out.insert(
                    format!("continuity_recall_fraction_gap_{bucket}@{k}"),
                    Value::from(summary.recall_fraction),
                );
            }
        }
    }

    insert_hub_metrics(out, scenario, trace);
    if matches!(
        trace.pattern,
        ScenarioPattern::TemporalStructure | ScenarioPattern::TemporalPatterns
    ) {
        for k in &config.ks_session {
            if let Some(summary) =
                retrieval_metrics(&retrieved_ids, &trace.expected.relevant_external_ids, *k)
            {
                out.insert(
                    format!("temporal_recall_fraction@{k}"),
                    Value::from(summary.recall_fraction),
                );
            }
        }
    }
    insert_correction_metrics(out, scenario, trace, &retrieved_ids, graph_outcomes);
    insert_pollution_metrics(out, trace);
}

fn insert_hub_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryObservation,
) {
    let hub_ids = scenario
        .entities
        .iter()
        .filter(|entity| entity.is_hub)
        .map(|entity| entity.external_id.as_str())
        .collect::<BTreeSet<_>>();
    if hub_ids.is_empty() {
        return;
    }
    let hub_incident_ids = scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Remember {
                external_id,
                entity_external_ids,
                ..
            } if entity_external_ids
                .iter()
                .any(|entity_id| hub_ids.contains(entity_id.as_str())) =>
            {
                Some(external_id.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    out.insert(
        "hub_context_share".to_string(),
        rate(
            trace
                .retrieval
                .items()
                .iter()
                .filter(|item| item_matches_any(item, &hub_incident_ids))
                .count(),
            trace.retrieval.items().len(),
        ),
    );
}

fn insert_correction_metrics(
    out: &mut Map<String, Value>,
    scenario: &ContinuityScenario,
    trace: &ContinuityQueryObservation,
    retrieved_ids: &[String],
    graph_outcomes: &[RetrieveOutcome],
) {
    if !matches!(
        scenario.pattern,
        ScenarioPattern::CorrectionChains | ScenarioPattern::EntrenchedCorrection
    ) {
        return;
    }
    if let Some(rate) =
        cmem_eval::lifecycle_safe_admission_rate(trace.retrieval.items(), graph_outcomes)
    {
        out.insert(
            "correction_lifecycle_safe_admission_rate".to_string(),
            Value::from(rate),
        );
    }
    let labeled_replacements = scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Correct {
                replacement_external_id,
                timestamp,
                ..
            } if *timestamp <= trace.timestamp
                && trace
                    .expected
                    .relevant_external_ids
                    .contains(replacement_external_id) =>
            {
                Some(replacement_external_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if !labeled_replacements.is_empty() {
        let retrieved = retrieved_ids.iter().collect::<BTreeSet<_>>();
        out.insert(
            "supersession_replacement_recall".to_string(),
            Value::from(
                labeled_replacements
                    .iter()
                    .filter(|external_id| retrieved.contains(external_id))
                    .count() as f64
                    / labeled_replacements.len() as f64,
            ),
        );
    }
}

fn insert_pollution_metrics(out: &mut Map<String, Value>, trace: &ContinuityQueryObservation) {
    let relevant = trace
        .expected
        .relevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let sampled_irrelevant = trace
        .expected
        .irrelevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let labeled = trace
        .retrieval
        .items()
        .iter()
        .filter(|item| {
            item_matches_any(item, &relevant) || item_matches_any(item, &sampled_irrelevant)
        })
        .collect::<Vec<_>>();
    let pollution = labeled
        .iter()
        .filter(|item| {
            !item_matches_any(item, &relevant) && item_matches_any(item, &sampled_irrelevant)
        })
        .collect::<Vec<_>>();
    out.insert(
        "sampled_context_pollution_rate".to_string(),
        rate(pollution.len(), labeled.len()),
    );
    let mut labeled_event_roots = BTreeMap::new();
    for item in &labeled {
        let is_relevant = item_matches_any(item, &relevant);
        let is_pollution = item_matches_any(item, &sampled_irrelevant);
        let Some(root_external_id) = item
            .episode_external_id
            .as_deref()
            .or(item.external_id.as_deref())
        else {
            continue;
        };
        let root_is_pollution = labeled_event_roots
            .entry(root_external_id)
            .or_insert(is_pollution);
        if is_relevant {
            *root_is_pollution = false;
        }
    }
    out.insert(
        "sampled_event_pollution_rate".to_string(),
        rate(
            labeled_event_roots
                .values()
                .filter(|is_pollution| **is_pollution)
                .count(),
            labeled_event_roots.len(),
        ),
    );
}

fn ranked_ids_for_gold(items: &[RetrievedItem], gold_ids: &[String]) -> Vec<String> {
    let gold = gold_ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
    items
        .iter()
        .map(|item| {
            [item.external_id.as_ref(), item.episode_external_id.as_ref()]
                .into_iter()
                .flatten()
                .find(|external_id| gold.contains(external_id.as_str()))
                .or(item.external_id.as_ref())
                .or(item.episode_external_id.as_ref())
                .cloned()
                .unwrap_or_else(|| format!("internal:{}.{}", item.kind, item.internal_id))
        })
        .collect()
}

fn gap_days(scenario: &ContinuityScenario, trace: &ContinuityQueryObservation) -> Option<f64> {
    let relevant = trace
        .expected
        .relevant_external_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    scenario
        .events
        .iter()
        .filter_map(|event| match event {
            InteractionEvent::Remember {
                external_id,
                timestamp,
                ..
            }
            | InteractionEvent::Link {
                external_id,
                timestamp,
                ..
            } if relevant.contains(external_id.as_str()) && *timestamp <= trace.timestamp => {
                Some(*timestamp)
            }
            InteractionEvent::Correct {
                replacement_external_id,
                timestamp,
                ..
            } if relevant.contains(replacement_external_id.as_str())
                && *timestamp <= trace.timestamp =>
            {
                Some(*timestamp)
            }
            _ => None,
        })
        .map(|timestamp| (trace.timestamp - timestamp).num_seconds() as f64 / 86_400.0)
        .max_by(|left, right| left.total_cmp(right))
}

fn gap_bucket(days: f64) -> &'static str {
    if days < 30.0 {
        "short"
    } else if days < 180.0 {
        "medium"
    } else {
        "long"
    }
}

fn item_matches_any(item: &RetrievedItem, ids: &BTreeSet<&str>) -> bool {
    item.external_id
        .as_deref()
        .is_some_and(|external_id| ids.contains(external_id))
        || item
            .episode_external_id
            .as_deref()
            .is_some_and(|external_id| ids.contains(external_id))
}

fn rate(numerator: usize, denominator: usize) -> Value {
    if denominator == 0 {
        Value::Null
    } else {
        Value::from(numerator as f64 / denominator as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContinuityScenarioEmbedding, EntityDeclaration, ExpectedRelevanceRecord};
    use chrono::{TimeZone, Utc};
    use cmem_eval::character_memory::{
        ContextPackSection, ContinuityContextPack, LifecycleFilterAction, LifecycleFilterDecision,
        LifecycleFilterReason, MemoryObjectRef, RetentionState, RetrievalRationale, RetrievalTrace,
        RetrieveOutcome, SectionAssignment, SectionAssignmentReason, SectionScoreComponents,
    };
    use cmem_eval::{ContextRenderer, ObjectType, RetrievedContextPack};

    fn object(id: &str) -> MemoryObjectRef {
        MemoryObjectRef::new(
            ObjectType::Episode,
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, id.as_bytes()),
        )
    }
    fn assignment(id: &str, cues: Vec<cmem_eval::character_memory::CueKind>) -> SectionAssignment {
        SectionAssignment {
            object: object(id),
            section: ContextPackSection::RelevantEpisodes,
            rank: Some(1),
            reason: SectionAssignmentReason::Selected {
                scores: SectionScoreComponents {
                    final_score: 1.0,
                    cue_score: None,
                    graph_score: None,
                    salience_score: None,
                },
            },
            cue_kinds: cues.into_iter().collect(),
        }
    }
    fn unsafe_decision(id: &str) -> LifecycleFilterDecision {
        LifecycleFilterDecision {
            object: object(id),
            retention_state: Some(RetentionState::Suppressed),
            superseded_by: Vec::new(),
            action: LifecycleFilterAction::Included,
            reason: LifecycleFilterReason::SuppressedIncludedByPolicy,
        }
    }

    fn item(id: &str, rank: usize) -> RetrievedItem {
        RetrievedItem {
            kind: ObjectType::Episode,
            internal_id: object(id).id.to_string(),
            external_id: Some(id.to_string()),
            episode_external_id: None,
            score: Some(1.0 / rank as f64),
            rank,
            rationale: vec!["typed trace".to_string()],
            text: None,
        }
    }

    #[test]
    fn situated_cue_predicate_distinguishes_missing_wrong_and_unavailable_facts() {
        use crate::CueKind;
        use cmem_eval::character_memory::CueKind as NativeCueKind;
        let observed = BTreeSet::from([NativeCueKind::Participant, NativeCueKind::Place]);
        assert!(check_cue(CueKind::Pair, true, Some(&observed)));
        assert!(!check_cue(CueKind::Pair, false, Some(&observed)));
        assert!(check_cue(CueKind::Place, true, Some(&observed)));
        assert!(!check_cue(CueKind::Activity, true, Some(&observed)));
        assert!(!check_cue(CueKind::Due, true, Some(&observed)));
        assert!(!check_cue(CueKind::Due, false, Some(&observed)));
        assert!(!check_cue(CueKind::Due, false, None));
    }

    #[test]
    fn cue_facts_union_selected_assignments_and_keep_missing_facts_unavailable() {
        use cmem_eval::character_memory::CueKind as Native;
        let mut episode = assignment("visit", vec![Native::Topic, Native::Place]);
        let mut observation =
            assignment("observation", vec![Native::Participant, Native::Activity]);
        observation.object.object_type = ObjectType::Observation;
        let refs = [
            (&episode, "visit".to_string()),
            (&observation, crate::observation_external_id("visit")),
        ]
        .into_iter()
        .map(|(assignment, external_id)| {
            (
                assignment.object.id.to_string(),
                cmem_eval::MemoryEndpointInput {
                    external_id,
                    object_type: assignment.object.object_type,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
        let make_pack = |assignments: Vec<SectionAssignment>| {
            let outcomes = assignments
                .into_iter()
                .map(|assignment| {
                    let mut outcome =
                        trace(ScenarioPattern::SelectiveEntity).retrieval.outcomes()[0].clone();
                    outcome.trace.as_mut().unwrap().section_assignments = vec![assignment];
                    outcome
                })
                .collect();
            RetrievedContextPack::from_ranked_items(vec![], outcomes, ContextRenderer::PlainText)
                .with_object_refs(refs.clone())
        };
        let pack = make_pack(vec![episode.clone(), observation.clone()]);
        let observed = native_cues(&pack, "visit").unwrap();
        assert_eq!(
            observed,
            BTreeSet::from([
                Native::Topic,
                Native::Place,
                Native::Participant,
                Native::Activity
            ])
        );
        for cue in [
            crate::CueKind::Topic,
            crate::CueKind::Place,
            crate::CueKind::Pair,
            crate::CueKind::Activity,
        ] {
            assert!(check_cue(cue, true, Some(&observed)));
            assert!(!check_cue(cue, false, Some(&observed)));
        }
        let SectionAssignmentReason::Selected { scores } = episode.reason else {
            unreachable!()
        };
        episode.reason = SectionAssignmentReason::OmittedByLimit {
            intended_section: episode.section,
            scores,
        };
        let pack = make_pack(vec![episode.clone(), observation]);
        let observed = native_cues(&pack, "visit").unwrap();
        assert!(check_cue(crate::CueKind::Topic, false, Some(&observed)));
        let pack = make_pack(vec![episode]);
        for memory in ["visit", "missing"] {
            let observed = native_cues(&pack, memory);
            assert!(observed.is_none());
            for expected in [true, false] {
                assert!(!check_cue(
                    crate::CueKind::Topic,
                    expected,
                    observed.as_ref()
                ));
            }
        }
    }

    #[test]
    fn situated_measures_and_assertions_use_every_native_outcome_and_authored_id_counts() {
        use crate::{
            CarriedAssertion, MemorySection, OmissionReason, OmittedAssertion, ProbeAssertions,
            ProbeMeasures, RecallReason, ScenarioStatus, SceneSelection,
        };
        use cmem_eval::character_memory::{
            DerivedMemoryDraft, EpisodeDraft, LifecycleFilterAction, LifecycleFilterDecision,
            LifecycleFilterReason, LifecycleOmissionSummary, MemoryObjectRef, MemoryThreadDraft,
            ObservationDraft,
        };
        let mut scenario = crate::driver::tests::situated_scenario();
        let mut authored_thread = scenario.events[2].clone();
        if let InteractionEvent::Derive {
            event_id, memory, ..
        } = &mut authored_thread
        {
            *event_id = "thread".into();
            memory.subtype = crate::AuthoredMemoryKind::Thread;
        }
        scenario.events.insert(3, authored_thread);
        scenario.validate().unwrap();
        let visit = EpisodeDraft {
            scene: Some(cmem_eval::character_memory::Scene::at(
                chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
            )),
            ..EpisodeDraft::new("visit")
        }
        .into_domain()
        .unwrap();
        let noise = EpisodeDraft {
            scene: Some(cmem_eval::character_memory::Scene::at(
                chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
            )),
            ..EpisodeDraft::new("noise")
        }
        .into_domain()
        .unwrap();
        let observation = ObservationDraft::new(visit.id, "observation")
            .into_domain()
            .unwrap();
        let noise_observation = ObservationDraft::new(noise.id, "noise observation")
            .into_domain()
            .unwrap();
        let noise_observation_external = crate::observation_external_id("noise");
        let mut draft = DerivedMemoryDraft::new(cmem_eval::DerivedType::Commitment, "promise");
        draft.derived_from_episode_ids.push(visit.id);
        let promise = draft.into_domain().unwrap();
        let thread = MemoryThreadDraft::new("thread", "thread")
            .into_domain()
            .unwrap();
        let refs = [
            (visit.id, "visit", ObjectType::Episode),
            (noise.id, "noise", ObjectType::Episode),
            (promise.id, "promise", ObjectType::DerivedMemory),
            (thread.id, "thread", ObjectType::MemoryThread),
            (
                noise_observation.id,
                noise_observation_external.as_str(),
                ObjectType::Observation,
            ),
        ]
        .into_iter()
        .map(|(id, external_id, object_type)| {
            (
                id.to_string(),
                cmem_eval::MemoryEndpointInput {
                    external_id: external_id.into(),
                    object_type,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
        let mut native = ContinuityContextPack::empty();
        native.relevant_episodes = vec![visit.clone()];
        native.salient_observations = vec![observation, noise_observation.clone()];
        native.commitments = vec![promise.clone().into()];
        native.derived_memories = vec![promise.into()];
        native.active_threads = vec![thread];
        let mut outcome = RetrieveOutcome {
            time_range: None,
            activity: None,
            scene: cmem_eval::character_memory::Scene::at(
                chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
            ),
            scene_references: Vec::new(),
            memory_scenes: Vec::new(),
            pack: native,
            rationale: RetrievalRationale::new("test"),
            trace: Some(RetrievalTrace::empty()),
        };
        let make_pack = |mut outcome: RetrieveOutcome| {
            let observation_outcome = RetrieveOutcome {
                time_range: None,
                activity: None,
                scene: cmem_eval::character_memory::Scene::at(
                    chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
                ),
                scene_references: Vec::new(),
                memory_scenes: Vec::new(),
                pack: ContinuityContextPack {
                    salient_observations: std::mem::take(&mut outcome.pack.salient_observations),
                    commitments: std::mem::take(&mut outcome.pack.commitments),
                    derived_memories: std::mem::take(&mut outcome.pack.derived_memories),
                    ..ContinuityContextPack::empty()
                },
                rationale: std::mem::replace(
                    &mut outcome.rationale,
                    RetrievalRationale::new("episode"),
                ),
                trace: outcome.trace.take(),
            };
            let mut rendered = item("flattened-decoy", 1);
            rendered.text = Some("hello world".into());
            RetrievedContextPack::from_ranked_items(
                vec![rendered],
                vec![outcome, observation_outcome],
                ContextRenderer::PlainText,
            )
            .with_object_refs(refs.clone())
        };
        let assertions = ProbeAssertions {
            carried: vec![
                CarriedAssertion {
                    memory: "visit".into(),
                    reason: RecallReason::Pair,
                    section: Some(MemorySection::Episodes),
                },
                CarriedAssertion {
                    memory: "promise".into(),
                    reason: RecallReason::Due,
                    section: Some(MemorySection::Commitments),
                },
            ],
            in_order: vec![vec!["visit".into(), "promise".into()]],
            ..Default::default()
        };
        let measures = ProbeMeasures {
            bystanders: vec!["noise".into()],
        };
        let mut event = InteractionEvent::Probe {
            event_id: "probe".into(),
            query_id: "probe".into(),
            timestamp: scenario.events[3].timestamp(),
            scene: SceneSelection::Named {
                name: "pair".into(),
            },
            topic: None,
            assertions: Box::new(assertions.clone()),
            measures: measures.clone(),
        };
        let pack = make_pack(outcome.clone());
        assert_eq!(pack.outcomes().len(), 2);
        let measured = situated_probe_measures(&scenario, &assertions, &measures, Some(&pack));
        assert_eq!(measured.bystander_context_share, Some(1.0 / 3.0));
        assert_eq!(measured.context_tokens, Some(2));
        assert_eq!(measured.carried_recall_by_reason["pair"].recall, Some(1.0));
        assert_eq!(measured.carried_recall_by_reason["due"].admitted, Some(1));
        assert!(
            check_probe_assertions(&scenario, &event, &pack)
                .iter()
                .all(|result| result.check.status == ScenarioStatus::Passed)
        );
        let not_run = situated_probe_measures(&scenario, &assertions, &measures, None);
        assert_eq!(not_run.context_tokens, None);
        assert_eq!(not_run.bystander_context_share, None);
        assert_eq!(not_run.carried_recall_by_reason["pair"].recall, None);
        let empty = situated_probe_measures(
            &scenario,
            &assertions,
            &measures,
            Some(&RetrievedContextPack::default()),
        );
        assert_eq!(empty.context_tokens, Some(0));
        assert_eq!(empty.bystander_context_share, None);
        assert_eq!(empty.carried_recall_by_reason["pair"].recall, Some(0.0));

        let InteractionEvent::Probe { assertions, .. } = &mut event else {
            unreachable!()
        };
        assertions.carried[0].section = Some(MemorySection::Commitments);
        assertions.in_order[0].reverse();
        assertions.omitted.push(OmittedAssertion {
            memory: "noise".into(),
            reason: OmissionReason::Suppression,
        });
        outcome
            .pack
            .relevant_episodes
            .retain(|episode| episode.id != noise.id);
        outcome
            .pack
            .salient_observations
            .retain(|observation| observation.episode_id != noise.id);
        outcome.pack.active_threads.clear();
        let failed = check_probe_assertions(&scenario, &event, &make_pack(outcome.clone()));
        assert_eq!(
            failed
                .iter()
                .filter(|result| result.check.status == ScenarioStatus::Failed)
                .map(|result| (
                    result.identity.assertion.clone(),
                    result.check.reason.as_str()
                ))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                (
                    crate::AssertionSubject::Carried("visit".into()),
                    "memory is absent from the requested native section",
                ),
                (
                    crate::AssertionSubject::InOrder(vec!["promise".into(), "visit".into()]),
                    "a memory is missing or the native pack order differs",
                ),
                (
                    crate::AssertionSubject::Omitted("noise".into()),
                    "memory is present or the requested native omission reason is absent",
                ),
            ])
        );
        outcome
            .trace
            .as_mut()
            .unwrap()
            .lifecycle_filter_decisions
            .push(LifecycleFilterDecision {
                object: MemoryObjectRef::new(ObjectType::Observation, noise_observation.id),
                retention_state: Some(cmem_eval::RetentionState::Suppressed),
                superseded_by: Vec::new(),
                action: LifecycleFilterAction::Omitted,
                reason: LifecycleFilterReason::SuppressedOmitted,
            });
        outcome.rationale.lifecycle_omission_count = 1;
        assert!(!omissions_have_reasons(&[]));
        assert!(!omissions_have_reasons(&[outcome.clone()]));
        outcome
            .rationale
            .lifecycle_omission_reasons
            .push(LifecycleOmissionSummary {
                reason: LifecycleFilterReason::SuppressedOmitted,
                count: 1,
            });
        assert!(omissions_have_reasons(&[outcome.clone()]));
        // Resolution is not inferred from absence, or from a different native reason.
        for reason in [
            None,
            Some(LifecycleFilterReason::SuppressedOmitted),
            Some(LifecycleFilterReason::ResolvedOmitted),
        ] {
            let mut native = outcome.clone();
            let id = native.pack.commitments[0].memory.id;
            native.pack.commitments.clear();
            native.pack.derived_memories.clear();
            native.trace.as_mut().unwrap().lifecycle_filter_decisions = reason
                .into_iter()
                .map(|reason| LifecycleFilterDecision {
                    object: MemoryObjectRef::new(ObjectType::DerivedMemory, id),
                    retention_state: Some(cmem_eval::RetentionState::Active),
                    superseded_by: Vec::new(),
                    action: LifecycleFilterAction::Omitted,
                    reason,
                })
                .collect();
            let mut expected = event.clone();
            let InteractionEvent::Probe { assertions, .. } = &mut expected else {
                unreachable!()
            };
            assertions.omitted = vec![OmittedAssertion {
                memory: "promise".into(),
                reason: OmissionReason::Resolution,
            }];
            let check = check_probe_assertions(&scenario, &expected, &make_pack(native))
                .into_iter()
                .find(|check| {
                    matches!(
                        check.identity.assertion,
                        crate::AssertionSubject::Omitted(_)
                    )
                })
                .unwrap();
            assert_eq!(
                check.check.status == ScenarioStatus::Passed,
                reason == Some(LifecycleFilterReason::ResolvedOmitted)
            );
        }
        let checked = check_probe_assertions(&scenario, &event, &make_pack(outcome));
        let omission = checked
            .iter()
            .find(|result| {
                matches!(
                    result.identity.assertion,
                    crate::AssertionSubject::Omitted(_)
                )
            })
            .unwrap();
        assert_eq!(omission.check.status, ScenarioStatus::Passed);
    }

    fn scenario(pattern: ScenarioPattern) -> ContinuityScenario {
        ContinuityScenario {
            catalog_situations: Vec::new(),
            character_entity: None,
            scenes: BTreeMap::new(),
            requirements: Default::default(),
            fixture_id: "fixture".to_string(),
            namespace: "namespace".to_string(),
            pattern,
            entities: vec![EntityDeclaration {
                external_id: "hub-person".to_string(),
                label: "Hub".to_string(),
                is_hub: true,
            }],
            embedding: ContinuityScenarioEmbedding::controllable_similarity_provider(
                cmem_eval::ControllableSimilarityFixture {
                    seed: 1,
                    vector_size: 2,
                    noise_magnitude: 0.0,
                    clusters: BTreeMap::new(),
                    concepts: BTreeMap::new(),
                },
            ),
            events: vec![
                InteractionEvent::Remember {
                    event_id: "remember".to_string(),
                    external_id: "relevant".to_string(),
                    timestamp: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
                    text: "relevant".to_string(),
                    surface_texts: None,
                    entity_external_ids: vec!["hub-person".to_string()],
                    thread: None,
                    salience: 1.0,
                },
                InteractionEvent::Remember {
                    event_id: "negative".to_string(),
                    external_id: "sampled-negative".to_string(),
                    timestamp: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
                    text: "negative".to_string(),
                    surface_texts: None,
                    entity_external_ids: vec!["hub-person".to_string()],
                    thread: None,
                    salience: 0.5,
                },
                InteractionEvent::Correct {
                    event_id: "correct".to_string(),
                    target_external_id: "old".to_string(),
                    replacement_external_id: "relevant".to_string(),
                    timestamp: Utc.with_ymd_and_hms(2025, 7, 1, 0, 0, 0).unwrap(),
                    replacement_text: "corrected".to_string(),
                },
            ],
        }
    }

    fn trace(pattern: ScenarioPattern) -> ContinuityQueryObservation {
        let items = vec![item("relevant", 1), item("sampled-negative", 2)];
        let mut native_trace = RetrievalTrace::empty();
        native_trace.section_assignments = vec![
            assignment(
                "relevant",
                vec![cmem_eval::character_memory::CueKind::Participant],
            ),
            assignment(
                "sampled-negative",
                vec![cmem_eval::character_memory::CueKind::Topic],
            ),
        ];
        ContinuityQueryObservation {
            fixture_id: "fixture".to_string(),
            namespace: "namespace".to_string(),
            pattern,
            event_id: "query".to_string(),
            query_id: "query".to_string(),
            timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            query: "query".to_string(),
            expected: ExpectedRelevanceRecord {
                relevant_external_ids: vec!["relevant".to_string()],
                irrelevant_external_ids: vec!["sampled-negative".to_string()],
            },
            history_text: String::new(),
            retrieval: RetrievedContextPack::from_ranked_items(
                items,
                vec![RetrieveOutcome {
                    time_range: None,
                    activity: None,
                    scene: cmem_eval::character_memory::Scene::at(
                        chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.into(),
                    ),
                    scene_references: Vec::new(),
                    memory_scenes: Vec::new(),
                    pack: ContinuityContextPack::empty(),
                    rationale: RetrievalRationale::new("test"),
                    trace: Some(native_trace),
                }],
                ContextRenderer::PlainText,
            ),
            write_outcomes: Vec::new(),
            link_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
        }
    }

    fn mutate_trace(
        trace: &mut ContinuityQueryObservation,
        mutate: impl FnOnce(&mut Option<RetrievalTrace>),
    ) {
        let (items, _, _, _, mut outcomes) = std::mem::take(&mut trace.retrieval).into_parts();
        mutate(&mut outcomes[0].trace);
        trace.retrieval =
            RetrievedContextPack::from_ranked_items(items, outcomes, ContextRenderer::PlainText);
    }

    fn replace_items(trace: &mut ContinuityQueryObservation, items: Vec<RetrievedItem>) {
        let (_, _, _, _, telemetry) = std::mem::take(&mut trace.retrieval).into_parts();
        trace.retrieval =
            RetrievedContextPack::from_ranked_items(items, telemetry, ContextRenderer::PlainText);
    }

    fn metrics(pattern: ScenarioPattern) -> Map<String, Value> {
        let scenario = scenario(pattern);
        let trace = trace(pattern);
        let mut out = Map::new();
        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );
        out
    }

    #[test]
    fn continuity_recall_is_bucketed_by_hand_computed_gap() {
        let out = metrics(ScenarioPattern::LongGapRecall);
        assert_eq!(out["continuity_gap_days"], 365.0);
        assert_eq!(out["continuity_recall_fraction_gap_long@5"], 1.0);
        assert!(out.get("continuity_recall_fraction_gap_short@5").is_none());
    }

    #[test]
    fn entity_continuity_measures_hub_context_share() {
        let scenario = scenario(ScenarioPattern::RecurringHubEntity);
        let trace = trace(ScenarioPattern::RecurringHubEntity);
        let mut out = Map::new();
        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );
        assert_eq!(out["hub_context_share"], 1.0);
    }

    #[test]
    fn temporal_quality_reuses_hand_computed_recall() {
        let out = metrics(ScenarioPattern::TemporalStructure);
        assert_eq!(out["temporal_recall_fraction@5"], 1.0);
    }

    #[test]
    fn temporal_patterns_route_to_temporal_recall_with_hand_computed_expectation() {
        let scenario = scenario(ScenarioPattern::TemporalPatterns);
        let mut trace = trace(ScenarioPattern::TemporalPatterns);
        trace
            .expected
            .relevant_external_ids
            .push("relevant-not-returned".to_string());
        let mut out = Map::new();

        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );

        assert_eq!(out["temporal_recall_fraction@5"], 0.5);
    }

    #[test]
    fn correction_safety_combines_lifecycle_telemetry_and_replacement_labels() {
        let scenario = scenario(ScenarioPattern::CorrectionChains);
        let trace = trace(ScenarioPattern::CorrectionChains);
        let config = MetricsConfig::default();
        let family = continuity_metric_family(&config, std::slice::from_ref(&scenario));
        for mode in [
            RetrievalMode::Hybrid,
            RetrievalMode::VectorOnly,
            RetrievalMode::Bm25Only,
        ] {
            let mut out = Map::new();
            cmem_eval::initialize_registry_metrics_for(&mut out, std::slice::from_ref(&family));
            insert_continuity_metrics(&mut out, &scenario, &trace, &config, mode);
            let native = [(
                "correction_lifecycle_safe_admission_rate",
                &out["correction_lifecycle_safe_admission_rate"],
            )];
            for (key, value) in native {
                if mode == RetrievalMode::Hybrid {
                    assert!(value.is_number(), "{key}: {value}");
                } else {
                    assert!(value.is_null(), "{mode:?} {key}: {value}");
                }
            }
            assert_eq!(out["supersession_replacement_recall"], 1.0);
            assert_eq!(out["hub_context_share"], 1.0);
            assert_eq!(out["sampled_context_pollution_rate"], 0.5);
            assert_eq!(out["sampled_event_pollution_rate"], 0.5);
        }
    }

    #[test]
    fn entrenched_correction_routes_to_correction_metrics_with_hand_computed_expectations() {
        let scenario = scenario(ScenarioPattern::EntrenchedCorrection);
        let mut trace = trace(ScenarioPattern::EntrenchedCorrection);
        mutate_trace(&mut trace, |telemetry| {
            telemetry
                .as_mut()
                .unwrap()
                .lifecycle_filter_decisions
                .push(unsafe_decision("relevant"));
        });
        let mut out = Map::new();

        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );

        assert_eq!(trace.retrieval.items().len(), 2);
        assert_eq!(out["correction_lifecycle_safe_admission_rate"], 0.5);
        assert_eq!(out["supersession_replacement_recall"], 1.0);
    }

    #[test]
    fn correction_metrics_stay_unsupported_for_unrelated_scenarios() {
        let scenario = scenario(ScenarioPattern::LongGapRecall);
        let trace = trace(ScenarioPattern::LongGapRecall);
        let family =
            continuity_metric_family(&MetricsConfig::default(), std::slice::from_ref(&scenario));
        let mut out = Map::new();
        cmem_eval::initialize_registry_metrics_for(&mut out, std::slice::from_ref(&family));

        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );

        assert_eq!(out["correction_lifecycle_safe_admission_rate"], Value::Null);
        assert_eq!(out["supersession_replacement_recall"], Value::Null);
    }

    #[test]
    fn sampled_pollution_does_not_classify_unlabeled_items_as_negative() {
        let scenario = scenario(ScenarioPattern::MixedSalienceAccumulation);
        let mut trace = trace(ScenarioPattern::MixedSalienceAccumulation);
        let mut items = trace.retrieval.items().to_vec();
        items.push(item("unlabeled", 3));
        replace_items(&mut trace, items);
        let mut out = Map::new();
        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );
        assert_eq!(out["sampled_context_pollution_rate"], 0.5);
        assert_eq!(out["sampled_event_pollution_rate"], 0.5);
    }

    #[test]
    fn abstention_is_scored_with_pollution_metrics_only() {
        let scenario = scenario(ScenarioPattern::Abstention);
        let mut trace = trace(ScenarioPattern::Abstention);
        trace.expected.relevant_external_ids.clear();
        let mut out = Map::new();

        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );

        assert_eq!(out["sampled_context_pollution_rate"], 1.0);
        assert_eq!(out["sampled_event_pollution_rate"], 1.0);
        assert!(out.get("continuity_gap_days").is_none());
        assert!(out.get("hub_context_share").is_none());
        assert!(out.get("fanout_over_budget_count").is_none());
    }

    #[test]
    fn event_pollution_deduplicates_surfaces_by_episode_root() {
        let scenario = scenario(ScenarioPattern::SurfaceContribution);
        let mut trace = trace(ScenarioPattern::SurfaceContribution);
        replace_items(
            &mut trace,
            vec![
                RetrievedItem {
                    kind: ObjectType::Episode,
                    internal_id: "relevant-episode".to_string(),
                    external_id: Some("relevant".to_string()),
                    episode_external_id: None,
                    score: None,
                    rank: 1,
                    rationale: Vec::new(),
                    text: None,
                },
                RetrievedItem {
                    kind: ObjectType::Observation,
                    internal_id: "relevant-observation".to_string(),
                    external_id: Some("relevant:observation".to_string()),
                    episode_external_id: Some("relevant".to_string()),
                    score: None,
                    rank: 2,
                    rationale: Vec::new(),
                    text: None,
                },
                RetrievedItem {
                    kind: ObjectType::Episode,
                    internal_id: "negative-episode".to_string(),
                    external_id: Some("sampled-negative".to_string()),
                    episode_external_id: None,
                    score: None,
                    rank: 3,
                    rationale: Vec::new(),
                    text: None,
                },
            ],
        );
        mutate_trace(&mut trace, |telemetry| {
            *telemetry = None;
        });
        let mut out = Map::new();

        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );

        assert_eq!(out["sampled_context_pollution_rate"], 1.0 / 3.0);
        assert_eq!(out["sampled_event_pollution_rate"], 0.5);
    }

    #[test]
    fn relevant_surface_identity_wins_over_a_sampled_negative_provenance_root() {
        let scenario = scenario(ScenarioPattern::CorrectionChains);
        let mut trace = trace(ScenarioPattern::CorrectionChains);
        replace_items(
            &mut trace,
            vec![RetrievedItem {
                kind: ObjectType::DerivedMemory,
                internal_id: "replacement".to_string(),
                external_id: Some("relevant".to_string()),
                episode_external_id: Some("sampled-negative".to_string()),
                score: None,
                rank: 1,
                rationale: Vec::new(),
                text: None,
            }],
        );
        mutate_trace(&mut trace, |telemetry| {
            *telemetry = None;
        });
        let mut out = Map::new();

        insert_continuity_metrics(
            &mut out,
            &scenario,
            &trace,
            &MetricsConfig::default(),
            RetrievalMode::Hybrid,
        );

        assert_eq!(out["sampled_context_pollution_rate"], 0.0);
        assert_eq!(out["sampled_event_pollution_rate"], 0.0);
    }
}
