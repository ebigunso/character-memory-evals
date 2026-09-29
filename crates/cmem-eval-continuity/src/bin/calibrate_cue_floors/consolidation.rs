//! Matched controls for keyed places and activity pressure, using public writes.
use super::*;
use cmem_eval::{MemoryThreadInput, ThreadStatus};

const HOME_TOPICS: [&str; 6] = [
    "Mending a wool scarf",
    "Growing basil",
    "Studying constellations",
    "Baking rye bread",
    "Tuning a violin",
    "Folding paper cranes",
];
const UNLIVED: &str = "Navigating a submarine";
const THREAD: &str = "archive-project";
pub(super) const METHOD: &str = "Two generated families: keyed-setting (separate home and office stores) and activity-pressure. Every home experience has the same setting key, and every home belief cites a home experience through the public write path. Six orthogonal lived topics and one unlived direction share that store; the older unique top-salience belief has salience 1.0, all other beliefs and experiences 0.5. Office probes retain six keyed people and the topic while removing only the office key. Activity probes retain the topic while adding a real thread of 24 derived members, at floors 0, native default and 2. Semantic cohorts come from authored source fields and embedding assignments; selected identities, sections, cue credit, scores, recorded scenes, and returned pack objects come from native retrieval. An unlived topic has no authored relevant memory: report native topic-bearing slots divided by all selected slots, not semantic relevance. Floor excess compares net losses within each section with the requested activity reservation; also retain added/removed identities and native stage credits, without pairing an admission to a causal eviction. Original and opposed native ID orders use the same content, times, salience and links. No production similarity threshold, behavioral pass assertion, clock, or fixture edits.";

pub(super) fn is_family(family: &Family) -> bool {
    matches!(
        family.name.as_str(),
        "keyed-setting-home" | "keyed-setting-office" | "activity-pressure"
    )
}

pub(super) fn vector(axis: usize, cosine: f32) -> Vec<f32> {
    let mut value = vec![0.0; 9];
    value[axis] = cosine;
    value[8] = (1.0 - cosine * cosine).sqrt();
    value
}

fn link(family: &mut Family, from: &str, relation: RelationType, kind: ObjectType, to: &str) {
    family.graph.links.push(MemoryLinkInput {
        external_id: format!("link-{:03}", family.graph.links.len()),
        from: MemoryEndpointInput {
            object_type: ObjectType::DerivedMemory,
            external_id: from.into(),
        },
        relation,
        to: MemoryEndpointInput {
            object_type: kind,
            external_id: to.into(),
        },
        rationale: None,
    });
}

// A derived memory's setting is formed by its source; metadata never supplies it.
pub(super) fn belief(
    family: &mut Family,
    text: String,
    place: Option<&str>,
    person: Option<&str>,
    salience: f32,
    embedding: Vec<f32>,
) {
    let n = family.graph.derived_memories.len();
    let id = format!("belief-{n:03}");
    let source = format!("occasion-{n:03}");
    let at =
        (timestamp("2025-08-01T12:00:00+09:00").unwrap() + Duration::hours(n as i64)).to_rfc3339();
    experience(family, &source, &at, false, 0.5, 0.0);
    let write = &mut family.experiences.last_mut().unwrap().write;
    write.scene.setting.key = place.map(str::to_owned);
    if let Some(person) = person {
        write.scene.participants.push(SceneParticipantInput {
            key: Some(person.into()),
            ..Default::default()
        });
    }
    // Source bodies are distinct from the belief and do not match any topic.
    let mut background = vec![0.0; 9];
    background[8] = -1.0;
    assign(&mut family.embedding, &write.content, background);
    let mut memory = derived(
        &id,
        &at,
        text.clone(),
        DerivedType::Reflection,
        person.map(|p| vec![p.into()]).unwrap_or_default(),
        salience,
    );
    memory.given_by_application = false;
    memory.source_episode_external_ids.push(source.clone());
    assign(&mut family.embedding, &text, embedding);
    family.graph.derived_memories.push(memory);
    link(
        family,
        &id,
        RelationType::DerivedFrom,
        ObjectType::Episode,
        &source,
    );
}

pub(super) fn home_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("keyed-setting-home");
    belief(
        &mut family,
        "Home is a place to leave room for quiet thought.".into(),
        Some("home"),
        None,
        1.0,
        vector(7, 1.0),
    );
    for (axis, topic) in HOME_TOPICS.iter().enumerate() {
        assign(&mut family.embedding, topic, vector(axis, 1.0));
        for n in 0..6 {
            belief(
                &mut family,
                format!("{topic}: learned detail {n}"),
                Some("home"),
                None,
                0.5,
                vector(axis, 0.9 - n as f32 * 0.04),
            );
        }
    }
    assign(&mut family.embedding, UNLIVED, vector(6, 1.0));
    for (n, topic) in HOME_TOPICS.into_iter().chain([UNLIVED]).enumerate() {
        for at_home in [false, true] {
            let mut query = input(config, &family.namespace, false, Some(topic));
            query.scene.setting.key = at_home.then(|| "home".into());
            family.probes.push(probe(
                &format!(
                    "home-topic-{n}-{}",
                    if at_home { "with-key" } else { "without-key" }
                ),
                query,
                &[],
            ));
        }
    }
    family
}

pub(super) fn office_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("keyed-setting-office");
    for person in 0..6 {
        let key = format!("person-{person}");
        family.graph.entities.push(EntityInput {
            external_id: key.clone(),
        });
        for n in 0..3 {
            belief(
                &mut family,
                format!("Person {person} prefers meeting arrangement {n}."),
                Some("cafe"),
                Some(&key),
                0.5,
                vector(7, 1.0),
            );
        }
    }
    for n in 0..12 {
        belief(
            &mut family,
            format!("Office routine {n} keeps the stationery organized."),
            Some("office"),
            None,
            0.5,
            vector(7, 1.0),
        );
    }
    topic_beliefs(&mut family, Some("office"));
    for with_office in [false, true] {
        let mut query = input(config, &family.namespace, false, Some(TOPIC));
        query.scene.participants = (0..6)
            .map(|n| SceneParticipantInput {
                key: Some(format!("person-{n}")),
                ..Default::default()
            })
            .collect();
        query.scene.setting.key = with_office.then(|| "office".into());
        family.probes.push(probe(
            if with_office {
                "six-people-with-office"
            } else {
                "six-people-without-office"
            },
            query,
            &[],
        ));
    }
    family
}

fn topic_beliefs(family: &mut Family, place: Option<&str>) {
    for n in 0..8 {
        belief(
            family,
            format!("{TOPIC}: learned detail {n}"),
            place,
            None,
            0.5,
            vector(0, 0.9 - n as f32 * 0.3 / 7.0),
        );
        family.topic_targets.push(
            family
                .graph
                .derived_memories
                .last()
                .unwrap()
                .external_id
                .clone(),
        );
    }
}

pub(super) fn activity_family(config: &BenchmarkRunConfig) -> Family {
    let mut family = empty("activity-pressure");
    family.graph.threads.push(MemoryThreadInput {
        external_id: THREAD.into(),
        title: "Sorting the archive".into(),
        summary: String::new(),
        status: ThreadStatus::Active,
        last_touched_at: Some(EVENING.into()),
        salience_score: 0.5,
        canonical_key: Some(THREAD.into()),
    });
    assign(&mut family.embedding, "Sorting the archive", vector(7, 1.0));
    for n in 0..24 {
        belief(
            &mut family,
            format!("Archive shelf {n} needs its catalogue checked."),
            None,
            None,
            0.5,
            vector(7, 1.0),
        );
        let memory = family.graph.derived_memories.last_mut().unwrap();
        memory.thread_external_ids.push(THREAD.into());
        let id = memory.external_id.clone();
        link(
            &mut family,
            &id,
            RelationType::PartOfThread,
            ObjectType::MemoryThread,
            THREAD,
        );
    }
    topic_beliefs(&mut family, None);
    family.probes.push(probe(
        "topic-alone",
        input(config, &family.namespace, false, Some(TOPIC)),
        &[],
    ));
    for floor in [0, 1, 2] {
        let mut query = input(config, &family.namespace, false, Some(TOPIC));
        query.activity = Some(ActivityInput::Thread(THREAD.into()));
        query.cue_floors = Some(floors("activity", floor));
        family.probes.push(probe(
            &format!("topic-with-activity-floor-{floor}"),
            query,
            &[],
        ));
    }
    family
}

fn selected<'a>(observed: &'a Value, ids: &[String]) -> Vec<&'a Value> {
    observed["selected"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| ids.iter().any(|id| s["external_id"] == *id))
        .collect()
}

pub(super) fn composition(family: &Family, query: &RetrieveInput, observed: &Value) -> Value {
    let all = observed["selected"].as_array().unwrap();
    let people = query.scene.participants.iter().filter_map(|p| p.key.as_ref()).map(|person| {
        let ids = family.graph.derived_memories.iter().filter(|m| m.entity_external_ids.contains(person)).map(|m| m.external_id.clone()).collect::<Vec<_>>();
        let kept = selected(observed, &ids);
        json!({"person":person,"authored_state_ids":ids,"selected_count":kept.len(),"selected":kept})
    }).collect::<Vec<_>>();
    let top = family
        .graph
        .derived_memories
        .iter()
        .max_by(|a, b| a.salience_score.total_cmp(&b.salience_score))
        .unwrap();
    let topical = family
        .graph
        .derived_memories
        .iter()
        .filter(|m| {
            query
                .topic
                .as_ref()
                .is_some_and(|topic| m.text.starts_with(&format!("{topic}: learned detail ")))
        })
        .map(|m| m.external_id.clone())
        .collect::<Vec<_>>();
    let on_topic = selected(observed, &topical);
    let native_topic = all
        .iter()
        .filter(|s| {
            s["cue_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k == "topic")
        })
        .collect::<Vec<_>>();
    let members = family
        .graph
        .derived_memories
        .iter()
        .filter(|m| m.thread_external_ids.iter().any(|id| id == THREAD))
        .map(|m| m.external_id.clone())
        .collect::<Vec<_>>();
    json!({"all_pack_slots":all.len(),"people":people,"on_topic_authored_ids":topical,
        "on_topic_selected_count":on_topic.len(),"on_topic_selected":on_topic,
        "native_topic_slots":native_topic.len(),"native_topic_share":(!all.is_empty()).then(|| native_topic.len() as f64 / all.len() as f64),
        "native_topic_records":native_topic,"unlived_topic":query.topic.as_deref()==Some(UNLIVED),
        "unique_top_salience_belief":if family.name=="keyed-setting-home" { json!({"authored_id":top.external_id,"authored_salience":top.salience_score,"selected":all.iter().any(|s| s["external_id"]==top.external_id)}) } else { Value::Null },
        "authored_thread_members":members,"selected_thread_members":selected(observed,&members)})
}

fn comparisons(family: &Family, rows: &[Value]) -> Value {
    if family.name == "keyed-setting-home" {
        let lived = rows
            .iter()
            .filter(|r| {
                r["input"]["scene"]["setting"]["key"] == "home"
                    && r["composition"]["unlived_topic"] == false
            })
            .collect::<Vec<_>>();
        let repeated = lived
            .iter()
            .filter(|r| r["composition"]["unique_top_salience_belief"]["selected"] == true)
            .count();
        let unlived = rows
            .iter()
            .filter(|r| r["composition"]["unlived_topic"] == true)
            .collect::<Vec<_>>();
        return json!({"lived_home_topic_count":lived.len(),"top_salience_belief_pack_count":repeated,
            "same_top_salience_belief_in_every_lived_topic_pack":!lived.is_empty() && repeated==lived.len(),
            "unlived_topic_controls":unlived,
            "falsifier":"The same top-salience home belief appears in every lived-topic pack, or AFTER unlived-topic share exceeds the same-order BEFORE share. A single run reports the share; it cannot decide the cross-pin comparison."});
    }
    let control = &rows[0];
    let paired = rows[1..].iter().map(|row| {
        let mut result = json!({"probe":row["probe"],"control_probe":control["probe"],
            "pack_delta":pack_delta(&control["observed"],&row["observed"])});
        if family.name == "keyed-setting-office" {
            result["people"] = json!(row["composition"]["people"].as_array().unwrap().iter().map(|person| {
                let prior = control["composition"]["people"].as_array().unwrap().iter().find(|p| p["person"]==person["person"]).unwrap();
                json!({"person":person["person"],"without_office":prior["selected_count"],"with_office":person["selected_count"],
                    "shrunk":person["selected_count"].as_u64().unwrap()<prior["selected_count"].as_u64().unwrap()})
            }).collect::<Vec<_>>());
        } else {
            let floor = row["input"]["cue_floors"]["activity"].as_u64().unwrap();
            let old = control["composition"]["on_topic_selected"].as_array().unwrap();
            let new = row["composition"]["on_topic_selected"].as_array().unwrap();
            let sections = old.iter().map(|s| s["section"].as_str().unwrap()).collect::<std::collections::BTreeSet<_>>();
            result["activity_floor"] = json!(floor);
            result["on_topic_by_section"] = json!(sections.iter().map(|section| {
                let before = old.iter().filter(|s| s["section"]==**section).count();
                let after = new.iter().filter(|s| s["section"]==**section).count();
                let lost = before.saturating_sub(after);
                json!({"section":section,"topic_alone":before,"with_activity":after,"net_loss":lost,
                    "loss_beyond_activity_reservation":lost.saturating_sub(floor as usize),
                    "exceeds_activity_reservation":lost>floor as usize})
            }).collect::<Vec<_>>());
        }
        result
    }).collect::<Vec<_>>();
    json!({"matched_controls":paired})
}

pub(super) async fn measure(runtime: &ContinuityRuntime, family: &Family) -> Result<Value> {
    let mut rows = Vec::new();
    for probe in &family.probes {
        let query = &probe.supported_input;
        let pack = runtime.adapter().retrieve(query.clone()).await?;
        let observed = snapshot(&pack, query)?;
        let returned = pack.outcomes()[0].pack.derived_memories.iter().map(|included| {
            let memory = &included.memory;
            json!({"native_id":memory.id,"external_id":pack.object_refs().get(&memory.id.to_string()).map(|r| &r.external_id),
                "text":memory.text,"salience":memory.salience_score,"created_at":memory.created_at,
                "subject_ids":memory.entity_ids,"thread_ids":memory.thread_ids,"source_episode_ids":memory.derived_from_episode_ids})
        }).collect::<Vec<_>>();
        for memory in &returned {
            let authored = family
                .graph
                .derived_memories
                .iter()
                .find(|m| memory["external_id"] == m.external_id)
                .context("returned belief lacks authored identity")?;
            ensure!(
                memory["salience"] == json!(authored.salience_score),
                "returned belief salience differs from authored input"
            );
        }
        rows.push(json!({"probe":probe.name,"status":"executed","input":query,
            "composition":composition(family,query,&observed),"observed":observed,
            "returned_beliefs":returned}));
    }
    Ok(json!({"comparisons":comparisons(family,&rows),"rows":rows}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_falsifier_counts_net_loss_beyond_each_section_reservation() {
        let family = activity_family(&config());
        let row = |count: usize, floor: usize| {
            let selected = family.topic_targets.iter().take(count).map(|id| json!({
                "external_id":id,"object":{"id":id,"object_type":"derived_memory"},
                "section":"derived_memories","rank":0,"section_score_components":{},"cue_kinds":["topic"]
            })).collect::<Vec<_>>();
            json!({"probe":"test","input":{"cue_floors":{"activity":floor}},
                "observed":{"selected":selected,"roots":[],"candidates":[]},
                "composition":{"on_topic_selected":selected}})
        };
        let read = comparisons(&family, &[row(3, 0), row(1, 1), row(2, 1), row(2, 0)]);
        let pairs = read["matched_controls"].as_array().unwrap();
        assert_eq!(
            pairs[0]["on_topic_by_section"][0]["loss_beyond_activity_reservation"],
            1
        );
        assert_eq!(
            pairs[1]["on_topic_by_section"][0]["exceeds_activity_reservation"],
            false
        );
        assert_eq!(
            pairs[2]["on_topic_by_section"][0]["exceeds_activity_reservation"],
            true
        );
    }

    #[test]
    fn generated_controls_preserve_cues_and_permutation_preserves_membership() {
        let config = config();
        let home = home_family(&config);
        assert_eq!(home.probes.len(), 14);
        assert!(
            home.experiences
                .iter()
                .all(|e| e.write.scene.setting.key.as_deref() == Some("home"))
        );
        assert_eq!(
            home.graph
                .derived_memories
                .iter()
                .filter(|m| m.salience_score == 1.0)
                .count(),
            1
        );
        let provider =
            ControllableSimilarityEmbeddingProvider::new(home.embedding.clone()).unwrap();
        let absent = provider.vector_for_text(UNLIVED).unwrap();
        for m in &home.graph.derived_memories {
            assert!(!m.given_by_application && m.source_episode_external_ids.len() == 1);
            assert!(
                super::super::super::descriptions::cosine(
                    &absent,
                    &provider.vector_for_text(&m.text).unwrap()
                )
                .abs()
                    < 0.001
            );
        }
        let office = office_family(&config);
        let mut keyed = office.probes[1].supported_input.clone();
        keyed.scene.setting.key = None;
        assert_eq!(
            serde_json::to_value(keyed).unwrap(),
            serde_json::to_value(&office.probes[0].supported_input).unwrap()
        );
        assert_eq!(office.probes[0].supported_input.scene.participants.len(), 6);
        let activity = activity_family(&config);
        assert_eq!(
            activity
                .graph
                .derived_memories
                .iter()
                .filter(|m| !m.thread_external_ids.is_empty())
                .count(),
            24
        );
        assert_eq!(activity.topic_targets.len(), 8);
        for original in [home, office, activity] {
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
                .map(|(n, id)| (id, format!("native-{n:04}")))
                .collect();
            let (permuted, order) = opposed(&original, &ids).unwrap();
            for (old, new) in original
                .graph
                .derived_memories
                .iter()
                .zip(&permuted.graph.derived_memories)
            {
                assert_eq!(old.text, new.text);
                assert_eq!(old.salience_score, new.salience_score);
                assert_eq!(old.thread_external_ids, new.thread_external_ids);
                assert_eq!(old.entity_external_ids, new.entity_external_ids);
                let source = |f: &Family, id: &str| {
                    f.experiences
                        .iter()
                        .find(|e| e.write.episode_external_id == id)
                        .unwrap()
                        .write
                        .content
                        .clone()
                };
                assert_eq!(
                    source(&original, &old.source_episode_external_ids[0]),
                    source(&permuted, &new.source_episode_external_ids[0])
                );
            }
            for kind in ["episode", "derived_memory"] {
                let rows = order
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["object_type"] == kind)
                    .collect::<Vec<_>>();
                assert!(
                    rows.windows(2)
                        .all(|w| w[0]["native_id_from_original_ingest"].as_str()
                            > w[1]["native_id_from_original_ingest"].as_str())
                );
            }
            for link in &permuted.graph.links {
                assert!(
                    permuted
                        .graph
                        .derived_memories
                        .iter()
                        .any(|m| m.external_id == link.from.external_id)
                );
                assert!(if link.to.object_type == ObjectType::MemoryThread {
                    link.to.external_id == THREAD
                } else {
                    permuted
                        .experiences
                        .iter()
                        .any(|e| e.write.episode_external_id == link.to.external_id)
                });
            }
        }
    }
}
