use super::*;
use cmem_eval::RetrievalMode;

#[tokio::test]
async fn situated_toml_cli_keeps_mixed_and_all_not_run_scenarios() {
    use cmem_eval_continuity::{ScenarioStatus, read_continuity_report};
    let dir = tempfile::tempdir().unwrap();
    let dataset = dir.path().join("situated.toml");
    let config =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs/continuity_situated.toml");
    let control = serde_json::json!({
        "fixture_id":"control", "namespace":"control", "pattern":"situated", "catalog_situations":["D4"], "character_entity":"self",
        "entities":[{"external_id":"self", "label":"Character", "is_hub":false}],
        "scenes":{"alone":{"who":[{"reference":{"by":"key","key":"self"}}]}},
        "embedding":{"provider":"controllable_similarity", "own_concept":true,"seed":7,"vector_size":16,"noise_magnitude":0.01,"clusters":{},"concepts":{}},
        "events":[
            {"kind":"experience","event_id":"visit","timestamp":"2024-01-01T09:00:00Z","text":"Garden","scene":{"kind":"named","name":"alone"}},
            {"kind":"query","event_id":"ask","query_id":"ask","timestamp":"2024-01-02T09:00:00Z","text":"Garden","expected":{"relevant_external_ids":["visit"],"irrelevant_external_ids":[]}}
        ]
    });
    let mut gated = control.clone();
    gated["fixture_id"] = "gated".into();
    gated["namespace"] = "gated".into();
    gated["events"][1] = serde_json::json!({
        "kind":"probe", "event_id":"probe", "query_id":"probe", "timestamp":"2024-01-02T09:00:00Z", "scene":{"kind":"named","name":"alone"},
        "assertions":{"carried":[{"memory":"visit","reason":"own_day"}], "cued":[{"memory":"visit","cue":"own_day"}]}, "measures":{"bystanders":[]}
    });
    for (name, scenarios) in [
        ("mixed", vec![control, gated.clone()]),
        ("all-not-run", vec![gated]),
    ] {
        let fixture = serde_json::json!({"schema_version":3,"seed":7,"scenarios":scenarios});
        fs::write(&dataset, toml::to_string(&fixture).unwrap()).unwrap();
        let out_dir = dir.path().join(name);
        let out = out_dir.join("traces.jsonl");
        // The all-not-run case uses an unreachable service endpoint: constructing
        // or opening an adapter would fail rather than silently exercising a store.
        let config_path = if name == "all-not-run" {
            let path = dir.path().join("unreachable.toml");
            let text = fs::read_to_string(&config).unwrap().replace("[backend]", "[backend]\nvector_store_mode = \"service\"\nqdrant_connection_string = \"http://127.0.0.1:1\"");
            fs::write(&path, text.replace("vector_size = 32", "")).unwrap();
            path
        } else {
            config.clone()
        };
        Cli::try_parse_from([
            "cmem-eval",
            "run",
            "continuity",
            "--dataset",
            dataset.to_str().unwrap(),
            "--config",
            config_path.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap()
        .run()
        .await
        .unwrap();
        assert!(!out_dir.join("stores").exists());
        let report = read_continuity_report(&out_dir.join("report.json")).unwrap();
        let gated = &report.scenarios["gated"].outcome;
        assert_eq!(gated.status, ScenarioStatus::NotRun);
        assert!(
            !gated
                .missing_features
                .contains(&cmem_eval_continuity::ScenarioFeature::ProbeScene)
        );
        assert_eq!(gated.assertions.len(), 2);
        assert_eq!(gated.assertions[0].check.status, ScenarioStatus::NotRun);
        assert_eq!(gated.assertions[1].check.status, ScenarioStatus::NotRun);
        assert!(
            gated
                .missing_features
                .contains(&cmem_eval_continuity::ScenarioFeature::OwnDayCue)
        );
        assert_eq!(gated.probes["probe"].context_tokens, None);
        assert_eq!(gated.probes["probe"].bystander_context_share, None);
        assert_eq!(
            gated.probes["probe"].carried_recall_by_reason["own_day"].recall,
            None
        );
        assert!(
            report
                .aggregate
                .carried_recall_by_reason
                .values()
                .all(|count| count.recall.is_none())
        );
        assert!(
            report.scenarios["gated"]
                .carried_recall_by_reason
                .values()
                .all(|count| count.recall.is_none())
        );
        let header: serde_json::Value =
            serde_json::from_slice(&fs::read(out_dir.join("header.json")).unwrap()).unwrap();
        assert!(header["embedding_bindings"].get("gated").is_none());
        if name == "mixed" {
            assert_eq!(report.scenarios.len(), 2);
            assert_eq!(
                report.scenarios["control"].outcome.status,
                ScenarioStatus::Passed
            );
            assert_eq!(report.aggregate.query_count, 1);
            assert_eq!(
                report.aggregate.omission_reason_invariant.status,
                ScenarioStatus::Passed
            );
        } else {
            assert_eq!(report.scenarios.len(), 1);
            assert_eq!(report.aggregate.query_count, 0);
            assert_eq!(
                report.aggregate.omission_reason_invariant.status,
                ScenarioStatus::NotRun
            );
            assert_eq!(fs::metadata(&out).unwrap().len(), 0);
        }
        Cli::try_parse_from([
            "cmem-eval",
            "compare-continuity",
            out_dir.join("report.json").to_str().unwrap(),
            out_dir.join("report.json").to_str().unwrap(),
        ])
        .unwrap()
        .run()
        .await
        .unwrap();

        let mut changed = report.clone();
        changed
            .aggregate
            .carried_recall_by_reason
            .get_mut("own_day")
            .unwrap()
            .recall = Some(0.0);
        changed
            .scenarios
            .get_mut("gated")
            .unwrap()
            .outcome
            .assertions
            .iter_mut()
            .find(|result| {
                matches!(
                    result.identity.assertion,
                    cmem_eval_continuity::AssertionSubject::Cued { .. }
                )
            })
            .unwrap()
            .check
            .status = ScenarioStatus::Passed;
        let changed_path = out_dir.join("changed.json");
        cmem_eval_continuity::write_continuity_report(&changed_path, &changed).unwrap();
        let reread = read_continuity_report(&changed_path).unwrap();
        let differences = cmem_eval_continuity::compare_continuity_reports(&report, &reread);
        assert_eq!(differences.len(), 2);
        assert!(
            differences
                .iter()
                .any(|d| d.field == "carried_recall_by_reason" && d.scenario_id.is_none())
        );
        assert!(
            differences
                .iter()
                .any(|d| d.assertion.as_ref().is_some_and(|identity| matches!(
                    identity.assertion,
                    cmem_eval_continuity::AssertionSubject::Cued { .. }
                )))
        );
        Cli::try_parse_from([
            "cmem-eval",
            "compare-continuity",
            out_dir.join("report.json").to_str().unwrap(),
            changed_path.to_str().unwrap(),
        ])
        .unwrap()
        .run()
        .await
        .unwrap();
    }
}

#[test]
fn checked_in_vector_configs_use_raw_candidate_ingestion_only() {
    for path in [
        "../../configs/longmemeval_s_vector.toml",
        "../../configs/locomo_vector.toml",
    ] {
        let path = PathBuf::from(path);
        let config = read_config(&path)
            .unwrap_or_else(|err| panic!("read vector config {}: {err}", path.display()));
        assert_eq!(config.retrieval.mode, RetrievalMode::VectorOnly);
        assert_eq!(
            config.backend.embedding.provider,
            cmem_eval::EmbeddingProviderConfig::OpenAi
        );
        assert_eq!(
            config.retrieval.surface_policy.object_types,
            [
                cmem_eval::ObjectType::Episode,
                cmem_eval::ObjectType::Observation,
            ]
        );
        assert_eq!(config.retrieval.surface_policy.sections.derived_memories, 0);
        assert_eq!(config.retrieval.surface_policy.sections.active_threads, 0);
        assert!(!config.ingest.index_session_summaries);
        assert!(!config.ingest.index_generated_observations);
        assert!(config.ingest.enrichment_path.is_none());
        config.validate().unwrap();
    }
}

#[test]
fn config_reader_rejects_misspelled_backend_table() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        r#"
run_id = "typo"
dataset = "locomo"

[backend.character_memroy]
selectivity_gamma = 0.5
"#,
    )
    .unwrap();

    let error = read_config(&path).unwrap_err().to_string();
    assert!(
        error.contains("unknown field `character_memroy`"),
        "{error}"
    );
}

#[test]
fn current_checked_in_configs_parse_and_validate_under_the_strict_schema() {
    let mut paths = fs::read_dir("../../configs")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            // Sealed continuity configs are cited by hash and may predate the
            // current schema; maintained smoke and cross-mode configs must track it.
            path.extension()
                .is_some_and(|extension| extension == "toml")
                && (!name.starts_with("continuity_")
                    || name == "continuity_smoke.toml"
                    || name.starts_with("continuity_crossmode_"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());

    for path in paths {
        read_config(&path)
            .and_then(|config| config.validate())
            .unwrap_or_else(|error| {
                panic!("checked-in config {} failed: {error:#}", path.display())
            });
    }
}
