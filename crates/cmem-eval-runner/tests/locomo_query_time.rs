use cmem_eval::{ObjectType, read_jsonl};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

fn run_fixture(value: Value) -> (tempfile::TempDir, PathBuf, std::process::Output) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker2");
    fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let dataset = directory.path().join("fixture.json");
    let config = directory.path().join("config.toml");
    let output = directory.path().join("run/results.jsonl");
    fs::write(&dataset, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut run_config: cmem_eval::BenchmarkRunConfig =
        toml::from_str(include_str!("../../../configs/locomo_retrieval.toml")).unwrap();
    run_config.backend.embedding.provider = cmem_eval::EmbeddingProviderConfig::Deterministic;
    run_config.ingest.enrichment_snapshot_path = None;
    fs::write(&config, toml::to_string(&run_config).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_cmem-eval"))
        .env_remove("OPENAI_API_KEY")
        .args(["run", "locomo", "--dataset"])
        .arg(dataset)
        .arg("--config")
        .arg(config)
        .arg("--out")
        .arg(&output)
        .output()
        .unwrap();
    (directory, output, result)
}

#[test]
fn locomo_queries_include_the_final_source_timestamp() {
    let (_directory, output, result) = run_fixture(json!([{
        "sample_id": "source_time",
        "conversation": {
            "session_1_date_time": "2024-02-01T22:00:00+09:00",
            "session_1": [{"dia_id": "D1:1", "text": "lighthouse trip"}],
            "session_2_date_time": "1:00 pm on 1 January, 2024",
            "session_2": [{"dia_id": "D2:1", "text": "earlier lighthouse plans"}]
        },
        "qa": [{"question": "lighthouse"}, {"question": "trip"}]
    }]));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rows = read_jsonl(&output).unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert!(row.retrieved.iter().any(|item| {
            item.kind == ObjectType::Observation && item.external_id.as_deref() == Some("D1:1")
        }));
    }
    assert!(!output.parent().unwrap().join("stores").exists());
}

#[test]
fn locomo_without_a_parseable_timestamp_names_the_conversation() {
    for timestamp in [Value::Null, json!("not-a-date")] {
        let (_directory, output, result) = run_fixture(json!([{
            "sample_id": "missing_source_time",
            "conversation": {
                "session_1_date_time": timestamp,
                "session_1": [{"dia_id": "D1:1", "text": "lighthouse"}]
            },
            "qa": [{"question": "lighthouse"}]
        }]));
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("missing_source_time"));
        assert!(!output.parent().unwrap().exists());
    }
}
