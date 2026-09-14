use cmem_eval_locomo::{AdmissionLocation, LoadError, load_value};
use serde_json::{Value, json};

#[test]
fn load_path_distinguishes_io_and_json_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("syntax.json");
    std::fs::write(&path, "{]").unwrap();
    let syntax_error = cmem_eval_locomo::load_path(&path).unwrap_err();
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(syntax_error, LoadError::Json(_)));
    let io_error = cmem_eval_locomo::load_path(&path).unwrap_err();
    assert!(
        matches!(io_error, LoadError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
    );
}

fn item() -> Value {
    json!({"sample_id":"p1","conversation":[{"session_id":"s1","turns":[{"dia_id":"d1","text":""}]}],"qa":[{"question":"What?"}]})
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
    for value in [json!(null), json!(1), json!({}), json!({"samples": {}})] {
        assert!(matches!(load_value(value), Err(LoadError::Admission {
            location: AdmissionLocation::Root, field, ..
        }) if field == "root"));
    }
}

#[test]
fn rejects_zero_items_at_root() {
    for value in [
        json!([]),
        json!({"data":[]}),
        json!({"samples":[]}),
        json!({"items":[]}),
    ] {
        assert!(matches!(load_value(value), Err(LoadError::Admission {
            location: AdmissionLocation::Root, field, ..
        }) if field == "root"));
    }
}

#[test]
fn rejects_missing_sample_id() {
    for value in [None, Some(json!(null)), Some(json!(17)), Some(json!(" \t"))] {
        let mut row = item();
        row.as_object_mut().unwrap().remove("sample_id");
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
fn rejects_non_object_array_sessions() {
    for value in [json!(null), json!(true), json!("session"), json!([])] {
        let mut row = item();
        row["conversation"] = json!([value, {"session_id":"s2","turns":[]}]);
        rejected(json!([row]), "conversation[0]", 0, Some("p1"));
    }
}

#[test]
fn rejects_missing_session_id() {
    for value in [None, Some(json!(null)), Some(json!(1)), Some(json!(" "))] {
        let mut row = item();
        row["conversation"][0]
            .as_object_mut()
            .unwrap()
            .remove("session_id");
        if let Some(value) = value {
            row["conversation"][0]["session_id"] = value;
        }
        row["conversation"][0]["session_number"] = json!(1);
        rejected(json!([row]), "conversation[0].session_id", 0, Some("p1"));
    }
}

#[test]
fn rejects_repeated_session_ids_with_different_turns() {
    let mut row = item();
    row["conversation"] = json!([{"session_id":"s1","turns":[{"dia_id":"d1","text":""}]},{"id":"s1","dialog":[{"dia_id":"d1","text":"different"}]}]);
    rejected(json!([row]), "conversation[1].session_id", 0, Some("p1"));
}

#[test]
fn rejects_repeated_session_ids_with_identical_turns_and_different_dates() {
    let mut row = item();
    row["conversation"] = json!([
        {"session_id":"s1","date":"2023-01-01T00:00:00Z","turns":[{"dia_id":"d1","text":"same"}]},
        {"id":"s1","date":"2023-01-02T00:00:00Z","dialog":[{"dia_id":"d1","text":"same"}]}
    ]);
    rejected(json!([row]), "conversation[1].session_id", 0, Some("p1"));
}

#[test]
fn rejects_missing_empty_or_malformed_object_session_turns() {
    for alias in ["turns", "dialog", "conversation"] {
        for value in [
            None,
            Some(json!(null)),
            Some(json!({})),
            Some(json!("turns")),
            Some(json!([])),
        ] {
            let mut row = item();
            row["conversation"][0]
                .as_object_mut()
                .unwrap()
                .remove("turns");
            if let Some(value) = value {
                row["conversation"][0][alias] = value;
            }
            rejected(json!([row]), "conversation[0].turns", 0, Some("p1"));
        }
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
    for keyed in [false, true] {
        let field = if keyed {
            "conversation.session_1[0]"
        } else {
            "conversation[0].turns[0]"
        };
        for turn in [json!(null), json!(1), json!("text"), json!([])] {
            let mut row = item();
            row["conversation"] = if keyed {
                json!({"session_1":[turn]})
            } else {
                json!([{"session_id":"s1","turns":[turn]}])
            };
            rejected(json!([row]), field, 0, Some("p1"));
        }
        for alias in ["text", "content", "utterance"] {
            for value in [None, Some(json!(null)), Some(json!(1)), Some(json!({}))] {
                let mut turn = json!({"dia_id":"d1"});
                if let Some(value) = value {
                    turn[alias] = value;
                }
                let mut row = item();
                row["conversation"] = if keyed {
                    json!({"session_1":[turn]})
                } else {
                    json!([{"session_id":"s1","turns":[turn]}])
                };
                rejected(json!([row]), &format!("{field}.text"), 0, Some("p1"));
            }
        }
    }
}

#[test]
fn admits_empty_turn_text_under_each_alias() {
    for alias in ["text", "content", "utterance"] {
        let mut row = item();
        row["conversation"][0]["turns"] = json!([{"dia_id":"d1",alias:""}]);
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(rows[0].sessions[0].turns[0].text, "", "{alias}");
    }
}

#[test]
fn rejects_missing_turn_ids_in_every_session_shape() {
    for alias in ["dia_id", "dialog_id", "id"] {
        for value in [None, Some(json!(null)), Some(json!(17)), Some(json!(" \t"))] {
            let mut turn = json!({"text":""});
            if let Some(value) = value {
                turn[alias] = value;
            }
            for keyed in [false, true] {
                let mut row = item();
                row["conversation"] = if keyed {
                    json!({"session_1":[turn]})
                } else {
                    json!([{"session_id":"s1","turns":[turn]}])
                };
                let field = if keyed {
                    "conversation.session_1[0].dia_id"
                } else {
                    "conversation[0].turns[0].dia_id"
                };
                rejected(json!([row]), field, 0, Some("p1"));
            }
        }
    }
}

#[test]
fn rejects_duplicate_turn_ids_within_and_across_sessions() {
    for keyed in [false, true] {
        for same_session in [false, true] {
            let mut row = item();
            let turns = json!([{"dia_id":"d1","text":""},{"id":"d1","text":""}]);
            let (conversation, field) = match (keyed, same_session) {
                (true, true) => (
                    json!({"session_1":turns}),
                    "conversation.session_1[1].dia_id",
                ),
                (true, false) => (
                    json!({"session_1":[turns[0]],"session_2":[turns[1]]}),
                    "conversation.session_2[0].dia_id",
                ),
                (false, true) => (
                    json!([{"session_id":"s1","turns":turns}]),
                    "conversation[0].turns[1].dia_id",
                ),
                (false, false) => (
                    json!([{"session_id":"s1","turns":[turns[0]]},{"session_id":"s2","turns":[turns[1]]}]),
                    "conversation[1].turns[0].dia_id",
                ),
            };
            row["conversation"] = conversation;
            rejected(json!([row]), field, 0, Some("p1"));
        }
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
    row["qa"] = json!([{"question":"First?"},{"question":"Second?","qid":"p1:qa:1"}]);
    rejected(json!([row]), "qa[1].question_id", 0, Some("p1"));
    for alias in ["question_id", "qid", "id"] {
        let mut second = item();
        second["sample_id"] = json!("p2");
        second["qa"][0][alias] = json!("p1:qa:1");
        rejected(json!([item(), second]), "qa[0].question_id", 1, Some("p2"));
    }
}

#[test]
fn admits_wrappers_and_key_aliases() {
    for wrapper in ["data", "samples", "items"] {
        for id in ["session_id", "session", "id"] {
            for turns in ["turns", "dialog", "conversation"] {
                let row = json!({"id":"p1","conversations":[{id:"s1",turns:[{"dialog_id":"d1","role":"A","content":"hello","search_query":"find"}]}],
                    "qa":[{"qid":"q1","q":"  What?  ","a":false,"type":2,"evidence_dialog_ids":["d1"]}]});
                let rows = load_value(json!({wrapper:[row]})).unwrap();
                assert_eq!(rows[0].sample_id, "p1");
                assert_eq!(rows[0].sessions[0].session_id, "s1", "{id}");
                assert_eq!(rows[0].qa[0].question_id, "q1");
                assert_eq!(rows[0].qa[0].question, "  What?  ");
                assert_eq!(rows[0].qa[0].answer.as_deref(), Some("false"));
                assert_eq!(rows[0].qa[0].question_type.as_deref(), Some("2"));
                assert_eq!(rows[0].qa[0].evidence_dialog_ids, vec!["d1"]);
                assert_eq!(rows[0].sessions[0].turns[0].dialog_id, "d1");
                assert_eq!(rows[0].sessions[0].turns[0].speaker.as_deref(), Some("A"));
                assert_eq!(rows[0].sessions[0].turns[0].text, "hello");
                assert_eq!(rows[0].sessions[0].turns[0].query.as_deref(), Some("find"));
            }
        }
    }
}

#[test]
fn preserves_numeric_annotation_lookup_for_noncanonical_record_ids() {
    for session_id in ["session_1", "session_01", "session_+1"] {
        let mut row = item();
        row["conversation"][0]["session_id"] = json!(session_id);
        row["session_summary"] = json!({"1":"Retained summary"});
        row["observation"] = json!({"1":["Retained observation"]});
        let rows = load_value(json!([row])).unwrap();
        let session = &rows[0].sessions[0];
        assert_eq!(session.session_id, session_id);
        assert_eq!(session.summary.as_deref(), Some("Retained summary"));
        assert_eq!(
            session
                .generated_observations
                .iter()
                .map(|observation| observation.statement.as_str())
                .collect::<Vec<_>>(),
            ["Retained observation"]
        );
    }
}

#[test]
fn admits_session_annotation_aliases() {
    for alias in ["timestamp", "date", "session_timestamp"] {
        let mut row = item();
        row["conversation"][0][alias] = json!("2023-01-01T00:00:00Z");
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(
            rows[0].sessions[0].raw_timestamp.as_deref(),
            Some("2023-01-01T00:00:00Z"),
            "{alias}"
        );
    }
    for alias in ["session_summary", "summary"] {
        let mut row = item();
        row["conversation"][0][alias] = json!("A summary");
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(
            rows[0].sessions[0].summary.as_deref(),
            Some("A summary"),
            "{alias}"
        );
    }
    for alias in ["observation", "observations", "generated_observations"] {
        let mut row = item();
        row["conversation"][0][alias] = json!(["A fact"]);
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(
            rows[0].sessions[0]
                .generated_observations
                .iter()
                .map(|observation| observation.statement.as_str())
                .collect::<Vec<_>>(),
            vec!["A fact"],
            "{alias}"
        );
    }
}

#[test]
fn admits_turn_identity_text_and_image_aliases() {
    for alias in ["dia_id", "dialog_id", "id"] {
        let mut row = item();
        row["conversation"][0]["turns"][0]
            .as_object_mut()
            .unwrap()
            .remove("dia_id");
        row["conversation"][0]["turns"][0][alias] = json!("d1");
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(rows[0].sessions[0].turns[0].dialog_id, "d1", "{alias}");
    }
    for alias in ["content", "utterance"] {
        let mut row = item();
        row["conversation"][0]["turns"][0]
            .as_object_mut()
            .unwrap()
            .remove("text");
        row["conversation"][0]["turns"][0][alias] = json!("hello");
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(rows[0].sessions[0].turns[0].text, "hello", "{alias}");
    }
    let mut row = item();
    row["conversation"][0]["turns"][0] = json!({"dia_id":"d1","text":"","image_urls":["https://example.test/image"], "caption":"A mural"});
    let rows = load_value(json!([row])).unwrap();
    let turn = &rows[0].sessions[0].turns[0];
    assert_eq!(turn.image_urls, vec!["https://example.test/image"]);
    assert_eq!(turn.blip_caption.as_deref(), Some("A mural"));
}

#[test]
fn admits_evidence_object_identity_aliases() {
    for field in ["evidence", "evidence_dialog_ids"] {
        for alias in ["dia_id", "dialog_id", "id"] {
            let mut row = item();
            row["qa"][0][field] = json!([{alias:"d1"}]);
            let rows = load_value(json!([row])).unwrap();
            assert_eq!(
                rows[0].qa[0].evidence_dialog_ids,
                vec!["d1"],
                "{field}/{alias}"
            );
        }
    }
}

#[test]
fn admits_qa_identity_and_category_aliases() {
    for alias in ["question_id", "qid", "id"] {
        let mut row = item();
        row["qa"][0][alias] = json!("explicit");
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(rows[0].qa[0].question_id, "explicit", "{alias}");
    }
    for alias in ["question_type", "type"] {
        let mut row = item();
        row["qa"][0][alias] = json!("kind");
        let rows = load_value(json!([row])).unwrap();
        assert_eq!(
            rows[0].qa[0].question_type.as_deref(),
            Some("kind"),
            "{alias}"
        );
    }
}

#[test]
fn admits_missing_or_malformed_optional_annotations_and_derives_missing_qa_ids() {
    for field in [
        "question_id",
        "qid",
        "id",
        "question_type",
        "category",
        "type",
        "answer",
        "a",
        "evidence",
    ] {
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
    for field in [
        "timestamp",
        "date",
        "session_timestamp",
        "summary",
        "session_summary",
        "observation",
        "observations",
        "generated_observations",
    ] {
        let mut row = item();
        row["conversation"][0][field] = json!(null);
        assert_eq!(load_value(json!([row])).unwrap().len(), 1, "{field}");
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
