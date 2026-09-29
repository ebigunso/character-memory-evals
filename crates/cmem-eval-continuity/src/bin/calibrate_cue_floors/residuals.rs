//! Additive consolidation readings; existing stores and probes remain unchanged.
use super::*;
use consolidation::{belief, vector};
use std::collections::BTreeSet;

#[derive(Clone, Serialize)]
struct ResidualFamily {
    family: Family,
    observations: Vec<ObservationInput>,
}

fn familiar(config: &BenchmarkRunConfig) -> ResidualFamily {
    let mut family = empty("familiar-salient-remarks");
    for n in 0..8 {
        belief(
            &mut family,
            format!("Iris values dependable habit {n}."),
            None,
            Some("iris"),
            0.95,
            vector(7, 1.0),
        );
    }
    for n in 0..32 {
        let at =
            (timestamp("2025-09-08T12:00:00+09:00").unwrap() + Duration::minutes(n)).to_rfc3339();
        experience(&mut family, &format!("remark-{n:02}"), &at, true, 1.0, -0.1);
        let write = &mut family.experiences.last_mut().unwrap().write;
        write.content = format!("Iris remarked on passing detail {n} during the conversation.");
        write.speaker_entity_external_id = Some("iris".into());
        assign(&mut family.embedding, &write.content, vector(7, 1.0));
    }
    family.probes.push(probe(
        "present-person-no-topic",
        input(config, &family.namespace, true, None),
        &[],
    ));
    ResidualFamily {
        family,
        observations: vec![],
    }
}

fn busy(config: &BenchmarkRunConfig) -> ResidualFamily {
    let mut family = empty("busy-occasion");
    experience(
        &mut family,
        "busy-occasion",
        "2025-08-01T12:00:00+09:00",
        false,
        0.5,
        -0.1,
    );
    family.experiences[0].write.scene.setting.words =
        Some("A quiet room lined with cedar shelves".into());
    assign(
        &mut family.embedding,
        "A quiet room lined with cedar shelves",
        vector(1, 1.0),
    );
    assign(
        &mut family.embedding,
        "The peaceful cedar-shelved reading room",
        vector(1, 0.9),
    );
    // Background recency has newer occasions; a scene reminder must reach the old one.
    for n in 0..20 {
        let at =
            (timestamp("2025-09-09T12:00:00+09:00").unwrap() + Duration::minutes(n)).to_rfc3339();
        experience(
            &mut family,
            &format!("newer-unrelated-{n:02}"),
            &at,
            false,
            0.5,
            -0.1,
        );
    }
    let mut observations = Vec::new();
    for n in 1..=24 {
        let text = format!("Busy occasion detail {n:02} was noticed beside the shelves.");
        assign(&mut family.embedding, &text, vector(7, 1.0));
        observations.push(ObservationInput {
            external_id: format!("busy-detail-{n:02}"),
            episode_external_id: "busy-occasion".into(),
            namespace: family.namespace.clone(),
            speaker: None,
            text,
            observed_at: Some(
                (timestamp("2025-08-01T12:00:00+09:00").unwrap() + Duration::minutes(n))
                    .to_rfc3339(),
            ),
            metadata: Value::Null,
        });
    }
    for reminder in [false, true] {
        let mut query = input(config, &family.namespace, false, None);
        query.scene.setting.words =
            reminder.then(|| "The peaceful cedar-shelved reading room".into());
        family.probes.push(probe(
            if reminder {
                "description-reminder"
            } else {
                "time-only-control"
            },
            query,
            &[],
        ));
    }
    ResidualFamily {
        family,
        observations,
    }
}

fn activity(config: &BenchmarkRunConfig) -> ResidualFamily {
    let mut family = consolidation::activity_family(config);
    family.name = "activity-people-pressure".into();
    for n in 0..2 {
        let person = format!("present-person-{n}");
        family.graph.entities.push(EntityInput {
            external_id: person.clone(),
        });
        for detail in 0..4 {
            belief(
                &mut family,
                format!("Person {n} prefers working arrangement {detail}."),
                None,
                Some(&person),
                0.5,
                vector(7, 1.0),
            );
        }
    }
    for probe in &mut family.probes {
        probe.name = format!("people-{}", probe.name);
        probe.supported_input.scene.participants = (0..2)
            .map(|n| SceneParticipantInput {
                key: Some(format!("present-person-{n}")),
                ..Default::default()
            })
            .collect();
    }
    ResidualFamily {
        family,
        observations: vec![],
    }
}

fn old_place(config: &BenchmarkRunConfig) -> ResidualFamily {
    let mut family = consolidation::home_family(config);
    family.name = "old-salient-place".into();
    family.probes.clear();
    for place in [false, true] {
        let mut query = input(config, &family.namespace, false, None);
        query.scene.setting.key = place.then(|| "home".into());
        family.probes.push(probe(
            if place {
                "keyed-place-no-topic"
            } else {
                "time-only-control"
            },
            query,
            &[],
        ));
    }
    family.probes.push(probe(
        "old-belief-topic-control",
        input(
            config,
            &family.namespace,
            false,
            Some(&family.graph.derived_memories[0].text),
        ),
        &[],
    ));
    ResidualFamily {
        family,
        observations: vec![],
    }
}

fn beliefs_only(family: &Family) -> Family {
    let mut control = family.clone();
    let sources = family
        .graph
        .derived_memories
        .iter()
        .flat_map(|m| &m.source_episode_external_ids)
        .collect::<BTreeSet<_>>();
    control
        .experiences
        .retain(|e| sources.contains(&e.write.episode_external_id));
    control
}

async fn ingest_extra(
    runtime: &ContinuityRuntime,
    input: &ResidualFamily,
) -> Result<BTreeMap<String, String>> {
    let mut ids = BTreeMap::new();
    let mut links = Vec::new();
    for observation in &input.observations {
        let result = runtime
            .adapter()
            .remember_observation(observation.clone())
            .await?;
        healthy(&result.outcome, 1)?;
        ids.insert(observation.external_id.clone(), result.value);
        links.push(MemoryLinkInput {
            external_id: format!("busy-link-{}", observation.external_id),
            from: MemoryEndpointInput {
                object_type: ObjectType::Episode,
                external_id: observation.episode_external_id.clone(),
            },
            relation: RelationType::HasObservation,
            to: MemoryEndpointInput {
                object_type: ObjectType::Observation,
                external_id: observation.external_id.clone(),
            },
            rationale: None,
        });
    }
    if !links.is_empty() {
        let count = links.len();
        let result = runtime
            .adapter()
            .remember_enrichment(GraphEnrichmentInput {
                namespace: input.family.namespace.clone(),
                links,
                ..Default::default()
            })
            .await?
            .context("missing busy observation links")?;
        healthy(&result, 0)?;
        ensure!(
            result.persisted_link_ids.len() == count,
            "busy observation links not all persisted"
        );
    }
    Ok(ids)
}

// Oppose all 25 observations of the busy occasion, including its prepared companion.
fn opposed_observations(
    original: &ResidualFamily,
    next: &mut ResidualFamily,
    ids: &BTreeMap<String, String>,
) -> Value {
    if original.observations.is_empty() {
        return json!([]);
    }
    // Episode opposition also renames companions. Restore that independent population
    // before permuting the busy observations, or a newer companion can reuse an ID.
    for (old, new) in original
        .family
        .experiences
        .iter()
        .zip(&mut next.family.experiences)
    {
        new.write.observation_external_id = old.write.observation_external_id.clone();
    }
    let original_busy = &original.family.experiences[0].write;
    let mut population = vec![(
        original_busy.observation_external_id.clone(),
        original_busy.observation_observed_at.clone().unwrap(),
    )];
    population.extend(
        original
            .observations
            .iter()
            .map(|o| (o.external_id.clone(), o.observed_at.clone().unwrap())),
    );
    population.sort_by_key(|(_, at)| timestamp(at).unwrap());
    let mut assignment = population
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    assignment.sort_by(|a, b| ids[b].cmp(&ids[a]));
    let mut permutation = BTreeMap::new();
    let evidence = population.into_iter().zip(assignment).map(|((old, at), new)| {
        permutation.insert(old.clone(), new.clone());
        json!({"object_type":"observation","original_external_id":old,"external_id":new,"authored_time":at,"native_id_from_original_ingest":ids[&new]})
    }).collect::<Vec<_>>();
    next.family.experiences[0].write.observation_external_id =
        permutation[&original_busy.observation_external_id].clone();
    let episode = next.family.experiences[0].write.episode_external_id.clone();
    for (old, new) in original.observations.iter().zip(&mut next.observations) {
        new.external_id = permutation[&old.external_id].clone();
        new.episode_external_id = episode.clone();
    }
    json!(evidence)
}

async fn measure(
    runtime: &ContinuityRuntime,
    spec: &ResidualFamily,
    condition: &str,
) -> Result<Value> {
    let family = &spec.family;
    let mut rows = Vec::new();
    for probe in &family.probes {
        let input = &probe.supported_input;
        let pack = runtime.adapter().retrieve(input.clone()).await?;
        let observed = snapshot(&pack, input)?;
        ensure!(
            observed["telemetry"]["graph_expansion"]["bounded_failure_count"] == 0,
            "degraded residual recall"
        );
        ensure!(
            matches!(
                observed["telemetry"]["vector_recall_completeness"]["kind"].as_str(),
                Some("exhaustive" | "not_requested")
            ),
            "incomplete residual vector recall"
        );
        for observation in &pack.outcomes()[0].pack.salient_observations {
            let external = &pack.object_refs()[&observation.id.to_string()].external_id;
            let (episode, at, salience) = if let Some(extra) = spec
                .observations
                .iter()
                .find(|o| &o.external_id == external)
            {
                (
                    &extra.episode_external_id,
                    extra.observed_at.as_deref().unwrap(),
                    0.5,
                )
            } else {
                let write = &family
                    .experiences
                    .iter()
                    .find(|e| &e.write.observation_external_id == external)
                    .context("returned observation is not authored")?
                    .write;
                (
                    &write.episode_external_id,
                    write.observation_observed_at.as_deref().unwrap(),
                    write.salience.unwrap(),
                )
            };
            ensure!(
                observation.observed_at == Some(timestamp(at)?.with_timezone(&Utc)),
                "returned observation time changed"
            );
            ensure!(
                observation.salience_score == salience,
                "returned observation salience changed"
            );
            ensure!(
                pack.object_refs()[&observation.episode_id.to_string()].external_id == *episode,
                "returned observation source changed"
            );
        }
        for belief in &pack.outcomes()[0].pack.derived_memories {
            let external = &pack.object_refs()[&belief.memory.id.to_string()].external_id;
            let authored = family
                .graph
                .derived_memories
                .iter()
                .find(|m| &m.external_id == external)
                .context("returned belief is not authored")?;
            ensure!(
                belief.memory.salience_score == authored.salience_score,
                "returned belief salience changed"
            );
        }
        let observations = pack.outcomes()[0].pack.salient_observations.iter().map(|o| json!({
            "native_id":o.id,"external_id":pack.object_refs().get(&o.id.to_string()).map(|r| &r.external_id),
            "episode_id":o.episode_id,"text":o.text,"observed_at":o.observed_at,"salience":o.salience_score
        })).collect::<Vec<_>>();
        let beliefs = pack.outcomes()[0].pack.derived_memories.iter().map(|b| json!({
            "native_id":b.memory.id,"external_id":pack.object_refs().get(&b.memory.id.to_string()).map(|r| &r.external_id),
            "text":b.memory.text,"salience":b.memory.salience_score,"source_episode_ids":b.memory.derived_from_episode_ids,"subject_ids":b.memory.entity_ids
        })).collect::<Vec<_>>();
        let selected = observed["selected"].as_array().unwrap();
        let old = (family.name == "old-salient-place").then(|| &family.graph.derived_memories[0]);
        let busy_id = &family.experiences[0].write.episode_external_id;
        let busy_native = pack
            .object_refs()
            .iter()
            .find(|(_, r)| r.external_id == *busy_id)
            .map(|(id, _)| id);
        let busy_selected = observations
            .iter()
            .filter(|o| busy_native.is_some_and(|id| o["episode_id"] == *id))
            .collect::<Vec<_>>();
        rows.push(json!({"probe":probe.name,"condition":condition,"status":"executed","input":input,
            "composition":if family.graph.derived_memories.is_empty() { Value::Null } else { consolidation::composition(family,input,&observed) },
            "old_salient_belief":old.map(|m| json!({"authored_id":m.external_id,"authored_salience":m.salience_score,"selected":selected.iter().any(|s| s["external_id"]==m.external_id)})),
            "busy_occasion":if family.name=="busy-occasion" { json!({"authored_episode_id":busy_id,"authored_observation_count":spec.observations.len()+1,"selected_observations":busy_selected,"selected_count":busy_selected.len()}) } else { Value::Null },
            "returned_observations":observations,"returned_beliefs":beliefs,"observed":observed}));
    }
    Ok(json!({"rows":rows}))
}

pub(super) async fn run(
    stores: &Path,
    config: &BenchmarkRunConfig,
    timings: &mut timing::Timings,
) -> Result<Value> {
    let mut inputs = Vec::new();
    let mut measurements = Vec::new();
    for build in [familiar, busy, activity, old_place] {
        let original = build(config);
        let mut next = original.clone();
        let mut order = Value::Null;
        let mut observation_order = Value::Null;
        for opposed_ids in [false, true] {
            let root = stores.join(format!("residual-{}-{opposed_ids}", next.family.name));
            fs::create_dir(&root)?;
            let runtime = ContinuityRuntime::new(
                &root,
                config,
                EmbeddingRuntimeBinding::Controllable {
                    fixture: next.family.embedding.clone(),
                    dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
                },
            )
            .await?;
            let result = async {
                let (ids, mut observation_ids) =
                    Box::pin(ingest_all(&runtime, &next.family)).await?;
                observation_ids.extend(ingest_extra(&runtime, &next).await?);
                if opposed_ids {
                    for (rows, ids) in [(&order, &ids), (&observation_order, &observation_ids)] {
                        for row in rows.as_array().unwrap() {
                            ensure!(
                                ids[row["external_id"].as_str().unwrap()]
                                    == row["native_id_from_original_ingest"].as_str().unwrap(),
                                "residual native ID changed between orders"
                            );
                        }
                    }
                }
                let reading = Box::pin(measure(&runtime, &next, "full-store")).await?;
                timings
                    .record(
                        &runtime,
                        &next.family.name,
                        opposed_ids,
                        next.family
                            .probes
                            .iter()
                            .map(|p| (p.name.clone(), p.supported_input.clone()))
                            .collect(),
                    )
                    .await?;
                Ok::<_, anyhow::Error>((ids, observation_ids, reading))
            }
            .await;
            let cleanup = runtime.cleanup(&next.family.namespace).await;
            drop(runtime);
            cleanup?;
            let (ids, observation_ids, mut reading) = result?;
            let mut control_input = Value::Null;
            if next.family.name == "familiar-salient-remarks" {
                let control = ResidualFamily {
                    family: beliefs_only(&next.family),
                    observations: vec![],
                };
                let root = stores.join(format!("residual-familiar-control-{opposed_ids}"));
                fs::create_dir(&root)?;
                let runtime = ContinuityRuntime::new(
                    &root,
                    config,
                    EmbeddingRuntimeBinding::Controllable {
                        fixture: control.family.embedding.clone(),
                        dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
                    },
                )
                .await?;
                let result = async {
                    let control_ids = Box::pin(ingest(&runtime, &control.family)).await?;
                    ensure!(
                        control_ids
                            .iter()
                            .all(|(id, native)| ids.get(id) == Some(native)),
                        "matched familiar control changed an identity"
                    );
                    Box::pin(measure(&runtime, &control, "without-remarks")).await
                }
                .await;
                let cleanup = runtime.cleanup(&control.family.namespace).await;
                drop(runtime);
                cleanup?;
                let control_reading = result?;
                reading["control"] = control_reading;
                control_input = serde_json::to_value(&control)?;
            }
            inputs.push(json!({"family":next.family.name,"opposed_ids":opposed_ids,"input":next,"control_input":control_input,"id_order":order,"observation_id_order":observation_order}));
            measurements.push(json!({"family":next.family.name,"opposed_ids":opposed_ids,"native_ids":ids,"native_observation_ids":observation_ids,"reading":reading}));
            eprintln!(
                "measured residual {} opposed={opposed_ids}",
                next.family.name
            );
            if !opposed_ids {
                let (family, mapping) = opposed(&original.family, &ids)?;
                next = ResidualFamily {
                    family,
                    observations: original.observations.clone(),
                };
                order = mapping;
                observation_order = opposed_observations(&original, &mut next, &observation_ids);
            }
        }
    }
    Ok(json!({"inputs":inputs,"measurements":measurements,
        "method":"Additive public-write residual stores. Familiar control removes only remark experiences and their observations, keeping belief/source identities identical in each order. Busy occasion has 25 linked observations with distinct authored observation times; all 25 native observation IDs oppose time in the second order. Activity controls keep present people and topic while adding the thread. Old-place control targets the old belief deliberately by topic. Membership, native scores and returned observation times/salience are readings, never behavior pass/fail assertions."}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_keep_people_and_beliefs_and_busy_ids_oppose_observed_time() {
        let config = config();
        let rich = familiar(&config);
        let control = beliefs_only(&rich.family);
        assert_eq!(control.experiences.len(), 8);
        assert_eq!(
            rich.family.experiences.len() - control.experiences.len(),
            32
        );
        assert_eq!(
            serde_json::to_value(&control.graph).unwrap(),
            serde_json::to_value(&rich.family.graph).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&control.probes).unwrap(),
            serde_json::to_value(&rich.family.probes).unwrap()
        );
        let busy = busy(&config);
        let ids = std::iter::once(&busy.family.experiences[0].write.observation_external_id)
            .chain(busy.observations.iter().map(|o| &o.external_id))
            .enumerate()
            .map(|(n, id)| (id.clone(), format!("native-{n:03}")))
            .collect();
        let episode_ids = busy
            .family
            .experiences
            .iter()
            .enumerate()
            .map(|(n, e)| {
                (
                    e.write.episode_external_id.clone(),
                    format!("episode-{n:03}"),
                )
            })
            .collect();
        let mut next = ResidualFamily {
            family: opposed(&busy.family, &episode_ids).unwrap().0,
            observations: busy.observations.clone(),
        };
        let order = opposed_observations(&busy, &mut next, &ids);
        let all_observation_ids = next
            .family
            .experiences
            .iter()
            .map(|e| &e.write.observation_external_id)
            .chain(next.observations.iter().map(|o| &o.external_id))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            all_observation_ids.len(),
            next.family.experiences.len() + next.observations.len()
        );
        assert_eq!(order.as_array().unwrap().len(), 25);
        assert!(
            order
                .as_array()
                .unwrap()
                .windows(2)
                .all(|pair| pair[0]["native_id_from_original_ingest"].as_str()
                    > pair[1]["native_id_from_original_ingest"].as_str())
        );
        for (a, b) in busy.observations.iter().zip(&next.observations) {
            assert_eq!(a.text, b.text);
            assert_eq!(a.observed_at, b.observed_at);
        }
        let activity = activity(&config);
        for probe in &activity.family.probes[1..] {
            let mut input = probe.supported_input.clone();
            input.activity = None;
            input.cue_floors = None;
            assert_eq!(
                serde_json::to_value(input).unwrap(),
                serde_json::to_value(&activity.family.probes[0].supported_input).unwrap()
            );
        }
    }
}
