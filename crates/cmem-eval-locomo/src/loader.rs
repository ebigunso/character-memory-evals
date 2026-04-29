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
    LoCoMoSample {
        sample_id,
        sessions: parse_sessions(&raw),
        qa: parse_qa(raw.get("qa")),
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
            .enumerate()
            .map(|(idx, (key, item))| {
                let mut session = parse_session(item, idx);
                if session.session_id.starts_with("session_") {
                    session.session_id = key.clone();
                }
                session
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_session(value: &Value, idx: usize) -> LoCoMoSession {
    let session_id = string_field(value, &["session_id", "session", "id"])
        .or_else(|| value.get("session_number").map(Value::to_string))
        .unwrap_or_else(|| format!("session_{}", idx + 1));
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
                })
                .collect()
        })
        .unwrap_or_default();
    LoCoMoSession {
        session_id,
        timestamp: string_field(value, &["timestamp", "date", "session_timestamp"]),
        summary: string_field(value, &["session_summary", "summary"]),
        turns,
    }
}

fn parse_qa(value: Option<&Value>) -> Vec<LoCoMoQa> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(idx, qa)| LoCoMoQa {
                    question_id: string_field(qa, &["question_id", "qid", "id"])
                        .unwrap_or_else(|| format!("qa_{}", idx + 1)),
                    question_type: string_field(qa, &["question_type", "category", "type"]),
                    question: string_field(qa, &["question", "q"]).unwrap_or_default(),
                    answer: string_field(qa, &["answer", "a"]),
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
                item.as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| string_field(item, &["dia_id", "dialog_id", "id"]))
            })
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
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
}
