use crate::{LoCoMoQa, LoCoMoSample, LoCoMoSession, LoCoMoTurn};
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn load_path(path: &Path) -> Result<Vec<LoCoMoSample>> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    load_value(serde_json::from_str(&content)?)
}

pub fn load_value(value: Value) -> Result<Vec<LoCoMoSample>> {
    let rows = if let Some(array) = value.as_array() {
        array.clone()
    } else {
        ["data", "samples", "items"]
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_array).cloned())
            .unwrap_or_default()
    };
    Ok(rows.into_iter().map(parse_sample).collect())
}

fn parse_sample(raw: Value) -> LoCoMoSample {
    let sample_id = string_field(&raw, &["sample_id", "id"]).unwrap_or_else(|| "unknown".into());
    let conversation = raw
        .get("conversation")
        .or_else(|| raw.get("conversations"))
        .unwrap_or(&Value::Null);
    let speaker_a = string_field(conversation, &["speaker_a"]);
    let speaker_b = string_field(conversation, &["speaker_b"]);
    LoCoMoSample {
        sample_id: sample_id.clone(),
        sessions: parse_sessions(&raw),
        speaker_a,
        speaker_b,
        qa: parse_qa(raw.get("qa"), &sample_id),
        raw,
    }
}

fn parse_sessions(raw: &Value) -> Vec<LoCoMoSession> {
    let source = raw
        .get("conversation")
        .or_else(|| raw.get("conversations"))
        .unwrap_or(&Value::Null);
    match source {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(idx, item)| parse_session(item, idx))
            .collect(),
        Value::Object(map) => map
            .iter()
            .filter_map(|(key, item)| {
                session_number(key).and_then(|number| {
                    item.as_array().map(|_| {
                        let timestamp = map
                            .get(&format!("{key}_date_time"))
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                        (number, (key.clone(), item, timestamp))
                    })
                })
            })
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_iter()
            .map(|(_, (session_id, item, timestamp))| {
                parse_session_with_context(item, &session_id, timestamp, None)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_session(value: &Value, idx: usize) -> LoCoMoSession {
    let session_id = string_field(value, &["session_id", "session", "id"])
        .or_else(|| value.get("session_number").map(Value::to_string))
        .unwrap_or_else(|| format!("session_{}", idx + 1));
    parse_session_with_context(
        value,
        &session_id,
        string_field(value, &["timestamp", "date", "session_timestamp"]),
        string_field(value, &["session_summary", "summary"]),
    )
}

fn parse_session_with_context(
    value: &Value,
    session_id: &str,
    timestamp: Option<String>,
    summary: Option<String>,
) -> LoCoMoSession {
    let turns_value = value
        .get("turns")
        .or_else(|| value.get("dialog"))
        .or_else(|| value.get("conversation"))
        .or_else(|| value.as_array().map(|_| value));
    let turns = turns_value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(turn_idx, turn)| LoCoMoTurn {
                    dialog_id: string_field(turn, &["dia_id", "dialog_id", "id"])
                        .unwrap_or_else(|| format!("{session_id}:turn:{}", turn_idx + 1)),
                    speaker: string_field(turn, &["speaker", "role"]),
                    text: string_field(turn, &["text", "content", "utterance"]).unwrap_or_default(),
                    image_urls: string_array_field(
                        turn.get("img_url").or_else(|| turn.get("image_urls")),
                    ),
                    blip_caption: string_field(turn, &["blip_caption", "caption"]),
                    query: string_field(turn, &["query", "search_query"]),
                })
                .collect()
        })
        .unwrap_or_default();
    LoCoMoSession {
        session_id: session_id.to_string(),
        timestamp,
        summary,
        turns,
    }
}

fn parse_qa(value: Option<&Value>, sample_id: &str) -> Vec<LoCoMoQa> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(idx, qa)| LoCoMoQa {
                    question_id: string_field(qa, &["question_id", "qid", "id"])
                        .unwrap_or_else(|| format!("{sample_id}:qa:{}", idx + 1)),
                    qa_index: idx + 1,
                    question_type: scalar_field(qa, &["question_type", "category", "type"]),
                    question: string_field(qa, &["question", "q"]).unwrap_or_default(),
                    answer: scalar_field(qa, &["answer", "a"]),
                    evidence_dialog_ids: evidence_ids(
                        qa.get("evidence").or_else(|| qa.get("evidence_dialog_ids")),
                    ),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn evidence_ids(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                scalar_value(item).or_else(|| string_field(item, &["dia_id", "dialog_id", "id"]))
            })
            .collect(),
        Some(value) => scalar_value(value).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn scalar_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(scalar_value))
}

fn scalar_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn string_array_field(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn session_number(key: &str) -> Option<usize> {
    key.strip_prefix("session_")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_fixture() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": [{"session_id": "s1", "turns": [{"dia_id": "d1", "speaker": "A", "text": "likes tea"}]}],
            "qa": [{"question_id": "q1", "question": "What?", "evidence": ["d1"]}]
        }]))
        .unwrap();
        assert_eq!(rows[0].namespace(), "locomo:p1");
        assert_eq!(rows[0].evidence_sessions(&rows[0].qa[0]), vec!["s1"]);
    }

    #[test]
    fn parses_official_keyed_conversation_shape() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "speaker_a": "Caroline",
                "speaker_b": "Melanie",
                "session_10_date_time": "1:00 pm on 10 May, 2023",
                "session_10": [{"dia_id": "D10:1", "speaker": "Caroline", "text": "later"}],
                "session_2_date_time": "1:00 pm on 2 May, 2023",
                "session_2": [{
                    "dia_id": "D2:1",
                    "speaker": "Melanie",
                    "text": "look",
                    "img_url": ["https://example.test/a.jpg"],
                    "blip_caption": "a mural",
                    "query": "public mural"
                }]
            },
            "qa": [{"question": "What?", "answer": 2022, "category": 3, "evidence": ["D2:1"]}]
        }]))
        .unwrap();

        assert_eq!(rows[0].speaker_a.as_deref(), Some("Caroline"));
        assert_eq!(rows[0].speaker_b.as_deref(), Some("Melanie"));
        assert_eq!(rows[0].sessions.len(), 2);
        assert_eq!(rows[0].sessions[0].session_id, "session_2");
        assert_eq!(
            rows[0].sessions[0].timestamp.as_deref(),
            Some("1:00 pm on 2 May, 2023")
        );
        assert_eq!(
            rows[0].sessions[0].turns[0].blip_caption.as_deref(),
            Some("a mural")
        );
        assert_eq!(
            rows[0].sessions[0].turns[0].query.as_deref(),
            Some("public mural")
        );
        assert_eq!(
            rows[0].sessions[0].turns[0].image_urls,
            vec!["https://example.test/a.jpg"]
        );
        assert_eq!(rows[0].qa[0].question_id, "p1:qa:1");
        assert_eq!(rows[0].qa[0].qa_index, 1);
        assert_eq!(rows[0].qa[0].question_type.as_deref(), Some("3"));
        assert_eq!(rows[0].qa[0].answer.as_deref(), Some("2022"));
    }
}
