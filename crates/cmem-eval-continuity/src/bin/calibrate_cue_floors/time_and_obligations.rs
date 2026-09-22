//! Fixed inputs for before/after time and prospective-memory measurements.
use super::*;
use cmem_eval::{
    CandidateValidationStatus, CommitWriteOptions, ControllableSimilarityEmbeddingProvider,
    DerivedMemoryInput, DerivedType, EntityInput, EpisodeInput, GraphEnrichmentInput,
    MemoryEndpointInput, MemoryLinkInput, ObjectType, ObservationInput, PrepareWriteInput,
    RelationType,
};

const EVENING: &str = "2025-09-09T20:00:00+09:00";
const TOPIC: &str = "Repairing a copper bell";
const DISTINCT: &str = "The disconnected observation records a violet ribbon.";

#[derive(Clone, Serialize)]
struct Experience {
    write: PrepareWriteInput,
    created_at: Option<String>,
    // This one observation is written separately, without a MemoryLink.
    unlinked_observation: Option<String>,
}

#[derive(Clone, Serialize)]
struct TimeRange {
    start: String,
    end: String,
}

#[derive(Clone, Serialize)]
struct PlannedProbe {
    name: String,
    supported_input: RetrieveInput,
    recency_floor: Option<usize>,
    time_range: Option<TimeRange>,
    required_routes: Vec<String>,
}

#[derive(Clone, Serialize)]
struct Obligation {
    label: String,
    memory_external_id: String,
    // Planned assertion subjects, not inferred labels and not current native fields.
    actor_subjects: Vec<String>,
    counterpart_subjects: Vec<String>,
    due_instant: Option<String>,
}

#[derive(Clone, Serialize)]
struct Family {
    name: String,
    namespace: String,
    embedding: ControllableSimilarityFixture,
    experiences: Vec<Experience>,
    graph: GraphEnrichmentInput,
    obligations: Vec<Obligation>,
    character_entity: Option<String>,
    probes: Vec<PlannedProbe>,
    topic_targets: Vec<String>,
}

fn timestamp(value: &str) -> Result<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).context("authored fixed timestamp")
}

fn input(
    config: &BenchmarkRunConfig,
    namespace: &str,
    person: bool,
    topic: Option<&str>,
) -> RetrieveInput {
    RetrieveInput {
        mode: config.retrieval.mode,
        namespace: namespace.into(),
        topic: topic.map(str::to_owned),
        scene: MemorySceneInput {
            time: Some(EVENING.into()),
            participants: if person {
                vec![SceneParticipantInput {
                    key: Some("iris".into()),
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            ..Default::default()
        },
        activity: None,
        cue_floors: None,
        surface_policy: config.retrieval.surface_policy.clone(),
    }
}

fn probe(name: &str, input: RetrieveInput, required: &[&str]) -> PlannedProbe {
    PlannedProbe {
        name: name.into(),
        supported_input: input,
        recency_floor: None,
        time_range: None,
        required_routes: required.iter().map(|s| (*s).into()).collect(),
    }
}

fn empty(name: &str) -> Family {
    let namespace = format!("cue-floor-{name}-{CHECKED_FIXTURE_SEED:016x}");
    let mut embedding = ControllableSimilarityFixture {
        seed: CHECKED_FIXTURE_SEED,
        vector_size: 9,
        noise_magnitude: 0.000001,
        clusters: BTreeMap::new(),
        concepts: BTreeMap::new(),
    };
    let mut topic = vec![0.0; 9];
    topic[0] = 1.0;
    assign(&mut embedding, TOPIC, topic);
    Family {
        name: name.into(),
        namespace: namespace.clone(),
        embedding,
        experiences: vec![],
        graph: GraphEnrichmentInput {
            namespace,
            entities: vec![
                EntityInput {
                    external_id: "iris".into(),
                },
                EntityInput {
                    external_id: "self".into(),
                },
            ],
            ..Default::default()
        },
        obligations: vec![],
        character_entity: None,
        probes: vec![],
        topic_targets: vec![],
    }
}

fn experience(
    family: &mut Family,
    label: &str,
    at: &str,
    person: bool,
    salience: f32,
    cosine: f32,
) {
    let content = format!("{} experience {label}", family.name);
    let mut vector = vec![0.0; 9];
    vector[0] = cosine;
    vector[8] = (1.0 - cosine * cosine).sqrt();
    assign(&mut family.embedding, &content, vector);
    family.experiences.push(Experience {
        write: PrepareWriteInput {
            namespace: family.namespace.clone(),
            content,
            episode_external_id: label.into(),
            observation_external_id: format!("{label}:observation"),
            scene: MemorySceneInput {
                time: Some(at.into()),
                participants: if person {
                    vec![SceneParticipantInput {
                        key: Some("iris".into()),
                        ..Default::default()
                    }]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            speaker_entity_external_id: None,
            salience: Some(salience),
            observation_observed_at: Some(at.into()),
            raw_refs: vec![],
            include_vector_index_candidates: true,
            include_stats_update_candidates: true,
        },
        created_at: None,
        unlinked_observation: None,
    });
}

fn topic_pressure(family: &mut Family) {
    // These precede the latest non-topical experience: a topic target cannot make recency look correct.
    for n in 0..8 {
        let id = format!("strong-topic-{n:02}");
        let at =
            (timestamp("2025-09-03T12:00:00+09:00").unwrap() + Duration::minutes(n)).to_rfc3339();
        experience(family, &id, &at, false, 0.5, 0.9 - n as f32 * 0.3 / 7.0);
        family.topic_targets.push(id);
    }
}

fn time_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("time");
    for (index, (label, at)) in [
        ("earlier-anniversary", "2023-09-09T13:00:00+09:00"),
        ("last-year-other-day", "2024-09-08T13:00:00+09:00"),
        ("last-year-anniversary", "2024-09-09T13:00:00+09:00"),
        ("last-month-salient", "2025-08-09T13:00:00+09:00"),
        ("last-week-monday", "2025-09-01T13:00:00+09:00"),
        ("last-tuesday-morning", "2025-09-02T09:00:00+09:00"),
        ("last-tuesday-noon", "2025-09-02T12:00:00+09:00"),
        ("last-tuesday-evening", "2025-09-02T18:00:00+09:00"),
        ("yesterday-morning", "2025-09-08T09:00:00+09:00"),
        ("yesterday-evening", "2025-09-08T18:00:00+09:00"),
        ("today-morning", "2025-09-09T09:00:00+09:00"),
        ("today-noon", "2025-09-09T12:00:00+09:00"),
        (
            "today-latest-unlinked",
            "2025-09-09T19:00:00.123456789+09:00",
        ),
        ("future", "2025-09-10T09:00:00+09:00"),
    ]
    .into_iter()
    .enumerate()
    {
        experience(
            &mut family,
            label,
            at,
            index % 2 == 0 && index != 12,
            if label == "last-month-salient" {
                1.0
            } else {
                0.5
            },
            -0.1,
        );
    }
    family.experiences[12].unlinked_observation = Some(DISTINCT.into());
    let mut distinct = vec![0.0; 9];
    distinct[7] = 1.0;
    assign(&mut family.embedding, DISTINCT, distinct);
    for (label, at) in [
        ("last-tuesday-afternoon", "2025-09-02T15:00:00+09:00"),
        ("last-tuesday-late", "2025-09-02T17:00:00+09:00"),
        ("last-month-other-day", "2025-08-08T12:00:00+09:00"),
        ("yesterday-noon", "2025-09-08T12:00:00+09:00"),
    ] {
        experience(&mut family, label, at, false, 0.5, -0.1);
    }
    topic_pressure(&mut family);
    family.probes.push(probe(
        "evening-time-only",
        input(config, &family.namespace, false, None),
        &[],
    ));
    for floor in [0, 1] {
        let mut p = probe(
            &format!("evening-loud-topic-recency-{floor}"),
            input(config, &family.namespace, false, Some(TOPIC)),
            &["recency_floor"],
        );
        p.recency_floor = Some(floor);
        family.probes.push(p);
    }
    for with_topic in [false, true] {
        let mut p = probe(
            if with_topic {
                "last-tuesday-with-topic"
            } else {
                "last-tuesday-no-topic"
            },
            input(
                config,
                &family.namespace,
                false,
                with_topic.then_some(TOPIC),
            ),
            &["time_range", "date_match"],
        );
        p.time_range = Some(TimeRange {
            start: "2025-09-02T00:00:00+09:00".into(),
            end: "2025-09-02T23:59:59.999999999+09:00".into(),
        });
        family.probes.push(p);
    }
    for person in [false, true] {
        family.probes.push(probe(
            if person {
                "anniversary-shared-person"
            } else {
                "anniversary-person-absent"
            },
            input(config, &family.namespace, person, Some(TOPIC)),
            &["date_match"],
        ));
    }
    let sources = [
        "last-tuesday-morning",
        "last-tuesday-noon",
        "last-tuesday-evening",
    ];
    for (n, source) in sources.iter().enumerate() {
        family
            .experiences
            .iter_mut()
            .find(|e| e.write.episode_external_id == *source)
            .unwrap()
            .created_at = Some(format!("2025-09-04T1{}:00:00+09:00", 3 - n));
    }
    let mut activity = derived(
        "activity-open-loop",
        "2025-09-05T12:00:00+09:00",
        "Review the three visits from Tuesday.".into(),
        DerivedType::OpenLoop,
        vec![],
        0.5,
    );
    activity.source_episode_external_ids = sources.into_iter().map(str::to_owned).collect();
    activity.given_by_application = false;
    let mut vector = vec![0.0; 9];
    vector[8] = 1.0;
    assign(&mut family.embedding, &activity.text, vector);
    family.graph.derived_memories.push(activity);
    let mut activity_input = input(config, &family.namespace, false, None);
    activity_input.activity = Some(ActivityInput::OpenLoop("activity-open-loop".into()));
    activity_input.surface_policy.max_graph_roots = Some(2);
    activity_input.surface_policy.sections.relevant_episodes = 1;
    family
        .probes
        .push(probe("activity-source-order", activity_input, &[]));
    family
}

fn derived(
    id: &str,
    at: &str,
    text: String,
    kind: DerivedType,
    subjects: Vec<String>,
    salience: f32,
) -> DerivedMemoryInput {
    DerivedMemoryInput {
        external_id: id.into(),
        created_at: Some(at.into()),
        derived_type: kind,
        text,
        source_episode_external_ids: vec![],
        source_observation_external_ids: vec![],
        thread_external_ids: vec![],
        entity_external_ids: subjects,
        assertions: vec![],
        given_by_application: true,
        salience_score: salience,
        supersedes_external_ids: vec![],
        metadata: Value::Null,
    }
}

fn obligations_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("obligations");
    family.character_entity = Some("self".into());
    topic_pressure(&mut family);
    // More current beliefs than the 20-item about-entity fanout; a low-salience obligation can be cut.
    for n in 0..24 {
        family.graph.derived_memories.push(derived(
            &format!("belief-{n:02}"),
            &format!("2025-08-03T12:{n:02}:00+09:00"),
            format!("Iris background belief {n}"),
            DerivedType::Reflection,
            vec!["iris".into()],
            0.9 + n as f32 * 0.001,
        ));
    }
    for (index, (label, actor, counterpart, due)) in [
        (
            "self-owes-overdue",
            Some("self"),
            Some("iris"),
            Some("2025-09-08T12:00:00+09:00"),
        ),
        (
            "iris-owes-overdue",
            Some("iris"),
            Some("self"),
            Some("2025-09-08T17:00:00+09:00"),
        ),
        (
            "self-owes-today",
            Some("self"),
            Some("iris"),
            Some("2025-09-09T22:00:00+09:00"),
        ),
        (
            "iris-owes-future",
            Some("iris"),
            Some("self"),
            Some("2025-09-10T09:00:00+09:00"),
        ),
        (
            "settled-self-owes",
            Some("self"),
            Some("iris"),
            Some("2025-09-08T12:00:00+09:00"),
        ),
        ("settled-iris-owes", Some("iris"), Some("self"), None),
        ("own-undated-promise", Some("self"), None, None),
        (
            "own-due-promise",
            Some("self"),
            None,
            Some("2025-09-09T12:00:00+09:00"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let text = match label {
            "self-owes-overdue" => "I promised Iris I would return the blue notebook.",
            "iris-owes-overdue" => "Iris promised to bring me the library receipt.",
            "self-owes-today" => "I promised Iris a sketch of the footbridge.",
            "iris-owes-future" => "Iris promised me a copy of the garden map.",
            "settled-self-owes" => "I promised Iris that I would mend the canvas bag.",
            "settled-iris-owes" => "Iris promised me a replacement notebook cover.",
            "own-undated-promise" => "I intend to finish my watercolor of the harbor.",
            "own-due-promise" => "I intend to sort the postcards in my desk.",
            _ => unreachable!(),
        }
        .into();
        family.graph.derived_memories.push(derived(
            label,
            &format!("2025-08-01T12:{index:02}:00+09:00"),
            text,
            if actor == Some("self") {
                DerivedType::Commitment
            } else {
                DerivedType::OpenLoop
            },
            actor
                .into_iter()
                .chain(counterpart)
                .map(str::to_owned)
                .collect(),
            0.2 + index as f32 * 0.01,
        ));
        family.obligations.push(Obligation {
            label: label.into(),
            memory_external_id: label.into(),
            actor_subjects: actor.into_iter().map(str::to_owned).collect(),
            counterpart_subjects: counterpart.into_iter().map(str::to_owned).collect(),
            due_instant: due.map(str::to_owned),
        });
    }
    for (n, settled) in ["settled-self-owes", "settled-iris-owes"]
        .into_iter()
        .enumerate()
    {
        let resolver = format!("resolver-{n}");
        family.graph.derived_memories.push(derived(
            &resolver,
            &format!("2025-08-02T13:0{n}:00+09:00"),
            if n == 0 {
                "I mended the canvas bag and returned it to Iris."
            } else {
                "Iris gave me the replacement notebook cover."
            }
            .into(),
            DerivedType::Reflection,
            vec!["self".into(), "iris".into()],
            0.6 + n as f32 * 0.01,
        ));
        family.graph.links.push(MemoryLinkInput {
            external_id: format!("resolution-{n}"),
            from: MemoryEndpointInput {
                object_type: ObjectType::DerivedMemory,
                external_id: resolver,
            },
            relation: RelationType::Resolves,
            to: MemoryEndpointInput {
                object_type: ObjectType::DerivedMemory,
                external_id: settled.into(),
            },
            rationale: None,
        });
    }
    let mut background = vec![0.0; 9];
    background[8] = 1.0;
    for memory in &family.graph.derived_memories {
        assign(&mut family.embedding, &memory.text, background.clone());
    }
    let settled_text = family
        .graph
        .derived_memories
        .iter()
        .find(|m| m.external_id == "settled-self-owes")
        .unwrap()
        .text
        .clone();
    let mut settled_topic = vec![0.0; 9];
    settled_topic[6] = 1.0;
    assign(&mut family.embedding, &settled_text, settled_topic);
    for with_topic in [false, true] {
        family.probes.push(probe(
            if with_topic {
                "meeting-with-topic"
            } else {
                "meeting-no-topic"
            },
            input(config, &family.namespace, true, with_topic.then_some(TOPIC)),
            &["actor_counterpart", "character_identity", "trigger"],
        ));
    }
    for with_topic in [false, true] {
        family.probes.push(probe(
            if with_topic {
                "due-with-topic"
            } else {
                "due-time-only"
            },
            input(
                config,
                &family.namespace,
                false,
                with_topic.then_some(TOPIC),
            ),
            &["due_instant", "due"],
        ));
    }
    family.probes.push(probe(
        "resolved-by-topic",
        input(config, &family.namespace, false, Some(&settled_text)),
        &["actor_counterpart", "direction", "due_instant"],
    ));
    family
}

fn familiar_person_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("familiar-person");
    let start = timestamp("2025-08-15T18:00:00+09:00").unwrap();
    for n in 0..25 {
        experience(
            &mut family,
            &format!("shared-day-{n:02}"),
            &(start + Duration::days(n)).to_rfc3339(),
            true,
            0.5,
            -0.1,
        );
    }
    experience(
        &mut family,
        "latest-alone",
        "2025-09-09T19:00:00+09:00",
        false,
        0.5,
        -0.1,
    );
    for (n, text) in [
        "Iris likes her tea without sugar.",
        "Iris prefers to read beside a window.",
        "Iris finds crowded markets tiring.",
        "Iris repairs the loose bindings of her books.",
        "Iris tends the herbs before breakfast.",
        "Iris walks by the river when she needs quiet.",
        "Iris saves postcards from friends.",
        "Iris asks for time to think before making plans.",
    ]
    .into_iter()
    .enumerate()
    {
        let mut memory = derived(
            &format!("known-about-iris-{n:02}"),
            &format!("2025-09-01T10:{n:02}:00+09:00"),
            text.into(),
            DerivedType::Reflection,
            vec!["iris".into()],
            0.5,
        );
        memory.given_by_application = false;
        memory.source_episode_external_ids = vec![format!("shared-day-{n:02}")];
        let mut vector = vec![0.0; 9];
        vector[8] = -1.0;
        assign(&mut family.embedding, text, vector);
        family.graph.derived_memories.push(memory);
    }
    for person in [false, true] {
        family.probes.push(probe(
            if person {
                "familiar-person-no-topic"
            } else {
                "familiar-person-time-only-control"
            },
            input(config, &family.namespace, person, None),
            &[],
        ));
    }
    family
}

fn shared_interpretation_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("shared-interpretation");
    // Enough newer occasions exclude the old source from the bounded recency pool.
    for n in 0..20 {
        experience(
            &mut family,
            &format!("unrelated-recent-{n:02}"),
            &format!("2025-09-09T16:{n:02}:00+09:00"),
            false,
            0.5,
            -0.1,
        );
    }
    for (id, at, words) in [
        (
            "much-older-source",
            "2025-08-01T12:00:00+09:00",
            "Open steps beside the harbor",
        ),
        (
            "description-occasion",
            "2025-09-09T18:00:00+09:00",
            "A quiet book room with cedar shelving",
        ),
    ] {
        experience(&mut family, id, at, false, 0.5, -0.1);
        family
            .experiences
            .last_mut()
            .unwrap()
            .write
            .scene
            .setting
            .words = Some(words.into());
        let mut vector = vec![0.0; 9];
        if id == "description-occasion" {
            vector[1] = 1.0;
        } else {
            vector[8] = -1.0;
        }
        assign(&mut family.embedding, words, vector);
    }
    let query = "The quiet reading space surrounded by cedar bookcases";
    let mut vector = vec![0.0; 9];
    vector[1] = 0.9;
    vector[8] = (1.0_f32 - 0.9 * 0.9).sqrt();
    assign(&mut family.embedding, query, vector);
    let text = "Rain interrupted two visits in different places.";
    let mut memory = derived(
        "shared-interpretation",
        "2025-09-09T18:30:00+09:00",
        text.into(),
        DerivedType::Reflection,
        vec![],
        0.5,
    );
    memory.given_by_application = false;
    memory.source_episode_external_ids =
        vec!["much-older-source".into(), "description-occasion".into()];
    // Provenance fields are not traversable MemoryLinks on the public typed-write path.
    for source in &memory.source_episode_external_ids {
        family.graph.links.push(MemoryLinkInput {
            external_id: format!("shared-source-{source}"),
            from: MemoryEndpointInput {
                object_type: ObjectType::DerivedMemory,
                external_id: memory.external_id.clone(),
            },
            relation: RelationType::DerivedFrom,
            to: MemoryEndpointInput {
                object_type: ObjectType::Episode,
                external_id: source.clone(),
            },
            rationale: None,
        });
    }
    family.graph.derived_memories.push(memory);
    let mut vector = vec![0.0; 9];
    vector[6] = 1.0;
    assign(&mut family.embedding, text, vector);
    family.probes.push(probe(
        "shared-interpretation-time-only-control",
        input(config, &family.namespace, false, None),
        &[],
    ));
    let mut description = input(config, &family.namespace, false, None);
    description.scene.setting.words = Some(query.into());
    family
        .probes
        .push(probe("description-shared-interpretation", description, &[]));
    family.probes.push(probe(
        "shared-interpretation-topic-control",
        input(config, &family.namespace, false, Some(text)),
        &[],
    ));
    family
}

fn healthy(outcome: &cmem_eval::RememberOutcome, expected_vectors: usize) -> Result<()> {
    ensure!(
        outcome.vector_indexing_failure.is_none()
            && outcome.repair_needed.is_empty()
            && outcome.stats_update_status.failure.is_none(),
        "degraded generated slice write: {outcome:?}"
    );
    ensure!(
        outcome.vector_indexed_object_ids.len() == expected_vectors,
        "unexpected vector census in slice write"
    );
    Ok(())
}

async fn ingest(runtime: &ContinuityRuntime, family: &Family) -> Result<BTreeMap<String, String>> {
    let adapter = runtime.adapter();
    adapter.open_namespace(&family.namespace).await?;
    let entities = GraphEnrichmentInput {
        namespace: family.namespace.clone(),
        entities: family.graph.entities.clone(),
        ..Default::default()
    };
    healthy(
        &adapter
            .remember_enrichment(entities)
            .await?
            .context("missing entity write")?,
        0,
    )?;
    let mut ids = BTreeMap::new();
    for experience in &family.experiences {
        let write = &experience.write;
        let native_id = if let Some(text) = &experience.unlinked_observation {
            ensure!(
                write.salience == Some(0.5),
                "direct episode path uses native default salience"
            );
            let episode = adapter
                .remember_episode(EpisodeInput {
                    external_id: write.episode_external_id.clone(),
                    namespace: family.namespace.clone(),
                    summary: write.content.clone(),
                    scene: write.scene.clone(),
                    ended_at: None,
                    metadata: Value::Null,
                })
                .await?;
            healthy(&episode.outcome, 1)?;
            let observation = adapter
                .remember_observation(ObservationInput {
                    external_id: write.observation_external_id.clone(),
                    episode_external_id: write.episode_external_id.clone(),
                    namespace: family.namespace.clone(),
                    speaker: None,
                    text: text.clone(),
                    observed_at: write.observation_observed_at.clone(),
                    metadata: Value::Null,
                })
                .await?;
            healthy(&observation.outcome, 1)?;
            ensure!(
                observation.outcome.persisted_link_ids.is_empty(),
                "distinct observation unexpectedly wrote a link"
            );
            episode.value
        } else {
            let mut plan = adapter.prepare(write.clone()).await?;
            if let Some(at) = &experience.created_at {
                for candidate in &mut plan.plan.candidates {
                    if let native::MemoryCandidate::Episode(episode) = candidate {
                        episode.draft.created_at = Some(timestamp(at)?.with_timezone(&Utc));
                    }
                }
            }
            let native_id = plan
                .plan
                .candidates
                .iter()
                .find_map(|candidate| match candidate {
                    native::MemoryCandidate::Episode(episode) => {
                        episode.draft.id.map(|id| id.to_string())
                    }
                    _ => None,
                })
                .context("prepared episode has no ID")?;
            plan.plan.validations = adapter.validate_plan(&plan).await?;
            ensure!(
                plan.plan
                    .validations
                    .iter()
                    .all(|v| v.status != CandidateValidationStatus::Invalid),
                "invalid slice write plan"
            );
            let result = adapter.commit(plan, CommitWriteOptions::default()).await?;
            healthy(&result.outcome, 2)?;
            ensure!(
                result
                    .outcome
                    .persisted_object_ids
                    .iter()
                    .any(|id| id.to_string() == native_id),
                "prepared episode was not persisted"
            );
            native_id
        };
        ensure!(
            ids.insert(write.episode_external_id.clone(), native_id)
                .is_none(),
            "duplicate experience ID"
        );
    }
    for memory in &family.graph.derived_memories {
        let graph = GraphEnrichmentInput {
            namespace: family.namespace.clone(),
            derived_memories: vec![memory.clone()],
            ..Default::default()
        };
        let outcome = adapter
            .remember_enrichment(graph)
            .await?
            .context("missing derived write")?;
        healthy(&outcome, 1)?;
        ensure!(
            outcome.persisted_object_ids.len() == 1,
            "derived identity census is not singular"
        );
        ensure!(
            ids.insert(
                memory.external_id.clone(),
                outcome.persisted_object_ids[0].to_string()
            )
            .is_none(),
            "duplicate derived ID"
        );
    }
    if !family.graph.links.is_empty() {
        let graph = GraphEnrichmentInput {
            namespace: family.namespace.clone(),
            links: family.graph.links.clone(),
            ..Default::default()
        };
        let outcome = adapter
            .remember_enrichment(graph)
            .await?
            .context("missing graph-link write")?;
        healthy(&outcome, 0)?;
        ensure!(
            outcome.persisted_link_ids.len() == family.graph.links.len(),
            "explicit graph links were not all persisted"
        );
    }
    Ok(ids)
}

fn opposed(original: &Family, ids: &BTreeMap<String, String>) -> Result<(Family, Value)> {
    let mut permutation = BTreeMap::new();
    let mut evidence = Vec::new();
    let episodes = original
        .experiences
        .iter()
        .map(|e| {
            (
                &e.write.episode_external_id,
                e.write.scene.time.as_deref().unwrap(),
                e.write.salience.unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let memories = original
        .graph
        .derived_memories
        .iter()
        .map(|m| {
            (
                &m.external_id,
                m.created_at.as_deref().unwrap(),
                m.salience_score,
            )
        })
        .collect::<Vec<_>>();
    for (kind, mut population) in [("episode", episodes), ("derived_memory", memories)] {
        population.sort_by_key(|(_, at, _)| timestamp(at).unwrap());
        let mut assignments = population.iter().map(|(id, _, _)| *id).collect::<Vec<_>>();
        assignments.sort_by(|a, b| ids[*b].cmp(&ids[*a]));
        for ((old, at, salience), new) in population.into_iter().zip(assignments) {
            permutation.insert(old.clone(), new.clone());
            evidence.push(
                json!({"object_type":kind,"original_external_id":old,"external_id":new,
                "native_id_from_original_ingest":ids[new],"authored_time":at,"salience":salience}),
            );
        }
    }
    let mut family = original.clone();
    for experience in &mut family.experiences {
        let write = &mut experience.write;
        write.episode_external_id = permutation[&write.episode_external_id].clone();
        write.observation_external_id = format!("{}:observation", write.episode_external_id);
    }
    for memory in &mut family.graph.derived_memories {
        memory.external_id = permutation[&memory.external_id].clone();
        for source in &mut memory.source_episode_external_ids {
            *source = permutation[source].clone();
        }
    }
    for link in &mut family.graph.links {
        link.from.external_id = permutation[&link.from.external_id].clone();
        link.to.external_id = permutation[&link.to.external_id].clone();
    }
    for target in &mut family.topic_targets {
        *target = permutation[target].clone();
    }
    for obligation in &mut family.obligations {
        obligation.memory_external_id = permutation[&obligation.memory_external_id].clone();
    }
    for probe in &mut family.probes {
        if let Some(ActivityInput::OpenLoop(id)) = &mut probe.supported_input.activity {
            *id = permutation[id].clone();
        }
    }
    Ok((family, json!(evidence)))
}

fn pack_delta(control: &Value, observed: &Value) -> Value {
    let old = control["selected"].as_array().unwrap();
    let new = observed["selected"].as_array().unwrap();
    let changed = new
        .iter()
        .filter_map(|item| {
            old.iter()
                .find(|o| o["object"] == item["object"])
                .map(|prior| (prior, item))
        })
        .filter(|(prior, item)| {
            prior["section"] != item["section"]
                || prior["rank"] != item["rank"]
                || prior["section_score_components"] != item["section_score_components"]
        })
        .map(|(prior, item)| json!({"before":prior,"after":item}))
        .collect::<Vec<_>>();
    json!({"added":new.iter().filter(|item| !old.iter().any(|o| o["object"] == item["object"])).collect::<Vec<_>>(),
        "removed":displacements(control,observed),"retained_rank_or_score_changes":changed})
}

fn time_reading(
    pack: &RetrievedContextPack,
    observed: &Value,
    family: &Family,
    probe: &PlannedProbe,
) -> Result<Value> {
    let reference = timestamp(
        probe
            .supported_input
            .scene
            .time
            .as_deref()
            .context("missing fixed reference time")?,
    )?;
    let mut eligible = family
        .experiences
        .iter()
        .filter(|e| timestamp(e.write.scene.time.as_deref().unwrap()).unwrap() <= reference)
        .collect::<Vec<_>>();
    eligible.sort_by_key(|e| {
        std::cmp::Reverse(timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
    });
    let attempts = observed["trace"]["graph_expansions"].as_array().unwrap().iter()
        .filter(|r| r["source"] == "recency" || r["source"] == "date_match")
        .map(|r| json!({"native":r,"external_id":pack.object_refs().get(&id(&r["root"])).map(|r| &r.external_id)})).collect::<Vec<_>>();
    let recency_ids = attempts
        .iter()
        .filter(|r| {
            r["native"]["source"] == "recency" && r["native"]["root"]["object_type"] == "episode"
        })
        .filter_map(|r| r["external_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = eligible
        .iter()
        .take(recency_ids.len())
        .map(|e| e.write.episode_external_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let selected = observed["selected"].as_array().unwrap();
    let native_times_match = selected
        .iter()
        .filter(|item| item["object"]["object_type"] == "episode")
        .all(|item| {
            family.experiences.iter().any(|e| {
                item["external_id"] == e.write.episode_external_id
                    && item["recorded_scene"]["time"].as_str().is_some_and(|at| {
                        timestamp(at).ok() == timestamp(e.write.scene.time.as_deref().unwrap()).ok()
                    })
            })
        });
    let distinct = family
        .experiences
        .iter()
        .find(|e| e.unlinked_observation.is_some());
    Ok(
        json!({"native_time_root_attempts":attempts,"recency_root_ids":recency_ids,
        "recency_roots_are_latest_by_authored_time":(!recency_ids.is_empty()).then_some(recency_ids == expected),
        "authored_latest_eligible_ids":eligible.iter().map(|e| &e.write.episode_external_id).collect::<Vec<_>>(),
        "returned_native_times_match_authored_instants":native_times_match,
        "unlinked_distinct_observation_selected":distinct.map(|e| selected.iter().any(|item| item["external_id"] == e.write.observation_external_id)),
        "basis":"Latest membership is an authored-time expectation; returned episode times are native. Root attempts include rejected roots; kind credit is read only from native trace, never inferred from authored dates."}),
    )
}

fn obligation_reading(
    pack: &RetrievedContextPack,
    family: &Family,
    input: &RetrieveInput,
) -> Result<Value> {
    let shape = serde_json::to_value(&pack.outcomes()[0].pack)?;
    let reference = timestamp(input.scene.time.as_deref().unwrap())?;
    let day = reference.date_naive();
    let mut rows = Vec::new();
    for obligation in &family.obligations {
        let included = shape
            .as_object()
            .unwrap()
            .values()
            .filter_map(Value::as_array)
            .flatten()
            .find(|item| {
                item.pointer("/memory/id")
                    .and_then(Value::as_str)
                    .and_then(|id| pack.object_refs().get(id))
                    .is_some_and(|r| r.external_id == obligation.memory_external_id)
            });
        let resolvers = included.and_then(|item| item["resolved_by"].as_array()).map(|ids| ids.iter().map(|id| json!({"native_id":id,
            "external_id":id.as_str().and_then(|id| pack.object_refs().get(id)).map(|r| &r.external_id)})).collect::<Vec<_>>());
        let authored_due = obligation.due_instant.as_deref().map(|at| {
            let date = timestamp(at)
                .unwrap()
                .with_timezone(reference.offset())
                .date_naive();
            if date < day {
                "overdue"
            } else if date == day {
                "due"
            } else {
                "not_due"
            }
        });
        rows.push(json!({"label":obligation.label,"external_id":obligation.memory_external_id,"selected":included.is_some(),
            "native_resolution":resolvers.as_ref().map(|r| if r.is_empty() { "open" } else { "settled" }),
            "native_resolved_by":resolvers,"native_direction":included.and_then(|item| item.get("direction")),
            "native_due_state":included.and_then(|item| item.get("due_state")),
            "planned_roles":obligation,"authored_due_state_if_forwarded":authored_due,
            "role_and_due_fields_forwarded":false}));
    }
    Ok(json!(rows))
}

fn recency_input(probe: &PlannedProbe) -> Result<Option<RetrieveInput>> {
    let Some(floor) = probe.recency_floor else {
        return Ok(None);
    };
    let mut input = probe.supported_input.clone();
    let mut floors = serde_json::to_value(input.cue_floors.unwrap_or_default())?;
    // Only modify a field advertised by this pin's native type. Older pins must
    // remain not_run, never accept an ignored unknown serde field as execution.
    let Some(recency) = floors.get_mut("recency") else {
        return Ok(None);
    };
    *recency = json!(floor);
    input.cue_floors = Some(serde_json::from_value(floors)?);
    ensure!(
        serde_json::to_value(input.cue_floors)?["recency"] == floor,
        "native recency floor did not survive round-trip"
    );
    Ok(Some(input))
}

async fn retrieve_reading(
    runtime: &ContinuityRuntime,
    family: &Family,
    probe: &PlannedProbe,
    input: &RetrieveInput,
    control: &Value,
) -> Result<Value> {
    let pack = runtime.adapter().retrieve(input.clone()).await?;
    let observed = snapshot(&pack, input)?;
    let mut reading = json!({"status":"executed","input":input,
        "topic_targets":target_cohort(&observed,&family.topic_targets),
        "delta_vs_topic_alone":pack_delta(control,&observed),
        "time":time_reading(&pack,&observed,family,probe)?,
        "obligations":obligation_reading(&pack,family,input)?,
        "activity_sources":activity_reading(&pack,&observed,family,input),"observed":observed});
    if let Some(composition) = falsifier_reading(family, input, &observed)? {
        reading["falsifier"] = composition;
    }
    Ok(reading)
}

fn falsifier_reading(
    family: &Family,
    input: &RetrieveInput,
    observed: &Value,
) -> Result<Option<Value>> {
    if !matches!(
        family.name.as_str(),
        "familiar-person" | "shared-interpretation"
    ) {
        return Ok(None);
    }
    let selected = observed["selected"].as_array().unwrap();
    let selected_record =
        |external_id: &str| selected.iter().find(|s| s["external_id"] == external_id);
    let mut experiences = family.experiences.iter().collect::<Vec<_>>();
    experiences.sort_by_key(|e| {
        std::cmp::Reverse(timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
    });
    if family.name == "familiar-person" {
        let latest_interaction = experiences
            .iter()
            .find(|e| {
                e.write
                    .scene
                    .participants
                    .iter()
                    .any(|p| p.key.as_deref() == Some("iris"))
            })
            .unwrap();
        let recent = experiences
            .iter()
            .take(12)
            .map(|e| e.write.episode_external_id.as_str())
            .collect::<Vec<_>>();
        let episodes = selected
            .iter()
            .filter(|s| s["object"]["object_type"] == "episode")
            .collect::<Vec<_>>();
        let knowledge = family
            .graph
            .derived_memories
            .iter()
            .filter(|m| m.entity_external_ids.iter().any(|id| id == "iris"))
            .filter_map(|m| selected_record(&m.external_id))
            .collect::<Vec<_>>();
        let recent_selected = episodes
            .iter()
            .filter(|s| {
                s["external_id"]
                    .as_str()
                    .is_some_and(|id| recent.contains(&id))
            })
            .copied()
            .collect::<Vec<_>>();
        let last = selected_record(&latest_interaction.write.episode_external_id);
        return Ok(Some(json!({"selected_total":selected.len(),
            "recent_episode_slots":recent_selected.len(),"recent_episodes":recent_selected,
            "older_episode_slots":episodes.len()-recent_selected.len(),
            "person_knowledge_slots":knowledge.len(),"person_knowledge":knowledge,
            "other_slots":selected.len()-episodes.len()-knowledge.len(),
            "latest_interaction":last,"latest_interaction_selected":last.is_some(),
            "authored_latest_interaction_id":latest_interaction.write.episode_external_id,
            "authored_recent_episode_ids":recent,
            "native_recency_episode_slots":episodes.iter().filter(|s| s["cue_kinds"].as_array().unwrap().iter().any(|k| k=="recency")).count(),
            "basis":"Exclusive slot census: latest 12 authored occasions, older episodes, authored person knowledge returned natively, and other objects. Last interaction overlaps the recent-episode group and is not an extra slot. Record native cue kinds separately from authored semantic categories."})));
    }
    let old = experiences.last().unwrap();
    let shared = &family.graph.derived_memories[0];
    let old_selected = selected_record(&old.write.episode_external_id);
    let old_cues = old_selected.map(|s| &s["cue_kinds"]);
    let provider = ControllableSimilarityEmbeddingProvider::new(family.embedding.clone())?;
    let description = family
        .probes
        .iter()
        .find_map(|p| p.supported_input.scene.setting.words.as_deref())
        .unwrap();
    let query_vector = provider.vector_for_text(description)?;
    let mut geometry = Vec::new();
    for text in family
        .embedding
        .concepts
        .values()
        .flat_map(|c| &c.inputs)
        .filter(|text| text.as_str() != description)
    {
        let vector = provider.vector_for_text(text)?;
        geometry.push(
            json!({"query":description,"stored":text,"query_vector":query_vector,
            "stored_vector":vector,"cosine":super::descriptions::cosine(&query_vector,&vector),
            "vectors_equal":query_vector==vector}),
        );
    }
    Ok(Some(
        json!({"old_episode_selected":old_selected.is_some(),"old_episode":old_selected,
        "old_episode_native_place_credit":old_cues.is_some_and(|k| k.as_array().unwrap().iter().any(|kind| kind=="place")),
        "old_episode_native_recency_credit":old_cues.is_some_and(|k| k.as_array().unwrap().iter().any(|kind| kind=="recency")),
        "authored_old_episode_id":old.write.episode_external_id,
        "shared_interpretation_selected":selected_record(&shared.external_id),
        "authored_shared_source_episode_ids":shared.source_episode_external_ids,
        "description_present":input.scene.setting.words.is_some(),"geometry":geometry,
        "basis":"Old-source identity and shared sources are authored; membership and place/recency credit come only from native selected records. Time-only controls background recency; topic control checks an intentional route through the shared interpretation. The query is distinct from every emitted stored vector."}),
    ))
}

async fn measure(
    runtime: &ContinuityRuntime,
    family: &Family,
    config: &BenchmarkRunConfig,
) -> Result<Value> {
    let topic = input(config, &family.namespace, false, Some(TOPIC));
    let pack = runtime.adapter().retrieve(topic.clone()).await?;
    let control = snapshot(&pack, &topic)?;
    let mut rows = Vec::new();
    for probe in &family.probes {
        let projection = Box::pin(retrieve_reading(
            runtime,
            family,
            probe,
            &probe.supported_input,
            &control,
        ))
        .await?;
        let mut missing = probe.required_routes.clone();
        let executed = if let Some(input) = recency_input(probe)? {
            missing.retain(|route| route != "recency_floor");
            if missing.is_empty() {
                Box::pin(retrieve_reading(runtime, family, probe, &input, &control)).await?
            } else {
                Value::Null
            }
        } else if missing.is_empty() {
            projection.clone()
        } else {
            Value::Null
        };
        rows.push(json!({"probe":probe.name,"planned_input":probe,
            "status":if missing.is_empty() { "executed" } else { "not_run" },
            "missing_capabilities":missing,
            "executed_case":executed,"parent_control":projection}));
    }
    Ok(
        json!({"topic_alone_input":topic,"topic_alone_observed":control,
        "topic_alone_targets":target_cohort(&control,&family.topic_targets),"rows":rows}),
    )
}

fn activity_reading(
    pack: &RetrievedContextPack,
    observed: &Value,
    family: &Family,
    input: &RetrieveInput,
) -> Value {
    let Some(ActivityInput::OpenLoop(activity)) = &input.activity else {
        return Value::Null;
    };
    let source_ids = &family
        .graph
        .derived_memories
        .iter()
        .find(|m| &m.external_id == activity)
        .unwrap()
        .source_episode_external_ids;
    let mut sources = family
        .experiences
        .iter()
        .filter(|e| source_ids.contains(&e.write.episode_external_id))
        .collect::<Vec<_>>();
    sources.sort_by_key(|e| {
        std::cmp::Reverse(timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
    });
    json!({"authored_sources_by_scene_time":sources.iter().map(|e| json!({"external_id":e.write.episode_external_id,"scene_time":e.write.scene.time,"created_at":e.created_at})).collect::<Vec<_>>(),
        "retained_source_roots":observed["roots"].as_array().unwrap().iter().filter(|r| r["external_id"].as_str().is_some_and(|id| source_ids.iter().any(|s| s == id))).collect::<Vec<_>>(),
        "selected_sources":pack.outcomes()[0].pack.relevant_episodes.iter().filter_map(|e| pack.object_refs().get(&e.id.to_string()).filter(|r| source_ids.contains(&r.external_id)).map(|r| json!({"external_id":r.external_id,"native_id":e.id,"native_scene_time":e.scene.time,"native_created_at":e.created_at}))).collect::<Vec<_>>(),
        "native_activity":observed["activity"],"root_cap":input.surface_policy.max_graph_roots,"episode_section_cap":input.surface_policy.sections.relevant_episodes})
}

pub(super) async fn run(stores: &Path, config: &BenchmarkRunConfig) -> Result<Value> {
    let mut inputs = Vec::new();
    let mut measurements = Vec::new();
    for build in [
        time_family,
        obligations_family,
        familiar_person_family,
        shared_interpretation_family,
    ] {
        let original = build(config);
        let mut next = original.clone();
        let mut order = Value::Null;
        for opposed_order in [false, true] {
            let root = stores.join(format!(
                "{}-{}",
                original.name,
                if opposed_order { "opposed" } else { "original" }
            ));
            fs::create_dir(&root)?;
            let runtime = ContinuityRuntime::new(
                &root,
                config,
                EmbeddingRuntimeBinding::Controllable {
                    fixture: next.embedding.clone(),
                    dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
                },
            )
            .await?;
            let result = async {
                let ids = Box::pin(ingest(&runtime, &next)).await?;
                if opposed_order {
                    for row in order.as_array().unwrap() {
                        ensure!(
                            ids[row["external_id"].as_str().unwrap()]
                                == row["native_id_from_original_ingest"].as_str().unwrap(),
                            "native ID changed between orders"
                        );
                    }
                }
                let reading = Box::pin(measure(&runtime, &next, config)).await?;
                Ok::<_, anyhow::Error>((ids, reading))
            }
            .await;
            let cleanup = runtime.cleanup(&next.namespace).await;
            drop(runtime);
            cleanup?;
            let (ids, reading) = result?;
            inputs.push(json!({"family":original.name,"opposed_ids":opposed_order,"input":next,"id_order":order}));
            measurements.push(json!({"family":original.name,"opposed_ids":opposed_order,"native_ids":ids,"reading":reading}));
            eprintln!(
                "measured {} {}",
                original.name,
                if opposed_order {
                    "opposed IDs"
                } else {
                    "original IDs"
                }
            );
            if !opposed_order {
                (next, order) = opposed(&original, &ids)?;
            }
        }
    }
    let executed = measurements
        .iter()
        .flat_map(|m| m["reading"]["rows"].as_array().unwrap())
        .filter(|r| r["status"] == "executed")
        .count();
    let not_run = measurements
        .iter()
        .flat_map(|m| m["reading"]["rows"].as_array().unwrap())
        .filter(|r| r["status"] == "not_run")
        .count();
    Ok(json!({"inputs":inputs,"measurements":measurements,
        "full_cases_executed":executed,"full_cases_not_run":not_run,"supported_parent_controls_executed":executed+not_run,
        "same_day_description_continuity":["/keyless_measurements","/opposed_keyless_measurements"],
        "method":"Fixed generated intent for time/prospective plans; original and native-ID-opposed inputs. Unsupported fields are retained as typed planned intent and reported not_run. Each row separately executes a supported parent control with roles, due instant, range and explicit recency floor absent. executed_case records the full supported case; recency overrides run only when the serialized native floor type advertises the field and its value survives a typed round-trip. Missing fields remain not_run; no unavailable predicate is silently forwarded. Native resolution is reported; authored due classification is labelled hypothetical. Topic counts use eight graded episodes in each original family; added falsifiers use separate composition and spillover readings. No behavioral pass/fail threshold. No clock, paid calls or fixture changes."}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_falsifiers_separate_person_knowledge_and_shared_old_sources() {
        let config = config();
        let person = familiar_person_family(&config);
        let meetings = person
            .experiences
            .iter()
            .filter(|e| !e.write.scene.participants.is_empty())
            .count();
        assert!(meetings * 10 > person.experiences.len() * 9);
        assert!(
            person
                .probes
                .iter()
                .all(|p| p.supported_input.topic.is_none())
        );
        assert!(!person.graph.derived_memories.is_empty());
        assert!(
            person
                .graph
                .derived_memories
                .iter()
                .all(|m| !m.given_by_application
                    && m.entity_external_ids == ["iris"]
                    && !m.source_episode_external_ids.is_empty())
        );
        let last = person
            .experiences
            .iter()
            .filter(|e| !e.write.scene.participants.is_empty())
            .max_by_key(|e| timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
            .unwrap();
        let census = falsifier_reading(&person, &person.probes[1].supported_input, &json!({"selected":[
            {"external_id":last.write.episode_external_id,"object":{"object_type":"episode"},"cue_kinds":["participant"]},
            {"external_id":person.graph.derived_memories[0].external_id,"object":{"object_type":"derived_memory"}},
            {"external_id":"observation","object":{"object_type":"observation"}}
        ]})).unwrap().unwrap();
        assert_eq!(census["recent_episode_slots"], 1);
        assert_eq!(census["person_knowledge_slots"], 1);
        assert_eq!(census["other_slots"], 1);
        assert_eq!(census["selected_total"], 3);
        assert_eq!(census["latest_interaction_selected"], true);
        assert_eq!(census["native_recency_episode_slots"], 0);
        let shared = shared_interpretation_family(&config);
        let description = shared
            .probes
            .iter()
            .find(|p| p.supported_input.scene.setting.words.is_some())
            .unwrap();
        let result = falsifier_reading(
            &shared,
            &description.supported_input,
            &json!({"selected":[]}),
        )
        .unwrap()
        .unwrap();
        let pairs = result["geometry"].as_array().unwrap();
        assert!(
            pairs
                .iter()
                .all(|p| p["vectors_equal"] == false && p["cosine"].as_f64().unwrap() < 0.95)
        );
        let best = pairs
            .iter()
            .map(|p| p["cosine"].as_f64().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((0.89..0.91).contains(&best));
        let sources = &shared.graph.derived_memories[0].source_episode_external_ids;
        assert_eq!(sources.len(), 2);
        assert_eq!(shared.graph.links.len(), sources.len());
        assert!(
            sources
                .iter()
                .all(|source| shared.graph.links.iter().any(|link| link.relation
                    == RelationType::DerivedFrom
                    && link.from.object_type == ObjectType::DerivedMemory
                    && link.from.external_id == shared.graph.derived_memories[0].external_id
                    && link.to.object_type == ObjectType::Episode
                    && &link.to.external_id == source))
        );
        let old = shared
            .experiences
            .iter()
            .min_by_key(|e| timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
            .unwrap();
        assert!(sources.contains(&old.write.episode_external_id));
        assert!(shared.experiences.len() > 16);
        assert!(description.supported_input.topic.is_none());
        assert!(
            shared
                .probes
                .iter()
                .any(|p| p.supported_input.topic.is_some())
        );
        for original in [person, shared] {
            let ids = original
                .experiences
                .iter()
                .map(|e| e.write.episode_external_id.clone())
                .chain(
                    original
                        .graph
                        .derived_memories
                        .iter()
                        .map(|m| m.external_id.clone()),
                )
                .enumerate()
                .map(|(n, id)| (id, format!("native-{n:03}")))
                .collect();
            let (permuted, _) = opposed(&original, &ids).unwrap();
            for (old, new) in original
                .graph
                .derived_memories
                .iter()
                .zip(&permuted.graph.derived_memories)
            {
                for (old_id, new_id) in old
                    .source_episode_external_ids
                    .iter()
                    .zip(&new.source_episode_external_ids)
                {
                    let source = |family: &Family, id: &str| {
                        family
                            .experiences
                            .iter()
                            .find(|e| e.write.episode_external_id == id)
                            .unwrap()
                            .write
                            .content
                            .clone()
                    };
                    assert_eq!(source(&original, old_id), source(&permuted, new_id));
                }
            }
        }
    }

    #[test]
    fn recency_override_requires_an_advertised_native_field() {
        let family = time_family(&config());
        let defaults = serde_json::to_value(native::RetrievalCueFloors::default()).unwrap();
        for probe in family.probes.iter().filter(|p| p.recency_floor.is_some()) {
            let mapped = recency_input(probe).unwrap();
            if defaults.get("recency").is_some() {
                let mapped = mapped.expect("advertised recency must execute");
                let mut actual = serde_json::to_value(&mapped).unwrap();
                assert_eq!(actual["cue_floors"]["recency"], json!(probe.recency_floor));
                actual["cue_floors"] = Value::Null;
                assert_eq!(
                    actual,
                    serde_json::to_value(&probe.supported_input).unwrap()
                );
            } else {
                assert!(
                    mapped.is_none(),
                    "an absent native field must remain not_run"
                );
            }
        }
    }

    #[test]
    fn generated_cases_preserve_intent_when_native_ids_oppose_time() {
        let config = config();
        let time = time_family(&config);
        let obligations = obligations_family(&config);
        assert_eq!(
            serde_json::to_value(&time).unwrap(),
            serde_json::to_value(time_family(&config)).unwrap()
        );
        assert_eq!(
            time.probes
                .iter()
                .filter(|p| p.required_routes.is_empty())
                .count(),
            2
        );
        let range = time
            .probes
            .iter()
            .find_map(|p| p.time_range.as_ref())
            .unwrap();
        assert_eq!(
            time.experiences
                .iter()
                .filter(|e| {
                    let at = timestamp(e.write.scene.time.as_deref().unwrap()).unwrap();
                    timestamp(&range.start).unwrap() <= at && at <= timestamp(&range.end).unwrap()
                })
                .count(),
            5
        );
        let latest = time
            .experiences
            .iter()
            .filter(|e| {
                timestamp(e.write.scene.time.as_deref().unwrap()).unwrap()
                    <= timestamp(EVENING).unwrap()
            })
            .max_by_key(|e| timestamp(e.write.scene.time.as_deref().unwrap()).unwrap())
            .unwrap();
        assert_eq!(latest.unlinked_observation.as_deref(), Some(DISTINCT));
        assert!(time.experiences.iter().any(|e| e.write.salience.unwrap()
            > latest.write.salience.unwrap()
            && timestamp(e.write.scene.time.as_deref().unwrap()).unwrap()
                < timestamp(latest.write.scene.time.as_deref().unwrap()).unwrap()));
        assert!(
            obligations
                .probes
                .iter()
                .all(|p| !p.required_routes.is_empty())
        );
        assert!(
            obligations
                .graph
                .derived_memories
                .iter()
                .all(|m| m.assertions.is_empty())
        );
        assert_eq!(obligations.graph.links.len(), 2);
        let created = time
            .experiences
            .iter()
            .filter_map(|e| {
                e.created_at.as_ref().map(|at| {
                    (
                        timestamp(e.write.scene.time.as_deref().unwrap()).unwrap(),
                        timestamp(at).unwrap(),
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(created.len(), 3);
        assert!(
            created
                .windows(2)
                .all(|p| p[0].0 < p[1].0 && p[0].1 > p[1].1)
        );
        assert!(
            obligations
                .obligations
                .iter()
                .any(|o| o.counterpart_subjects.is_empty() && o.due_instant.is_none())
        );
        assert!(
            obligations
                .obligations
                .iter()
                .any(|o| o.actor_subjects == ["iris"] && o.counterpart_subjects == ["self"])
        );
        for original in [time, obligations] {
            let ids = original
                .experiences
                .iter()
                .map(|e| e.write.episode_external_id.clone())
                .chain(
                    original
                        .graph
                        .derived_memories
                        .iter()
                        .map(|m| m.external_id.clone()),
                )
                .enumerate()
                .map(|(i, id)| (id, format!("native-{i:03}")))
                .collect();
            let (permuted, order) = opposed(&original, &ids).unwrap();
            assert_eq!(original.topic_targets.len(), 8);
            assert_eq!(
                serde_json::to_value(&original.embedding).unwrap(),
                serde_json::to_value(&permuted.embedding).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&original.probes).unwrap(),
                serde_json::to_value(&permuted.probes).unwrap()
            );
            for kind in ["episode", "derived_memory"] {
                let rows = order
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["object_type"] == kind)
                    .collect::<Vec<_>>();
                for pair in rows.windows(2) {
                    assert!(
                        timestamp(pair[0]["authored_time"].as_str().unwrap()).unwrap()
                            < timestamp(pair[1]["authored_time"].as_str().unwrap()).unwrap()
                    );
                    assert!(
                        pair[0]["native_id_from_original_ingest"].as_str()
                            > pair[1]["native_id_from_original_ingest"].as_str()
                    );
                    if kind == "derived_memory" {
                        assert!(pair[0]["salience"].as_f64() < pair[1]["salience"].as_f64());
                    }
                }
            }
            for (old, new) in original.experiences.iter().zip(&permuted.experiences) {
                let mut restored = new.clone();
                restored.write.episode_external_id = old.write.episode_external_id.clone();
                restored.write.observation_external_id = old.write.observation_external_id.clone();
                assert_eq!(
                    serde_json::to_value(old).unwrap(),
                    serde_json::to_value(restored).unwrap()
                );
            }
            for (old, new) in original
                .graph
                .derived_memories
                .iter()
                .zip(&permuted.graph.derived_memories)
            {
                let mut restored = new.clone();
                restored.external_id = old.external_id.clone();
                restored.source_episode_external_ids = old.source_episode_external_ids.clone();
                assert_eq!(
                    serde_json::to_value(old).unwrap(),
                    serde_json::to_value(restored).unwrap()
                );
            }
            for (old, new) in original.graph.links.iter().zip(&permuted.graph.links) {
                for (old_end, new_end) in [(&old.from, &new.from), (&old.to, &new.to)] {
                    let old_memory = original
                        .graph
                        .derived_memories
                        .iter()
                        .find(|m| m.external_id == old_end.external_id)
                        .unwrap();
                    let new_memory = permuted
                        .graph
                        .derived_memories
                        .iter()
                        .find(|m| m.external_id == new_end.external_id)
                        .unwrap();
                    assert_eq!(old_memory.text, new_memory.text);
                }
            }
        }
    }
}
