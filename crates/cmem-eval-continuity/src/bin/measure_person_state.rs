//! Small synthetic, unsealed meeting-state measurement; gold is scored after retrieval.
use std::{collections::BTreeMap, env, fs, io::Write, path::Path, process::Command};

use anyhow::{Context, Result, ensure};
use chrono::{Duration, TimeZone, Utc};
use cmem_eval::{
    BenchmarkRunConfig, ControllableDimensionPolicy, ControllableSimilarityFixture, DatasetId,
    DerivedMemoryInput, DerivedType, EmbeddingProviderConfig, EmbeddingRuntimeBinding, EntityInput,
    EpisodeInput, GraphEnrichmentInput, MemoryEndpointInput, MemoryLinkInput, MemorySceneInput,
    ObjectType, RelationType, RetrieveInput, RetrievedContextPack, SceneParticipantInput,
    SimilarityConceptFixture, character_memory::api::types as native, text_sha256,
};
use cmem_eval_continuity::{CHECKED_FIXTURE_SEED, ContinuityRuntime};
use serde_json::{Value, json};

const NS: &str = "poster-person-state";
const AT: &str = "2025-09-01T12:00:00Z";
const TOPIC: &str = "Repairing a copper bell";
const OPEN: &str = "Iris still needs her borrowed blue notebook returned.";
const SETTLED: &str = "I returned the borrowed blue notebook to Iris; the matter is settled.";

fn config() -> BenchmarkRunConfig {
    let mut config = BenchmarkRunConfig {
        run_id: NS.into(),
        dataset: DatasetId::new("continuity").unwrap(),
        backend: Default::default(),
        retrieval: Default::default(),
        ingest: Default::default(),
        metrics: Default::default(),
    };
    config.backend.embedding.provider = EmbeddingProviderConfig::ControllableSimilarity;
    config.backend.embedding.vector_size = Some(4);
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

fn probe(config: &BenchmarkRunConfig, person: bool, topic: Option<&str>) -> RetrieveInput {
    RetrieveInput {
        mode: config.retrieval.mode,
        namespace: NS.into(),
        topic: topic.map(str::to_owned),
        scene: MemorySceneInput {
            time: Some(AT.into()),
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

fn generated() -> (
    ControllableSimilarityFixture,
    Vec<EpisodeInput>,
    GraphEnrichmentInput,
    Vec<String>,
) {
    let mut embedding = ControllableSimilarityFixture {
        seed: CHECKED_FIXTURE_SEED,
        vector_size: 4,
        noise_magnitude: 0.000001,
        clusters: BTreeMap::new(),
        concepts: BTreeMap::new(),
    };
    assign(&mut embedding, TOPIC, vec![1.0, 0.0, 0.0, 0.0]);
    let start = Utc.with_ymd_and_hms(2024, 9, 1, 12, 0, 0).unwrap();
    let mut episodes = Vec::new();
    for index in 0..64 {
        let (id, text, participants, vector) = if index < 40 {
            (
                format!("meeting-{index:02}"),
                format!("Iris and I shared a meal on visit {index:02}."),
                vec![SceneParticipantInput {
                    key: Some("iris".into()),
                    ..Default::default()
                }],
                vec![0.0, 1.0, 0.0, 0.0],
            )
        } else if index < 48 {
            let n = index - 40;
            let cosine = 0.9 - n as f32 * 0.3 / 7.0;
            (
                format!("topic-{n:02}"),
                format!("Copper bell repair technique {n:02}."),
                vec![],
                vec![cosine, 0.0, (1.0 - cosine * cosine).sqrt(), 0.0],
            )
        } else {
            (
                format!("background-{index:02}"),
                format!("Rowan discussed a garden detail {index:02}."),
                vec![SceneParticipantInput {
                    key: Some("rowan".into()),
                    ..Default::default()
                }],
                vec![0.0, 0.0, 0.0, 1.0],
            )
        };
        assign(&mut embedding, &text, vector);
        episodes.push(EpisodeInput {
            external_id: id,
            namespace: NS.into(),
            summary: text,
            scene: MemorySceneInput {
                time: Some(
                    (start
                        + Duration::days(if index < 40 {
                            index * 9
                        } else {
                            300 + index - 40
                        }))
                    .to_rfc3339(),
                ),
                participants,
                ..Default::default()
            },
            ended_at: None,
            metadata: Value::Null,
        });
    }
    let mut graph = GraphEnrichmentInput {
        namespace: NS.into(),
        entities: vec![
            EntityInput {
                external_id: "iris".into(),
            },
            EntityInput {
                external_id: "rowan".into(),
            },
        ],
        ..Default::default()
    };
    let mut gold = Vec::new();
    for index in 0..300 {
        let id = format!("belief-{index:03}");
        // The last 20 beliefs are the independently declared meeting-fitting set.
        // Their high salience is ordinary input; their gold membership is never sent.
        let fitting = index >= 280;
        if fitting {
            gold.push(id.clone());
        }
        let text = if fitting {
            format!(
                "Iris meeting consideration {:02}: she values a quiet, unhurried conversation.",
                index - 280
            )
        } else {
            format!("Iris background detail {index:03}: a preference from a past conversation.")
        };
        assign(&mut embedding, &text, vec![0.0, 1.0, 0.0, 0.0]);
        let age_days = if fitting && index % 2 == 0 {
            1 + index % 10
        } else {
            30 + index % 300
        };
        graph.derived_memories.push(DerivedMemoryInput {
            external_id: id,
            created_at: Some(
                (AT.parse::<chrono::DateTime<Utc>>().unwrap() - Duration::days(age_days))
                    .to_rfc3339(),
            ),
            derived_type: DerivedType::Reflection,
            text,
            source_episode_external_ids: vec![],
            source_observation_external_ids: vec![],
            thread_external_ids: vec![],
            entity_external_ids: vec!["iris".into()],
            assertions: vec![],
            given_by_application: true,
            salience_score: if fitting {
                0.99 - (index - 280) as f32 * 0.005
            } else {
                0.10 + index as f32 * 0.002
            },
            supersedes_external_ids: vec![],
            metadata: Value::Null,
        });
    }
    (embedding, episodes, graph, gold)
}

fn score(pack: &RetrievedContextPack, gold: &[String]) -> Result<Value> {
    let ids = pack
        .items()
        .iter()
        .filter_map(|item| item.external_id.clone())
        .collect::<Vec<_>>();
    let beliefs = ids
        .iter()
        .filter(|id| id.starts_with("belief-"))
        .collect::<Vec<_>>();
    let fitting = beliefs.iter().filter(|id| gold.contains(id)).count();
    let experiences = ids.iter().filter(|id| id.starts_with("meeting-")).count();
    let topics = ids.iter().filter(|id| id.starts_with("topic-")).count();
    let outcome = &pack.outcomes()[0];
    let trace = serde_json::to_value(outcome.trace.as_ref().context("missing trace")?)?;
    // Native construction timestamps vary; selected identities, ranks, scores and trace do not.
    Ok(
        json!({"pack_slots": ids.len(), "person_beliefs": beliefs.len(),
        "person_experiences": experiences, "fitting_beliefs": fitting, "gold_beliefs":gold.len(),
        "belief_precision": if beliefs.is_empty() { None } else { Some(fitting as f64 / beliefs.len() as f64) },
        "person_memory_precision": if beliefs.len()+experiences==0 { None } else { Some(fitting as f64/(beliefs.len()+experiences) as f64) },
        "fitting_recall":if gold.is_empty() { None } else { Some(fitting as f64/gold.len() as f64) },
        "on_topic_episodes":topics, "selected_external_ids":ids,
        "selected_items":pack.items(), "trace":trace, "telemetry":outcome.rationale.telemetry,
        "object_refs":pack.object_refs()}),
    )
}

fn revision(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-c", &format!("safe.directory={}", path.display()), "-C"])
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

fn settlement_graph(resolved: bool) -> GraphEnrichmentInput {
    let (_, _, graph, _) = generated();
    let mut memory = graph.derived_memories[299].clone();
    memory.external_id = if resolved {
        "notebook-returned"
    } else {
        "notebook-open"
    }
    .into();
    memory.text = if resolved { SETTLED } else { OPEN }.into();
    memory.created_at = Some(
        if resolved {
            "2025-08-31T12:00:00Z"
        } else {
            "2025-08-01T12:00:00Z"
        }
        .into(),
    );
    memory.derived_type = if resolved {
        DerivedType::Reflection
    } else {
        DerivedType::OpenLoop
    };
    memory.salience_score = 1.0;
    GraphEnrichmentInput {
        namespace: NS.into(),
        derived_memories: vec![memory],
        links: if resolved {
            vec![MemoryLinkInput {
                external_id: "notebook-resolution".into(),
                from: MemoryEndpointInput {
                    object_type: ObjectType::DerivedMemory,
                    external_id: "notebook-returned".into(),
                },
                relation: RelationType::Resolves,
                to: MemoryEndpointInput {
                    object_type: ObjectType::DerivedMemory,
                    external_id: "notebook-open".into(),
                },
                rationale: None,
            }]
        } else {
            vec![]
        },
        ..Default::default()
    }
}

async fn measure_settlement(
    runtime: &ContinuityRuntime,
    config: &BenchmarkRunConfig,
) -> Result<Value> {
    let adapter = runtime.adapter();
    let meeting = probe(config, true, None);
    adapter.remember_enrichment(settlement_graph(false)).await?;
    let before = adapter.retrieve(meeting.clone()).await?;
    adapter.remember_enrichment(settlement_graph(true)).await?;
    let after = adapter.retrieve(meeting.clone()).await?;
    let topic_input = probe(config, false, Some(OPEN));
    let by_topic = adapter.retrieve(topic_input.clone()).await?;
    let shape = serde_json::to_value(&after.outcomes()[0].pack.derived_memories)?;
    ensure!(
        shape
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m.get("resolved_by").is_some()),
        "settled probe requires the library Task_3 resolved_by response"
    );
    let contains = |pack: &RetrievedContextPack, id: &str| {
        pack.items()
            .iter()
            .any(|i| i.external_id.as_deref() == Some(id))
    };
    let resolution = |pack: &RetrievedContextPack| -> Result<Vec<Option<String>>> {
        let old = pack.outcomes()[0]
            .pack
            .open_loops
            .iter()
            .find(|m| m.memory.text == OPEN);
        let old = old.map(serde_json::to_value).transpose()?;
        Ok(old
            .as_ref()
            .and_then(|m| m.get("resolved_by"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|id| {
                pack.object_refs()
                    .get(id.as_str().unwrap())
                    .map(|r| r.external_id.clone())
            })
            .collect())
    };
    let meeting_resolvers = resolution(&after)?;
    let topic_resolvers = resolution(&by_topic)?;
    Ok(json!({"checks":{
        "open_loop_present_before_resolution":contains(&before,"notebook-open"),
        "resolver_present_on_meeting":contains(&after,"notebook-returned"),
        "open_loop_present_on_meeting":contains(&after,"notebook-open"),
        "meeting_open_loop_marked_resolved_by_resolver":meeting_resolvers.contains(&Some("notebook-returned".into())),
        "topic_recalls_open_loop":contains(&by_topic,"notebook-open"),
        "topic_open_loop_marked_resolved_by_resolver":topic_resolvers.contains(&Some("notebook-returned".into()))},
        "meeting_resolved_by_external_ids":meeting_resolvers,
        "topic_resolved_by_external_ids":topic_resolvers,
        "before":{"input":meeting,"observed":score(&before,&[])?},
        "after":{"input":meeting,"observed":score(&after,&[])?},
        "topic":{"input":topic_input,"observed":score(&by_topic,&[])?}}))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(
        args.len() == 2 || (args.len() == 3 && args[2] == "--settled"),
        "usage: measure_person_state <new-report.json> <linked-library-checkout> [--settled]"
    );
    let output = Path::new(&args[0]);
    let library = Path::new(&args[1]);
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let manifest: toml::Value = fs::read_to_string(workspace.join("Cargo.toml"))?.parse()?;
    let dependency = manifest["workspace"]["dependencies"]["character-memory"]["path"]
        .as_str()
        .context("workspace must name the library path dependency")?;
    ensure!(
        workspace.join(dependency).canonicalize()? == library.canonicalize()?,
        "reported library checkout must match the workspace path dependency; run via cargo"
    );
    let library_commit = revision(library)?;
    let harness_commit = revision(workspace)?;
    let config = config();
    config.validate()?;
    let (mut embedding, episodes, graph, gold) = generated();
    let settled = args.len() == 3;
    if settled {
        assign(&mut embedding, OPEN, vec![0.0, 0.0, 1.0, 0.0]);
        assign(&mut embedding, SETTLED, vec![0.0, 0.0, 0.9, 0.4358899]);
    }
    let probes = [
        ("meeting-no-topic", probe(&config, true, None)),
        ("meeting-with-topic", probe(&config, true, Some(TOPIC))),
        ("topic-only-control", probe(&config, false, Some(TOPIC))),
    ];
    let input = json!({"embedding":embedding,"episodes":episodes,"graph":graph,"probes":probes,"gold_fitting_beliefs":gold,
        "settled_matter":settled.then(||json!({"open":settlement_graph(false),"resolved":settlement_graph(true),
            "meeting_probe":probe(&config,true,None),"topic_probe":probe(&config,false,Some(OPEN))}))});
    let mut file =
        fs::File::create_new(output).context("refusing to replace an existing report")?;
    let stores = output.with_extension("stores");
    fs::create_dir(&stores).context("store directory must be new")?;
    let result = async {
        let runtime = ContinuityRuntime::new(
            &stores,
            &config,
            EmbeddingRuntimeBinding::Controllable {
                fixture: embedding,
                dimension_policy: ControllableDimensionPolicy::Exact { vector_size: 4 },
            },
        )
        .await?;
        let measured = async {
            let adapter = runtime.adapter();
            adapter.open_namespace(NS).await?;
            adapter.remember_enrichment(graph).await?;
            for episode in episodes {
                adapter.remember_episode(episode).await?;
            }
            let mut rows = Vec::new();
            for (name, input) in &probes {
                let pack = adapter.retrieve(input.clone()).await?;
                rows.push(json!({"probe":name,"input":input,"observed":score(&pack,&gold)?}));
            }
            let settlement = if settled {
                Some(measure_settlement(&runtime, &config).await?)
            } else {
                None
            };
            Ok::<_, anyhow::Error>((rows, settlement))
        }
        .await;
        let cleanup = runtime.cleanup(NS).await;
        drop(runtime);
        cleanup?;
        measured
    }
    .await;
    fs::remove_dir_all(&stores).context("remove owned measurement stores")?;
    let (measurements, settlement) = result?;
    ensure!(
        revision(library)? == library_commit && revision(workspace)? == harness_commit,
        "pin changed during measurement"
    );
    let report = json!({"header": {"harness_commit":harness_commit,"library_commit":library_commit,
        "generator_source_sha256":text_sha256(include_str!("measure_person_state.rs")),
        "input_sha256":text_sha256(&serde_json::to_string(&input)?),"config_sha256":text_sha256(&serde_json::to_string(&config)?),
        "config":config,"seed":CHECKED_FIXTURE_SEED,"native_candidate_limits":native::RetrievalCandidateLimits::default(),
        "native_graph_limits":native::RetrievalGraphLimits::default(),"native_section_limits":native::ContinuitySectionLimits::default(),
        "native_default_floors":native::RetrievalCueFloors::default(),"stores_cleaned":true},
        "method":"Small synthetic store: 300 application-given current Reflection beliefs about one person; 20 high-salience meeting-fitting beliefs (10 recent), 40 experiences over a year, 8 graded topic episodes unrelated to the person, 16 background episodes about another person. Public adapter writes; keyed meeting; deterministic 4-D embeddings. Gold is scoring-only. Belief precision uses returned person beliefs; person-memory precision additionally includes returned experiences in its denominator. Topic score counts authored episodes once.",
        "generated_input":input,"measurements":measurements,"settled_matter":settlement});
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_store_matches_the_measurement_denominators() {
        let (_, episodes, graph, gold) = generated();
        assert_eq!(graph.derived_memories.len(), 300);
        assert_eq!(gold.len(), 20);
        assert_eq!(
            episodes
                .iter()
                .filter(|e| e.external_id.starts_with("meeting-"))
                .count(),
            40
        );
        assert_eq!(
            episodes
                .iter()
                .filter(|e| e.external_id.starts_with("topic-"))
                .count(),
            8
        );
        assert!(
            graph
                .derived_memories
                .iter()
                .all(|m| m.metadata.is_null() && m.entity_external_ids == ["iris"])
        );
        assert!(
            graph
                .derived_memories
                .iter()
                .filter(|m| gold.contains(&m.external_id))
                .all(|m| m.salience_score > 0.89)
        );
    }
}
