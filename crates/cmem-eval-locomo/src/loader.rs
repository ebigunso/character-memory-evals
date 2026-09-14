use crate::{
    AdmissionLocation, LoCoMoGeneratedObservation, LoCoMoQa, LoCoMoSample, LoCoMoSession,
    LoCoMoTurn, LoadError,
};
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn load_path(path: &Path) -> Result<Vec<LoCoMoSample>, LoadError> {
    let content = fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    load_value(serde_json::from_str(&content).map_err(LoadError::Json)?)
}

pub fn load_value(value: Value) -> Result<Vec<LoCoMoSample>, LoadError> {
    let rows = if let Some(array) = value.as_array() {
        array.clone()
    } else {
        ["data", "samples", "items"]
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_array).cloned())
            .ok_or_else(|| {
                AdmissionLocation::Root
                    .error("root", "expected an array or a data/samples/items array")
            })?
    };
    if rows.is_empty() {
        return Err(AdmissionLocation::Root.error("root", "expected at least one item"));
    }
    let mut ids = HashSet::new();
    let mut qa_ids = HashSet::new();
    rows.into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let sample = parse_sample(raw, index)?;
            let location = AdmissionLocation::Item {
                index,
                id: Some(sample.sample_id.clone()),
            };
            if !ids.insert(sample.sample_id.clone()) {
                return Err(location.error("sample_id", "duplicate item id"));
            }
            for (qa_index, qa) in sample.qa.iter().enumerate() {
                if !qa_ids.insert(qa.question_id.clone()) {
                    return Err(location.error(
                        format!("qa[{qa_index}].question_id"),
                        "duplicate effective QA id",
                    ));
                }
            }
            Ok(sample)
        })
        .collect()
}

fn parse_sample(raw: Value, index: usize) -> Result<LoCoMoSample, LoadError> {
    let id = nonblank_string_field(&raw, &["sample_id", "id"]);
    let location = AdmissionLocation::Item {
        index,
        id: id.clone(),
    };
    let sample_id = id.ok_or_else(|| location.error("sample_id", "expected a non-blank string"))?;
    let conversation = raw
        .get("conversation")
        .or_else(|| raw.get("conversations"))
        .unwrap_or(&Value::Null);
    let speaker_a = string_field(conversation, &["speaker_a"]);
    let speaker_b = string_field(conversation, &["speaker_b"]);
    let mut sample = LoCoMoSample {
        sample_id: sample_id.clone(),
        sessions: parse_sessions(&raw, &location)?,
        speaker_a,
        speaker_b,
        qa: parse_qa(raw.get("qa"), &sample_id, &location)?,
        unresolved_evidence_references: 0,
        dropped_observation_entries: 0,
    };
    apply_benchmark_derived_fields(&raw, &mut sample);
    Ok(sample)
}

fn parse_sessions(
    raw: &Value,
    location: &AdmissionLocation,
) -> Result<Vec<LoCoMoSession>, LoadError> {
    let source = raw
        .get("conversation")
        .or_else(|| raw.get("conversations"))
        .unwrap_or(&Value::Null);
    let sessions = match source {
        Value::Array(items) => {
            let mut ids = HashSet::new();
            items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let session = parse_session(item, idx, location)?;
                    if !ids.insert(session.session_id.clone()) {
                        return Err(location.error(
                            format!("conversation[{idx}].session_id"),
                            "duplicate session id",
                        ));
                    }
                    Ok(session)
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        Value::Object(map) => {
            let mut entries = Vec::new();
            for (key, item) in map {
                if let Some(number) = canonical_session_number(key) {
                    entries.push((number, key, item));
                } else if key.starts_with("session_")
                    && key
                        .strip_suffix("_date_time")
                        .and_then(canonical_session_number)
                        .is_none()
                {
                    return Err(
                        location.error(format!("conversation.{key}"), "unrecognized session key")
                    );
                }
            }
            entries.sort_by_key(|(number, _, _)| (number.len(), *number));
            entries
                .into_iter()
                .map(|(_, session_id, item)| {
                    if item.as_array().is_none_or(Vec::is_empty) {
                        return Err(location.error(
                            format!("conversation.{session_id}"),
                            "expected a non-empty turn array",
                        ));
                    }
                    let timestamp = map
                        .get(&format!("{session_id}_date_time"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    parse_session_with_context(
                        item,
                        session_id,
                        timestamp,
                        None,
                        location,
                        &format!("conversation.{session_id}"),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => {
            return Err(location.error(
                "conversation",
                "expected a session array or keyed session object",
            ));
        }
    };
    if sessions.is_empty() {
        return Err(location.error("conversation", "expected at least one session"));
    }
    let mut turn_ids = HashSet::new();
    for (idx, session) in sessions.iter().enumerate() {
        for (turn_idx, turn) in session.turns.iter().enumerate() {
            if !turn_ids.insert(&turn.dialog_id) {
                let field = if source.is_array() {
                    format!("conversation[{idx}].turns[{turn_idx}].dia_id")
                } else {
                    format!("conversation.{}[{turn_idx}].dia_id", session.session_id)
                };
                return Err(location.error(field, "duplicate turn id"));
            }
        }
    }
    Ok(sessions)
}

fn parse_session(
    value: &Value,
    idx: usize,
    location: &AdmissionLocation,
) -> Result<LoCoMoSession, LoadError> {
    if !value.is_object() {
        return Err(location.error(format!("conversation[{idx}]"), "expected a session object"));
    }
    let session_id =
        nonblank_string_field(value, &["session_id", "session", "id"]).ok_or_else(|| {
            location.error(
                format!("conversation[{idx}].session_id"),
                "expected a non-blank string",
            )
        })?;
    parse_session_with_context(
        value,
        &session_id,
        string_field(value, &["timestamp", "date", "session_timestamp"]),
        string_field(value, &["session_summary", "summary"]),
        location,
        &format!("conversation[{idx}].turns"),
    )
}

fn parse_session_with_context(
    value: &Value,
    session_id: &str,
    timestamp: Option<String>,
    summary: Option<String>,
    location: &AdmissionLocation,
    field: &str,
) -> Result<LoCoMoSession, LoadError> {
    let raw_timestamp = timestamp;
    let turns = turn_values(value)
        .filter(|turns| !turns.is_empty())
        .ok_or_else(|| location.error(field, "expected a non-empty turn array"))?
        .iter()
        .enumerate()
        .map(|(turn_idx, turn)| {
            if !turn.is_object() {
                return Err(
                    location.error(format!("{field}[{turn_idx}]"), "expected a turn object")
                );
            }
            Ok(LoCoMoTurn {
                dialog_id: nonblank_string_field(turn, &["dia_id", "dialog_id", "id"]).ok_or_else(
                    || {
                        location.error(
                            format!("{field}[{turn_idx}].dia_id"),
                            "expected a non-blank string",
                        )
                    },
                )?,
                speaker: string_field(turn, &["speaker", "role"]),
                text: string_field(turn, &["text", "content", "utterance"]).ok_or_else(|| {
                    location.error(format!("{field}[{turn_idx}].text"), "expected a string")
                })?,
                image_urls: string_array_field(
                    turn.get("img_url").or_else(|| turn.get("image_urls")),
                ),
                blip_caption: string_field(turn, &["blip_caption", "caption"]),
                query: string_field(turn, &["query", "search_query"]),
            })
        })
        .collect::<Result<_, LoadError>>()?;
    Ok(LoCoMoSession {
        session_id: session_id.to_string(),
        timestamp: normalize_timestamp(raw_timestamp.as_deref()),
        raw_timestamp,
        summary,
        generated_observations: Vec::new(),
        turns,
    })
}

fn turn_values(value: &Value) -> Option<&Vec<Value>> {
    value
        .get("turns")
        .or_else(|| value.get("dialog"))
        .or_else(|| value.get("conversation"))
        .or_else(|| value.as_array().map(|_| value))
        .and_then(Value::as_array)
}

fn apply_benchmark_derived_fields(raw: &Value, sample: &mut LoCoMoSample) {
    let turn_ids = sample
        .sessions
        .iter()
        .flat_map(|session| &session.turns)
        .map(|turn| turn.dialog_id.clone())
        .collect::<HashSet<_>>();
    let records = raw
        .get("conversation")
        .or_else(|| raw.get("conversations"))
        .and_then(Value::as_array);
    let mut parser = ObservationParser {
        turn_ids,
        unresolved: 0,
        dropped: 0,
    };
    if raw
        .get("observation")
        .is_some_and(|value| !value.is_object())
    {
        parser.dropped += 1;
    }
    for (index, session) in sample.sessions.iter_mut().enumerate() {
        if session.summary.is_none() {
            session.summary =
                annotation(raw.get("session_summary"), &session.session_id, "summary")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
        }
        if let Some(record) = records.and_then(|records| records.get(index))
            && let Some(value) = ["observation", "observations", "generated_observations"]
                .iter()
                .find_map(|key| record.get(*key))
        {
            parser.append(value, &mut session.generated_observations);
        }
        if let Some(value) = annotation(raw.get("observation"), &session.session_id, "observation")
        {
            parser.append(value, &mut session.generated_observations);
        }
    }
    sample.unresolved_evidence_references = parser.unresolved;
    sample.dropped_observation_entries = parser.dropped;
}

fn annotation<'a>(map: Option<&'a Value>, session_id: &str, suffix: &str) -> Option<&'a Value> {
    let map = map?.as_object()?;
    map.get(session_id).or_else(|| {
        let number = session_number(session_id)?;
        map.get(&format!("session_{number}_{suffix}"))
            .or_else(|| map.get(&number.to_string()))
    })
}

struct ObservationParser {
    turn_ids: HashSet<String>,
    unresolved: usize,
    dropped: usize,
}

impl ObservationParser {
    fn append(&mut self, value: &Value, output: &mut Vec<LoCoMoGeneratedObservation>) {
        match value {
            Value::Object(speakers) => {
                for (speaker, entries) in speakers {
                    if let Some(entries) = entries.as_array() {
                        for entry in entries {
                            self.entry(entry, Some(speaker), output);
                        }
                    } else {
                        self.dropped += 1;
                    }
                }
            }
            Value::Array(entries) => {
                for entry in entries {
                    self.entry(entry, None, output);
                }
            }
            entry => self.entry(entry, None, output),
        }
    }

    fn entry(
        &mut self,
        value: &Value,
        speaker: Option<&str>,
        output: &mut Vec<LoCoMoGeneratedObservation>,
    ) {
        let parsed = match value {
            Value::String(statement) => Some((statement.as_str(), Vec::new())),
            Value::Array(pair) if pair.len() == 2 => pair[0].as_str().and_then(|statement| {
                let evidence = match &pair[1] {
                    Value::String(id) => Some(vec![id.as_str()]),
                    Value::Array(ids) => ids.iter().map(Value::as_str).collect::<Option<Vec<_>>>(),
                    _ => None,
                }?;
                Some((statement, evidence))
            }),
            _ => None,
        };
        let Some((statement, evidence)) =
            parsed.filter(|(statement, _)| !statement.trim().is_empty())
        else {
            self.dropped += 1;
            return;
        };
        let mut evidence_dialog_ids = Vec::new();
        for evidence in evidence {
            // Dialog IDs are opaque: an exact ID containing a comma stays whole.
            if self.turn_ids.contains(evidence) {
                evidence_dialog_ids.push(evidence.to_string());
            } else {
                evidence_dialog_ids.extend(
                    evidence
                        .split(',')
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                        .map(ToOwned::to_owned),
                );
            }
        }
        self.unresolved += evidence_dialog_ids
            .iter()
            .filter(|id| !self.turn_ids.contains(*id))
            .count();
        output.push(LoCoMoGeneratedObservation {
            speaker: speaker.map(ToOwned::to_owned),
            statement: statement.to_string(),
            evidence_dialog_ids,
        });
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
            parse_official_locomo_timestamp(trimmed)
                .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true))
                .unwrap_or_else(|| trimmed.to_string()),
        )
    })
}

fn parse_official_locomo_timestamp(value: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(value, "%I:%M %P on %-d %B, %Y")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%I:%M %p on %-d %B, %Y"))
        .ok()
        .map(|timestamp| DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc))
}

fn parse_qa(
    value: Option<&Value>,
    sample_id: &str,
    location: &AdmissionLocation,
) -> Result<Vec<LoCoMoQa>, LoadError> {
    let items = value
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| location.error("qa", "expected a non-empty QA array"))?;
    items
        .iter()
        .enumerate()
        .map(|(idx, qa)| {
            if !qa.is_object() {
                return Err(location.error(format!("qa[{idx}]"), "expected a QA object"));
            }
            let question = nonblank_string_field(qa, &["question", "q"]).ok_or_else(|| {
                location.error(format!("qa[{idx}].question"), "expected a non-blank string")
            })?;
            Ok(LoCoMoQa {
                question_id: nonblank_string_field(qa, &["question_id", "qid", "id"])
                    .unwrap_or_else(|| format!("{sample_id}:qa:{}", idx + 1)),
                qa_index: idx + 1,
                question_type: scalar_field(qa, &["question_type", "category", "type"]),
                question,
                answer: scalar_field(qa, &["answer", "a"]),
                evidence_dialog_ids: evidence_ids(
                    qa.get("evidence").or_else(|| qa.get("evidence_dialog_ids")),
                ),
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

fn canonical_session_number(key: &str) -> Option<&str> {
    let number = key.strip_prefix("session_")?;
    (!number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && (number.len() == 1 || !number.starts_with('0')))
    .then_some(number)
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
            rows[0].sessions[0].raw_timestamp.as_deref(),
            Some("1:00 pm on 2 May, 2023")
        );
        assert_eq!(
            rows[0].sessions[0].timestamp.as_deref(),
            Some("2023-05-02T13:00:00Z")
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

    #[test]
    fn parses_session_summaries_and_generated_observations() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "session_1": [{"dia_id": "D1:1", "speaker": "A", "text": "hello"}]
            },
            "session_summary": {
                "session_1": "They discussed a trip."
            },
            "observation": {
                "session_1": ["A likes quiet cafes."]
            },
            "qa": [{"question": "What?"}]
        }]))
        .unwrap();

        assert_eq!(
            rows[0].sessions[0].summary.as_deref(),
            Some("They discussed a trip.")
        );
        assert_eq!(
            rows[0].sessions[0]
                .generated_observations
                .iter()
                .map(|observation| observation.statement.as_str())
                .collect::<Vec<_>>(),
            vec!["A likes quiet cafes."]
        );
    }

    #[test]
    fn normalizes_official_timestamp_to_rfc3339_utc() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "session_1_date_time": "1:56 pm on 8 May, 2023",
                "session_1": [{"dia_id": "D1:1", "speaker": "A", "text": "hello"}]
            },
            "qa": [{"question": "What?"}]
        }]))
        .unwrap();

        assert_eq!(
            rows[0].sessions[0].raw_timestamp.as_deref(),
            Some("1:56 pm on 8 May, 2023")
        );
        assert_eq!(
            rows[0].sessions[0].timestamp.as_deref(),
            Some("2023-05-08T13:56:00Z")
        );
    }

    #[test]
    fn preserves_unrecognized_non_empty_timestamp_for_adapter_error_context() {
        let rows = load_value(serde_json::json!([{
            "sample_id": "p1",
            "conversation": {
                "session_1_date_time": "not a timestamp",
                "session_1": [{"dia_id": "D1:1", "speaker": "A", "text": "hello"}]
            },
            "qa": [{"question": "What?"}]
        }]))
        .unwrap();

        assert_eq!(
            rows[0].sessions[0].timestamp.as_deref(),
            Some("not a timestamp")
        );
    }
}
