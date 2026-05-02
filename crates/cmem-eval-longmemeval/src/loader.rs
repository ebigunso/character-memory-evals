use crate::{LongMemEvalInstance, LongMemEvalSession, LongMemEvalTurn};
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
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
                    raw_date: string_field(item, &["date", "timestamp"])
                        .or_else(|| dates.get(idx).cloned()),
                    date: normalize_timestamp(
                        string_field(item, &["date", "timestamp"])
                            .or_else(|| dates.get(idx).cloned())
                            .as_deref(),
                    ),
                    turns: parse_turns(item),
                }
            })
            .collect(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(session_id, item)| LongMemEvalSession {
                session_id: session_id.clone(),
                raw_date: string_field(item, &["date", "timestamp"]),
                date: normalize_timestamp(string_field(item, &["date", "timestamp"]).as_deref()),
                turns: parse_turns(item),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn normalize_timestamp(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(timestamp) = DateTime::parse_from_rfc3339(trimmed) {
            return Some(
                timestamp
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::Secs, true),
            );
        }
        Some(
            parse_official_longmemeval_timestamp(trimmed)
                .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true))
                .unwrap_or_else(|| trimmed.to_string()),
        )
    })
}

fn parse_official_longmemeval_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let normalized = if let Some((date, rest)) = value.split_once(" (") {
        let (_, time) = rest.split_once(") ")?;
        format!("{date} {time}")
    } else {
        value.to_string()
    };
    NaiveDateTime::parse_from_str(&normalized, "%Y/%m/%d %H:%M")
        .ok()
        .map(|timestamp| DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc))
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

    #[test]
    fn normalizes_official_dates_to_rfc3339_utc() {
        let rows = load_value(serde_json::json!([{
            "question_id": "q1",
            "question": "Where is the answer?",
            "haystack_session_ids": ["s1"],
            "haystack_dates": ["2023/05/30 (Tue) 23:40"],
            "haystack_sessions": [[
                {"role": "user", "content": "hello", "has_answer": true}
            ]],
            "answer_session_ids": ["s1"]
        }]))
        .unwrap();

        assert_eq!(
            rows[0].sessions[0].raw_date.as_deref(),
            Some("2023/05/30 (Tue) 23:40")
        );
        assert_eq!(
            rows[0].sessions[0].date.as_deref(),
            Some("2023-05-30T23:40:00Z")
        );
    }

    #[test]
    fn preserves_unrecognized_non_empty_date_for_adapter_error_context() {
        let rows = load_value(serde_json::json!([{
            "question_id": "q1",
            "question": "Where is the answer?",
            "haystack_session_ids": ["s1"],
            "haystack_dates": ["not a timestamp"],
            "haystack_sessions": [[{"role": "user", "content": "hello"}]]
        }]))
        .unwrap();

        assert_eq!(rows[0].sessions[0].date.as_deref(), Some("not a timestamp"));
    }
}
