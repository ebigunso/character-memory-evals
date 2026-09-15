use cmem_eval::{ObjectType, RetrievedItem};
use cmem_eval_longmemeval::{ingest::to_memory_inputs, load_path, load_value, scoring::score};
use serde_json::json;
use std::collections::HashSet;

#[test]
fn admitted_copies_keep_dates_text_and_collision_safe_identities() {
    let rows = load_value(json!([{
        "question_id": "q", "question": "q",
        "haystack_session_ids": ["s", "s", "s", "s#2", "single"],
        "haystack_dates": ["2024-01-01", "2024-01-02", "2024-01-03", null, null],
        "haystack_sessions": [
            [{"content": "same", "role": "user"}],
            [{"content": "same", "role": "user"}],
            [{"content": "same", "role": "user"}],
            [{"content": "reserved"}], [{"content": "single"}]
        ]
    }]))
    .unwrap();
    let item = &rows[0];
    let mapped = to_memory_inputs(item);
    let ids = mapped
        .episodes
        .iter()
        .map(|e| e.external_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["s", "s#3", "s#4", "s#2", "single"]);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    for ((episode, observation), source) in mapped
        .episodes
        .iter()
        .zip(&mapped.observations)
        .zip(&item.sessions)
    {
        assert_eq!(episode.started_at, source.date);
        assert_eq!(episode.ended_at, source.date);
        assert_eq!(observation.observed_at, source.date);
        assert_eq!(observation.episode_external_id, episode.external_id);
        assert_eq!(
            observation.external_id,
            format!("{}:turn:1", episode.external_id)
        );
        assert_eq!(observation.text, source.turns[0].text);
    }
    for episode in &mapped.episodes[..3] {
        assert_eq!(
            episode.summary,
            "Conversation session s containing messages between user."
        );
    }
}

#[test]
fn loaded_dataset_identities_are_unique_within_namespace_and_object_kind() {
    let rows = load_value(json!(["q1", "q2"].map(|question_id| json!({
        "question_id": question_id, "question": "q",
        "haystack_session_ids": ["s", "s", "s:turn:1", "s#2"],
        "haystack_sessions": [
            [{"content": "repeat"}], [{"content": "repeat"}],
            [{"content": "delimiter"}], [{"content": "suffix"}]
        ]
    }))))
    .unwrap();
    let mut identities = HashSet::new();
    for item in rows {
        let mapped = to_memory_inputs(&item);
        assert_eq!(mapped.episodes[1].external_id, "s#3");
        assert_eq!(mapped.episodes[2].external_id, "s:turn:1");
        assert_eq!(mapped.observations[0].external_id, "s:turn:1");
        assert_eq!(mapped.observations[2].external_id, "s:turn:1:turn:1");
        // Adapter registries and deterministic IDs include namespace and kind.
        for episode in mapped.episodes {
            assert!(identities.insert((episode.namespace, "episode", episode.external_id)));
        }
        for observation in mapped.observations {
            assert!(identities.insert((
                observation.namespace,
                "observation",
                observation.external_id
            )));
        }
    }
    assert_eq!(identities.len(), 16);
}

fn hit(kind: ObjectType, id: &str) -> RetrievedItem {
    RetrievedItem {
        kind,
        internal_id: id.into(),
        external_id: Some(id.into()),
        episode_external_id: None,
        score: None,
        rank: 1,
        rationale: vec![],
        text: None,
    }
}

#[test]
fn scoring_maps_exact_ids_and_credits_copies_only_at_the_first_rank() {
    let rows = load_value(json!([{
        "question_id": "q", "question": "q",
        "haystack_session_ids": ["s:turn:7", "s:turn:7", "other"],
        "haystack_sessions": [
            [{"content": "a", "has_answer": true}],
            [{"content": "a", "has_answer": true}],
            [{"content": "b", "has_answer": true}]
        ], "answer_session_ids": ["s:turn:7", "other"]
    }]))
    .unwrap();
    let item = &rows[0];
    let later = [
        hit(ObjectType::Episode, "s:turn:7#2"),
        hit(ObjectType::Observation, "s:turn:7#2:turn:1"),
    ];
    let metrics = score(item, &later, &[1], &[1]);
    for family in ["session", "turn"] {
        assert_eq!(metrics[format!("{family}_recall_any@1")], 1.0);
        assert_eq!(metrics[format!("{family}_recall_fraction@1")], 0.5);
    }
    let copies = [
        hit(ObjectType::Episode, "s:turn:7#2"),
        hit(ObjectType::Episode, "s:turn:7"),
        hit(ObjectType::Episode, "other"),
        hit(ObjectType::Observation, "s:turn:7#2:turn:1"),
        hit(ObjectType::Observation, "s:turn:7:turn:1"),
        hit(ObjectType::Observation, "other:turn:1"),
    ];
    let metrics = score(item, &copies, &[2, 3], &[2, 3]);
    for family in ["session", "turn"] {
        assert_eq!(metrics[format!("{family}_recall_fraction@2")], 0.5);
        assert_eq!(metrics[format!("{family}_recall_fraction@3")], 1.0);
        let expected = 1.5 / (1.0 + 1.0 / 3.0_f64.log2());
        assert!((metrics[format!("{family}_ndcg@3")].as_f64().unwrap() - expected).abs() < 1e-12);
    }
    assert!(
        metrics
            .as_object()
            .unwrap()
            .values()
            .all(|m| (0.0..=1.0).contains(&m.as_f64().unwrap()))
    );
    let mut one_gold = item.clone();
    one_gold.answer_session_ids.pop();
    one_gold.sessions[2].turns[0].has_answer = false;
    let metrics = score(&one_gold, &copies, &[3], &[3]);
    for value in metrics.as_object().unwrap().values() {
        assert_eq!(value, 1.0);
    }
}

#[test]
#[ignore = "set LONGMEMEVAL_DATASET and LONGMEMEVAL_IDENTITY_DUMP for the local official-file census"]
fn official_repeated_session_census_and_identity_dump() {
    let path = std::env::var("LONGMEMEVAL_DATASET").expect("LONGMEMEVAL_DATASET");
    let output = std::env::var("LONGMEMEVAL_IDENTITY_DUMP").expect("LONGMEMEVAL_IDENTITY_DUMP");
    let rows = load_path(std::path::Path::new(&path)).unwrap();
    assert_eq!(rows.len(), 500);
    let mut dump = Vec::new();
    let mut changed = Vec::new();
    let mut global_identities = HashSet::new();
    for item in &rows {
        let mapped = to_memory_inputs(item);
        let mut raw_seen = std::collections::HashMap::new();
        let mut assigned_seen = HashSet::new();
        for (session, episode) in item.sessions.iter().zip(&mapped.episodes) {
            assert!(!session.session_id.contains('#'));
            assert!(assigned_seen.insert(&episode.external_id));
            assert!(global_identities.insert((
                episode.namespace.clone(),
                "episode",
                episode.external_id.clone()
            )));
            assert_eq!(episode.started_at, session.date);
            if let Some(previous) = raw_seen.insert(&session.session_id, session) {
                assert_eq!(
                    serde_json::to_value(&previous.turns).unwrap(),
                    serde_json::to_value(&session.turns).unwrap()
                );
                assert_ne!(previous.date, session.date);
                assert!(!item.answer_session_ids.contains(&session.session_id));
                assert_eq!(episode.external_id, format!("{}#2", session.session_id));
                changed.push(item.question_id.clone());
            } else {
                assert_eq!(episode.external_id, session.session_id);
            }
            dump.push(
                json!({"item": item.question_id, "before": session.session_id,
                "after": episode.external_id, "date": episode.started_at}),
            );
        }
        for observation in &mapped.observations {
            assert!(global_identities.insert((
                observation.namespace.clone(),
                "observation",
                observation.external_id.clone()
            )));
        }
        assert_eq!(
            mapped
                .observations
                .iter()
                .map(|o| &o.external_id)
                .collect::<HashSet<_>>()
                .len(),
            mapped.observations.len()
        );
    }
    assert_eq!(changed.len(), 13);
    assert_eq!(changed.iter().collect::<HashSet<_>>().len(), 13);
    std::fs::write(output, serde_json::to_vec(&dump).unwrap()).unwrap();
    println!(
        "items={} episodes={} changed_copies=13 changed_items={}",
        rows.len(),
        dump.len(),
        serde_json::to_string(&changed).unwrap()
    );
}
