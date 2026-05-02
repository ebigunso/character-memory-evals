use crate::{
    RetrievalTelemetry, RetrievedItem, aggregate_numeric_metrics, metric_support_summary,
    registry_coverage_summary,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct PerQuestionResult {
    pub run_id: String,
    pub dataset: String,
    #[serde(default)]
    pub adapter: RunAdapterMetadata,
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub gold_episode_ids: Vec<String>,
    pub gold_observation_ids: Vec<String>,
    pub retrieved: Vec<RetrievedItem>,
    pub metrics: Value,
    pub latency_ms: u128,
    pub context_char_count: usize,
    pub context_word_count: usize,
    #[serde(default)]
    pub context: ResultContextMetrics,
    #[serde(default)]
    pub telemetry: RetrievalTelemetry,
    #[serde(default)]
    pub composition: ResultCompositionMetrics,
    #[serde(default)]
    pub integrity: ResultIntegrityDetails,
    #[serde(default)]
    pub reader: ReaderResult,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub dataset: String,
    #[serde(default)]
    pub adapter: RunAdapterMetadata,
    pub config: Value,
    pub num_questions: usize,
    pub metrics: Value,
    #[serde(default)]
    pub metric_support: Value,
    #[serde(default)]
    pub registry_coverage: Value,
    pub latency: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultContextMetrics {
    #[serde(default)]
    pub retrieved_context_chars: usize,
    #[serde(default)]
    pub retrieved_context_words: usize,
    #[serde(default)]
    pub retrieved_context_estimated_tokens: usize,
    #[serde(default)]
    pub full_history_chars: Option<usize>,
    #[serde(default)]
    pub full_history_words: Option<usize>,
    #[serde(default)]
    pub full_history_estimated_tokens: Option<usize>,
    #[serde(default)]
    pub compression_ratio: Option<f64>,
    #[serde(default)]
    pub reduction_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultCompositionMetrics {
    #[serde(default)]
    pub total_items: usize,
    #[serde(default)]
    pub episodes: usize,
    #[serde(default)]
    pub observations: usize,
    #[serde(default)]
    pub derived_memories: usize,
    #[serde(default)]
    pub memory_threads: usize,
    #[serde(default)]
    pub entities: Option<usize>,
    #[serde(default)]
    pub items_with_rationale: usize,
    #[serde(default)]
    pub rationale_coverage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ResultIntegrityDetails {
    #[serde(default)]
    pub returned_items_without_external_id: usize,
    #[serde(default)]
    pub returned_derived_memories_without_provenance: usize,
    #[serde(default)]
    pub suppressed_or_deleted_returned_count: Option<usize>,
    #[serde(default)]
    pub superseded_current_returned_count: Option<usize>,
    #[serde(default)]
    pub provenance_coverage: Option<f64>,
    #[serde(default)]
    pub context_validation_pass_rate: Option<f64>,
    #[serde(default)]
    pub suppressed_memory_leakage_rate: Option<f64>,
    #[serde(default)]
    pub orphan_vector_leakage_rate: Option<f64>,
    #[serde(default)]
    pub superseded_current_leakage_rate: Option<f64>,
    #[serde(default)]
    pub cross_store_id_validation_pass_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ReaderResult {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub qa_score: Option<f64>,
    #[serde(default)]
    pub qa_metric_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunAdapterMetadata {
    pub adapter: String,
    pub mode: String,
    pub is_mock: bool,
}

impl RunAdapterMetadata {
    pub fn live() -> Self {
        Self {
            adapter: "real".to_string(),
            mode: "live".to_string(),
            is_mock: false,
        }
    }

    pub fn mock_smoke() -> Self {
        Self {
            adapter: "mock".to_string(),
            mode: "mock_smoke".to_string(),
            is_mock: true,
        }
    }
}

impl Default for RunAdapterMetadata {
    fn default() -> Self {
        Self {
            adapter: "unknown".to_string(),
            mode: "unknown".to_string(),
            is_mock: false,
        }
    }
}

pub fn write_jsonl(path: &Path, rows: &[PerQuestionResult]) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    for row in rows {
        serde_json::to_writer(&mut file, row)?;
        file.write_all(b"\n")?;
    }
    Ok(())
}

pub fn write_summary(path: &Path, summary: &RunSummary) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, summary)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn summarize_rows(
    run_id: String,
    dataset: String,
    adapter: RunAdapterMetadata,
    config: Value,
    rows: &[PerQuestionResult],
) -> RunSummary {
    let metric_rows = rows
        .iter()
        .filter_map(|row| row.metrics.as_object().cloned())
        .collect::<Vec<Map<String, Value>>>();
    let latency_values = rows
        .iter()
        .map(|row| row.latency_ms as f64)
        .collect::<Vec<_>>();
    RunSummary {
        run_id,
        dataset,
        adapter,
        config,
        num_questions: rows.len(),
        metrics: aggregate_numeric_metrics(&metric_rows),
        metric_support: metric_support_summary(&metric_rows),
        registry_coverage: registry_coverage_summary(&metric_rows),
        latency: serde_json::json!({
            "latency_ms": {
                "mean": crate::mean(&latency_values),
                "median": crate::median(&latency_values),
                "p50": crate::percentile(&latency_values, 50.0),
                "p95": crate::percentile(&latency_values, 95.0),
            }
        }),
    }
}

pub fn read_jsonl(path: &Path) -> Result<Vec<PerQuestionResult>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line)?);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_per_question_result() {
        let row = PerQuestionResult {
            run_id: "r".into(),
            dataset: "synthetic".into(),
            adapter: RunAdapterMetadata::mock_smoke(),
            question_id: "q".into(),
            question_type: None,
            question: "question".into(),
            gold_episode_ids: vec!["s1".into()],
            gold_observation_ids: vec!["s1:turn:1".into()],
            retrieved: vec![],
            metrics: serde_json::json!({"recall_any@1": 1.0}),
            latency_ms: 1,
            context_char_count: 0,
            context_word_count: 0,
            context: ResultContextMetrics::default(),
            telemetry: RetrievalTelemetry::default(),
            composition: ResultCompositionMetrics::default(),
            integrity: ResultIntegrityDetails::default(),
            reader: ReaderResult::default(),
        };
        let value = serde_json::to_value(row).unwrap();
        assert_eq!(value["question_id"], "q");
        assert_eq!(value["adapter"]["mode"], "mock_smoke");
    }

    #[test]
    fn summary_preserves_null_metric_support() {
        let row = PerQuestionResult {
            run_id: "r".into(),
            dataset: "synthetic".into(),
            adapter: RunAdapterMetadata::mock_smoke(),
            question_id: "q".into(),
            question_type: None,
            question: "question".into(),
            gold_episode_ids: vec![],
            gold_observation_ids: vec![],
            retrieved: vec![],
            metrics: serde_json::json!({"suppressed_or_deleted_items_returned": null}),
            latency_ms: 1,
            context_char_count: 0,
            context_word_count: 0,
            context: ResultContextMetrics::default(),
            telemetry: RetrievalTelemetry::default(),
            composition: ResultCompositionMetrics::default(),
            integrity: ResultIntegrityDetails::default(),
            reader: ReaderResult::default(),
        };

        let summary = summarize_rows(
            "r".into(),
            "synthetic".into(),
            RunAdapterMetadata::mock_smoke(),
            serde_json::json!({}),
            &[row],
        );

        assert_eq!(
            summary.metric_support["suppressed_or_deleted_items_returned"]["unsupported"],
            true
        );
        assert_eq!(summary.registry_coverage["required_metrics_present"], 0);
        assert!(
            summary.registry_coverage["missing_required_metrics"]
                .as_array()
                .is_some_and(|missing| !missing.is_empty())
        );
    }
}
