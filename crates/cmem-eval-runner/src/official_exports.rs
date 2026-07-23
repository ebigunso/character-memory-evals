use anyhow::{Context, Result, anyhow, bail};
use cmem_eval_core::{
    LegacyPerQuestionResultV1, LegacyRetrievedItemV1, ObjectType, PerQuestionResult, RetrievedItem,
    VersionedPerQuestionResult,
};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub type Predictions = HashMap<String, Prediction>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    pub hypothesis: String,
}

pub fn read_predictions_jsonl(path: &Path) -> Result<Predictions> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut predictions = HashMap::new();
    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse prediction JSONL line {}", line_idx + 1))?;
        let question_id = string_field(&value, &["question_id", "id", "qid"])
            .ok_or_else(|| anyhow!("prediction line {} is missing question_id", line_idx + 1))?;
        let hypothesis = string_field(&value, &["hypothesis", "prediction", "answer"])
            .ok_or_else(|| anyhow!("prediction line {} is missing hypothesis", line_idx + 1))?;
        if hypothesis.trim().is_empty() {
            bail!("prediction for question_id {question_id} has an empty hypothesis");
        }
        let previous = predictions.insert(question_id.clone(), Prediction { hypothesis });
        if previous.is_some() {
            bail!("duplicate prediction for question_id {question_id}");
        }
    }
    Ok(predictions)
}

pub fn write_longmemeval_retrieval(path: &Path, rows: &[VersionedPerQuestionResult]) -> Result<()> {
    write_values(
        path,
        &rows
            .iter()
            .map(longmemeval_retrieval_row)
            .collect::<Vec<_>>(),
    )
}

pub fn write_longmemeval_qa(
    path: &Path,
    rows: &[VersionedPerQuestionResult],
    predictions: &Predictions,
) -> Result<()> {
    write_values(
        path,
        &rows
            .iter()
            .map(|row| longmemeval_qa_row(row, predictions))
            .collect::<Result<Vec<_>>>()?,
    )
}

pub fn write_locomo(
    path: &Path,
    rows: &[VersionedPerQuestionResult],
    predictions: Option<&Predictions>,
) -> Result<()> {
    write_values(
        path,
        &rows
            .iter()
            .map(|row| locomo_row(row, predictions))
            .collect::<Result<Vec<_>>>()?,
    )
}

fn longmemeval_retrieval_row(row: &VersionedPerQuestionResult) -> Value {
    match row {
        VersionedPerQuestionResult::V1(row) => serde_json::json!({
            "question_id": row.question_id,
            "question": row.question,
            "question_type": row.question_type,
            "retrieval_results": { "ranked_items": ranked_items_v1(&row.retrieved) }
        }),
        VersionedPerQuestionResult::V2(row) => serde_json::json!({
            "question_id": row.question_id,
            "question": row.question,
            "question_type": row.question_type,
            "retrieval_results": { "ranked_items": ranked_items(&row.retrieved) }
        }),
    }
}

fn longmemeval_qa_row(
    row: &VersionedPerQuestionResult,
    predictions: &Predictions,
) -> Result<Value> {
    let question_id = match row {
        VersionedPerQuestionResult::V1(row) => &row.question_id,
        VersionedPerQuestionResult::V2(row) => &row.question_id,
    };
    let prediction = predictions
        .get(question_id)
        .ok_or_else(|| anyhow!("missing prediction for question_id {question_id}"))?;
    Ok(serde_json::json!({
        "question_id": question_id,
        "hypothesis": prediction.hypothesis
    }))
}

fn locomo_row(
    row: &VersionedPerQuestionResult,
    predictions: Option<&Predictions>,
) -> Result<Value> {
    match row {
        VersionedPerQuestionResult::V1(row) => locomo_row_v1(row, predictions),
        VersionedPerQuestionResult::V2(row) => locomo_row_v2(row, predictions),
    }
}

fn locomo_row_v2(row: &PerQuestionResult, predictions: Option<&Predictions>) -> Result<Value> {
    locomo_row_fields(
        &row.question_id,
        row.question_type.as_deref(),
        &row.question,
        predictions,
        retrieved_ids(&row.retrieved, ObjectType::Observation),
        retrieved_ids(&row.retrieved, ObjectType::Episode),
        row.context_text.clone(),
        ranked_items(&row.retrieved),
    )
}

fn locomo_row_v1(
    row: &LegacyPerQuestionResultV1,
    predictions: Option<&Predictions>,
) -> Result<Value> {
    locomo_row_fields(
        &row.question_id,
        row.question_type.as_deref(),
        &row.question,
        predictions,
        retrieved_ids_v1(&row.retrieved, "observation"),
        retrieved_ids_v1(&row.retrieved, "episode"),
        row.rendered_context_text(),
        ranked_items_v1(&row.retrieved),
    )
}

#[allow(clippy::too_many_arguments)]
fn locomo_row_fields(
    question_id: &str,
    question_type: Option<&str>,
    question: &str,
    predictions: Option<&Predictions>,
    retrieved_dialog_ids: Vec<String>,
    retrieved_session_ids: Vec<String>,
    context_text: String,
    ranked_items: Vec<Value>,
) -> Result<Value> {
    let (sample_id, qa_index) = parse_locomo_question_id(question_id)?;
    let prediction = predictions.and_then(|predictions| predictions.get(question_id));
    let mut out = Map::new();
    out.insert("sample_id".to_string(), Value::String(sample_id));
    out.insert("qa_index".to_string(), serde_json::json!(qa_index));
    out.insert(
        "question_id".to_string(),
        Value::String(question_id.to_string()),
    );
    out.insert(
        "category".to_string(),
        question_type
            .map(str::to_string)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    out.insert("question".to_string(), Value::String(question.to_string()));
    out.insert(
        "hypothesis".to_string(),
        prediction
            .map(|prediction| Value::String(prediction.hypothesis.clone()))
            .unwrap_or(Value::Null),
    );
    out.insert(
        "prediction".to_string(),
        prediction
            .map(|prediction| Value::String(prediction.hypothesis.clone()))
            .unwrap_or(Value::Null),
    );
    out.insert("answer".to_string(), Value::Null);
    out.insert(
        "retrieved_dialog_ids".to_string(),
        serde_json::json!(retrieved_dialog_ids),
    );
    out.insert(
        "retrieved_session_ids".to_string(),
        serde_json::json!(retrieved_session_ids),
    );
    out.insert("context".to_string(), Value::String(context_text.clone()));
    out.insert("context_text".to_string(), Value::String(context_text));
    out.insert(
        "retrieval_results".to_string(),
        serde_json::json!({ "ranked_items": ranked_items }),
    );
    Ok(Value::Object(out))
}

fn ranked_items(items: &[RetrievedItem]) -> Vec<Value> {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| item.rank);
    sorted
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "rank": item.rank,
                "kind": item.kind,
                "external_id": item.external_id,
                "episode_external_id": item.episode_external_id,
                "score": item.score,
                "text": item.text
            })
        })
        .collect()
}

fn retrieved_ids(items: &[RetrievedItem], kind: ObjectType) -> Vec<String> {
    let mut sorted = items
        .iter()
        .filter(|item| item.kind == kind)
        .filter_map(|item| item.external_id.as_ref().map(|id| (item.rank, id.clone())))
        .collect::<Vec<_>>();
    sorted.sort_by_key(|(rank, _)| *rank);
    sorted.into_iter().map(|(_, id)| id).collect()
}

fn ranked_items_v1(items: &[LegacyRetrievedItemV1]) -> Vec<Value> {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| item.rank);
    sorted
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "rank": item.rank,
                "kind": item.kind,
                "external_id": item.external_id,
                "episode_external_id": item.episode_external_id,
                "score": item.score,
                "text": item.text
            })
        })
        .collect()
}

fn retrieved_ids_v1(items: &[LegacyRetrievedItemV1], kind: &str) -> Vec<String> {
    let mut sorted = items
        .iter()
        .filter(|item| item.kind == kind)
        .filter_map(|item| item.external_id.as_ref().map(|id| (item.rank, id.clone())))
        .collect::<Vec<_>>();
    sorted.sort_by_key(|(rank, _)| *rank);
    sorted.into_iter().map(|(_, id)| id).collect()
}

fn parse_locomo_question_id(question_id: &str) -> Result<(String, usize)> {
    let Some((sample_id, rest)) = question_id.rsplit_once(":qa:") else {
        bail!(
            "LoCoMo export requires question_id formatted as <sample_id>:qa:<index>; got {question_id}"
        );
    };
    if sample_id.is_empty() {
        bail!("LoCoMo export found empty sample_id in question_id {question_id}");
    }
    let qa_index = rest
        .parse::<usize>()
        .with_context(|| format!("parse LoCoMo qa index from question_id {question_id}"))?;
    Ok((sample_id.to_string(), qa_index))
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn write_values(path: &Path, rows: &[Value]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    for row in rows {
        serde_json::to_writer(&mut file, row)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmem_eval_core::{
        DatasetId, DatasetKind, EmbeddingBindingRecord, LiveEmbeddingProvider, MetricsRecord,
        RetrievedItem, RunAdapterMetadata,
    };

    #[test]
    fn longmemeval_retrieval_export_pins_official_shape() {
        let row = VersionedPerQuestionResult::V2(Box::new(sample_row("q1", "longmemeval_s")));
        let value = longmemeval_retrieval_row(&row);

        assert_eq!(value["question_id"], "q1");
        assert!(value["retrieval_results"]["ranked_items"].is_array());
        assert_eq!(
            value["retrieval_results"]["ranked_items"][0]["external_id"],
            "s1"
        );
    }

    #[test]
    fn longmemeval_qa_export_requires_prediction() {
        let row = VersionedPerQuestionResult::V2(Box::new(sample_row("q1", "longmemeval_s")));
        let err = longmemeval_qa_row(&row, &Predictions::new())
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing prediction"));

        let mut predictions = Predictions::new();
        predictions.insert(
            "q1".to_string(),
            Prediction {
                hypothesis: "Paris".to_string(),
            },
        );
        let value = longmemeval_qa_row(&row, &predictions).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"question_id": "q1", "hypothesis": "Paris"})
        );
    }

    #[test]
    fn locomo_export_preserves_sample_qa_and_retrieved_ids() {
        let mut row = sample_row("sample_1:qa:2", "locomo");
        row.question_type = Some("3".to_string());
        row.retrieved.push(RetrievedItem {
            kind: ObjectType::Observation,
            internal_id: "i2".to_string(),
            external_id: Some("D1:3".to_string()),
            episode_external_id: Some("session_1".to_string()),
            score: Some(0.9),
            rank: 2,
            rationale: vec![],
            text: Some("dialog text".to_string()),
        });
        row.context_text = "authoritative persisted v2 context".to_string();
        let row = VersionedPerQuestionResult::V2(Box::new(row));
        let mut predictions = Predictions::new();
        predictions.insert(
            "sample_1:qa:2".to_string(),
            Prediction {
                hypothesis: "A remembered fact".to_string(),
            },
        );

        let value = locomo_row(&row, Some(&predictions)).unwrap();
        assert_eq!(value["sample_id"], "sample_1");
        assert_eq!(value["qa_index"], 2);
        assert_eq!(value["category"], "3");
        assert_eq!(value["hypothesis"], "A remembered fact");
        assert_eq!(value["prediction"], "A remembered fact");
        assert_eq!(value["answer"], Value::Null);
        assert_eq!(value["retrieved_session_ids"], serde_json::json!(["s1"]));
        assert_eq!(value["retrieved_dialog_ids"], serde_json::json!(["D1:3"]));
        assert_eq!(value["context"], "authoritative persisted v2 context");
        assert_eq!(value["context_text"], "authoritative persisted v2 context");
    }

    #[test]
    fn prediction_parser_rejects_duplicate_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.jsonl");
        std::fs::write(
            &path,
            "{\"question_id\":\"q1\",\"hypothesis\":\"a\"}\n{\"question_id\":\"q1\",\"prediction\":\"b\"}\n",
        )
        .unwrap();

        let err = read_predictions_jsonl(&path).unwrap_err().to_string();
        assert!(err.contains("duplicate prediction"));
    }

    #[test]
    fn locomo_export_rejects_unparseable_question_id() {
        let row = VersionedPerQuestionResult::V2(Box::new(sample_row("q1", "locomo")));
        let err = locomo_row(&row, None).unwrap_err().to_string();
        assert!(err.contains("<sample_id>:qa:<index>"));
    }

    fn sample_row(question_id: &str, dataset: &str) -> PerQuestionResult {
        let dataset_kind = match dataset {
            "longmemeval_s" => DatasetKind::LongMemEvalS,
            "locomo" => DatasetKind::LoCoMo,
            other => panic!("unsupported sample dataset {other:?}"),
        };
        PerQuestionResult {
            schema_version: cmem_eval_core::RESULT_SCHEMA_VERSION.to_string(),
            run_id: "r".to_string(),
            dataset: DatasetId::new(dataset).unwrap(),
            dataset_kind,
            embedding_binding: EmbeddingBindingRecord::Live {
                provider: LiveEmbeddingProvider::Deterministic,
                model: "deterministic-test".to_string(),
                vector_size: 8,
            },
            adapter: RunAdapterMetadata::live(),
            question_id: question_id.to_string(),
            question_type: Some("single".to_string()),
            question: "What?".to_string(),
            gold_episode_ids: vec!["s1".to_string()],
            gold_observation_ids: vec!["s1:turn:1".to_string()],
            retrieved: vec![RetrievedItem {
                kind: ObjectType::Episode,
                internal_id: "i1".to_string(),
                external_id: Some("s1".to_string()),
                episode_external_id: None,
                score: Some(1.0),
                rank: 1,
                rationale: vec!["because".to_string()],
                text: Some("episode text".to_string()),
            }],
            context_text: "persisted v2 context".to_string(),
            write_outcomes: Vec::new(),
            lifecycle_outcomes: Vec::new(),
            metrics: MetricsRecord::default(),
            latency_ms: 1,
            context_char_count: 0,
            context_word_count: 0,
            context: cmem_eval_core::ResultContextMetrics::default(),
            telemetry: cmem_eval_core::RetrievalTelemetry::default(),
            composition: cmem_eval_core::ResultCompositionMetrics::default(),
            integrity: cmem_eval_core::ResultIntegrityDetails::default(),
            reader: cmem_eval_core::ReaderResult::default(),
        }
    }
}
