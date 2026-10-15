use serde_json::json;
use std::{fs, path::PathBuf, process::Command};

#[test]
fn run_manifest_records_oversized_turn_split_identities_and_policy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker2");
    fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let dataset = directory.path().join("fixture.json");
    let config = directory.path().join("config.toml");
    let output = directory.path().join("run/results.jsonl");
    let text = format!(
        "{}End.\n\n{}End.",
        "alpha ".repeat(4500),
        "beta ".repeat(4500)
    );
    fs::write(
        &dataset,
        serde_json::to_vec(&json!([{
            "question_id": "manifest-split",
            "question": "What happened?",
            "question_date": "2023/05/30 (Tue) 23:40",
            "haystack_session_ids": ["s1"],
            "haystack_dates": ["2023/05/30 (Tue) 23:40"],
            "haystack_sessions": [[{"role": "user", "content": text, "has_answer": true}]],
            "answer_session_ids": ["s1"]
        }]))
        .unwrap(),
    )
    .unwrap();
    let mut run_config: cmem_eval::BenchmarkRunConfig = toml::from_str(include_str!(
        "../../../configs/longmemeval_s_retrieval.toml"
    ))
    .unwrap();
    run_config.backend.embedding.provider = cmem_eval::EmbeddingProviderConfig::Deterministic;
    run_config.ingest.enrichment_snapshot_path = None;
    fs::write(&config, toml::to_string(&run_config).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_cmem-eval"))
        .env_remove("OPENAI_API_KEY")
        .args(["run", "longmemeval-s", "--dataset"])
        .arg(dataset)
        .arg("--config")
        .arg(config)
        .arg("--out")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let header: cmem_eval::RunHeader =
        serde_json::from_slice(&fs::read(output.with_file_name("header.json")).unwrap()).unwrap();
    assert_eq!(header.observation_splits.len(), 1);
    let split = &header.observation_splits[0];
    assert_eq!(split.dataset_item_id, "manifest-split");
    assert_eq!(split.source_observation_id, "s1:turn:1");
    assert_eq!(
        split
            .parts
            .iter()
            .map(|part| part.observation_id.as_str())
            .collect::<Vec<_>>(),
        ["s1:turn:1", "s1:turn:1:part:2"]
    );
    assert!(
        split
            .parts
            .iter()
            .all(|part| part.boundary_kind == cmem_eval::ObservationBoundary::Paragraph)
    );
    assert_eq!(split.tokenizer, "cl100k_base");
    assert_eq!(split.max_input_tokens, 8192);
    assert_eq!(
        split.boundary_policy,
        "greedy_sentence_paragraph_then_newline_whitespace_v1"
    );
    let rows = cmem_eval::read_jsonl(&output).unwrap();
    assert_eq!(rows[0].question_id, "manifest-split");
    assert_eq!(rows[0].gold_observation_ids, ["s1:turn:1"]);
    assert!(
        rows[0]
            .retrieved
            .iter()
            .any(|item| item.external_id.as_deref() == Some("s1:turn:1:part:2"))
    );
    assert_eq!(
        serde_json::to_value(&rows[0].metrics).unwrap()["turn_recall_fraction@5"],
        1.0
    );
    assert!(!output.parent().unwrap().join("stores").exists());
}
