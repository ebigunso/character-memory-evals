use cmem_eval::{ObjectType, read_jsonl};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path, process::Command};

#[test]
fn lexical_rows_keep_assigned_retrieved_ids_and_raw_gold_ids() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agent-work/evals-worker2");
    fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let dataset = directory.path().join("fixture.json");
    let config = directory.path().join("config.toml");
    let output = directory.path().join("run/results.jsonl");
    fs::write(
        &dataset,
        serde_json::to_vec(&json!([{
            "question_id": "repeat", "question": "conversation repeatedneedle",
            "question_date": "2024-01-03T00:00:00Z",
            "haystack_session_ids": ["s", "s"],
            "haystack_dates": ["2024-01-01T00:00:00Z", "2024-01-02T00:00:00Z"],
            "haystack_sessions": [
                [{"role": "user", "content": "repeatedneedle", "has_answer": true}],
                [{"role": "user", "content": "repeatedneedle", "has_answer": true}]
            ], "answer_session_ids": ["s"]
        }]))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &config,
        include_str!("../../../configs/longmemeval_s_bm25.toml"),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_cmem-eval"))
        .args(["run", "longmemeval-s", "--dataset"])
        .arg(&dataset)
        .arg("--config")
        .arg(&config)
        .arg("--out")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rows = read_jsonl(&output).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.gold_episode_ids, ["s"]);
    assert_eq!(row.gold_observation_ids, ["s:turn:1", "s:turn:1"]);
    for (kind, expected) in [
        (ObjectType::Episode, ["s", "s#2"]),
        (ObjectType::Observation, ["s:turn:1", "s#2:turn:1"]),
    ] {
        let actual = row
            .retrieved
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| item.external_id.as_deref().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, BTreeSet::from(expected));
    }
    for item in row
        .retrieved
        .iter()
        .filter(|item| item.kind == ObjectType::Observation)
    {
        let episode = if item.external_id.as_deref() == Some("s#2:turn:1") {
            "s#2"
        } else {
            "s"
        };
        assert_eq!(item.episode_external_id.as_deref(), Some(episode));
    }
    let metrics = row.metrics.to_json_map();
    assert_eq!(metrics["session_ndcg@10"], 1.0);
    assert_eq!(metrics["turn_ndcg@50"], 1.0);
    assert!(!output.parent().unwrap().join("stores").exists());
}
