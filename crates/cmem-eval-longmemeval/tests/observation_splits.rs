use cmem_eval::{ObjectType, RetrievedItem};
use cmem_eval_longmemeval::{
    full_history_text, ingest::to_memory_inputs, load_value, scoring::score,
};

#[test]
fn splits_oversized_turn_into_consecutive_observations_without_changing_source_or_gold() {
    let text = format!(
        "{}End.\n\n{}End.",
        "alpha ".repeat(4500),
        "beta ".repeat(4500)
    );
    let input = serde_json::json!([{
        "question_id": "oversized-turn",
        "question": "What happened?",
        "haystack_session_ids": ["s1"],
        "haystack_dates": ["2023/05/30 (Tue) 23:40"],
        "haystack_sessions": [[
            {"role": "user", "content": text, "has_answer": true},
            {"role": "assistant", "content": "Next turn."}
        ]],
        "answer_session_ids": ["s1"]
    }]);
    let item = load_value(input.clone()).unwrap().remove(0);
    let mapped = to_memory_inputs(&item);
    assert_eq!(mapped.observations.len(), 3);
    assert_eq!(mapped.observations[0].external_id, "s1:turn:1");
    assert_eq!(mapped.observations[1].external_id, "s1:turn:1:part:2");
    assert_eq!(mapped.observations[2].external_id, "s1:turn:2");
    assert_eq!(
        mapped.observations[..2]
            .iter()
            .map(|o| o.text.as_str())
            .collect::<String>(),
        text
    );
    for observation in &mapped.observations[..2] {
        let surface = format!(
            "Observation excerpt: {}",
            observation
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        );
        assert!(
            tiktoken_rs::cl100k_base_singleton()
                .encode_ordinary(&surface)
                .len()
                <= 8192
        );
        assert_eq!(observation.episode_external_id, "s1");
        assert_eq!(observation.speaker.as_deref(), Some("user"));
        assert_eq!(
            observation.observed_at.as_deref(),
            Some("2023-05-30T23:40:00Z")
        );
        assert!(!observation.metadata.to_string().contains("has_answer"));
    }
    assert_eq!(
        full_history_text(&item),
        format!("user: {text}\nassistant: Next turn.")
    );
    assert_eq!(item.gold_turn_ids(), ["s1:turn:1"]);
    let retrieved = mapped.observations[..2]
        .iter()
        .enumerate()
        .map(|(index, observation)| RetrievedItem {
            kind: ObjectType::Observation,
            internal_id: format!("id-{index}"),
            external_id: Some(observation.external_id.clone()),
            episode_external_id: Some("s1".into()),
            score: None,
            rank: index + 1,
            rationale: vec![],
            text: None,
        })
        .collect::<Vec<_>>();
    let metrics = score(&item, &retrieved[1..], &[1], &[1]);
    assert_eq!(metrics["turn_recall_fraction@1"], 1.0);
    assert_eq!(metrics["turn_mrr@1"], 1.0);
    assert_eq!(
        score(&item, &retrieved, &[1], &[2])["turn_recall_fraction@2"],
        1.0
    );
    assert_eq!(
        serde_json::to_value(&item).unwrap(),
        serde_json::to_value(load_value(input).unwrap().remove(0)).unwrap()
    );
}

#[test]
fn leading_blank_lines_stay_with_the_first_content_part() {
    for prefix in ["\n\n", "\r\n\r\n"] {
        let text = format!("{prefix}{}", "alpha ".repeat(9000));
        let item = load_value(serde_json::json!([{
            "question_id": "leading-blank-lines",
            "question": "What happened?",
            "haystack_session_ids": ["s1"],
            "haystack_sessions": [[
                {"content": text, "has_answer": true},
                {"content": "Second gold turn.", "has_answer": true}
            ]]
        }]))
        .unwrap()
        .remove(0);
        let mapped = to_memory_inputs(&item);
        assert!(
            mapped
                .observations
                .iter()
                .all(|o| !o.text.trim().is_empty())
        );
        assert_eq!(mapped.observations.len(), 3);
        assert_eq!(mapped.observations[0].external_id, "s1:turn:1");
        assert!(mapped.observations[0].text.starts_with(prefix));
        assert!(mapped.observations[0].text.contains("alpha"));
        assert_eq!(
            mapped.observations[..2]
                .iter()
                .map(|o| o.text.as_str())
                .collect::<String>(),
            text
        );
        for observation in &mapped.observations[..2] {
            let clean = observation
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let tokens = tiktoken_rs::cl100k_base_singleton()
                .encode_ordinary(&format!("Observation excerpt: {clean}"))
                .len();
            assert!(tokens <= 8192);
        }
    }
}

#[test]
fn unsplittable_oversized_turn_names_the_question_before_ingest() {
    let error = load_value(serde_json::json!([{
        "question_id": "unsplittable-turn",
        "question": "What happened?",
        "haystack_session_ids": ["s1"],
        "haystack_sessions": [[{"content": "x".repeat(80000)}]]
    }]))
    .unwrap_err();
    assert!(matches!(
        error,
        cmem_eval_longmemeval::LoadError::Admission { .. }
    ));
    let message = error.to_string();
    assert!(message.contains("unsplittable-turn"));
    assert!(message.contains("haystack_sessions[0].turns[0].content"));
    assert!(message.contains("8192-token"));
}
