use crate::{RetrievedItem, aggregate_numeric_metrics};
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
    pub latency: Value,
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
        };
        let value = serde_json::to_value(row).unwrap();
        assert_eq!(value["question_id"], "q");
        assert_eq!(value["adapter"]["mode"], "mock_smoke");
    }
}
