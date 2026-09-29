use cmem_eval_locomo::{AdmissionLocation, LoadError, load_value};
use serde_json::{Value, json};

fn item() -> Value {
    json!({"sample_id":"p1","conversation":{"session_1":[{"dia_id":"d1","text":""}]},"qa":[{"question":"What?"}]})
}

fn rejected(value: Value, field: &str, index: usize, id: Option<&str>) {
    let error = load_value(value).unwrap_err();
    match &error {
        LoadError::Admission {
            location,
            field: actual,
            reason,
        } => {
            assert_eq!(
                location,
                &AdmissionLocation::Item {
                    index,
                    id: id.map(str::to_owned)
                }
            );
            assert_eq!(actual, field);
            assert!(!reason.is_empty());
        }
        _ => panic!("{error:?}"),
    }
    assert!(error.to_string().contains(field));
}

#[test]
fn rejects_unrecognized_root_shape() {
    for value in [json!(null), json!(1), json!({}), json!({"data": [item()]})] {
        assert!(matches!(load_value(value), Err(LoadError::Admission {
            location: AdmissionLocation::Root, field, ..
        }) if field == "root"));
    }
}

#[test]
fn rejects_zero_items_at_root() {
    assert!(matches!(load_value(json!([])), Err(LoadError::Admission {
        location: AdmissionLocation::Root, field, ..
    }) if field == "root"));
}

#[test]
fn rejects_missing_sample_id() {
    for value in [None, Some(json!(null)), Some(json!(17)), Some(json!(" \t"))] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("sample_id");
        row["id"] = json!("p1");
        if let Some(value) = value {
            row["sample_id"] = value;
        }
        rejected(json!([row]), "sample_id", 0, None);
    }
}

#[test]
fn rejects_duplicate_item_ids() {
    rejected(json!([item(), item()]), "sample_id", 1, Some("p1"));
}

#[test]
fn rejects_missing_empty_or_malformed_conversation() {
    for value in [
        None,
        Some(json!(null)),
        Some(json!(3)),
        Some(json!({})),
        Some(json!([])),
    ] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("conversation");
        if let Some(value) = value {
            row["conversation"] = value;
        }
        rejected(json!([row]), "conversation", 0, Some("p1"));
    }
}

#[test]
fn rejects_malformed_keyed_sessions_without_dropping_siblings() {
    for value in [json!(null), json!({}), json!("turns"), json!(1), json!([])] {
        let mut row = item();
        row["conversation"] = json!({"session_1":[{"dia_id":"d1","text":""}], "session_2":value});
        rejected(json!([row]), "conversation.session_2", 0, Some("p1"));
    }
}

#[test]
fn rejects_non_object_turns_and_missing_or_non_string_text() {
    for turn in [json!(null), json!(1), json!("text"), json!([])] {
        let mut row = item();
        row["conversation"]["session_1"] = json!([turn]);
        rejected(json!([row]), "conversation.session_1[0]", 0, Some("p1"));
    }
    for value in [None, Some(json!(null)), Some(json!(1)), Some(json!({}))] {
        let mut turn = json!({"dia_id":"d1"});
        if let Some(value) = value {
            turn["text"] = value;
        }
        let mut row = item();
        row["conversation"]["session_1"] = json!([turn]);
        rejected(
            json!([row]),
            "conversation.session_1[0].text",
            0,
            Some("p1"),
        );
    }
}

#[test]
fn admits_empty_turn_text() {
    let rows = load_value(json!([item()])).unwrap();
    assert_eq!(rows[0].sessions[0].turns[0].text, "");
}

#[test]
fn rejects_missing_turn_ids() {
    for value in [None, Some(json!(null)), Some(json!(17)), Some(json!(" \t"))] {
        let mut turn = json!({"text":""});
        if let Some(value) = value {
            turn["dia_id"] = value;
        }
        let mut row = item();
        row["conversation"]["session_1"] = json!([turn]);
        rejected(
            json!([row]),
            "conversation.session_1[0].dia_id",
            0,
            Some("p1"),
        );
    }
}

#[test]
fn rejects_duplicate_turn_ids_within_and_across_sessions() {
    for same_session in [false, true] {
        let mut row = item();
        let turns = json!([{"dia_id":"d1","text":""},{"dia_id":"d1","text":""}]);
        let (conversation, field) = if same_session {
            (
                json!({"session_1":turns}),
                "conversation.session_1[1].dia_id",
            )
        } else {
            (
                json!({"session_1":[turns[0]],"session_2":[turns[1]]}),
                "conversation.session_2[0].dia_id",
            )
        };
        row["conversation"] = conversation;
        rejected(json!([row]), field, 0, Some("p1"));
    }
    let mut second = item();
    second["sample_id"] = json!("p2");
    assert_eq!(load_value(json!([item(), second])).unwrap().len(), 2);
}

#[test]
fn admits_dangling_evidence_references() {
    let mut row = item();
    row["qa"][0]["evidence"] = json!(["no-such-turn"]);
    let rows = load_value(json!([row])).unwrap();
    assert_eq!(rows[0].qa[0].evidence_dialog_ids, vec!["no-such-turn"]);
}

#[test]
fn rejects_missing_empty_or_non_array_qa() {
    for value in [None, Some(json!(null)), Some(json!({})), Some(json!([]))] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("qa");
        if let Some(value) = value {
            row["qa"] = value;
        }
        rejected(json!([row]), "qa", 0, Some("p1"));
    }
}

#[test]
fn rejects_non_object_qa_entries() {
    for value in [json!(null), json!(17), json!([])] {
        let mut row = item();
        row["qa"] = json!([value]);
        rejected(json!([row]), "qa[0]", 0, Some("p1"));
    }
}

#[test]
fn rejects_missing_qa_question() {
    for value in [
        None,
        Some(json!(null)),
        Some(json!(false)),
        Some(json!("  ")),
    ] {
        let mut row = item();
        row["qa"][0].as_object_mut().unwrap().remove("question");
        if let Some(value) = value {
            row["qa"][0]["question"] = value;
        }
        rejected(json!([row]), "qa[0].question", 0, Some("p1"));
    }
}

#[test]
fn rejects_duplicate_effective_qa_ids_within_and_across_items() {
    let mut row = item();
    row["qa"] = json!([{"question":"First?"},{"question":"Second?","question_id":"p1:qa:1"}]);
    rejected(json!([row]), "qa[1].question_id", 0, Some("p1"));
    let mut second = item();
    second["sample_id"] = json!("p2");
    second["qa"][0]["question_id"] = json!("p1:qa:1");
    rejected(json!([item(), second]), "qa[0].question_id", 1, Some("p2"));
}

#[test]
fn admits_missing_or_malformed_optional_annotations_and_derives_missing_qa_ids() {
    for field in ["question_id", "category", "answer", "evidence"] {
        for value in [None, Some(json!(null)), Some(json!({})), Some(json!(""))] {
            let mut row = item();
            if let Some(value) = value {
                row["qa"][0][field] = value;
            }
            let rows = load_value(json!([row])).unwrap();
            assert_eq!(rows[0].qa[0].question_id, "p1:qa:1", "{field}");
            assert!(rows[0].speaker_a.is_none());
            assert!(rows[0].speaker_b.is_none());
            assert!(rows[0].sessions[0].timestamp.is_none());
            assert!(rows[0].sessions[0].turns[0].speaker.is_none());
        }
    }
}

#[test]
fn rejects_noncanonical_or_unrecognized_session_keys() {
    for key in [
        "session_01",
        "session_00",
        "session_+1",
        "session_-1",
        "session_1.0",
        "session_",
        "session_x",
        "session_1_summary",
        "session_01_date_time",
        "session_1_date_time_extra",
    ] {
        let mut row = item();
        row["conversation"] =
            json!({"session_1":[{"dia_id":"d1","text":""}],key:[{"dia_id":"d2","text":""}]});
        rejected(json!([row]), &format!("conversation.{key}"), 0, Some("p1"));
    }
}

#[test]
fn admits_orphan_session_annotations_and_preserves_all_keyed_sessions() {
    let mut row = item();
    row["conversation"] = json!({"speaker_a":null,"speaker_b":{},"session_0":[{"dia_id":"d0","text":""}],"session_2":[{"dia_id":"d2","text":""}],"session_10":[{"dia_id":"d10","text":""}],"session_999999999999999999999999999999":[{"dia_id":"big","text":""}], "session_20_date_time":"orphan"});
    let rows = load_value(json!([row])).unwrap();
    assert_eq!(
        rows[0]
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "session_0",
            "session_2",
            "session_10",
            "session_999999999999999999999999999999"
        ]
    );
    assert!(
        rows[0]
            .sessions
            .iter()
            .all(|session| session.turns.len() == 1)
    );
    assert!(rows[0].speaker_a.is_none());
    assert!(rows[0].speaker_b.is_none());
}
