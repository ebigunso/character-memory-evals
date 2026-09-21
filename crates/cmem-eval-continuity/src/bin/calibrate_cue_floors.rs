//! A generated, unasserted experiment for the library owner's floor decision.
use std::{collections::BTreeMap, env, fs, io::Write, path::Path, process::Command};

use anyhow::{Context, Result, ensure};
use chrono::{Duration, TimeZone, Utc};
use cmem_eval::{
    ActivityInput, BenchmarkRunConfig, ControllableDimensionPolicy, ControllableSimilarityFixture,
    DatasetId, EmbeddingProviderConfig, EmbeddingRuntimeBinding, MemorySceneInput, RetrieveInput,
    RetrievedContextPack, SceneParticipantInput, SimilarityConceptFixture,
    character_memory::api::types as native, text_sha256,
};
use cmem_eval_continuity::{
    CHECKED_FIXTURE_SEED, ContinuityRuntime, ContinuityScenario, ContinuityScenarioEmbedding,
    EntityDeclaration, ExpectedRelevance, InteractionEvent, PerceivedReference, ScenarioPattern,
    Scene, SceneParticipant, SceneSelection, ThreadMembership, run_continuity_scenario,
};
use serde::Serialize;
use serde_json::{Value, json};

#[path = "calibrate_cue_floors/descriptions.rs"]
mod descriptions;

const KINDS: [&str; 4] = ["participant", "place", "activity", "topic"];
const FLOORS: [usize; 5] = [0, 1, 2, 3, 5];
const AT: &str = "2025-09-01T12:00:00Z";

#[derive(Clone, Serialize)]
struct Probe {
    name: String,
    measured_kind: String,
    pressure: String,
    target: Option<String>, // Measurement only: never copied into RetrieveInput.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tracked_targets: Vec<String>,
    input: RetrieveInput,
}

fn config() -> BenchmarkRunConfig {
    let mut config = BenchmarkRunConfig {
        run_id: "cue-floor-calibration".into(),
        dataset: DatasetId::new("continuity").unwrap(),
        backend: Default::default(),
        retrieval: Default::default(),
        ingest: Default::default(),
        metrics: Default::default(),
    };
    config.backend.embedding.provider = EmbeddingProviderConfig::ControllableSimilarity;
    config.backend.embedding.vector_size = Some(9);
    config.retrieval.surface_policy.include_debug_rationale = true;
    config.retrieval.surface_policy.object_types = native::default_retrieval_object_types();
    config
}

fn assign(fixture: &mut ControllableSimilarityFixture, text: &str, vector: Vec<f32>) {
    fixture.clusters.insert(text.into(), vector);
    fixture.concepts.insert(
        text.into(),
        SimilarityConceptFixture {
            cluster: text.into(),
            inputs: vec![text.into()],
        },
    );
}

fn cue(input: &mut RetrieveInput, kind: &str, strength: &str) {
    let text = format!("{strength} {kind} words");
    match kind {
        "participant" => {
            input.scene.participants = vec![SceneParticipantInput {
                description: Some(text),
                ..Default::default()
            }]
        }
        "place" => input.scene.setting.words = Some(text),
        "activity" => input.activity = Some(ActivityInput::Thread("orchid-project".into())),
        "topic" => input.topic = Some(text),
        _ => unreachable!(),
    }
}

fn generated(config: &BenchmarkRunConfig) -> Result<(ContinuityScenario, Vec<Probe>)> {
    let mut embedding = ControllableSimilarityFixture {
        seed: CHECKED_FIXTURE_SEED,
        vector_size: 9,
        noise_magnitude: 0.000001,
        clusters: BTreeMap::new(),
        concepts: BTreeMap::new(),
    };
    let mut background = vec![0.0; 9];
    background[8] = -1.0;
    assign(&mut embedding, "Iris", background);
    let mut events = Vec::new();
    let start = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();
    for (axis, kind) in KINDS.iter().enumerate() {
        for (strength, cosine) in [("loud", 1.0_f32), ("quiet", 0.2), ("unlived", 0.01)] {
            let mut vector = vec![0.0; 9];
            vector[axis] = cosine;
            vector[8] = (1.0 - cosine * cosine).sqrt();
            assign(&mut embedding, &format!("{strength} {kind} words"), vector);
        }
        // Remember writes three vector objects: 17 memories exceed the default
        // 48-candidate cap without introducing a separate 64-edge hub bottleneck.
        let count = if *kind == "activity" { 16 } else { 17 };
        for index in 0..count {
            let id = format!("{kind}-{index:02}");
            let text = format!(
                "{} entry {index:02}",
                [
                    "Linen parcel",
                    "Cedar terrace",
                    "Orchid ledger",
                    "Copper bell"
                ][axis]
            );
            let mut vector = vec![0.0; 9];
            vector[axis] = 0.99 - index as f32 * 0.004;
            vector[axis + 4] = (1.0 - vector[axis] * vector[axis]).sqrt();
            assign(&mut embedding, &text, vector);
            events.push(InteractionEvent::Remember {
                event_id: format!("write-{id}"),
                external_id: id,
                timestamp: start + Duration::minutes(events.len() as i64),
                text,
                surface_texts: None,
                entity_external_ids: if *kind == "participant" {
                    vec!["iris".into()]
                } else {
                    vec![]
                },
                thread: (*kind == "activity").then(|| ThreadMembership {
                    thread_external_id: "orchid-project".into(),
                }),
                salience: 0.5,
            });
        }
    }
    // The required scripted Query is also the topic-only control; there are no assertions.
    events.push(InteractionEvent::Query {
        event_id: "topic-control".into(),
        query_id: "topic-control".into(),
        timestamp: AT.parse()?,
        text: "loud topic words".into(),
        expected: ExpectedRelevance {
            relevant_external_ids: vec!["topic-00".into()],
            irrelevant_external_ids: vec![],
        },
    });
    let scenario = ContinuityScenario {
        fixture_id: "cue-floor-calibration".into(),
        namespace: format!("cue-floor-calibration-{:016x}", CHECKED_FIXTURE_SEED),
        pattern: ScenarioPattern::GradedSimilarity,
        catalog_situations: vec![],
        character_entity: None,
        scenes: BTreeMap::new(),
        entities: vec![EntityDeclaration {
            external_id: "iris".into(),
            label: "Iris".into(),
            is_hub: false,
        }],
        embedding: ContinuityScenarioEmbedding::controllable_similarity_provider(embedding),
        events,
        requirements: Default::default(),
    };
    ensure!(
        cmem_eval_continuity::scenario_missing_features(&scenario)?.is_empty(),
        "calibration ingest requires an unsupported feature"
    );
    let empty = || RetrieveInput {
        mode: config.retrieval.mode,
        namespace: scenario.namespace.clone(),
        topic: None,
        scene: MemorySceneInput {
            time: Some(AT.into()),
            ..Default::default()
        },
        activity: None,
        cue_floors: None,
        surface_policy: config.retrieval.surface_policy.clone(),
    };
    let mut probes = Vec::new();
    for kind in KINDS {
        for pressure in KINDS
            .into_iter()
            .filter(|other| *other != kind)
            .chain(["all"])
        {
            let mut input = empty();
            if kind != "topic" {
                cue(&mut input, "topic", "loud");
            }
            for other in KINDS {
                if other != kind && (pressure == "all" || pressure == other) {
                    cue(&mut input, other, "loud");
                }
            }
            cue(&mut input, kind, "quiet");
            probes.push(Probe {
                name: format!("{kind}-under-{pressure}"),
                tracked_targets: vec![],
                measured_kind: kind.into(),
                pressure: pressure.into(),
                target: Some(format!(
                    "{kind}-{}",
                    if kind == "activity" {
                        "15:derived"
                    } else {
                        "00"
                    }
                )),
                input,
            });
        }
    }
    for kind in ["participant", "place", "topic"] {
        let mut input = empty();
        cue(
            &mut input,
            if kind == "topic" { "activity" } else { "topic" },
            "loud",
        );
        cue(&mut input, kind, "unlived");
        probes.push(Probe {
            name: format!("unlived-{kind}"),
            tracked_targets: vec![],
            measured_kind: kind.into(),
            pressure: "unlived words; synthetic nearest cosine about 0.01, no relevant memory"
                .into(),
            target: None,
            input,
        });
    }
    let mut input = empty();
    cue(&mut input, "topic", "loud");
    cue(&mut input, "activity", "loud");
    probes.push(Probe {
        name: "saturated-thread-and-topic".into(),
        tracked_targets: vec![],
        measured_kind: "topic".into(),
        pressure: "16 thread members".into(),
        target: Some("topic-00".into()),
        input,
    });
    let mut input = empty();
    cue(&mut input, "topic", "loud");
    input.scene.participants = vec![SceneParticipantInput {
        key: Some("iris".into()),
        ..Default::default()
    }];
    probes.push(Probe {
        name: "participant-key-inheritance".into(),
        tracked_targets: vec![],
        measured_kind: "participant".into(),
        pressure: "loud topic; key only, no participant vector words".into(),
        target: Some("participant-16".into()),
        input,
    });
    Ok((scenario, probes))
}

fn id(value: &Value) -> String {
    value["id"].as_str().unwrap().into()
}

fn generated_overlap(
    config: &BenchmarkRunConfig,
    reworded: bool,
) -> Result<(ContinuityScenario, Vec<Probe>)> {
    let place = descriptions::probe_words("place", reworded);
    let participant = descriptions::probe_words("participant", reworded);
    let topic = "Copper bell repair";
    let targets = (0..8)
        .map(|i| format!("strong-topic-{i:02}"))
        .collect::<Vec<_>>();
    let mut embedding = ControllableSimilarityFixture {
        seed: CHECKED_FIXTURE_SEED,
        vector_size: 9,
        noise_magnitude: 0.000001,
        clusters: BTreeMap::new(),
        concepts: BTreeMap::new(),
    };
    let vector = |x: f32, y: f32| {
        let mut v = vec![0.0; 9];
        v[0] = x;
        v[1] = y;
        v[8] = (1.0 - x * x - y * y).max(0.0).sqrt();
        v
    };
    let mut background = vec![0.0; 9];
    background[8] = -1.0;
    assign(&mut embedding, "Character", background.clone());
    assign(&mut embedding, place, vector(1.0, 0.0));
    assign(&mut embedding, participant, vector(1.0, 0.0));
    assign(&mut embedding, topic, vector(0.0, 1.0));
    if reworded {
        for kind in ["place", "participant"] {
            for (index, words) in descriptions::word_pool(kind).iter().take(3).enumerate() {
                assign(
                    &mut embedding,
                    words,
                    vector(1.0 - index as f32 * 0.012, 0.0),
                );
            }
        }
    }
    let alone = Scene {
        who: vec![SceneParticipant {
            reference: PerceivedReference::Key { key: "self".into() },
            gold_entity: None,
        }],
        ..Default::default()
    };
    let mut shared = alone.clone();
    shared.place = Some(PerceivedReference::Description { text: place.into() });
    shared.who.push(SceneParticipant {
        reference: PerceivedReference::Description {
            text: participant.into(),
        },
        gold_entity: None,
    });
    let mut scenario = ContinuityScenario {
        fixture_id: "cue-floor-overlapping-pressure".into(),
        namespace: format!("cue-floor-overlap-{:016x}", CHECKED_FIXTURE_SEED),
        pattern: ScenarioPattern::Situated,
        catalog_situations: vec!["generated-overlapping-pressure".into()],
        character_entity: Some("self".into()),
        scenes: BTreeMap::from([("shared".into(), shared), ("alone".into(), alone)]),
        entities: vec![EntityDeclaration {
            external_id: "self".into(),
            label: "Character".into(),
            is_hub: false,
        }],
        embedding: ContinuityScenarioEmbedding::controllable_similarity_provider(embedding.clone()),
        events: Vec::new(),
        requirements: Default::default(),
    };
    if reworded {
        scenario.fixture_id.push_str("-reworded");
        scenario.namespace.push_str("-reworded");
    }
    let start = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();
    // At least 48 actual episodes match the scene words. Their body-only
    // observations do not; the native Setting/With write surface creates overlap.
    let scene_count = native::RetrievalCandidateLimits::default().max_vector_candidates;
    for index in 0..scene_count + targets.len() {
        let strong = index >= scene_count;
        let text = if strong {
            let topic_index = index - scene_count;
            let text = format!("Repairing the copper bell, detail {topic_index:02}");
            assign(
                &mut embedding,
                &text,
                vector(0.0, 0.9 - 0.3 * topic_index as f32 / 7.0),
            );
            text
        } else {
            format!("Ledger entry {index:02}")
        };
        let scene = if reworded && !strong {
            let mut scene = scenario.scenes["shared"].clone();
            scene.place = Some(PerceivedReference::Description {
                text: descriptions::write_words("place", index).into(),
            });
            scene.who[1].reference = PerceivedReference::Description {
                text: descriptions::write_words("participant", index).into(),
            };
            SceneSelection::Inline { scene }
        } else {
            SceneSelection::Named {
                name: if strong { "alone" } else { "shared" }.into(),
            }
        };
        scenario.events.push(InteractionEvent::Experience {
            event_id: if strong {
                targets[index - scene_count].clone()
            } else {
                format!("shared-{index:02}")
            },
            timestamp: start + Duration::minutes(index as i64),
            text,
            scene,
            speaker: None,
            salience: Some(0.5),
        });
    }
    // Use the driver's existing normalized-input inventory, including scene lines.
    for input in scenario.runtime_embedding_inputs() {
        if input.starts_with("Ledger entry ") {
            let index: usize = input.lines().next().unwrap()[13..].parse()?;
            let v = if input.contains("\nSetting:") {
                vector(0.994 - index as f32 * 0.0001, 0.05 - index as f32 * 0.0002)
            } else {
                background.clone()
            };
            assign(&mut embedding, &input, v);
        }
    }
    scenario.events.push(InteractionEvent::Query {
        event_id: "topic-control".into(),
        query_id: "topic-control".into(),
        timestamp: AT.parse()?,
        text: topic.into(),
        expected: ExpectedRelevance {
            relevant_external_ids: targets.clone(),
            irrelevant_external_ids: vec![],
        },
    });
    scenario.embedding = ContinuityScenarioEmbedding::controllable_similarity_provider(embedding);
    ensure!(
        cmem_eval_continuity::scenario_missing_features(&scenario)?.is_empty(),
        "overlapping calibration requires unsupported features"
    );
    let mut probes = Vec::new();
    for pressure in ["place", "participant", "both"] {
        for kind in KINDS {
            let mut input = RetrieveInput {
                mode: config.retrieval.mode,
                namespace: scenario.namespace.clone(),
                topic: Some(topic.into()),
                scene: MemorySceneInput {
                    time: Some(AT.into()),
                    ..Default::default()
                },
                activity: None,
                cue_floors: None,
                surface_policy: config.retrieval.surface_policy.clone(),
            };
            if pressure != "participant" {
                input.scene.setting.words = Some(place.into());
            }
            if pressure != "place" {
                input.scene.participants.push(SceneParticipantInput {
                    description: Some(participant.into()),
                    ..Default::default()
                });
            }
            probes.push(Probe {
                name: format!("overlap-{pressure}-sweep-{kind}"),
                measured_kind: kind.into(),
                pressure: format!(
                    "48 scene-word episodes; weak topic overlap; {pressure}; activity absent"
                ),
                target: Some(targets[0].clone()),
                tracked_targets: targets.clone(),
                input,
            });
        }
    }
    Ok((scenario, probes))
}

fn retained_roots(trace: &Value) -> Vec<&Value> {
    trace["graph_expansions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|root| root["outcome"] != "root_limit")
        .collect()
}

fn target_stages(observed: &Value, target: &str) -> Value {
    let contains = |field: &str| {
        observed[field]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["external_id"] == target)
    };
    json!({"candidate_merge":contains("candidates"),"graph_roots":contains("roots"),"pack":contains("selected")})
}

fn target_cohort(observed: &Value, targets: &[String]) -> Value {
    if targets.is_empty() {
        return Value::Null;
    }
    let mut stages = serde_json::Map::new();
    for (field, stage) in [
        ("candidates", "candidate_merge"),
        ("roots", "graph_roots"),
        ("selected", "pack"),
    ] {
        let occupants = observed[field].as_array().unwrap();
        let survives = |target: &&String| occupants.iter().any(|s| s["external_id"] == **target);
        let survived = targets.iter().filter(survives).collect::<Vec<_>>();
        let missing = targets.iter().filter(|t| !survives(t)).collect::<Vec<_>>();
        let other_occupants = occupants
            .iter()
            .filter(|s| !targets.iter().any(|t| s["external_id"] == *t))
            .collect::<Vec<_>>();
        stages.insert(stage.into(), json!({"survived_count":survived.len(),"survived":survived,"missing":missing,"other_occupants":other_occupants}));
    }
    Value::Object(stages)
}

fn snapshot(pack: &RetrievedContextPack, input: &RetrieveInput) -> Result<Value> {
    ensure!(pack.outcomes().len() == 1, "expected one native outcome");
    let outcome = &pack.outcomes()[0];
    let trace = serde_json::to_value(outcome.trace.as_ref().context("native trace absent")?)?;
    let assignments = trace["section_assignments"].as_array().unwrap();
    let candidates = trace["vector_candidates"].as_array().unwrap();
    let roots = retained_roots(&trace);
    let describe = |object: &Value| {
        let key = id(object);
        let assignment = assignments.iter().find(|a| id(&a["object"]) == key);
        let candidate = candidates.iter().find(|c| id(&c["object"]) == key);
        let scene = outcome
            .pack
            .relevant_episodes
            .iter()
            .find(|episode| episode.id.to_string() == key)
            .map(|episode| &episode.scene);
        json!({"object":object, "external_id":pack.object_refs().get(&key).map(|r| &r.external_id),
            "section":assignment.map(|a| &a["section"]), "section_score_components":assignment.map(|a| &a["reason"]["scores"]),
            "vector_score":candidate.map(|c| &c["score"]), "cue_kinds":assignment.map(|a| &a["cue_kinds"]),
            "recorded_scene":scene})
    };
    let selected = assignments
        .iter()
        .filter(|a| a["reason"]["kind"] == "selected")
        .map(|a| {
            let mut row = describe(&a["object"]);
            row["rank"] = a["rank"].clone();
            row
        })
        .collect::<Vec<_>>();
    let mut admissions = Vec::new();
    for a in trace["floor_admissions"].as_array().unwrap() {
        let object_id = id(&a["object"]);
        let kind = a["cue_kind"].as_str().unwrap();
        let explicit = roots
            .iter()
            .any(|r| id(&r["root"]) == object_id && r["source"] == kind);
        let in_candidates = candidates.iter().any(|c| id(&c["object"]) == object_id);
        let key_only = kind == "participant"
            && input
                .scene
                .participants
                .iter()
                .all(|p| p.description.is_none() && p.name.is_none());
        let origin = if a["stage"]["kind"] != "section" {
            "direct before graph expansion"
        } else if explicit {
            "direct explicit root"
        } else if kind == "activity" || key_only || !in_candidates {
            "inherited through expansion"
        } else {
            "unknown: candidate/root trace lacks per-kind vector origin"
        };
        admissions.push(
            json!({"native":a, "memory":describe(&a["object"]), "credit_origin":origin,
            "selected_in_pack":selected.iter().any(|s| id(&s["object"]) == object_id),
            "displacement_group":stage_key(&a["stage"])}),
        );
    }
    Ok(json!({"selected":selected, "floor_admissions":admissions,
        "roots":roots.iter().map(|r| { let mut v=describe(&r["root"]); v["source"]=r["source"].clone(); v["outcome"]=r["outcome"].clone(); v }).collect::<Vec<_>>(),
        "candidates":candidates.iter().map(|c| describe(&c["object"])).collect::<Vec<_>>(),
        "activity":outcome.activity, "scene_references":outcome.scene_references,"telemetry":outcome.rationale.telemetry,"trace":trace}))
}

fn stage_key(stage: &Value) -> String {
    if stage["kind"] == "section" {
        format!("section:{}", stage["section"].as_str().unwrap())
    } else {
        stage["kind"].as_str().unwrap().into()
    }
}

fn displacements(zero: &Value, current: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for (field, stage) in [
        ("candidates", "candidate_merge"),
        ("roots", "graph_roots"),
        ("selected", "section"),
    ] {
        for object in zero[field].as_array().unwrap() {
            if current[field]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| id(&c["object"]) == id(&object["object"]))
            {
                continue;
            }
            let key = if stage == "section" {
                format!("section:{}", object["section"].as_str().unwrap())
            } else {
                stage.into()
            };
            result
                .entry(key)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .unwrap()
                .push(object.clone());
        }
    }
    Value::Object(result)
}

fn floors(kind: &str, value: usize) -> native::RetrievalCueFloors {
    let mut floors = native::RetrievalCueFloors::default();
    match kind {
        "participant" => floors.participant = value,
        "place" => floors.place = value,
        "activity" => floors.activity = value,
        "topic" => floors.topic = value,
        _ => unreachable!(),
    }
    floors
}

fn summarize(rows: &[Value]) -> Vec<Value> {
    KINDS.into_iter().flat_map(|kind| FLOORS.into_iter().map(move |floor| (kind,floor))).map(|(kind,floor)| {
        let group=rows.iter().filter(|r| r["measured_kind"]==kind && r["floor"]==floor).collect::<Vec<_>>();
        let mut credits=BTreeMap::<String,usize>::new();
        let mut origins=BTreeMap::<String,usize>::new();
        let mut displaced=0;
        for row in &group {
            for admission in row["observed"]["floor_admissions"].as_array().unwrap() {
                if admission["native"]["cue_kind"]==kind {
                    *credits.entry(stage_key(&admission["native"]["stage"])).or_default()+=1;
                    *origins.entry(admission["credit_origin"].as_str().unwrap().into()).or_default()+=1;
                }
            }
            displaced+=row["displaced_vs_same_probe_floor_zero"].as_object().unwrap().iter()
                .filter(|(stage,_)| stage.starts_with("section:")).map(|(_,items)|items.as_array().unwrap().len()).sum::<usize>();
        }
        json!({"kind":kind,"floor":floor,"sweep_rows":group.len(),
            "starvation_measurable":group.iter().filter(|r| r["starved"].is_boolean()).count(),
            "starved":group.iter().filter(|r| r["starved"]==true).count(),
            "native_floor_credits_for_this_kind":credits,"credit_origins_for_this_kind":origins,"displaced_pack_items_across_probes":displaced,
            "unlived_words":group.iter().filter(|r|r["probe"].as_str().unwrap().starts_with("unlived-")).map(|r|json!({
                "probe":r["probe"],"displaced":r["displaced_vs_same_probe_floor_zero"],
                "floor_admissions":r["observed"]["floor_admissions"]
            })).collect::<Vec<_>>()})
    }).collect()
}

fn remove_cue(input: &mut RetrieveInput, kind: &str) {
    match kind {
        "participant" => input.scene.participants.clear(),
        "place" => input.scene.setting = Default::default(),
        "activity" => input.activity = None,
        "topic" => input.topic = None,
        _ => unreachable!(),
    }
}

async fn reachability_control(
    runtime: &ContinuityRuntime,
    input: RetrieveInput,
    target: &str,
) -> Result<Value> {
    let pack = runtime.adapter().retrieve(input.clone()).await?;
    let observed = snapshot(&pack, &input)?;
    let selected = observed["selected"].as_array().unwrap();
    Ok(
        json!({"input":input,"target_selected":selected.iter().find(|s| s["external_id"] == target),"target_stage_survival":target_stages(&observed, target),
        "selected":selected,"telemetry":observed["telemetry"]}),
    )
}

async fn measure(
    runtime: &mut ContinuityRuntime,
    scenario: &ContinuityScenario,
    probes: &[Probe],
    config: &BenchmarkRunConfig,
) -> Result<Value> {
    let initial = Box::pin(run_continuity_scenario(
        runtime,
        scenario,
        &config.retrieval,
    ))
    .await?;
    ensure!(
        initial.traces.len() == 1,
        "calibration fixture did not execute its topic-only control"
    );
    let topic_candidates = initial.traces[0].retrieval.outcomes()[0]
        .trace
        .as_ref()
        .context("control trace missing")?
        .vector_candidates
        .iter()
        .map(|c| c.object.id.to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut controls = BTreeMap::new();
    for probe in probes {
        let mut eligible = false;
        if let Some(target) = &probe.target {
            let mut isolated = probe.input.clone();
            for kind in KINDS {
                if kind != probe.measured_kind {
                    remove_cue(&mut isolated, kind);
                }
            }
            let mut removed = probe.input.clone();
            remove_cue(&mut removed, &probe.measured_kind);
            let isolated = reachability_control(runtime, isolated, target).await?;
            let removed = reachability_control(runtime, removed, target).await?;
            eligible = isolated["target_selected"]["cue_kinds"] == json!([probe.measured_kind])
                && removed["target_selected"].is_null();
            controls.insert(probe.name.clone(), json!({"isolated":isolated,"removed":removed,"eligible_for_starvation_measure":eligible}));
        }
        let mut zero = Value::Null;
        for value in FLOORS {
            let mut input = probe.input.clone();
            input.cue_floors = Some(floors(&probe.measured_kind, value));
            let pack = runtime
                .adapter()
                .retrieve(input.clone())
                .await
                .with_context(|| format!("{} floor {value}", probe.name))?;
            let observed = snapshot(&pack, &input)?;
            if value == 0 {
                zero = observed.clone();
            }
            let selected = observed["selected"].as_array().unwrap();
            let target = probe
                .target
                .as_ref()
                .and_then(|target| selected.iter().find(|s| s["external_id"] == *target));
            let credit = observed["floor_admissions"]
                .as_array()
                .unwrap()
                .iter()
                .fold(BTreeMap::<String, usize>::new(), |mut counts, admission| {
                    *counts
                        .entry(admission["credit_origin"].as_str().unwrap().into())
                        .or_default() += 1;
                    counts
                });
            rows.push(json!({"probe":probe.name,"measured_kind":probe.measured_kind,"pressure":probe.pressure,"floor":value,"effective_floors":input.cue_floors,
                "target":probe.target,"target_admitted":probe.target.as_ref().map(|_| target.is_some()),
                "target_stage_survival":probe.target.as_ref().map(|target|target_stages(&observed,target)),
                "tracked_target_cohort":target_cohort(&observed,&probe.tracked_targets),
                "starved":eligible.then_some(target.is_none()),
                "target_native_cue_kinds":target.map(|t| &t["cue_kinds"]),
                "target_exclusively_measured_kind":target.map(|t| t["cue_kinds"] == json!([probe.measured_kind])),
                "pack_slots":selected.len(), "topic_pack_slots":selected.iter().filter(|s| s["cue_kinds"].as_array().unwrap().contains(&json!("topic"))).count(),
                "roots_in_topic_only_candidate_control":observed["roots"].as_array().unwrap().iter().filter(|r| topic_candidates.contains(&id(&r["object"]))).count(),
                "credit_origin_counts":credit,"displaced_vs_same_probe_floor_zero":displacements(&zero,&observed), "observed":observed}));
        }
        eprintln!("measured {}", probe.name);
    }
    let control = &initial.traces[0];
    let mut control_input = probes[0].input.clone();
    for kind in ["participant", "place", "activity"] {
        remove_cue(&mut control_input, kind);
    }
    let control_observed = snapshot(&control.retrieval, &control_input)?;
    Ok(
        json!({"executed":rows.len(),"not_run":0,"control_retrievals":1+2*controls.len(),
        "summary":summarize(&rows),
        "topic_only_control_cohort":target_cohort(&control_observed,&probes[0].tracked_targets),
        "topic_only_control_observed":control_observed,
        "topic_only_control_targets": control.expected.relevant_external_ids.iter().map(|target| json!({
            "target":target,"admitted":control.retrieval.items().iter().any(|item| item.external_id.as_ref()==Some(target))
        })).collect::<Vec<_>>(),"reachability_controls":controls,"rows":rows}),
    )
}

fn revision(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()?;
    ensure!(
        output.status.success(),
        "git revision failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let output = args.next().context("usage: calibrate_cue_floors <new-report.json>; run via cargo after pinning the sibling library")?;
    ensure!(args.next().is_none(), "expected one new report path");
    let output = Path::new(&output);
    let config = config();
    config.validate()?;
    let (scenario, probes) = generated(&config)?;
    let (overlap_scenario, overlap_probes) = generated_overlap(&config, false)?;
    let (reworded_scenario, reworded_probes) = generated_overlap(&config, true)?;
    let keyless = descriptions::generated(&config, &reworded_scenario, &reworded_probes)?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let library = workspace.join("../CharacterMemory");
    let library_commit = revision(&library)?;
    let harness_commit = revision(workspace)?;
    let mut file = std::io::BufWriter::new(
        fs::File::create_new(output)
            .context("refusing to replace an existing calibration report")?,
    );
    let stores = output.with_extension("stores");
    fs::create_dir(&stores).context("calibration store directory must be new")?;
    let input = json!({"scenario":scenario,"probes":probes});
    let overlap_input = json!({"scenario":overlap_scenario,"probes":overlap_probes});
    let reworded_input = json!({"scenario":reworded_scenario,"probes":reworded_probes});
    let mut opposed_inputs = Vec::new();
    let result = async {
        let mut results = Vec::new();
        for (name, scenario, probes) in [
            ("orthogonal", &scenario, &probes),
            ("overlapping", &overlap_scenario, &overlap_probes),
            ("reworded", &reworded_scenario, &reworded_probes),
        ] {
            let binding = EmbeddingRuntimeBinding::Controllable {
                fixture: scenario
                    .embedding
                    .controllable_similarity()
                    .unwrap()
                    .clone(),
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
            };
            let run_root = stores.join(name);
            fs::create_dir(&run_root)?;
            let mut runtime = ContinuityRuntime::new(&run_root, &config, binding).await?;
            let result = Box::pin(measure(&mut runtime, scenario, probes, &config)).await;
            let cleanup = runtime.cleanup(&scenario.namespace).await;
            drop(runtime);
            cleanup?;
            results.push(result?);
        }
        let run_root = stores.join("keyless");
        fs::create_dir(&run_root)?;
        let binding = EmbeddingRuntimeBinding::Controllable {
            fixture: keyless.embedding.clone(),
            dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
        };
        let runtime = ContinuityRuntime::new(&run_root, &config, binding).await?;
        let result = Box::pin(descriptions::measure(&runtime, &keyless)).await;
        let cleanup = runtime.cleanup(&keyless.namespace).await;
        drop(runtime);
        cleanup?;
        results.push(result?);
        for (name, scenario, probes) in [
            ("identical", &overlap_scenario, &overlap_probes),
            ("reworded", &reworded_scenario, &reworded_probes),
        ] {
            let run_root = stores.join(format!("{name}-ids-opposed"));
            fs::create_dir(&run_root)?;
            let binding = EmbeddingRuntimeBinding::Controllable {
                fixture: scenario
                    .embedding
                    .controllable_similarity()
                    .unwrap()
                    .clone(),
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
            };
            let mut runtime = ContinuityRuntime::new(&run_root, &config, binding).await?;
            let (opposed, ids) = descriptions::opposed_scenario(&runtime, scenario).await?;
            opposed_inputs
                .push(json!({"family":name,"scenario":opposed,"probes":probes,"id_order":ids}));
            let result = Box::pin(measure(&mut runtime, &opposed, probes, &config)).await;
            let cleanup = runtime.cleanup(&opposed.namespace).await;
            drop(runtime);
            cleanup?;
            results.push(result?);
        }
        let run_root = stores.join("keyless-ids-opposed");
        fs::create_dir(&run_root)?;
        let binding = EmbeddingRuntimeBinding::Controllable {
            fixture: keyless.embedding.clone(),
            dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
        };
        let runtime = ContinuityRuntime::new(&run_root, &config, binding).await?;
        let (opposed, ids) = descriptions::opposed_keyless(&runtime, &keyless).await?;
        opposed_inputs.push(json!({"family":"keyless","input":opposed,"id_order":ids}));
        let result = Box::pin(descriptions::measure(&runtime, &opposed)).await;
        let cleanup = runtime.cleanup(&opposed.namespace).await;
        drop(runtime);
        cleanup?;
        results.push(result?);
        Ok::<_, anyhow::Error>(results)
    }
    .await;
    fs::remove_dir_all(&stores).context("remove owned calibration stores")?;
    let measurements = result?;
    ensure!(
        revision(&library)? == library_commit && revision(workspace)? == harness_commit,
        "checkout revision changed during calibration"
    );
    let report = json!({"header":{
        "harness_commit":harness_commit,"library_commit":library_commit,"profile":if cfg!(debug_assertions){"debug"}else{"release"},
        "seed":CHECKED_FIXTURE_SEED,"input_sha256":text_sha256(&serde_json::to_string(&input)?),"config_sha256":text_sha256(&serde_json::to_string(&config)?),
        "overlapping_input_sha256":text_sha256(&serde_json::to_string(&overlap_input)?),
        "reworded_input_sha256":text_sha256(&serde_json::to_string(&reworded_input)?),
        "keyless_input_sha256":text_sha256(&serde_json::to_string(&keyless)?),
        "opposed_input_sha256":text_sha256(&serde_json::to_string(&opposed_inputs)?),
        "generator_source_sha256":text_sha256(concat!(include_str!("calibrate_cue_floors.rs"), include_str!("calibrate_cue_floors/descriptions.rs"))),
        "config":config,"native_candidate_limits":native::RetrievalCandidateLimits::default(),"native_graph_limits":native::RetrievalGraphLimits::default(),
        "native_section_limits":native::ContinuitySectionLimits::default(),"native_default_floors":native::RetrievalCueFloors::default(),"sweep":FLOORS,"stores_cleaned":true},
        "method":{
            "design":"One generated corpus, 17 memories (51 vector objects) per vector kind and 16 activity-thread members. Same store, same probe, one floor swept; other floors stay at native defaults. No scenario pass/fail assertions. Metadata targets are used only after native retrieval. Starvation is measured only when native isolated-cue control admits the target exclusively by that kind and removing the tested cue makes the target absent; otherwise it is null, with controls retained.",
            "geometry":"Seeded synthetic vectors: unrelated groups orthogonal; loud cue cosine about 1, quiet cue about 0.2, unlived words about 0.01 to the least-bad neighbour. Values are controlled pressure, not empirical natural-language relevance thresholds.",
            "overlapping_pressure":"Separate generated situated corpus: 48 Experience episodes with native Setting and With words, no place key, and eight strong topic-only experiences graded from cosine 0.9 to 0.6. Body-only scene observations are background; the real normalized episode surface receives a vector with scene cosine about 0.99 and topic cosine about 0.05. Place-only, participant-only and combined scene probes each sweep all four floors; activity is absent, its sweep is a control. Each cohort stage lists the surviving authored episode identities, missing identities and other scored occupants (including companion observations, never counted as authored episode survival). The native topic-only control has the same cohort census, exposing losses even without scene competition. Occupancy is not a uniquely paired causal eviction. Non-topic sweeps are target-survival measurements, not exclusively-that-kind starvation claims; the existing single-target starvation control tracks the strongest episode.",
            "displacements":"Set differences versus the identical probe at tested-kind floor zero. Floor zero does not disable a cue: spare-room policy depends on the pinned library (979643f shares turns even at zero). Native floor credits are stage events, not causal admissions. Each admission names its stage/section displacement group; multiple admissions cannot be uniquely paired to displaced objects. All available native vector and final section score components are retained; root ordering score is not exposed.",
            "origin":"Candidate-merge/root floor credits precede graph expansion and are direct. At section selection, explicit matching Participant/Activity roots are direct; activity/key-only participant descendants or objects absent from retained vector candidates are inherited. Remaining cases are unknown because vector candidates and roots omit per-kind origin; no fixture labels reconstruct it.",
            "topic_roots":"Root IDs also found in the independent topic-only native candidate control, not an exclusive attribution of a root to topic. Pack slots count native Selected assignments containing topic and may overlap other kinds.",
            "limits":"Native default caps and depth. PLACE currently uses words/vector roots; saturated explicit PLACE roots promised by the future state slice are not simulated. Native write construction timestamps are omitted from observations; generated inputs and query times use no clock."},
        "generated_input":input,"measurements":measurements[0],"overlapping_input":overlap_input,"overlapping_measurements":measurements[1],
        "reworded_input":reworded_input,"reworded_measurements":measurements[2],
        "keyless_input":keyless,"keyless_measurements":measurements[3],
        "opposed_input":opposed_inputs,
        "opposed_identical_measurements":measurements[4],
        "opposed_reworded_measurements":measurements[5],
        "opposed_keyless_measurements":measurements[6],
        "paraphrase_geometry":descriptions::paraphrase_geometry()?});
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.flush()?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlap_generation_uses_actual_scene_write_surfaces() {
        let (scenario, probes) = generated_overlap(&config(), false).unwrap();
        let inputs = scenario.runtime_embedding_inputs();
        assert_eq!(
            inputs
                .iter()
                .filter(|s| s
                    .contains("\nSetting: Cedar reading room\nWith: Visitor wearing a linen coat"))
                .count(),
            native::RetrievalCandidateLimits::default().max_vector_candidates
        );
        assert_eq!(
            inputs
                .iter()
                .filter(|s| s.starts_with("Repairing the copper bell, detail "))
                .count(),
            8
        );
        assert_eq!(probes.len(), 3 * KINDS.len());
        let observed = json!({"candidates":[{"external_id":"target"}],"roots":[],"selected":[{"external_id":"target"}]});
        assert_eq!(
            target_stages(&observed, "target"),
            json!({"candidate_merge":true,"graph_roots":false,"pack":true})
        );
        let cohort = target_cohort(&observed, &["target".into(), "absent".into()]);
        assert_eq!(cohort["pack"]["survived_count"], 1);
        assert_eq!(cohort["pack"]["missing"], json!(["absent"]));
    }
    #[test]
    fn starvation_denominator_excludes_unavailable_controls() {
        let row = |starved| {
            json!({"measured_kind":"topic","floor":1,"starved":starved,"probe":"control",
            "observed":{"floor_admissions":[]},"displaced_vs_same_probe_floor_zero":{}})
        };
        let summary = summarize(&[row(json!(true)), row(json!(false)), row(Value::Null)]);
        let group = summary
            .iter()
            .find(|s| s["kind"] == "topic" && s["floor"] == 1)
            .unwrap();
        assert_eq!(group["starvation_measurable"], 2);
        assert_eq!(group["starved"], 1);
    }
    #[test]
    fn root_allocation_excludes_rejected_roots_but_keeps_bounded_attempts() {
        let trace = json!({"graph_expansions":[{"outcome":"root_limit"},{"outcome":"expanded"},{"outcome":"bounded"},{"outcome":"missing_root"}]});
        let roots = retained_roots(&trace);
        assert_eq!(roots.len(), 3);
        assert!(roots.iter().all(|r| r["outcome"] != "root_limit"));
    }
    #[test]
    fn displacement_keeps_native_scores_and_matches_identity_not_rank() {
        let item = |id: &str, score: f64| json!({"object":{"id":id},"section":"relevant_episodes","section_score_components":{"final_score":score}});
        let old =
            json!({"candidates":[],"roots":[],"selected":[item("kept",0.9),item("lost",0.8)]});
        let new =
            json!({"candidates":[],"roots":[],"selected":[item("added",0.2),item("kept",0.9)]});
        assert_eq!(
            displacements(&old, &new),
            json!({"section:relevant_episodes":[item("lost",0.8)]})
        );
    }
}
