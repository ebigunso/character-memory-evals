use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cmem_eval_core::{ControllableSimilarityFixture, SimilarityConceptFixture};

use crate::{
    CONTINUITY_FIXTURE_SCHEMA_VERSION, ContinuityEntityKind, ContinuityFixtureSet,
    ContinuityScenario, EntityDeclaration, ExpectedRelevance, InteractionEvent, ScenarioPattern,
    ThreadMembership,
};

pub const CHECKED_FIXTURE_SEED: u64 = 0x0000_0000_0135_2768;
const EMBEDDING_VECTOR_SIZE: usize = 8;

pub fn generate_fixture_set(seed: u64) -> Result<ContinuityFixtureSet> {
    Ok(ContinuityFixtureSet {
        schema_version: CONTINUITY_FIXTURE_SCHEMA_VERSION,
        seed,
        scenarios: vec![
            long_gap_recall(seed)?,
            recurring_hub_entity(seed)?,
            selective_entity(seed)?,
            correction_chains(seed)?,
            thread_drift(seed)?,
            temporal_structure(seed)?,
            mixed_salience_accumulation(seed)?,
            cross_store_stress(seed)?,
        ],
    })
}

fn long_gap_recall(seed: u64) -> Result<ContinuityScenario> {
    let id = "long-gap-recall";
    let target = "A dormant project uses the cobalt protocol.";
    let distractor = "A recent project uses the amber protocol.";
    let query_text = "Which protocol belongs to the dormant project?";
    scenario(
        seed,
        id,
        ScenarioPattern::LongGapRecall,
        standard_entities(false),
        vec![
            remember(
                1,
                "memory-dormant",
                "2025-01-05T10:00:00Z",
                target,
                vec!["entity-person"],
                None,
                0.8,
            )?,
            remember(
                2,
                "memory-recent",
                "2025-06-05T10:00:00Z",
                distractor,
                vec!["entity-organization"],
                None,
                0.6,
            )?,
            query(
                3,
                "query-long-gap",
                "2025-12-20T10:00:00Z",
                query_text,
                vec!["memory-dormant"],
                vec!["memory-recent"],
            )?,
        ],
        concepts([
            ("dormant", "target", vec![target, query_text]),
            ("recent", "distractor", vec![distractor]),
        ]),
    )
}

fn recurring_hub_entity(seed: u64) -> Result<ContinuityScenario> {
    let id = "recurring-hub-entity";
    let entities = vec![
        entity("entity-person", ContinuityEntityKind::Person, "Hub A", true),
        entity(
            "entity-organization",
            ContinuityEntityKind::Organization,
            "Hub B",
            true,
        ),
        entity(
            "entity-location",
            ContinuityEntityKind::Location,
            "Hub C",
            true,
        ),
    ];
    let mut events = Vec::new();
    let mut hub_inputs = Vec::new();
    for index in 0..6 {
        let text = format!("Hub incident {index} connects all declared entity kinds.");
        hub_inputs.push(text.clone());
        events.push(remember(
            index + 1,
            &format!("hub-memory-{index}"),
            &format!("2025-{:02}-10T10:00:00Z", index + 1),
            &text,
            vec!["entity-person", "entity-organization", "entity-location"],
            None,
            0.5,
        )?);
    }
    let query_text = "Which incident most recently connected every hub?";
    hub_inputs.push(query_text.to_string());
    events.push(query(
        7,
        "query-hub",
        "2025-07-10T10:00:00Z",
        query_text,
        vec!["hub-memory-5"],
        vec!["hub-memory-0"],
    )?);
    scenario(
        seed,
        id,
        ScenarioPattern::RecurringHubEntity,
        entities,
        events,
        concepts([(
            "hub",
            "hub",
            hub_inputs.iter().map(String::as_str).collect(),
        )]),
    )
}

fn selective_entity(seed: u64) -> Result<ContinuityScenario> {
    let id = "selective-entity";
    let rare = "A rare location carries the violet access token.";
    let common = "A frequent organization carries routine status updates.";
    let query_text = "Which access token belongs to the rare location?";
    scenario(
        seed,
        id,
        ScenarioPattern::SelectiveEntity,
        standard_entities(false),
        vec![
            remember(
                1,
                "common-memory",
                "2025-01-01T09:00:00Z",
                common,
                vec!["entity-organization"],
                None,
                0.2,
            )?,
            remember(
                2,
                "rare-memory",
                "2025-05-01T09:00:00Z",
                rare,
                vec!["entity-location"],
                None,
                0.95,
            )?,
            query(
                3,
                "query-selective",
                "2025-09-01T09:00:00Z",
                query_text,
                vec!["rare-memory"],
                vec!["common-memory"],
            )?,
        ],
        concepts([
            ("rare", "target", vec![rare, query_text]),
            ("common", "distractor", vec![common]),
        ]),
    )
}

fn correction_chains(seed: u64) -> Result<ContinuityScenario> {
    let id = "correction-chains";
    let original = "The delivery window is Monday.";
    let first = "The delivery window is Tuesday.";
    let final_text = "The delivery window is Thursday.";
    let query_text = "What is the corrected delivery window?";
    scenario(
        seed,
        id,
        ScenarioPattern::CorrectionChains,
        standard_entities(false),
        vec![
            remember(
                1,
                "delivery-v1",
                "2025-01-01T08:00:00Z",
                original,
                vec!["entity-organization"],
                None,
                0.7,
            )?,
            correct(
                2,
                "delivery-v1",
                "delivery-v2",
                "2025-02-01T08:00:00Z",
                first,
            )?,
            correct(
                3,
                "delivery-v2",
                "delivery-v3",
                "2025-03-01T08:00:00Z",
                final_text,
            )?,
            forget(4, "delivery-v1", "2025-03-02T08:00:00Z")?,
            query(
                5,
                "query-correction",
                "2025-04-01T08:00:00Z",
                query_text,
                vec!["delivery-v3"],
                vec!["delivery-v1", "delivery-v2"],
            )?,
        ],
        concepts([
            ("old", "superseded", vec![original, first]),
            ("current", "current", vec![final_text, query_text]),
        ]),
    )
}

fn thread_drift(seed: u64) -> Result<ContinuityScenario> {
    let id = "thread-drift";
    let texts = [
        "Thread starts with a focused migration.",
        "Thread broadens to deployment details.",
        "Thread drifts into unrelated travel planning.",
    ];
    let query_text = "What was the thread's original focus?";
    scenario(
        seed,
        id,
        ScenarioPattern::ThreadDrift,
        standard_entities(false),
        vec![
            remember(
                1,
                "thread-focus",
                "2025-01-01T08:00:00Z",
                texts[0],
                vec!["entity-organization"],
                Some(("thread-1", 0.95)),
                0.8,
            )?,
            remember(
                2,
                "thread-broader",
                "2025-02-01T08:00:00Z",
                texts[1],
                vec!["entity-organization"],
                Some(("thread-1", 0.65)),
                0.5,
            )?,
            remember(
                3,
                "thread-drifted",
                "2025-03-01T08:00:00Z",
                texts[2],
                vec!["entity-location"],
                Some(("thread-1", 0.25)),
                0.2,
            )?,
            query(
                4,
                "query-thread",
                "2025-04-01T08:00:00Z",
                query_text,
                vec!["thread-focus"],
                vec!["thread-drifted"],
            )?,
        ],
        concepts([
            ("focus", "target", vec![texts[0], query_text]),
            ("drift", "distractor", vec![texts[1], texts[2]]),
        ]),
    )
}

fn temporal_structure(seed: u64) -> Result<ContinuityScenario> {
    let id = "temporal-structure";
    let old = "The archive was stored in the west wing in January.";
    let current = "The archive moved to the east wing in October.";
    let query_text = "Where was the archive after the October move?";
    scenario(
        seed,
        id,
        ScenarioPattern::TemporalStructure,
        standard_entities(false),
        vec![
            remember(
                1,
                "archive-january",
                "2025-01-15T12:00:00Z",
                old,
                vec!["entity-location"],
                None,
                0.6,
            )?,
            remember(
                2,
                "archive-october",
                "2025-10-15T12:00:00Z",
                current,
                vec!["entity-location"],
                None,
                0.8,
            )?,
            query(
                3,
                "query-temporal",
                "2025-11-15T12:00:00Z",
                query_text,
                vec!["archive-october"],
                vec!["archive-january"],
            )?,
        ],
        concepts([
            ("old", "past", vec![old]),
            ("current", "current", vec![current, query_text]),
        ]),
    )
}

fn mixed_salience_accumulation(seed: u64) -> Result<ContinuityScenario> {
    let id = "mixed-salience-accumulation";
    let low = "A low-salience routine note was recorded.";
    let medium = "A medium-salience schedule change was recorded.";
    let high = "A high-salience emergency access code is delta-nine.";
    let query_text = "What is the emergency access code?";
    scenario(
        seed,
        id,
        ScenarioPattern::MixedSalienceAccumulation,
        standard_entities(false),
        vec![
            remember(
                1,
                "salience-low",
                "2025-01-01T07:00:00Z",
                low,
                vec!["entity-person"],
                None,
                0.1,
            )?,
            remember(
                2,
                "salience-medium",
                "2025-02-01T07:00:00Z",
                medium,
                vec!["entity-organization"],
                None,
                0.5,
            )?,
            remember(
                3,
                "salience-high",
                "2025-03-01T07:00:00Z",
                high,
                vec!["entity-location"],
                None,
                0.95,
            )?,
            query(
                4,
                "query-salience",
                "2025-08-01T07:00:00Z",
                query_text,
                vec!["salience-high"],
                vec!["salience-low", "salience-medium"],
            )?,
        ],
        concepts([
            ("signal", "target", vec![high, query_text]),
            ("routine", "distractor", vec![low, medium]),
        ]),
    )
}

fn cross_store_stress(seed: u64) -> Result<ContinuityScenario> {
    let id = "cross-store-stress";
    let text = "The persistent cross-store marker is quartz-seven.";
    let query_text = "What marker must survive the restart?";
    scenario(
        seed,
        id,
        ScenarioPattern::CrossStoreStress,
        standard_entities(false),
        vec![
            remember(
                1,
                "restart-marker",
                "2025-01-01T06:00:00Z",
                text,
                vec!["entity-person", "entity-organization"],
                None,
                0.9,
            )?,
            link(
                2,
                "restart-link",
                "2025-01-01T06:05:00Z",
                "entity-person",
                "restart-marker",
            )?,
            InteractionEvent::Restart {
                event_id: "event-003".into(),
                timestamp: timestamp("2025-01-01T06:10:00Z")?,
                reopen_graph: true,
                reopen_stats: true,
            },
            query(
                4,
                "query-restart",
                "2025-01-01T06:15:00Z",
                query_text,
                vec!["restart-marker"],
                vec!["restart-link"],
            )?,
        ],
        concepts([("marker", "target", vec![text, query_text])]),
    )
}

fn scenario(
    seed: u64,
    id: &str,
    pattern: ScenarioPattern,
    mut entities: Vec<EntityDeclaration>,
    events: Vec<InteractionEvent>,
    mut concepts: BTreeMap<String, SimilarityConceptFixture>,
) -> Result<ContinuityScenario> {
    entities.sort_by(|left, right| left.external_id.cmp(&right.external_id));
    assign_entity_embedding_inputs(id, &entities, &events, &mut concepts)?;
    let clusters = concepts
        .values()
        .map(|concept| concept.cluster.clone())
        .collect::<BTreeSet<_>>();
    let cluster_count = clusters.len();
    if cluster_count > EMBEDDING_VECTOR_SIZE {
        bail!(
            "continuity scenario `{id}` declares {cluster_count} embedding clusters, exceeding configured vector_size {EMBEDDING_VECTOR_SIZE}"
        );
    }
    let mut cluster_vectors = BTreeMap::new();
    for (index, cluster) in clusters.into_iter().enumerate() {
        let mut vector = vec![0.0; EMBEDDING_VECTOR_SIZE];
        vector[index] = 1.0;
        cluster_vectors.insert(cluster, vector);
    }
    Ok(ContinuityScenario {
        fixture_id: id.to_string(),
        namespace: format!("continuity-{id}-{seed:016x}"),
        collection_name: format!("cmem_continuity_{}_{seed:016x}", id.replace('-', "_")),
        pattern,
        entities,
        embedding: ControllableSimilarityFixture {
            seed,
            vector_size: EMBEDDING_VECTOR_SIZE,
            noise_magnitude: 1.0 / 1024.0,
            clusters: cluster_vectors,
            concepts,
        },
        events,
    })
}

fn assign_entity_embedding_inputs(
    scenario_id: &str,
    entities: &[EntityDeclaration],
    events: &[InteractionEvent],
    concepts: &mut BTreeMap<String, SimilarityConceptFixture>,
) -> Result<()> {
    let mut background_labels = Vec::new();
    for entity in entities {
        let first_referencing_text = events.iter().find_map(|event| match event {
            InteractionEvent::Remember {
                text,
                entity_external_ids,
                ..
            } if entity_external_ids.contains(&entity.external_id) => Some(text),
            _ => None,
        });
        if let Some(text) = first_referencing_text {
            let Some(concept) = concepts
                .values_mut()
                .find(|concept| concept.inputs.contains(text))
            else {
                bail!(
                    "continuity scenario {scenario_id:?} Remember text {text:?} is missing from embedding concepts"
                );
            };
            concept.inputs.push(entity.label.clone());
        } else {
            background_labels.push(entity.label.clone());
        }
    }
    if !background_labels.is_empty() {
        if concepts.contains_key("entity_background") {
            bail!(
                "continuity scenario {scenario_id:?} collides with reserved embedding concept ID \"entity_background\""
            );
        }
        concepts.insert(
            "entity_background".to_string(),
            SimilarityConceptFixture {
                cluster: "entity_background".to_string(),
                inputs: background_labels,
            },
        );
    }
    Ok(())
}

fn concepts<const N: usize>(
    values: [(&str, &str, Vec<&str>); N],
) -> BTreeMap<String, SimilarityConceptFixture> {
    values
        .into_iter()
        .map(|(id, cluster, inputs)| {
            (
                id.to_string(),
                SimilarityConceptFixture {
                    cluster: cluster.to_string(),
                    inputs: inputs.into_iter().map(str::to_string).collect(),
                },
            )
        })
        .collect()
}

fn standard_entities(hubs: bool) -> Vec<EntityDeclaration> {
    vec![
        entity(
            "entity-person",
            ContinuityEntityKind::Person,
            "Entity A",
            hubs,
        ),
        entity(
            "entity-organization",
            ContinuityEntityKind::Organization,
            "Entity B",
            hubs,
        ),
        entity(
            "entity-location",
            ContinuityEntityKind::Location,
            "Entity C",
            hubs,
        ),
    ]
}

fn entity(
    external_id: &str,
    entity_type: ContinuityEntityKind,
    label: &str,
    is_hub: bool,
) -> EntityDeclaration {
    EntityDeclaration {
        external_id: external_id.into(),
        entity_type,
        label: label.into(),
        is_hub,
    }
}

fn remember(
    number: usize,
    external_id: &str,
    at: &str,
    text: &str,
    entity_ids: Vec<&str>,
    thread: Option<(&str, f32)>,
    salience: f32,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Remember {
        event_id: format!("event-{number:03}"),
        external_id: external_id.into(),
        timestamp: timestamp(at)?,
        text: text.into(),
        entity_external_ids: entity_ids.into_iter().map(str::to_string).collect(),
        thread: thread.map(|(id, confidence)| ThreadMembership {
            thread_external_id: id.into(),
            confidence,
        }),
        salience,
    })
}

fn correct(
    number: usize,
    target: &str,
    replacement: &str,
    at: &str,
    text: &str,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Correct {
        event_id: format!("event-{number:03}"),
        target_external_id: target.into(),
        replacement_external_id: replacement.into(),
        timestamp: timestamp(at)?,
        replacement_text: text.into(),
    })
}

fn forget(number: usize, target: &str, at: &str) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Forget {
        event_id: format!("event-{number:03}"),
        target_external_id: target.into(),
        timestamp: timestamp(at)?,
    })
}

fn link(
    number: usize,
    external_id: &str,
    at: &str,
    from: &str,
    to: &str,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Link {
        event_id: format!("event-{number:03}"),
        external_id: external_id.into(),
        timestamp: timestamp(at)?,
        from_external_id: from.into(),
        relation: "about".into(),
        to_external_id: to.into(),
    })
}

fn query(
    number: usize,
    query_id: &str,
    at: &str,
    text: &str,
    relevant: Vec<&str>,
    irrelevant: Vec<&str>,
) -> Result<InteractionEvent> {
    Ok(InteractionEvent::Query {
        event_id: format!("event-{number:03}"),
        query_id: query_id.into(),
        timestamp: timestamp(at)?,
        text: text.into(),
        expected: ExpectedRelevance {
            relevant_external_ids: relevant.into_iter().map(str::to_string).collect(),
            irrelevant_external_ids: irrelevant.into_iter().map(str::to_string).collect(),
        },
    })
}

fn timestamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("parse continuity fixture timestamp {value:?}"))
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process::Command};

    use super::*;
    use crate::{canonical_fixture_bytes, parse_fixture_bytes, scenario_patterns};

    const PROCESS_PROBE_PATH: &str = "CMEM_CONTINUITY_FIXTURE_PROBE_PATH";
    const CHECKED_FIXTURE: &[u8] = include_bytes!("../fixtures/continuity_v2.json");

    #[test]
    fn checked_fixture_is_canonical_and_covers_every_scenario_pattern() {
        let generated = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let bytes = canonical_fixture_bytes(&generated).unwrap();
        assert_eq!(bytes, CHECKED_FIXTURE);
        assert_eq!(parse_fixture_bytes(CHECKED_FIXTURE).unwrap(), generated);

        let patterns = scenario_patterns(&generated);
        assert_eq!(patterns.len(), 8);
        assert!(patterns.values().all(|count| *count == 1));
        assert_eq!(
            generated
                .scenarios
                .iter()
                .map(|scenario| scenario.fixture_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "long-gap-recall",
                "recurring-hub-entity",
                "selective-entity",
                "correction-chains",
                "thread-drift",
                "temporal-structure",
                "mixed-salience-accumulation",
                "cross-store-stress",
            ]
        );
    }

    #[test]
    fn scenario_rejects_more_clusters_than_the_declared_vector_size() {
        let concepts = (0..=EMBEDDING_VECTOR_SIZE)
            .map(|index| {
                (
                    format!("concept-{index}"),
                    SimilarityConceptFixture {
                        cluster: format!("cluster-{index}"),
                        inputs: Vec::new(),
                    },
                )
            })
            .collect();

        let error = super::scenario(
            CHECKED_FIXTURE_SEED,
            "too-many-clusters",
            ScenarioPattern::LongGapRecall,
            Vec::new(),
            Vec::new(),
            concepts,
        )
        .unwrap_err()
        .to_string();

        assert_eq!(
            error,
            "continuity scenario `too-many-clusters` declares 9 embedding clusters, exceeding configured vector_size 8"
        );
    }

    #[test]
    fn scenario_reports_missing_remember_embedding_input_without_panicking() {
        let scenario_id = "extension-missing-concept";
        let remembered_text = "A newly extended Remember event.";
        let events = vec![
            remember(
                1,
                "memory-new",
                "2025-01-01T00:00:00Z",
                remembered_text,
                vec!["entity-new"],
                None,
                0.5,
            )
            .unwrap(),
        ];
        let error = super::scenario(
            CHECKED_FIXTURE_SEED,
            scenario_id,
            ScenarioPattern::LongGapRecall,
            vec![entity(
                "entity-new",
                ContinuityEntityKind::Person,
                "New Entity",
                false,
            )],
            events,
            BTreeMap::new(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains(scenario_id), "{error}");
        assert!(error.contains(remembered_text), "{error}");
        assert!(error.contains("missing from embedding concepts"), "{error}");
    }

    #[test]
    fn scenario_reports_reserved_background_concept_collision_without_panicking() {
        let scenario_id = "extension-reserved-concept";
        let error = super::scenario(
            CHECKED_FIXTURE_SEED,
            scenario_id,
            ScenarioPattern::LongGapRecall,
            vec![entity(
                "entity-new",
                ContinuityEntityKind::Person,
                "New Entity",
                false,
            )],
            Vec::new(),
            concepts([("entity_background", "custom", vec!["custom input"])]),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains(scenario_id), "{error}");
        assert!(error.contains("entity_background"), "{error}");
        assert!(error.contains("reserved"), "{error}");
    }

    #[test]
    fn invalid_extension_timestamp_returns_contextual_error() {
        let error = timestamp("not-a-timestamp").unwrap_err().to_string();
        assert!(error.contains("continuity fixture timestamp"), "{error}");
        assert!(error.contains("not-a-timestamp"), "{error}");
    }

    #[test]
    fn same_seed_is_byte_identical_across_two_process_runs() {
        let current_exe = env::current_exe().unwrap();
        let mut outputs = Vec::new();
        for run in 0..2 {
            let output_path = env::temp_dir().join(format!(
                "cmem-continuity-fixtures-{}-{run}.json",
                std::process::id()
            ));
            let status = Command::new(&current_exe)
                .args([
                    "--exact",
                    "generator::tests::cross_process_fixture_probe",
                    "--nocapture",
                ])
                .env(PROCESS_PROBE_PATH, &output_path)
                .status()
                .unwrap();
            assert!(status.success());
            outputs.push(fs::read(&output_path).unwrap());
            fs::remove_file(output_path).unwrap();
        }
        assert_eq!(outputs[0], outputs[1]);
        assert_eq!(outputs[0], CHECKED_FIXTURE);
    }

    #[test]
    fn cross_process_fixture_probe() {
        let Ok(output_path) = env::var(PROCESS_PROBE_PATH) else {
            return;
        };
        let bytes =
            canonical_fixture_bytes(&generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap()).unwrap();
        fs::write(output_path, bytes).unwrap();
    }

    #[test]
    fn long_gap_queries_and_pollution_labels_are_explicit() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let long_gap = scenario(&fixtures, ScenarioPattern::LongGapRecall);
        let first_write = long_gap.events.first().unwrap().timestamp();
        let query_time = long_gap.events.last().unwrap().timestamp();
        assert!((query_time - first_write).num_days() >= 180);

        for scenario in &fixtures.scenarios {
            for event in &scenario.events {
                if let InteractionEvent::Query { expected, .. } = event {
                    assert!(!expected.relevant_external_ids.is_empty());
                    assert!(!expected.irrelevant_external_ids.is_empty());
                }
            }
        }
    }

    #[test]
    fn public_parser_requires_every_entity_label_embedding_assignment() {
        let mut fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let scenario = &mut fixtures.scenarios[0];
        let entity_label = scenario
            .entities
            .iter()
            .find(|entity| entity.external_id == "entity-person")
            .unwrap()
            .label
            .clone();
        for concept in scenario.embedding.concepts.values_mut() {
            concept.inputs.retain(|input| input != &entity_label);
        }
        let bytes = serde_json::to_vec(&fixtures).unwrap();

        let error = parse_fixture_bytes(&bytes).unwrap_err().to_string();
        assert!(error.contains("entity label"), "{error}");
        assert!(error.contains(&entity_label), "{error}");
    }

    #[test]
    fn recurring_hubs_span_three_entity_kinds_at_high_degree() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let hub = scenario(&fixtures, ScenarioPattern::RecurringHubEntity);
        let hubs = hub
            .entities
            .iter()
            .filter(|entity| entity.is_hub)
            .collect::<Vec<_>>();
        assert_eq!(
            hubs.iter()
                .map(|entity| entity.entity_type.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        for entity in hubs {
            let degree = hub
                .events
                .iter()
                .filter(|event| match event {
                    InteractionEvent::Remember {
                        entity_external_ids,
                        ..
                    } => entity_external_ids.contains(&entity.external_id),
                    _ => false,
                })
                .count();
            assert!(degree >= 5, "{} degree was {degree}", entity.external_id);
        }
    }

    #[test]
    fn scripted_patterns_include_required_lifecycle_and_structure() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let corrections = scenario(&fixtures, ScenarioPattern::CorrectionChains);
        assert_eq!(
            corrections
                .events
                .iter()
                .filter(|event| matches!(event, InteractionEvent::Correct { .. }))
                .count(),
            2
        );
        assert!(
            corrections
                .events
                .iter()
                .any(|event| matches!(event, InteractionEvent::Forget { .. }))
        );

        let drift = scenario(&fixtures, ScenarioPattern::ThreadDrift);
        let confidences = drift
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember {
                    thread: Some(thread),
                    ..
                } => Some(thread.confidence),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(confidences.windows(2).all(|pair| pair[0] > pair[1]));

        let salience = scenario(&fixtures, ScenarioPattern::MixedSalienceAccumulation);
        let values = salience
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Remember { salience, .. } => Some(*salience),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(values.iter().any(|value| *value < 0.2));
        assert!(values.iter().any(|value| *value > 0.9));

        let restart = scenario(&fixtures, ScenarioPattern::CrossStoreStress);
        let restart_index = restart
            .events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    InteractionEvent::Restart {
                        reopen_graph: true,
                        reopen_stats: true,
                        ..
                    }
                )
            })
            .unwrap();
        assert!(
            restart.events[..restart_index]
                .iter()
                .any(|event| matches!(event, InteractionEvent::Remember { .. }))
        );
        assert!(
            restart.events[restart_index + 1..]
                .iter()
                .any(|event| matches!(event, InteractionEvent::Query { .. }))
        );
    }

    #[test]
    fn schema_is_role_free_and_all_embedding_inputs_are_fixture_assigned() {
        let fixtures = generate_fixture_set(CHECKED_FIXTURE_SEED).unwrap();
        let serialized = String::from_utf8(canonical_fixture_bytes(&fixtures).unwrap()).unwrap();
        assert!(!serialized.contains("\"role\""));
        for scenario in &fixtures.scenarios {
            let provider = cmem_eval_core::ControllableSimilarityEmbeddingProvider::new(
                scenario.embedding.clone(),
            )
            .unwrap();
            for event in &scenario.events {
                let text = match event {
                    InteractionEvent::Remember { text, .. }
                    | InteractionEvent::Query { text, .. } => Some(text),
                    InteractionEvent::Correct {
                        replacement_text, ..
                    } => Some(replacement_text),
                    _ => None,
                };
                if let Some(text) = text {
                    provider.vector_for_text(text).unwrap();
                }
            }
            for entity in &scenario.entities {
                provider.vector_for_text(&entity.label).unwrap();
            }
        }
    }

    fn scenario(fixtures: &ContinuityFixtureSet, pattern: ScenarioPattern) -> &ContinuityScenario {
        fixtures
            .scenarios
            .iter()
            .find(|scenario| scenario.pattern == pattern)
            .unwrap()
    }
}
