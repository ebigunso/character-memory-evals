use cmem_eval_longmemeval::{AdmissionLocation, LoadError, load_value};
use serde_json::{Value, json};

fn item() -> Value {
    json!({"question_id":"q1","question":"What?","haystack_session_ids":["s1"],"haystack_sessions":[[{"content":""}]]})
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
fn rejects_missing_question_id() {
    for value in [None, Some(json!(null)), Some(json!(17)), Some(json!(" \t"))] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("question_id");
        row["id"] = json!("q1");
        if let Some(value) = value {
            row["question_id"] = value;
        }
        rejected(json!([row]), "question_id", 0, None);
    }
}

#[test]
fn rejects_duplicate_item_ids() {
    rejected(json!([item(), item()]), "question_id", 1, Some("q1"));
}

#[test]
fn rejects_missing_question() {
    for value in [
        None,
        Some(json!(null)),
        Some(json!(false)),
        Some(json!("  ")),
    ] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("question");
        if let Some(value) = value {
            row["question"] = value;
        }
        rejected(json!([row]), "question", 0, Some("q1"));
    }
}

#[test]
fn rejects_missing_empty_or_malformed_sessions() {
    for value in [
        None,
        Some(json!(null)),
        Some(json!(3)),
        Some(json!({})),
        Some(json!([])),
    ] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("haystack_sessions");
        if let Some(value) = value {
            row["haystack_sessions"] = value;
        }
        rejected(json!([row]), "haystack_sessions", 0, Some("q1"));
    }
}

#[test]
fn rejects_scalar_session_entries() {
    for value in [
        json!(null),
        json!(true),
        json!("session"),
        json!(1),
        json!({"turns":[{"content":""}]}),
    ] {
        let mut row = item();
        row["haystack_sessions"] = json!([value]);
        rejected(json!([row]), "haystack_sessions[0]", 0, Some("q1"));
    }
}

#[test]
fn rejects_empty_session_turns() {
    let mut row = item();
    row["haystack_sessions"] = json!([[]]);
    rejected(json!([row]), "haystack_sessions[0].turns", 0, Some("q1"));
}

#[test]
fn rejects_non_object_turns_and_missing_or_non_string_text() {
    for turn in [json!(null), json!(1), json!("text"), json!([])] {
        let mut row = item();
        row["haystack_sessions"] = json!([[turn]]);
        rejected(json!([row]), "haystack_sessions[0].turns[0]", 0, Some("q1"));
    }
    for value in [None, Some(json!(null)), Some(json!(1)), Some(json!({}))] {
        let mut turn = json!({});
        if let Some(value) = value {
            turn["content"] = value;
        }
        let mut row = item();
        row["haystack_sessions"] = json!([[turn]]);
        rejected(
            json!([row]),
            "haystack_sessions[0].turns[0].content",
            0,
            Some("q1"),
        );
    }
}

#[test]
fn admits_empty_turn_text() {
    let rows = load_value(json!([item()])).unwrap();
    assert_eq!(rows[0].sessions[0].turns[0].text, "");
}

#[test]
fn rejects_missing_session_ids() {
    let mut row = item();
    row.as_object_mut().unwrap().remove("haystack_session_ids");
    rejected(json!([row]), "haystack_session_ids", 0, Some("q1"));
}

#[test]
fn rejects_misaligned_or_invalid_parallel_session_ids() {
    for ids in [
        json!(null),
        json!({}),
        json!("s1"),
        json!([]),
        json!(["s1", "s2"]),
    ] {
        let mut row = item();
        row["haystack_session_ids"] = ids;
        rejected(json!([row]), "haystack_session_ids", 0, Some("q1"));
    }
    for id in [json!(null), json!(1), json!({}), json!(false), json!(" ")] {
        for idx in 0..2 {
            let mut row = item();
            row["haystack_sessions"] = json!([[{"content":""}],[{"content":""}]]);
            row["haystack_session_ids"] = json!(["s1", "s2"]);
            row["haystack_session_ids"][idx] = id.clone();
            rejected(
                json!([row]),
                &format!("haystack_session_ids[{idx}]"),
                0,
                Some("q1"),
            );
        }
    }
}

#[test]
fn rejects_misaligned_or_malformed_parallel_dates() {
    for dates in [
        json!(0),
        json!({}),
        json!("date"),
        json!([]),
        json!([null, null]),
    ] {
        let mut row = item();
        row["haystack_dates"] = dates;
        rejected(json!([row]), "haystack_dates", 0, Some("q1"));
    }
}

#[test]
fn admits_missing_date_slots_without_shifting_later_dates() {
    for date in [json!(null), json!({}), json!(1), json!(false)] {
        let mut row = item();
        row["haystack_sessions"] = json!([[{"content":""}], [{"content":""}]]);
        row["haystack_session_ids"] = json!(["s1", "s2"]);
        row["haystack_dates"] = json!([date, "2023-01-02T00:00:00Z"]);
        let rows = load_value(json!([row])).unwrap();
        assert!(rows[0].sessions[0].raw_date.is_none());
        assert!(rows[0].sessions[0].date.is_none());
        assert_eq!(
            rows[0].sessions[1].raw_date.as_deref(),
            Some("2023-01-02T00:00:00Z")
        );
        assert_eq!(
            rows[0].sessions[1].date.as_deref(),
            Some("2023-01-02T00:00:00Z")
        );
    }
}

#[test]
fn rejects_repeated_session_ids_with_different_turns_including_has_answer() {
    for turn in [
        json!({"content":"different","has_answer":true}),
        json!({"content":"same","has_answer":false}),
    ] {
        let mut row = item();
        row["haystack_sessions"] = json!([
            [{"content":"same","has_answer":true}],
            [turn]
        ]);
        row["haystack_session_ids"] = json!(["s1", "s1"]);
        rejected(
            json!([row]),
            "haystack_sessions[1].session_id",
            0,
            Some("q1"),
        );
    }
}

#[test]
fn admits_parallel_session_repeats_with_identical_turns_and_different_dates() {
    let mut row = item();
    row["haystack_sessions"] = json!([
        [{"content":"same","has_answer":true}],
        [{"content":"same","has_answer":true}]
    ]);
    row["haystack_session_ids"] = json!(["s1", "s1"]);
    row["haystack_dates"] = json!(["2023-01-01T00:00:00Z", "2023-01-02T00:00:00Z"]);
    let rows = load_value(json!([row])).unwrap();
    assert_eq!(rows[0].sessions.len(), 2);
    for (session, date) in rows[0]
        .sessions
        .iter()
        .zip(["2023-01-01T00:00:00Z", "2023-01-02T00:00:00Z"])
    {
        assert_eq!(session.session_id, "s1");
        assert_eq!(session.raw_date.as_deref(), Some(date));
        assert_eq!(session.date.as_deref(), Some(date));
        assert_eq!(session.turns[0].text, "same");
        assert!(session.turns[0].has_answer);
    }
}

#[test]
fn admits_missing_or_malformed_optional_annotations() {
    for field in [
        "question_type",
        "answer",
        "question_date",
        "answer_session_ids",
    ] {
        for value in [None, Some(json!(null)), Some(json!({}))] {
            let mut row = item();
            if let Some(value) = value {
                row[field] = value;
            }
            let rows = load_value(json!([row])).unwrap();
            assert_eq!(rows.len(), 1, "{field}");
            assert!(rows[0].question_type.is_none());
            assert!(rows[0].answer.is_none());
            assert!(rows[0].question_date.is_none());
            assert!(rows[0].sessions[0].date.is_none());
            assert!(rows[0].sessions[0].turns[0].speaker.is_none());
        }
    }
    for value in [None, Some(json!(null))] {
        let mut row = item();
        if let Some(value) = value {
            row["haystack_dates"] = value;
        }
        let rows = load_value(json!([row])).unwrap();
        assert!(rows[0].sessions[0].raw_date.is_none());
        assert!(rows[0].sessions[0].date.is_none());
    }
}

#[test]
fn admits_abstention_answers_and_empty_turn_text() {
    let mut row = item();
    row["question_id"] = json!("q1_abs");
    row["answer"] = json!("The information provided is not enough.");
    let rows = load_value(json!([row])).unwrap();
    assert_eq!(
        rows[0].answer.as_deref(),
        Some("The information provided is not enough.")
    );
    assert!(rows[0].sessions[0].turns[0].text.is_empty());
}
