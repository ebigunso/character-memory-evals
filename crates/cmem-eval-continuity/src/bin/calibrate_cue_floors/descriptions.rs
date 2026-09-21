//! Description-only inputs and explicitly controlled paraphrase geometry.
use super::*;
use cmem_eval::{
    CandidateValidationStatus, CommitWriteOptions, ControllableSimilarityEmbeddingProvider,
    PrepareWriteInput,
};

const PLACES: [&str; 4] = [
    "Cedar reading room",
    "The reading room lined with cedar shelves",
    "A quiet book room with cedar shelving",
    "The quiet reading space surrounded by cedar bookcases",
];
const PEOPLE: [&str; 4] = [
    "Visitor wearing a linen coat",
    "The visitor in a loose linen overcoat",
    "A familiar guest dressed in a light linen coat",
    "The familiar visitor with the loose light linen overcoat",
];

pub(super) fn word_pool(kind: &str) -> &'static [&'static str; 4] {
    match kind {
        "place" => &PLACES,
        "participant" => &PEOPLE,
        _ => unreachable!(),
    }
}

pub(super) fn probe_words(kind: &str, reworded: bool) -> &'static str {
    word_pool(kind)[if reworded { 3 } else { 0 }]
}

pub(super) fn write_words(kind: &str, index: usize) -> &'static str {
    // Stable per-referent sampling, without a new RNG dependency. The last wording is held out.
    let hash = text_sha256(&format!("{CHECKED_FIXTURE_SEED}:{kind}:{index}"));
    let choice = u64::from_str_radix(&hash[..16], 16).unwrap() as usize % 3;
    word_pool(kind)[choice]
}

#[derive(Clone, Serialize)]
pub(super) struct KeylessFamily {
    pub namespace: String,
    pub embedding: ControllableSimilarityFixture,
    writes: Vec<PrepareWriteInput>,
    probes: Vec<Probe>,
    topic_alone: RetrieveInput,
    targets: Vec<String>,
}

// Discover identities through the public adapter; do not duplicate its private UUID recipe.
async fn opposed_ids(
    runtime: &ContinuityRuntime,
    namespace: &str,
) -> Result<Vec<(String, String)>> {
    runtime.adapter().open_namespace(namespace).await?;
    let result = async {
        let mut ids = Vec::new();
        for index in 0..48 {
            let external_id = format!("shared-{index:02}");
            let plan = runtime
                .adapter()
                .prepare(PrepareWriteInput {
                    namespace: namespace.into(),
                    content: "Identity planning only".into(),
                    episode_external_id: external_id.clone(),
                    observation_external_id: format!("{external_id}:observation"),
                    scene: MemorySceneInput {
                        time: Some(AT.into()),
                        ..Default::default()
                    },
                    speaker_entity_external_id: None,
                    salience: None,
                    observation_observed_at: Some(AT.into()),
                    raw_refs: vec![],
                    include_vector_index_candidates: false,
                    include_stats_update_candidates: false,
                })
                .await?;
            let id = plan
                .plan
                .candidates
                .iter()
                .find_map(|candidate| {
                    if let native::MemoryCandidate::Episode(episode) = candidate {
                        episode.draft.id.map(|id| id.to_string())
                    } else {
                        None
                    }
                })
                .context("prepared episode ID missing")?;
            ids.push((external_id, id));
        }
        ids.sort_by(|left, right| right.1.cmp(&left.1));
        ensure!(
            ids.windows(2).all(|pair| pair[0].1 > pair[1].1),
            "episode IDs are not strictly opposed"
        );
        Ok(ids)
    }
    .await;
    // These identity-only plans are never committed; the measurement starts with an empty namespace.
    runtime.cleanup(namespace).await?;
    result
}

pub(super) async fn opposed_scenario(
    runtime: &ContinuityRuntime,
    original: &ContinuityScenario,
) -> Result<(ContinuityScenario, Value)> {
    let ids = opposed_ids(runtime, &original.namespace).await?;
    let mut scenario = original.clone();
    let mut index = 0;
    let mut evidence = Vec::new();
    for event in &mut scenario.events {
        if let InteractionEvent::Experience {
            event_id,
            timestamp,
            ..
        } = event
            && event_id.starts_with("shared-")
        {
            evidence.push(
                json!({"original_external_id":event_id,"external_id":ids[index].0,
                "prepared_native_episode_id":ids[index].1,"authored_scene_time":timestamp}),
            );
            *event_id = ids[index].0.clone();
            index += 1;
        }
    }
    ensure!(index == ids.len(), "unexpected shared occasion count");
    Ok((scenario, json!(evidence)))
}

pub(super) async fn opposed_keyless(
    runtime: &ContinuityRuntime,
    original: &KeylessFamily,
) -> Result<(KeylessFamily, Value)> {
    let ids = opposed_ids(runtime, &original.namespace).await?;
    let mut family = original.clone();
    let mut index = 0;
    let mut evidence = Vec::new();
    for write in &mut family.writes {
        if write.episode_external_id.starts_with("shared-") {
            evidence.push(
                json!({"original_external_id":write.episode_external_id,"external_id":ids[index].0,
                "prepared_native_episode_id":ids[index].1,"authored_scene_time":write.scene.time}),
            );
            write.episode_external_id = ids[index].0.clone();
            write.observation_external_id = format!("{}:observation", write.episode_external_id);
            index += 1;
        }
    }
    ensure!(index == ids.len(), "unexpected shared occasion count");
    Ok((family, json!(evidence)))
}

pub(super) fn generated(
    config: &BenchmarkRunConfig,
    scenario: &ContinuityScenario,
    overlap_probes: &[Probe],
) -> Result<KeylessFamily> {
    let namespace = format!("cue-floor-keyless-{:016x}", CHECKED_FIXTURE_SEED);
    let start = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();
    let mut writes = Vec::new();
    for event in &scenario.events {
        let InteractionEvent::Experience {
            event_id,
            text,
            scene,
            ..
        } = event
        else {
            continue;
        };
        let index = writes.len();
        let at = if index < 48 {
            start + Duration::days(index as i64 / 4) + Duration::hours(index as i64 % 4)
        } else {
            start + Duration::days(11) + Duration::hours(4) + Duration::minutes(index as i64 - 48)
        }
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let scene = match scene {
            SceneSelection::Named { name } => &scenario.scenes[name],
            SceneSelection::Inline { scene } => scene,
        };
        let mut input_scene = MemorySceneInput {
            time: Some(at.clone()),
            ..Default::default()
        };
        if let Some(PerceivedReference::Description { text }) = &scene.place {
            input_scene.setting.words = Some(text.clone());
        }
        input_scene.participants = scene
            .who
            .iter()
            .filter_map(|person| {
                if let PerceivedReference::Description { text } = &person.reference {
                    Some(SceneParticipantInput {
                        description: Some(text.clone()),
                        ..Default::default()
                    })
                } else {
                    None
                }
            })
            .collect();
        writes.push(PrepareWriteInput {
            namespace: namespace.clone(),
            content: text.clone(),
            episode_external_id: event_id.clone(),
            observation_external_id: format!("{event_id}:observation"),
            scene: input_scene,
            speaker_entity_external_id: None,
            salience: Some(0.5),
            observation_observed_at: Some(at),
            raw_refs: vec![],
            include_vector_index_candidates: true,
            include_stats_update_candidates: true,
        });
    }
    let mut base = overlap_probes
        .iter()
        .find(|p| p.name == "overlap-both-sweep-topic")
        .context("combined topic probe missing")?
        .input
        .clone();
    base.namespace = namespace.clone();
    base.surface_policy = config.retrieval.surface_policy.clone();
    let targets = overlap_probes[0].tracked_targets.clone();
    let mut probes = Vec::new();
    for name in ["topic-and-scene", "scene-only", "same-day-by-descriptions"] {
        let mut input = base.clone();
        if name != "topic-and-scene" {
            input.topic = None;
        }
        if name == "same-day-by-descriptions" {
            input.scene.time = Some(
                (start + Duration::days(11) + Duration::hours(8))
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            );
        }
        for kind in ["topic", "place", "participant"] {
            probes.push(Probe {
                name: name.into(), measured_kind: kind.into(),
                pressure: "48 reworded scene occasions over 12 days, 8 topic targets; no identity or setting keys".into(),
                target: None, tracked_targets: targets.clone(), input: input.clone(),
            });
        }
    }
    let mut topic_alone = base;
    remove_cue(&mut topic_alone, "participant");
    remove_cue(&mut topic_alone, "place");
    Ok(KeylessFamily {
        namespace,
        embedding: scenario
            .embedding
            .controllable_similarity()
            .unwrap()
            .clone(),
        writes,
        probes,
        topic_alone,
        targets,
    })
}

fn scene_reading(observed: &Value, family: &KeylessFamily, input: &RetrieveInput) -> Value {
    let episodes = observed["selected"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            s["object"]["object_type"] == "episode"
                && s["external_id"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("shared-"))
        })
        .collect::<Vec<_>>();
    let mut occasions = family
        .writes
        .iter()
        .filter(|w| w.episode_external_id.starts_with("shared-"))
        .map(|w| (&w.episode_external_id, w.scene.time.as_ref().unwrap()))
        .collect::<Vec<_>>();
    occasions.sort_by(|a, b| b.1.cmp(a.1));
    let mut latest = occasions
        .iter()
        .take(episodes.len())
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();
    let mut selected = episodes
        .iter()
        .map(|s| s["external_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    latest.sort();
    selected.sort();
    let query_day = &input.scene.time.as_ref().unwrap()[..10];
    let same_day = episodes
        .iter()
        .filter(|s| {
            s["recorded_scene"]["time"]
                .as_str()
                .is_some_and(|t| &t[..10] == query_day)
        })
        .count();
    json!({"occasion_count":episodes.len(), "occasions":episodes,
        "most_recent_n_occasions":(!episodes.is_empty()).then_some(selected == latest),
        "expected_most_recent_ids_for_returned_count":latest,
        "same_day_occasions":same_day,"other_day_occasions":episodes.len()-same_day,
        "available_same_day_occasions":occasions.iter().filter(|(_, time)| &time[..10] == query_day).count(),
        "recorded_times_match_written_scene_times":episodes.iter().all(|s| occasions.iter().any(|(id,time)|
            s["external_id"] == **id && s["recorded_scene"]["time"] == **time))})
}

pub(super) async fn measure(runtime: &ContinuityRuntime, family: &KeylessFamily) -> Result<Value> {
    runtime.adapter().open_namespace(&family.namespace).await?;
    // Only the public write path: there is no entity registration or language-header workaround.
    for input in &family.writes {
        let mut plan = runtime.adapter().prepare(input.clone()).await?;
        let validations = runtime.adapter().validate_plan(&plan).await?;
        ensure!(
            validations
                .iter()
                .all(|v| v.status != CandidateValidationStatus::Invalid),
            "invalid generated keyless write plan"
        );
        plan.plan.validations = validations;
        let committed = runtime
            .adapter()
            .commit(plan, CommitWriteOptions::default())
            .await?;
        let outcome = committed.outcome;
        ensure!(
            outcome.vector_indexing_failure.is_none()
                && outcome.repair_needed.is_empty()
                && outcome.stats_update_status.failure.is_none()
                && !outcome.vector_indexed_object_ids.is_empty(),
            "degraded generated keyless write: {}",
            input.episode_external_id
        );
    }
    let pack = runtime
        .adapter()
        .retrieve(family.topic_alone.clone())
        .await?;
    let control = snapshot(&pack, &family.topic_alone)?;
    let mut rows = Vec::new();
    for probe in &family.probes {
        for floor in FLOORS {
            let mut input = probe.input.clone();
            input.cue_floors = Some(floors(&probe.measured_kind, floor));
            let pack = runtime.adapter().retrieve(input.clone()).await?;
            let observed = snapshot(&pack, &input)?;
            rows.push(json!({"probe":probe.name,"measured_kind":probe.measured_kind,"floor":floor,
                "input":input,"scene":scene_reading(&observed,family,&input),
                "tracked_target_cohort":target_cohort(&observed,&family.targets),
                "scene_slots_new_vs_topic_alone":observed["selected"].as_array().unwrap().iter()
                    .filter(|s| s["cue_kinds"].as_array().unwrap().iter().any(|k| k=="place" || k=="participant"))
                    .filter(|s| !control["selected"].as_array().unwrap().iter().any(|c| c["object"]==s["object"]))
                    .collect::<Vec<_>>(),
                "observed":observed}));
        }
        eprintln!(
            "measured keyless {} sweep {}",
            probe.name, probe.measured_kind
        );
    }
    Ok(
        json!({"executed":rows.len(),"not_run":0,"entity_registration_calls":0,
        "topic_only_control_cohort":target_cohort(&control,&family.targets),"topic_only_control_observed":control,
        "best_scene_surface_score_per_description":{"status":"not_available","reason":"The pinned trace has no best scene-surface score per description cue; raw native traces are retained."},
        "method":"No person/entity keys, setting keys, names, activities or speaker keys on any write or probe; internal namespace and memory external IDs are storage bookkeeping, not perceived identities. Shared occasions span 12 days (4/day); topic targets follow the last day's occasions. Same-day probe uses that day's evening and descriptions only. Latest-N membership and the available same-day denominator are expectations from AUTHORED scene times. Returned same-day counts use native recorded scenes, and each returned occasion's native time is checked against its authored value. No native census of all stored scenes is obtained from the public adapter, so persisted times for unreturned occasions are not independently verified. No consolidation or temporal-filter success is claimed.",
        "rows":rows}),
    )
}

pub(super) fn paraphrase_geometry() -> Result<Value> {
    let mut fixture = ControllableSimilarityFixture {
        seed: CHECKED_FIXTURE_SEED,
        vector_size: 9,
        noise_magnitude: 0.000001,
        clusters: BTreeMap::new(),
        concepts: BTreeMap::new(),
    };
    let other_places = [
        "Oak waiting room",
        "The waiting space with oak shelves",
        "A quiet waiting room lined with oak",
        "The oak-panelled lobby with books",
    ];
    let other_people = [
        "Courier wearing a cotton coat",
        "The courier in a loose cotton overcoat",
        "A delivery worker dressed in a light cotton coat",
        "The delivery visitor with the loose light cotton overcoat",
    ];
    let mut labels = Vec::new();
    for (kind, pools, axis) in [
        ("place", [PLACES, other_places], 0),
        ("participant", [PEOPLE, other_people], 2),
    ] {
        for (referent, pool) in pools.iter().enumerate() {
            for (wording, text) in pool.iter().enumerate() {
                // Deliberately includes a hard paraphrase and a similar different referent.
                let angle =
                    [[0.0_f32, 0.12, 0.35, 0.8], [0.65, 0.75, 1.05, 1.2]][referent][wording];
                let mut vector = vec![0.0; 9];
                vector[axis] = angle.cos();
                vector[axis + 1] = angle.sin();
                assign(&mut fixture, text, vector);
                labels.push((kind, referent, *text));
            }
        }
    }
    let provider = ControllableSimilarityEmbeddingProvider::new(fixture.clone())?;
    let mut pairs = Vec::new();
    for (i, (kind, referent, left)) in labels.iter().enumerate() {
        for (other_kind, other_referent, right) in &labels[i + 1..] {
            if kind != other_kind {
                continue;
            }
            let a = provider.vector_for_text(left)?;
            let b = provider.vector_for_text(right)?;
            let dot = a
                .iter()
                .zip(&b)
                .map(|(x, y)| f64::from(*x) * f64::from(*y))
                .sum::<f64>();
            let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            pairs.push(json!({"kind":kind,"same_referent":referent==other_referent,"left":left,"right":right,"cosine":dot/(norm(&a)*norm(&b))}));
        }
    }
    let mut distributions = Vec::new();
    for kind in ["place", "participant"] {
        let values = |same| {
            let mut scores = pairs
                .iter()
                .filter(|p| p["kind"] == kind && p["same_referent"] == same)
                .map(|p| p["cosine"].as_f64().unwrap())
                .collect::<Vec<_>>();
            scores.sort_by(f64::total_cmp);
            scores
        };
        let same = values(true);
        let different = values(false);
        let threshold_sweep = (0..=20).map(|step| {
            let threshold = f64::from(step)/20.0;
            json!({"threshold":threshold,"false_remind_rate":different.iter().filter(|s| **s>=threshold).count() as f64/different.len() as f64,
                "missed_remind_rate":same.iter().filter(|s| **s<threshold).count() as f64/same.len() as f64})
        }).collect::<Vec<_>>();
        distributions.push(json!({"kind":kind,"same_referent_cosines":same,"different_referent_cosines":different,
            "strictly_separable_band":different.last().unwrap()<same.first().unwrap(),
            "overlap_interval":[same[0].max(different[0]),same.last().unwrap().min(*different.last().unwrap())],
            "threshold_sweep":threshold_sweep}));
    }
    Ok(
        json!({"provider":"controllable_similarity","fixture":fixture,"pairs":pairs,"distributions":distributions,
        "method":"Actual vectors emitted by the existing seeded controllable-similarity provider, with authored same/different-referent pair labels used only here for scoring. Newly authored descriptions are absent from the committed real-model caches; no paid calls or cache regeneration. Geometry and overlap are authored experimental inputs, not independent evidence about language. This validates a threshold sensitivity measurement and cannot choose a production embedder's bound. Frozen real-model paraphrase embeddings are needed for that decision."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn opposed_native_ids_preserve_every_nonidentity_input() {
        let config = config();
        for reworded in [false, true] {
            let (scenario, probes) = generated_overlap(&config, reworded).unwrap();
            let root = tempfile::tempdir().unwrap();
            let binding = EmbeddingRuntimeBinding::Controllable {
                fixture: scenario
                    .embedding
                    .controllable_similarity()
                    .unwrap()
                    .clone(),
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
            };
            let runtime = ContinuityRuntime::new(root.path(), &config, binding)
                .await
                .unwrap();
            let (opposed, order) = opposed_scenario(&runtime, &scenario).await.unwrap();
            let mut normalized = serde_json::to_value(&opposed).unwrap();
            let original = serde_json::to_value(&scenario).unwrap();
            for (changed, original) in normalized["events"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(original["events"].as_array().unwrap())
            {
                changed["event_id"] = original["event_id"].clone();
            }
            assert_eq!(normalized, original);
            assert_ne!(serde_json::to_value(&opposed).unwrap(), original);
            let order = order.as_array().unwrap();
            assert_eq!(order.len(), 48);
            assert!(
                order
                    .windows(2)
                    .all(|pair| pair[0]["prepared_native_episode_id"].as_str()
                        > pair[1]["prepared_native_episode_id"].as_str()
                        && pair[0]["authored_scene_time"].as_str()
                            < pair[1]["authored_scene_time"].as_str())
            );
            if reworded {
                let family = generated(&config, &scenario, &probes).unwrap();
                let (opposed, order) = opposed_keyless(&runtime, &family).await.unwrap();
                let mut normalized = serde_json::to_value(&opposed).unwrap();
                let original = serde_json::to_value(&family).unwrap();
                for (changed, original) in normalized["writes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .zip(original["writes"].as_array().unwrap())
                {
                    changed["episode_external_id"] = original["episode_external_id"].clone();
                    changed["observation_external_id"] =
                        original["observation_external_id"].clone();
                }
                assert_eq!(normalized, original);
                assert!(order.as_array().unwrap().windows(2).all(|pair| {
                    pair[0]["prepared_native_episode_id"].as_str()
                        > pair[1]["prepared_native_episode_id"].as_str()
                        && pair[0]["authored_scene_time"].as_str()
                            < pair[1]["authored_scene_time"].as_str()
                }));
            }
        }
    }

    #[test]
    fn rewording_and_keyless_inputs_keep_labels_out_of_writes() {
        let config = config();
        let (scenario, probes) = generated_overlap(&config, true).unwrap();
        let (identical, _) = generated_overlap(&config, false).unwrap();
        let family = generated(&config, &scenario, &probes).unwrap();
        assert_eq!(family.writes.len(), 56);
        for kind in ["place", "participant"] {
            let used = (0..48)
                .map(|i| write_words(kind, i))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(used.len(), 3);
            assert!(!used.contains(probe_words(kind, true)));
            assert_eq!(
                scenario
                    .embedding
                    .controllable_similarity()
                    .unwrap()
                    .clusters[probe_words(kind, true)],
                identical
                    .embedding
                    .controllable_similarity()
                    .unwrap()
                    .clusters[probe_words(kind, false)],
                "hold the query anchor fixed while varying written descriptions"
            );
        }
        assert!(family.writes.iter().all(|w| {
            w.scene.setting.key.is_none()
                && w.speaker_entity_external_id.is_none()
                && w.scene
                    .participants
                    .iter()
                    .all(|p| p.key.is_none() && p.name.is_none())
        }));
        assert!(family.probes.iter().all(|p| {
            p.input.scene.setting.key.is_none()
                && p.input.activity.is_none()
                && p.input
                    .scene
                    .participants
                    .iter()
                    .all(|p| p.key.is_none() && p.name.is_none())
        }));
        assert!(
            family
                .writes
                .iter()
                .all(|w| !serde_json::to_string(w).unwrap().contains("gold"))
        );
        let day_probe = &family
            .probes
            .iter()
            .find(|p| p.name == "same-day-by-descriptions")
            .unwrap()
            .input;
        let observed = |index: usize| {
            json!({"selected":[{"object":{"object_type":"episode"},
            "external_id":family.writes[index].episode_external_id,
            "recorded_scene":{"time":family.writes[index].scene.time}}]})
        };
        let latest = scene_reading(&observed(47), &family, day_probe);
        assert_eq!(latest["most_recent_n_occasions"], true);
        assert_eq!(latest["same_day_occasions"], 1);
        assert_eq!(latest["available_same_day_occasions"], 4);
        let old = scene_reading(&observed(0), &family, day_probe);
        assert_eq!(old["most_recent_n_occasions"], false);
        assert_eq!(old["same_day_occasions"], 0);
        let geometry = paraphrase_geometry().unwrap();
        assert_eq!(geometry["pairs"].as_array().unwrap().len(), 56);
        for distribution in geometry["distributions"].as_array().unwrap() {
            assert_eq!(
                distribution["same_referent_cosines"]
                    .as_array()
                    .unwrap()
                    .len(),
                12
            );
            assert_eq!(
                distribution["different_referent_cosines"]
                    .as_array()
                    .unwrap()
                    .len(),
                16
            );
            let sweep = distribution["threshold_sweep"].as_array().unwrap();
            assert_eq!(sweep[0]["false_remind_rate"], 1.0);
            assert_eq!(sweep[20]["missed_remind_rate"], 1.0);
        }
    }
}
