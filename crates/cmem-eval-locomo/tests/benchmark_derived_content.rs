use cmem_eval::{DerivedType, RetrievalMode};
use cmem_eval_locomo::{
    ConfigError, LoCoMoSample, ingest::to_memory_inputs, load_path, load_value, validate_config,
};
use serde_json::{Value, json};

fn fixture() -> Value {
    json!({
        "sample_id": "p1",
        "conversation": [{"session_id": "session_1", "turns": [
            {"dia_id": "d1", "speaker": "A", "text": "tea"},
            {"dia_id": "d2", "speaker": "B", "text": "coffee"},
            {"dia_id": "opaque,id", "text": "other"}
        ]}],
        "session_summary": {"session_1_summary": "UNIQUE_SUMMARY_TOKEN"},
        "observation": {"session_1_observation": {"A": [
            ["A likes tea", "d1, d2"], ["Opaque reference", "opaque,id"],
            ["Some unknown provenance", ["d1", "missing"]], "Legacy statement"
        ]}},
        "qa": [{"question": "tea?", "answer": "GOLD_ANSWER", "evidence": ["d2"]}]
    })
}

fn sample(value: Value) -> LoCoMoSample {
    load_value(json!([value])).unwrap().remove(0)
}

#[test]
fn observations_keep_statement_speaker_and_resolved_provenance() {
    let sample = sample(fixture());
    let observations = &sample.sessions[0].generated_observations;
    assert_eq!(observations.len(), 4);
    assert_eq!(observations[0].statement, "A likes tea");
    assert_eq!(observations[0].speaker.as_deref(), Some("A"));
    assert_eq!(observations[0].evidence_dialog_ids, ["d1", "d2"]);
    assert_eq!(observations[1].evidence_dialog_ids, ["opaque,id"]);
    assert!(observations[3].evidence_dialog_ids.is_empty());
    assert_eq!(sample.unresolved_evidence_references, 1);
    assert_eq!(sample.dropped_observation_entries, 0);
    let mapped = to_memory_inputs(&sample, false, true, true);
    assert_eq!(mapped.episodes[0].summary, "UNIQUE_SUMMARY_TOKEN");
    assert_eq!(mapped.derived_memories.len(), 5);
    assert_eq!(
        mapped.derived_memories[0].derived_type,
        DerivedType::Reflection
    );
    assert_eq!(
        mapped.derived_memories[0].source_episode_external_ids,
        ["session_1"]
    );
    for (input, observation) in mapped.derived_memories[1..].iter().zip(observations) {
        assert_eq!(input.derived_type, DerivedType::Claim);
        assert_eq!(input.text, observation.statement);
        assert_eq!(input.source_episode_external_ids, ["session_1"]);
        assert_eq!(input.metadata["speaker"], "A");
    }
    assert_eq!(
        mapped.derived_memories[1].source_observation_external_ids,
        ["d1", "d2"]
    );
    assert_eq!(
        mapped.derived_memories[2].source_observation_external_ids,
        ["opaque,id"]
    );
    assert_eq!(
        mapped.derived_memories[3].source_observation_external_ids,
        ["d1"]
    );
    assert!(
        mapped.derived_memories[4]
            .source_observation_external_ids
            .is_empty()
    );
    assert!(
        !serde_json::to_string(&mapped.derived_memories)
            .unwrap()
            .contains("GOLD_ANSWER")
    );
    for (summary, observation, count) in [(false, false, 0), (true, false, 1), (false, true, 4)] {
        let mapped = to_memory_inputs(&sample, false, summary, observation);
        assert_eq!(mapped.derived_memories.len(), count);
        assert_eq!(
            mapped.episodes[0].summary.contains("UNIQUE_SUMMARY_TOKEN"),
            summary
        );
    }
}

#[test]
fn annotation_lookup_precedence_and_record_content_are_preserved() {
    for id in ["session_1", "session_01", "session_+1"] {
        let mut value = fixture();
        value["conversation"][0]["session_id"] = json!(id);
        value["conversation"][0]["observation"] = json!("record observation");
        value["session_summary"] =
            json!({id: "exact", "session_1_summary": "official", "1": "numeric"});
        value["observation"] =
            json!({id: ["exact"], "session_1_observation": ["official"], "1": ["numeric"]});
        for expected in ["exact", "official", "numeric"] {
            let loaded = sample(value.clone());
            let session = &loaded.sessions[0];
            assert_eq!(session.summary.as_deref(), Some(expected));
            assert_eq!(
                session.generated_observations[0].statement,
                "record observation"
            );
            assert_eq!(session.generated_observations[0].speaker, None);
            assert_eq!(session.generated_observations[1].statement, expected);
            if expected == "exact" {
                value["session_summary"].as_object_mut().unwrap().remove(id);
                value["observation"].as_object_mut().unwrap().remove(id);
            } else {
                value["session_summary"]
                    .as_object_mut()
                    .unwrap()
                    .remove("session_1_summary");
                value["observation"]
                    .as_object_mut()
                    .unwrap()
                    .remove("session_1_observation");
            }
        }
        value["conversation"][0]["summary"] = json!("record summary");
        assert_eq!(
            sample(value).sessions[0].summary.as_deref(),
            Some("record summary")
        );
    }
}

#[test]
fn malformed_annotations_are_nonfatal_and_drops_are_counted() {
    for malformed in [
        Value::Null,
        json!(true),
        json!(1),
        json!({}),
        json!([]),
        json!(["one"]),
        json!(["too", "many", "values"]),
        json!([true, "d1"]),
        json!(["fact", 1]),
        json!(["fact", ["d1", null]]),
        json!(""),
    ] {
        let mut value = fixture();
        value["session_summary"]["session_1_summary"] = json!(["not a summary"]);
        value["observation"]["session_1_observation"] = json!({"A": [malformed, "retained"]});
        let loaded = sample(value);
        assert_eq!(loaded.sessions[0].summary, None);
        assert_eq!(loaded.dropped_observation_entries, 1, "{malformed}");
        assert_eq!(loaded.sessions[0].generated_observations.len(), 1);
        assert_eq!(
            loaded.sessions[0].generated_observations[0].statement,
            "retained"
        );
    }
    for malformed in [json!(null), json!(false), json!(3), json!("bad map")] {
        let mut value = fixture();
        value["observation"] = malformed.clone();
        assert_eq!(sample(value).dropped_observation_entries, 1);
        let mut value = fixture();
        value["observation"]["session_1_observation"] = json!({"A": malformed});
        assert_eq!(sample(value).dropped_observation_entries, 1);
    }
}

#[test]
fn baseline_config_rejects_each_derived_input_with_a_typed_error() {
    let mut config: cmem_eval::BenchmarkRunConfig =
        serde_json::from_value(json!({"run_id": "test", "dataset": "locomo"})).unwrap();
    for mode in [RetrievalMode::Bm25Only, RetrievalMode::VectorOnly] {
        config.retrieval.mode = mode;
        validate_config(&config).unwrap();
        for field in [
            "index_session_summaries",
            "index_generated_observations",
            "enrichment_path",
            "enrichment_snapshot_path",
        ] {
            let mut invalid = config.clone();
            match field {
                "index_session_summaries" => invalid.ingest.index_session_summaries = true,
                "index_generated_observations" => {
                    invalid.ingest.index_generated_observations = true
                }
                "enrichment_path" => invalid.ingest.enrichment_path = Some("missing.jsonl".into()),
                _ => invalid.ingest.enrichment_snapshot_path = Some("missing.jsonl".into()),
            }
            assert_eq!(
                validate_config(&invalid)
                    .unwrap_err()
                    .downcast_ref::<ConfigError>(),
                Some(&ConfigError::BaselineDerivedContent { mode, field })
            );
        }
    }
}

#[test]
#[ignore = "requires the untracked official file via LOCOMO_OFFICIAL_DATASET"]
fn official_derived_content_census() {
    let path = std::env::var_os("LOCOMO_OFFICIAL_DATASET").expect("set LOCOMO_OFFICIAL_DATASET");
    let samples = load_path(std::path::Path::new(&path)).unwrap();
    let sessions = samples
        .iter()
        .flat_map(|sample| &sample.sessions)
        .collect::<Vec<_>>();
    let observations = sessions
        .iter()
        .flat_map(|session| &session.generated_observations)
        .collect::<Vec<_>>();
    let counts = (
        sessions
            .iter()
            .filter(|session| session.summary.is_some())
            .count(),
        observations.len(),
        observations
            .iter()
            .map(|observation| observation.evidence_dialog_ids.len())
            .sum::<usize>(),
        samples
            .iter()
            .map(|sample| sample.unresolved_evidence_references)
            .sum::<usize>(),
        samples
            .iter()
            .map(|sample| sample.dropped_observation_entries)
            .sum::<usize>(),
    );
    println!(
        "summaries={} observations={} evidence_references={} unresolved={} dropped={}",
        counts.0, counts.1, counts.2, counts.3, counts.4
    );
    assert_eq!(counts, (272, 2541, 2561, 0, 0));
    let inputs = samples
        .iter()
        .map(|sample| to_memory_inputs(sample, false, true, true))
        .collect::<Vec<_>>();
    let memories = inputs
        .iter()
        .flat_map(|input| &input.derived_memories)
        .collect::<Vec<_>>();
    assert_eq!(
        memories
            .iter()
            .filter(|memory| memory.derived_type == DerivedType::Reflection)
            .count(),
        272
    );
    assert_eq!(
        memories
            .iter()
            .filter(|memory| memory.derived_type == DerivedType::Claim)
            .count(),
        2541
    );
    assert_eq!(
        memories
            .iter()
            .map(|memory| memory.source_observation_external_ids.len())
            .sum::<usize>(),
        2561
    );
}
