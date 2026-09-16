use crate::{
    AdmissionLocation, LoadError, LongMemEvalInstance, LongMemEvalSession, LongMemEvalTurn,
};
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub fn load_path(path: &Path) -> Result<Vec<LongMemEvalInstance>, LoadError> {
    let content = fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value = serde_json::from_str(&content).map_err(LoadError::Json)?;
    load_value(value)
}

pub fn load_value(value: Value) -> Result<Vec<LongMemEvalInstance>, LoadError> {
    let rows = if let Some(array) = value.as_array() {
        array.clone()
    } else {
        ["data", "instances", "questions"]
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_array).cloned())
            .ok_or_else(|| {
                AdmissionLocation::Root.error(
                    "root",
                    "expected an array or a data/instances/questions array",
                )
            })?
    };
    if rows.is_empty() {
        return Err(AdmissionLocation::Root.error("root", "expected at least one item"));
    }
    let mut ids = HashSet::new();
    rows.into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let item = parse_instance(raw, index)?;
            if !ids.insert(item.question_id.clone()) {
                return Err(AdmissionLocation::Item {
                    index,
                    id: Some(item.question_id.clone()),
                }
                .error("question_id", "duplicate item id"));
            }
            Ok(item)
        })
        .collect()
}

fn parse_instance(raw: Value, index: usize) -> Result<LongMemEvalInstance, LoadError> {
    let id = nonblank_string_field(&raw, &["question_id", "id"]);
    let location = AdmissionLocation::Item {
        index,
        id: id.clone(),
    };
    let question_id =
        id.ok_or_else(|| location.error("question_id", "expected a non-blank string"))?;
    let question = nonblank_string_field(&raw, &["question"])
        .ok_or_else(|| location.error("question", "expected a non-blank string"))?;
    let sessions = parse_sessions(&raw, &location)?;
    Ok(LongMemEvalInstance {
        question_id,
        question_type: string_field(&raw, &["question_type", "type"]),
        question,
        answer: string_field(&raw, &["answer"]),
        question_date: string_field(&raw, &["question_date"]),
        sessions,
        answer_session_ids: string_array(raw.get("answer_session_ids")),
    })
}

fn parse_sessions(
    raw: &Value,
    location: &AdmissionLocation,
) -> Result<Vec<LongMemEvalSession>, LoadError> {
    let items = raw
        .get("haystack_sessions")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| location.error("haystack_sessions", "expected a non-empty session array"))?;
    if let Some(value) = raw.get("haystack_session_ids") {
        let ids = value
            .as_array()
            .ok_or_else(|| location.error("haystack_session_ids", "expected an array"))?;
        if ids.len() != items.len() {
            return Err(location.error("haystack_session_ids", "expected one entry per session"));
        }
        for (idx, id) in ids.iter().enumerate() {
            if id.as_str().is_none_or(|id| id.trim().is_empty()) {
                return Err(location.error(
                    format!("haystack_session_ids[{idx}]"),
                    "expected a non-blank string",
                ));
            }
        }
    }
    let dates = match raw.get("haystack_dates") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_array()
                .ok_or_else(|| location.error("haystack_dates", "expected an array or null"))?,
        ),
    };
    if dates.is_some_and(|dates| dates.len() != items.len()) {
        return Err(location.error("haystack_dates", "expected one entry per session"));
    }
    let mut ids = HashMap::new();
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            if !item.is_object() && !item.is_array() {
                return Err(location.error(
                    format!("haystack_sessions[{idx}]"),
                    "expected a session object or turn array",
                ));
            }
            let record_id = nonblank_string_field(item, &["session_id", "id"]);
            let from_record = record_id.is_some();
            let session_id = record_id
                .or_else(|| {
                    raw.get("haystack_session_ids")?
                        .get(idx)?
                        .as_str()
                        .map(ToOwned::to_owned)
                })
                .ok_or_else(|| {
                    location.error(
                        format!("haystack_sessions[{idx}].session_id"),
                        "expected a non-blank session id in the record or parallel array",
                    )
                })?;
            let turns = turn_values(item)
                .filter(|turns| !turns.is_empty())
                .ok_or_else(|| {
                    location.error(
                        format!("haystack_sessions[{idx}].turns"),
                        "expected a non-empty turn array",
                    )
                })?;
            if ids
                .insert(session_id.clone(), (from_record, turns))
                .is_some_and(|(previous_from_record, previous_turns)| {
                    from_record || previous_from_record || previous_turns != turns
                })
            {
                return Err(location.error(
                    format!("haystack_sessions[{idx}].session_id"),
                    "repeated session id requires parallel-array ids and identical turns",
                ));
            }
            let raw_date = string_field(item, &["date", "timestamp"])
                .or_else(|| dates?.get(idx)?.as_str().map(ToOwned::to_owned));
            Ok(LongMemEvalSession {
                session_id,
                date: normalize_timestamp(raw_date.as_deref()),
                raw_date,
                turns: parse_turns(turns, location, &format!("haystack_sessions[{idx}].turns"))?,
            })
        })
        .collect()
}

fn nonblank_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
        })
        .map(ToOwned::to_owned)
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

fn turn_values(value: &Value) -> Option<&Vec<Value>> {
    value
        .get("turns")
        .or_else(|| value.get("messages"))
        .or_else(|| value.get("conversation"))
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
}

fn parse_turns(
    turns: &[Value],
    location: &AdmissionLocation,
    field: &str,
) -> Result<Vec<LongMemEvalTurn>, LoadError> {
    turns
        .iter()
        .enumerate()
        .map(|(idx, turn)| {
            if !turn.is_object() {
                return Err(location.error(format!("{field}[{idx}]"), "expected a turn object"));
            }
            Ok(LongMemEvalTurn {
                index: idx + 1,
                speaker: string_field(turn, &["role", "speaker"]),
                text: string_field(turn, &["content", "text"]).ok_or_else(|| {
                    location.error(format!("{field}[{idx}].content"), "expected a string")
                })?,
                has_answer: turn
                    .get("has_answer")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
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
    fn normalizes_official_session_dates_to_rfc3339_utc() {
        let rows = load_value(serde_json::json!([{
            "question_id": "q1",
            "question": "Where is the answer?",
            "haystack_session_ids": ["s1"],
            "haystack_dates": ["2023/05/20 (Sat) 02:21"],
            "haystack_sessions": [[{"role": "user", "content": "hello"}]]
        }]))
        .unwrap();

        assert_eq!(
            rows[0].sessions[0].date.as_deref(),
            Some("2023-05-20T02:21:00Z")
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
