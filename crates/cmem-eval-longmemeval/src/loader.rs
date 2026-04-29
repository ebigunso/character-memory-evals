use crate::{LongMemEvalInstance, LongMemEvalSession, LongMemEvalTurn};
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn load_path(path: &Path) -> Result<Vec<LongMemEvalInstance>> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value = serde_json::from_str(&content)?;
    load_value(value)
}

pub fn load_value(value: Value) -> Result<Vec<LongMemEvalInstance>> {
    let rows = if let Some(array) = value.as_array() {
        array.clone()
    } else {
        ["data", "instances", "questions"]
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_array).cloned())
            .unwrap_or_default()
    };
    rows.into_iter().map(parse_instance).collect()
}

fn parse_instance(raw: Value) -> Result<LongMemEvalInstance> {
    let question_id =
        string_field(&raw, &["question_id", "id"]).unwrap_or_else(|| "unknown".into());
    let session_ids = string_array(raw.get("haystack_session_ids"));
    let dates = string_array(raw.get("haystack_dates"));
    let sessions = parse_sessions(raw.get("haystack_sessions"), &session_ids, &dates);
    Ok(LongMemEvalInstance {
        question_id,
        question_type: string_field(&raw, &["question_type", "type"]),
        question: string_field(&raw, &["question"]).unwrap_or_default(),
        answer: string_field(&raw, &["answer"]),
        question_date: string_field(&raw, &["question_date"]),
        sessions,
        answer_session_ids: string_array(raw.get("answer_session_ids")),
        raw,
    })
}

fn parse_sessions(
    value: Option<&Value>,
    ids: &[String],
    dates: &[String],
) -> Vec<LongMemEvalSession> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let session_id = string_field(item, &["session_id", "id"])
                    .or_else(|| ids.get(idx).cloned())
                    .unwrap_or_else(|| format!("session_{}", idx + 1));
                LongMemEvalSession {
                    session_id,
                    date: string_field(item, &["date", "timestamp"])
                        .or_else(|| dates.get(idx).cloned()),
                    turns: parse_turns(item),
                }
            })
            .collect(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(session_id, item)| LongMemEvalSession {
                session_id: session_id.clone(),
                date: string_field(item, &["date", "timestamp"]),
                turns: parse_turns(item),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_turns(value: &Value) -> Vec<LongMemEvalTurn> {
    let turns = value
        .get("turns")
        .or_else(|| value.get("messages"))
        .or_else(|| value.get("conversation"))
        .and_then(Value::as_array)
        .or_else(|| value.as_array());
    turns
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(idx, turn)| LongMemEvalTurn {
            index: idx + 1,
            speaker: string_field(turn, &["role", "speaker"]),
            text: string_field(turn, &["content", "text"]).unwrap_or_default(),
            has_answer: turn
                .get("has_answer")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect()
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tolerant_fixture() {
        let value = serde_json::json!([{
            "question_id": "q1",
            "question": "Where is the answer?",
            "haystack_session_ids": ["s1"],
            "haystack_sessions": [[
                {"role": "user", "content": "hello", "has_answer": true}
            ]],
            "answer_session_ids": ["s1"]
        }]);
        let rows = load_value(value).unwrap();
        assert_eq!(rows[0].namespace(), "lme:q1");
        assert_eq!(rows[0].gold_turn_ids(), vec!["s1:turn:1"]);
    }
}
