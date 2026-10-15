//! Additive matched removals and a fixed size/skew cost instrument; gold stays outside this module.
use super::*;
use serde::Deserialize;

const CONTROLS: &str = include_str!("../../../fixtures/situated-before-controls.json");
const COST: &str = include_str!("../../../fixtures/situated-cost-store.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Controls {
    controls: Vec<Control>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    family: String,
    parent: String,
    remove: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cost {
    seed: u64,
    sizes: Vec<usize>,
    start: String,
    hub_percent: usize,
    hub_people: usize,
    hub_places: usize,
    tail_people: usize,
    tail_places: usize,
}

#[derive(Debug)]
struct InvalidControl(String);

impl std::fmt::Display for InvalidControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid matched control: {}", self.0)
    }
}

impl std::error::Error for InvalidControl {}

fn removed(input: &RetrieveInput, cue: &str) -> Result<RetrieveInput> {
    let mut result = input.clone();
    let present = match cue {
        "topic" => result.topic.take().is_some(),
        "participants" => {
            let present = !result.scene.participants.is_empty();
            result.scene.participants.clear();
            present
        }
        "place_description" => result.scene.setting.words.take().is_some(),
        cue if cue.starts_with("participant:") => {
            let index = cue[12..].parse::<usize>().ok();
            if let Some(index) = index.filter(|i| *i < result.scene.participants.len()) {
                result.scene.participants.remove(index);
                true
            } else {
                false
            }
        }
        _ => false,
    };
    if !present {
        return Err(InvalidControl(cue.into()).into());
    }
    Ok(result)
}

fn queries(
    family: &str,
    probes: &[(String, RetrieveInput)],
    controls: &Controls,
) -> Result<Vec<(String, RetrieveInput)>> {
    let mut result = Vec::new();
    for control in controls.controls.iter().filter(|c| c.family == family) {
        let parent = &probes
            .iter()
            .find(|(name, _)| *name == control.parent)
            .ok_or_else(|| InvalidControl(format!("{family}/{} missing parent", control.parent)))?
            .1;
        result.push((format!("{}/parent", control.parent), parent.clone()));
        for cue in &control.remove {
            result.push((
                format!("{}/without-{cue}", control.parent),
                removed(parent, cue)?,
            ));
        }
    }
    ensure!(!result.is_empty(), "no controls for {family}");
    Ok(result)
}

async fn capture(
    runtime: &ContinuityRuntime,
    queries: &[(String, RetrieveInput)],
) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for (name, input) in queries {
        let pack = runtime.adapter().retrieve(input.clone()).await?;
        let observed = snapshot(&pack, input)?;
        ensure!(
            observed["telemetry"]["graph_expansion"]["bounded_failure_count"] == 0,
            "degraded matched recall: {name}"
        );
        ensure!(
            matches!(
                observed["telemetry"]["vector_recall_completeness"]["kind"].as_str(),
                Some("exhaustive" | "not_requested")
            ),
            "incomplete matched recall: {name}"
        );
        rows.push(json!({"probe":name,"status":"executed","input":input,
            "input_sha256":text_sha256(&serde_json::to_string(input)?),"observed":observed}));
    }
    Ok(rows)
}

fn families(config: &BenchmarkRunConfig) -> Vec<Family> {
    vec![
        consolidation::office_family(config),
        obligations::meeting(config),
        residuals::activity(config).family,
        residuals::familiar(config).family,
        obligations::due_priority(config),
        consolidation::home_family(config),
    ]
}

fn planned_queries(family: &Family) -> Result<Vec<(String, RetrieveInput)>> {
    family
        .probes
        .iter()
        .map(|probe| {
            let (input, missing) =
                supported_probe_input(probe, false, prospective_fields_forwarded(family))?;
            if !missing.is_empty() {
                return Err(InvalidControl(format!(
                    "{}/{} unavailable: {missing:?}",
                    family.name, probe.name
                ))
                .into());
            }
            Ok((probe.name.clone(), input))
        })
        .collect()
}

fn cost_family(config: &BenchmarkRunConfig, spec: &Cost, size: usize) -> Result<Family> {
    ensure!(
        size > 0
            && spec.hub_percent > 0
            && spec.hub_percent < 100
            && [
                spec.hub_people,
                spec.hub_places,
                spec.tail_people,
                spec.tail_places
            ]
            .iter()
            .all(|n| *n > 0),
        "invalid fixed cost size/skew"
    );
    let mut family = empty("situated-cost");
    family.embedding.seed = spec.seed;
    family.graph.entities = (0..spec.hub_people + spec.tail_people)
        .map(|n| EntityInput {
            external_id: format!("cost-person-{n}"),
        })
        .collect();
    let start = timestamp(&spec.start)?;
    for n in 0..size {
        // The seed rotates a 100-day 80/20 schedule; hub keys and tail keys are disjoint.
        let slot = n as u64 + spec.seed;
        let hub = slot % 100 < spec.hub_percent as u64;
        let person = if hub {
            slot as usize % spec.hub_people
        } else {
            spec.hub_people + slot as usize % spec.tail_people
        };
        let place = if hub {
            slot as usize % spec.hub_places
        } else {
            spec.hub_places + slot as usize % spec.tail_places
        };
        let at = (start + Duration::days(n as i64)).to_rfc3339();
        experience(
            &mut family,
            &format!("cost-day-{n:04}"),
            &at,
            false,
            0.5,
            if n % 17 == 0 { 0.9 } else { -0.1 },
        );
        let write = &mut family.experiences.last_mut().unwrap().write;
        write.scene.setting.key = Some(format!("cost-place-{place}"));
        write.scene.participants.push(SceneParticipantInput {
            key: Some(format!("cost-person-{person}")),
            ..Default::default()
        });
    }
    family.name = format!("situated-cost-{size}");
    for (name, person, place, topic) in [
        ("empty", false, false, false),
        ("hub-person", true, false, false),
        ("hub-place", false, true, false),
        ("hub-people-place-topic", true, true, true),
    ] {
        let mut query = input(config, &family.namespace, false, topic.then_some(TOPIC));
        query.scene.time = Some((start + Duration::days(size as i64)).to_rfc3339());
        query.scene.setting.key = place.then(|| "cost-place-0".into());
        if person {
            query.scene.participants.push(SceneParticipantInput {
                key: Some("cost-person-0".into()),
                ..Default::default()
            });
        }
        family.probes.push(probe(name, query, &[]));
    }
    Ok(family)
}

fn skew(family: &Family) -> Value {
    let mut people = BTreeMap::<String, usize>::new();
    let mut places = BTreeMap::<String, usize>::new();
    for experience in &family.experiences {
        for person in &experience.write.scene.participants {
            *people.entry(person.key.clone().unwrap()).or_default() += 1;
        }
        *places
            .entry(experience.write.scene.setting.key.clone().unwrap())
            .or_default() += 1;
    }
    json!({"experiences":family.experiences.len(),"companion_observations":family.experiences.len(),
        "first_scene":family.experiences.first().unwrap().write.scene.time,
        "last_scene":family.experiences.last().unwrap().write.scene.time,
        "people_experience_counts":people,"places_experience_counts":places})
}

async fn run_family(
    stores: &Path,
    config: &BenchmarkRunConfig,
    original: &Family,
    controls: Option<&Controls>,
    timings: &mut timing::Timings,
) -> Result<Value> {
    let mut next = original.clone();
    let mut order = Value::Null;
    let mut runs = Vec::new();
    for opposed_ids in [false, true] {
        let root = stores.join(format!("{}-{opposed_ids}", original.name));
        fs::create_dir(&root)?;
        let runtime = ContinuityRuntime::new(
            &root,
            config,
            EmbeddingRuntimeBinding::Controllable {
                fixture: next.embedding.clone(),
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
            },
            next.character_entity.as_deref(),
        )
        .await?;
        let result = async {
            let ids = Box::pin(ingest(&runtime, &next)).await?;
            if opposed_ids {
                for row in order.as_array().unwrap() {
                    ensure!(
                        ids[row["external_id"].as_str().unwrap()]
                            == row["native_id_from_original_ingest"],
                        "native ID changed between orders"
                    );
                }
            }
            let probes = planned_queries(&next)?;
            let queries = match controls {
                Some(controls) => queries(&next.name, &probes, controls)?,
                None => probes,
            };
            let rows = Box::pin(capture(&runtime, &queries)).await?;
            timings
                .record(&runtime, &next.name, opposed_ids, queries)
                .await?;
            Ok::<_, anyhow::Error>((ids, rows))
        }
        .await;
        let cleanup = runtime.cleanup(&next.namespace).await;
        drop(runtime);
        cleanup?;
        let (ids, rows) = result?;
        runs.push(json!({"opposed_ids":opposed_ids,"input":next,
            "input_sha256":text_sha256(&serde_json::to_string(&next)?),
            "id_order":order,"native_ids":ids,"rows":rows}));
        eprintln!(
            "measured situated-before {} opposed={opposed_ids}",
            original.name
        );
        if !opposed_ids {
            (next, order) = opposed(original, &ids)?;
        }
    }
    Ok(json!({"family":original.name,"runs":runs}))
}

async fn run_keyless(
    stores: &Path,
    config: &BenchmarkRunConfig,
    controls: &Controls,
    timings: &mut timing::Timings,
) -> Result<Value> {
    let (scenario, probes) = generated_overlap(config, true)?;
    let original = descriptions::generated(config, &scenario, &probes)?;
    let mut next = original.clone();
    let mut order = Value::Null;
    let mut runs = Vec::new();
    for opposed_ids in [false, true] {
        let root = stores.join(format!("keyless-{opposed_ids}"));
        fs::create_dir(&root)?;
        let runtime = ContinuityRuntime::new(
            &root,
            config,
            EmbeddingRuntimeBinding::Controllable {
                fixture: next.embedding.clone(),
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 9 },
            },
            None,
        )
        .await?;
        let result = async {
            let alternate = if !opposed_ids {
                Some(descriptions::opposed_keyless(&runtime, &original).await?)
            } else {
                None
            };
            Box::pin(descriptions::ingest(&runtime, &next)).await?;
            // Three existing floor-sweep copies carry the same request; capture each parent once.
            let probes = next
                .probes
                .iter()
                .filter(|p| p.measured_kind == "topic")
                .map(|p| (p.name.clone(), p.input.clone()))
                .collect::<Vec<_>>();
            let queries = queries("keyless", &probes, controls)?;
            let rows = Box::pin(capture(&runtime, &queries)).await?;
            timings
                .record(&runtime, "keyless", opposed_ids, queries)
                .await?;
            Ok::<_, anyhow::Error>((rows, alternate))
        }
        .await;
        let cleanup = runtime.cleanup(&next.namespace).await;
        drop(runtime);
        cleanup?;
        let (rows, alternate) = result?;
        runs.push(json!({"opposed_ids":opposed_ids,"input":next,
            "input_sha256":text_sha256(&serde_json::to_string(&next)?),"id_order":order,"rows":rows}));
        eprintln!("measured situated-before keyless opposed={opposed_ids}");
        if let Some(alternate) = alternate {
            (next, order) = alternate;
        }
    }
    Ok(json!({"family":"keyless","runs":runs}))
}

pub(crate) async fn run(output: &Path, timing_output: &Path) -> Result<()> {
    let controls: Controls = serde_json::from_str(CONTROLS)?;
    let cost: Cost = serde_json::from_str(COST)?;
    let config = config();
    config.validate()?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let library = workspace.join("../CharacterMemory");
    let pins = (revision(workspace)?, revision(&library)?);
    let mut file = fs::File::create_new(output).context("new situated-before report required")?;
    let mut timing_file = fs::File::create_new(timing_output)
        .context("new situated-before timing report required")?;
    let stores = output.with_extension("stores");
    fs::create_dir(&stores).context("new owned situated-before stores required")?;
    let mut timings = timing::Timings {
        enabled: true,
        ..Default::default()
    };
    let result = async {
        let mut measurements = Vec::new();
        for mut family in families(&config) {
            forward_obligations(&mut family)?;
            measurements.push(
                Box::pin(run_family(
                    &stores,
                    &config,
                    &family,
                    Some(&controls),
                    &mut timings,
                ))
                .await?,
            );
        }
        measurements.push(Box::pin(run_keyless(&stores, &config, &controls, &mut timings)).await?);
        let mut costs = Vec::new();
        for size in &cost.sizes {
            let family = cost_family(&config, &cost, *size)?;
            let mut reading =
                Box::pin(run_family(&stores, &config, &family, None, &mut timings)).await?;
            reading["store_size_and_skew"] = skew(&family);
            costs.push(reading);
        }
        Ok::<_, anyhow::Error>((measurements, costs))
    }
    .await;
    fs::remove_dir_all(&stores).context("remove owned situated-before stores")?;
    let (measurements, costs) = result?;
    ensure!(
        (revision(workspace)?, revision(&library)?) == pins,
        "pins changed during situated-before run"
    );
    let header = json!({"harness_commit":pins.0,"library_commit":pins.1,
        "controls_sha256":text_sha256(CONTROLS),"cost_spec_sha256":text_sha256(COST),
        "generator_source_sha256":text_sha256(concat!(include_str!("../calibrate_cue_floors.rs"),include_str!("descriptions.rs"),include_str!("time_and_obligations.rs"),include_str!("consolidation.rs"),include_str!("residuals.rs"),include_str!("obligations.rs"),include_str!("timing.rs"),include_str!("situated_before.rs"))),
        "config_sha256":text_sha256(&serde_json::to_string(&config)?),"config":config,
        "native_candidate_limits":native::RetrievalCandidateLimits::default(),
        "native_graph_limits":native::RetrievalGraphLimits::default(),
        "native_section_limits":native::ContinuitySectionLimits::default(),
        "native_default_floors":native::RetrievalCueFloors::default(),
        "profile":if cfg!(debug_assertions){"debug"}else{"release"},"stores_cleaned":true});
    serde_json::to_writer_pretty(
        &mut file,
        &json!({"header":header,"controls":serde_json::from_str::<Value>(CONTROLS)?,
        "cost_spec":serde_json::from_str::<Value>(COST)?,"measurements":measurements,"costs":costs,
        "method":"Each parent and its single explicit-cue removal use the same store, time, floors, limits and lifecycle policy. No route ablation; zero floor is not removal. Gold is never loaded by this command. Native traces and selected projections use the existing snapshot reader; complete native DTOs and construction timestamps are not captured. Timing is separate. Both existing identifier orders run. No behavior or speed threshold."}),
    )?;
    file.write_all(b"\n")?;
    serde_json::to_writer_pretty(
        &mut timing_file,
        &json!({"header":header,
        "method":"Three additional warm adapter.retrieve calls per recorded request; Instant excludes cloning, ingestion, serialization and validation. Individual samples and medians; no speed threshold. Cost family suffix gives experience count, with equal companion observation count; exact hub counts are in the deterministic report.","rows":timings.rows}),
    )?;
    timing_file.write_all(b"\n")?;
    eprintln!("wrote situated-before {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn controls_change_only_the_named_cue_and_keep_every_parent() {
        let config = config();
        let controls: Controls = serde_json::from_str(CONTROLS).unwrap();
        let mut names = BTreeSet::new();
        let mut all = Vec::new();
        for mut family in families(&config) {
            forward_obligations(&mut family).unwrap();
            let probes = planned_queries(&family).unwrap();
            if family.name == "prospective-meeting" {
                let input = &probes.iter().find(|p| p.0 == "iris-rowan-loud").unwrap().1;
                assert_eq!(
                    serde_json::to_value(input.cue_floors).unwrap()["trigger"],
                    1
                );
            }
            all.push((family.name, probes));
        }
        let (scenario, probes) = generated_overlap(&config, true).unwrap();
        let keyless = descriptions::generated(&config, &scenario, &probes).unwrap();
        all.push((
            "keyless".into(),
            keyless
                .probes
                .into_iter()
                .filter(|p| p.measured_kind == "topic")
                .map(|p| (p.name, p.input))
                .collect(),
        ));
        for (family, probes) in all {
            let generated = queries(&family, &probes, &controls).unwrap();
            for control in controls.controls.iter().filter(|c| c.family == family) {
                assert!(names.insert((family.clone(), control.parent.clone())));
                let parent = &probes.iter().find(|p| p.0 == control.parent).unwrap().1;
                assert_eq!(
                    serde_json::to_value(parent).unwrap(),
                    serde_json::to_value(
                        &generated
                            .iter()
                            .find(|p| p.0 == format!("{}/parent", control.parent))
                            .unwrap()
                            .1
                    )
                    .unwrap()
                );
                for cue in &control.remove {
                    let actual = removed(parent, cue).unwrap();
                    let mut expected = serde_json::to_value(parent).unwrap();
                    match cue.as_str() {
                        "topic" => expected["topic"] = Value::Null,
                        "participants" => expected["scene"]["participants"] = json!([]),
                        "place_description" => expected["scene"]["setting"]["words"] = Value::Null,
                        cue => {
                            expected["scene"]["participants"]
                                .as_array_mut()
                                .unwrap()
                                .remove(cue[12..].parse().unwrap());
                        }
                    }
                    assert_eq!(
                        serde_json::to_value(actual).unwrap(),
                        expected,
                        "{family}/{} {cue}",
                        control.parent
                    );
                }
            }
        }
        assert_eq!(names.len(), controls.controls.len());
        let query = input(&config, "test", false, None);
        for cue in [
            "topic",
            "participants",
            "participant:0",
            "place_description",
            "due",
            "recency",
        ] {
            assert!(
                removed(&query, cue)
                    .unwrap_err()
                    .downcast_ref::<InvalidControl>()
                    .is_some()
            );
        }
    }

    #[test]
    fn fixed_cost_store_is_deterministic_years_sized_and_skewed() {
        let cost: Cost = serde_json::from_str(COST).unwrap();
        assert!(cost.sizes.windows(2).all(|w| w[0] < w[1]));
        assert!(cost.sizes.last().unwrap() >= &(365 * 5));
        let largest = cost_family(&config(), &cost, *cost.sizes.last().unwrap()).unwrap();
        for size in &cost.sizes {
            let a = cost_family(&config(), &cost, *size).unwrap();
            let b = cost_family(&config(), &cost, *size).unwrap();
            assert_eq!(
                serde_json::to_value(&a).unwrap(),
                serde_json::to_value(b).unwrap()
            );
            assert_eq!(a.experiences.len(), *size);
            assert_eq!(
                serde_json::to_value(&a.experiences).unwrap(),
                serde_json::to_value(&largest.experiences[..*size]).unwrap()
            );
            let counts = skew(&a);
            for kind in ["people_experience_counts", "places_experience_counts"] {
                let counts = counts[kind].as_object().unwrap();
                assert_eq!(
                    counts.values().map(|n| n.as_u64().unwrap()).sum::<u64>(),
                    *size as u64
                );
                let hub_count = if kind.starts_with("people") {
                    cost.hub_people
                } else {
                    cost.hub_places
                };
                let largest = counts.values().map(|n| n.as_u64().unwrap()).max().unwrap();
                let smallest = counts.values().map(|n| n.as_u64().unwrap()).min().unwrap();
                assert!(largest > smallest * 5);
                assert!(counts.len() > hub_count);
            }
        }
    }

    #[test]
    fn gold_names_only_authored_memories_and_never_enters_inputs() {
        let gold: Value = serde_json::from_str(include_str!(
            "../../../fixtures/situated-situation-gold.json"
        ))
        .unwrap();
        for labelled in gold["families"].as_array().unwrap() {
            let family = families(&config())
                .into_iter()
                .find(|f| f.name == labelled["family"])
                .unwrap();
            let known = family
                .experiences
                .iter()
                .map(|e| e.write.episode_external_id.as_str())
                .chain(
                    family
                        .graph
                        .derived_memories
                        .iter()
                        .map(|m| m.external_id.as_str()),
                )
                .collect::<BTreeSet<_>>();
            let mut seen = BTreeSet::new();
            for situation in labelled["situations"].as_array().unwrap() {
                assert!(!situation["name"].as_str().unwrap().is_empty());
                assert!(!situation["reason"].as_str().unwrap().is_empty());
                for memory in situation["memories"].as_array().unwrap() {
                    let id = memory.as_str().unwrap();
                    assert!(known.contains(id), "unknown gold memory {id}");
                    assert!(seen.insert(id), "duplicate gold membership {id}");
                }
            }
            let input = serde_json::to_string(&family).unwrap();
            for situation in labelled["situations"].as_array().unwrap() {
                assert!(!input.contains(situation["name"].as_str().unwrap()));
            }
        }
        assert!(
            !include_str!("situated_before.rs")
                .split("#[cfg(test)]")
                .next()
                .unwrap()
                .contains("situated-situation-gold")
        );
    }
}
